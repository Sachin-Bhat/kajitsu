pub(crate) mod geometry;

use super::{
    menu::{MenuKind, MenuNode},
    model::SlotAnchor,
    ui::{self, Navigation, PopupSession, TrayUi},
};
use crate::{fonts, settings::Settings, theme};
use amane::{
    Button, Center, Column, Full, Horizontal, Key, Keyboard, Layer, LayerWindow, Monitor, Parent,
    Pointer, Rectangle, Row, Service, Size, Stack, Text, Vertical, Widget, children,
};
use geometry::PopupRect;

const ROW: f32 = 30.0;
const PAD: f32 = 6.0;

fn menu_width<'a>(labels: impl Iterator<Item = &'a str>) -> f32 {
    labels
        .map(
            |s| match Widget::width(&Text::new(s).font(fonts::BODY).size(13.0)) {
                Size::Fixed(w) => w + 64.0,
                Size::Parent => 180.0,
            },
        )
        .fold(180.0, f32::max)
        .ceil()
}
fn placement_anchor(anchor: &SlotAnchor, monitor: &Monitor) -> (SlotAnchor, bool) {
    let settings = Settings::read();
    let bottom = settings.text("bar_position") == "bottom";
    let height = settings.number("bar_height");
    (
        SlotAnchor {
            y: if bottom {
                monitor.height as f32 - height
            } else {
                0.0
            },
            height,
            ..anchor.clone()
        },
        bottom,
    )
}
fn capacity(monitor: &Monitor) -> usize {
    ((monitor.height as f32 - crate::bar::height() - 24.0) / ROW)
        .floor()
        .max(1.0) as usize
}

pub(crate) fn view(monitor: &Monitor) -> LayerWindow {
    let popup = TrayUi::read().popup.clone();
    let popup = popup.filter(|p| match p {
        PopupSession::Menu(m) => m.anchor.output == monitor.name,
        PopupSession::Overflow(o) => o.anchor.output == monitor.name,
    });
    let window = LayerWindow::new()
        .width(Full)
        .height(Full)
        .anchor_vertical(Vertical::Top)
        .anchor_horizontal(Horizontal::Left)
        .layer(Layer::Overlay)
        .space(amane::Zone::Ignore)
        .namespace("kajitsu-tray-menu");
    let Some(popup) = popup else {
        return window.visible(false).click_through();
    };
    let bar_height = crate::bar::height().ceil() as i32;
    let bottom = Settings::read().text("bar_position") == "bottom";
    let window = window.input_region(vec![amane::InputArea {
        x: 0,
        y: if bottom { 0 } else { bar_height },
        width: monitor.width as i32,
        height: (monitor.height as i32 - bar_height).max(0),
    }]);
    let theme = theme::current();
    let size = (monitor.width as f32, monitor.height as f32);
    let mut panels: Vec<Box<dyn Widget>> = vec![Box::new(
        Rectangle::new()
            .width(Parent)
            .height(Parent)
            .on_click(|_| ui::close()),
    )];
    match popup {
        PopupSession::Menu(menu) => {
            let (anchor, bottom) = placement_anchor(&menu.anchor, monitor);
            let Some(tree) = &menu.tree else {
                let rect = geometry::place_root(&anchor, (180.0, ROW + PAD * 2.0), size, bottom);
                panels.push(Box::new(
                    surface(rect, &theme).child(
                        Text::new("Loading…")
                            .font(fonts::BODY)
                            .size(13.0)
                            .color(theme.secondary_text),
                    ),
                ));
                return window
                    .keyboard(Keyboard::Exclusive)
                    .on_key(key_pressed)
                    .child(Stack::new(panels).width(Parent).height(Parent));
            };
            let mut previous: Option<(PopupRect, Vec<i32>, usize)> = None;
            for parent_id in &menu.path {
                let Some(parent) = tree.nodes.get(parent_id) else {
                    break;
                };
                let nodes: Vec<_> = parent
                    .children
                    .iter()
                    .filter_map(|id| tree.nodes.get(id))
                    .filter(|n| n.visible)
                    .collect();
                let width = menu_width(nodes.iter().map(|n| n.label.as_str())).min(420.0);
                let count = nodes.len().max(1).min(capacity(monitor));
                let desired = (width, count as f32 * ROW + PAD * 2.0);
                let rect = if let Some((rect, ids, offset)) = &previous {
                    let index = ids
                        .iter()
                        .position(|id| id == parent_id)
                        .unwrap_or(0)
                        .saturating_sub(*offset);
                    geometry::place_submenu(
                        PopupRect {
                            y: rect.y + PAD + index as f32 * ROW,
                            height: ROW,
                            ..*rect
                        },
                        desired,
                        size,
                    )
                } else {
                    geometry::place_root(&anchor, desired, size, bottom)
                };
                let visible_count = ((rect.height - PAD * 2.0) / ROW).floor().max(1.0) as usize;
                let offset = menu
                    .scroll_offset(*parent_id)
                    .min(nodes.len().saturating_sub(visible_count));
                let rows: Vec<Box<dyn Widget>> = if nodes.is_empty() {
                    vec![Box::new(
                        Rectangle::new()
                            .width(rect.width - PAD * 2.0)
                            .height(ROW)
                            .align_child(Center, Center)
                            .child(Text::new("No actions").size(13.0).color(theme.muted_text)),
                    )]
                } else {
                    nodes
                        .iter()
                        .skip(offset)
                        .take(visible_count)
                        .map(|node| {
                            Box::new(menu_row(
                                node,
                                rect.width - PAD * 2.0,
                                &theme,
                                menu.request,
                                menu.selected == Some(node.id) || menu.path.contains(&node.id),
                            )) as Box<dyn Widget>
                        })
                        .collect()
                };
                let request = menu.request;
                let parent_id = *parent_id;
                let total = nodes.len();
                panels.push(Box::new(
                    surface(rect, &theme)
                        .on_scroll(move |scroll| {
                            let mut ui = TrayUi::write();
                            if let Some(menu) = ui.menu_mut().filter(|m| m.request == request) {
                                menu.scroll(
                                    parent_id,
                                    scroll.y.round() as isize,
                                    total,
                                    visible_count,
                                );
                            }
                        })
                        .child(Column::new(rows)),
                ));
                previous = Some((rect, nodes.iter().map(|n| n.id).collect(), offset));
            }
        }
        PopupSession::Overflow(overflow) => {
            let items = super::Tray::read().snapshot().items.clone();
            let items: Vec<_> = overflow
                .keys
                .iter()
                .filter_map(|key| items.iter().find(|i| &i.key == key).cloned())
                .collect();
            let (anchor, bottom) = placement_anchor(&overflow.anchor, monitor);
            let labels: Vec<_> = items.iter().map(|i| item_label(i)).collect();
            let width = menu_width(labels.iter().map(String::as_str)).min(420.0);
            let count = items.len().min(capacity(monitor));
            let rect = geometry::place_root(
                &anchor,
                (width, count as f32 * ROW + PAD * 2.0),
                size,
                bottom,
            );
            let visible_count = ((rect.height - PAD * 2.0) / ROW).floor().max(1.0) as usize;
            let offset = overflow
                .scroll_rows
                .min(items.len().saturating_sub(visible_count));
            let mut rows: Vec<Box<dyn Widget>> = Vec::new();
            for (index, item) in items.iter().enumerate().skip(offset).take(visible_count) {
                let slot = SlotAnchor {
                    output: monitor.name.clone(),
                    x: rect.x + PAD,
                    y: rect.y + PAD + (index - offset) as f32 * ROW,
                    width: rect.width - PAD * 2.0,
                    height: ROW,
                };
                let hovered = item.key.clone();
                let hovered_anchor = slot.clone();
                let clicked = item.clone();
                let click_anchor = slot.clone();
                let scrolled = item.key.clone();
                let scroll_anchor = slot;
                rows.push(Box::new(
                    Rectangle::new()
                        .width(rect.width - PAD * 2.0)
                        .height(ROW)
                        .cursor(Pointer)
                        .radius(5.0)
                        .fill(if index == overflow.selected {
                            theme.selected_surface
                        } else {
                            theme.surface
                        })
                        .on_hover(move |inside| {
                            ui::hover(inside.then(|| (hovered.clone(), hovered_anchor.clone())))
                        })
                        .on_click(move |button| {
                            crate::bar::systray::click(
                                clicked.clone(),
                                button,
                                click_anchor.clone(),
                            )
                        })
                        .on_scroll(move |scroll| {
                            super::actions::scroll(scrolled.clone(), scroll, scroll_anchor.clone())
                        })
                        .child(
                            Row::new(children![
                                crate::bar::systray::icon(item, &theme),
                                Rectangle::new()
                                    .width(rect.width - 50.0)
                                    .height(ROW)
                                    .align_child(amane::Start, Center)
                                    .child(
                                        Text::new(&labels[index])
                                            .font(fonts::BODY)
                                            .size(13.0)
                                            .color(theme.text)
                                            .elide()
                                    )
                            ])
                            .gap(6.0)
                            .align(Center),
                        ),
                ));
            }
            let total = items.len();
            panels.push(Box::new(
                surface(rect, &theme)
                    .on_scroll(move |scroll| {
                        if let Some(PopupSession::Overflow(o)) = &mut TrayUi::write().popup {
                            o.scroll_rows = o
                                .scroll_rows
                                .saturating_add_signed(scroll.y.round() as isize)
                                .min(total.saturating_sub(visible_count));
                        }
                    })
                    .child(Column::new(rows)),
            ));
        }
    }
    let visible_count = capacity(monitor);
    window
        .keyboard(Keyboard::Exclusive)
        .on_key(move |key| key_pressed_with_count(key, visible_count))
        .child(Stack::new(panels).width(Parent).height(Parent))
}
fn item_label(item: &super::model::TrayItem) -> String {
    if !item.properties.title.is_empty() {
        item.properties.title.clone()
    } else {
        item.properties.id.clone()
    }
}
fn surface(rect: PopupRect, theme: &theme::Theme) -> Rectangle {
    Rectangle::new()
        .width(rect.width)
        .height(rect.height)
        .translate(rect.x, rect.y)
        .radius(8.0)
        .fill(theme.surface)
        .border(1.0, theme.border)
        .clip()
        .padding(PAD)
        .on_click(|_| {})
}
fn menu_row(
    node: &MenuNode,
    width: f32,
    theme: &theme::Theme,
    request: u64,
    selected: bool,
) -> Rectangle {
    if node.kind == MenuKind::Separator {
        return Rectangle::new()
            .width(width)
            .height(ROW)
            .align_child(Center, Center)
            .child(
                Rectangle::new()
                    .width(width - 12.0)
                    .height(1.0)
                    .fill(theme.border),
            );
    }
    let color = if node.enabled {
        theme.text
    } else {
        theme.muted_text
    };
    let toggle = match node.kind {
        MenuKind::Check { state: 1 } => "󰄲",
        MenuKind::Check { .. } => "󰄱",
        MenuKind::Radio { state: 1 } => "󰐾",
        MenuKind::Radio { .. } => "󰐿",
        _ => "",
    };
    let mut marks: Vec<Box<dyn Widget>> = Vec::new();
    if !toggle.is_empty() || node.icon.normal.is_none() {
        marks.push(Box::new(
            Rectangle::new()
                .width(18.0)
                .height(18.0)
                .align_child(Center, Center)
                .child(Text::new(toggle).font(fonts::NERD).size(15.0).color(color)),
        ));
    }
    if let Some(image) = &node.icon.normal {
        marks.push(Box::new(
            Rectangle::new()
                .width(18.0)
                .height(18.0)
                .fill(image.clone()),
        ));
    }
    let mark = Row::new(marks).gap(4.0).align(Center);
    let extra = if !toggle.is_empty() && node.icon.normal.is_some() {
        22.0
    } else {
        0.0
    };
    let row = Rectangle::new()
        .width(width)
        .height(ROW)
        .radius(5.0)
        .fill(if selected {
            theme.selected_surface
        } else {
            theme.surface
        })
        .padding(amane::Padding {
            left: 6.0,
            right: 6.0,
            top: 0.0,
            bottom: 0.0,
        })
        .child(
            Row::new(children![
                mark,
                Rectangle::new()
                    .width((width - 58.0 - extra).max(0.0))
                    .height(ROW)
                    .align_child(amane::Start, Center)
                    .child(
                        Text::new(&node.label)
                            .font(fonts::BODY)
                            .size(13.0)
                            .color(color)
                            .elide()
                    ),
                Text::new(if node.has_submenu { "›" } else { "" })
                    .size(16.0)
                    .color(color)
            ])
            .gap(6.0)
            .align(Center),
        );
    if !node.enabled {
        return row;
    }
    let id = node.id;
    let submenu = node.has_submenu;
    row.cursor(Pointer)
        .on_hover(move |inside| {
            if inside {
                ui::select_row(request, id);
            }
        })
        .on_click(move |button| {
            if button != Button::Left {
                return;
            }
            if submenu {
                ui::enter_submenu(request, id);
            } else {
                super::menu::client::activate(request, id, timestamp_ms());
            }
        })
}
fn timestamp_ms() -> u32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u32
}
pub(crate) fn key_pressed(key: Key) {
    key_pressed_with_count(key, 10);
}
fn key_pressed_with_count(key: Key, visible_count: usize) {
    let action = TrayUi::write().navigate(key, visible_count);
    match action {
        Navigation::Open(request, row) => ui::enter_submenu(request, row),
        Navigation::Activate(request, row) => {
            super::menu::client::activate(request, row, timestamp_ms())
        }
        Navigation::Overflow(key, anchor) => {
            let item = super::Tray::read()
                .snapshot()
                .items
                .iter()
                .find(|i| i.key == key)
                .cloned();
            if let Some(item) = item {
                crate::bar::systray::click(item, Button::Left, anchor);
            }
        }
        Navigation::None => {}
    }
}
pub(crate) fn tooltip_view(monitor: &Monitor) -> LayerWindow {
    let tip = TrayUi::read()
        .tooltip
        .clone()
        .filter(|t| t.shown && t.anchor.output == monitor.name);
    let window = LayerWindow::new()
        .width(Full)
        .height(Full)
        .anchor_vertical(Vertical::Top)
        .anchor_horizontal(Horizontal::Left)
        .layer(Layer::Overlay)
        .space(amane::Zone::Ignore)
        .namespace("kajitsu-tray-tooltip")
        .click_through();
    let Some(tip) = tip else {
        return window.visible(false);
    };
    let item = super::Tray::read()
        .snapshot()
        .items
        .iter()
        .find(|i| i.key == tip.key)
        .cloned();
    let Some(item) = item else {
        return window.visible(false);
    };
    let title = if item.properties.tooltip.title.is_empty() {
        item_label(&item)
    } else {
        item.properties.tooltip.title.clone()
    };
    let text = if item.properties.tooltip.description.is_empty() {
        title
    } else {
        format!("{title}\n{}", item.properties.tooltip.description)
    };
    let theme = theme::current();
    let width = menu_width(text.lines())
        .clamp(160.0, 360.0)
        .min(monitor.width as f32 - 16.0);
    let label = Text::new(text)
        .font(fonts::BODY)
        .size(13.0)
        .color(theme.text)
        .wrap()
        .max_lines(6)
        .elide();
    let height = label.height_in(width - PAD * 2.0) + PAD * 2.0;
    let (anchor, bottom) = placement_anchor(&tip.anchor, monitor);
    let rect = geometry::place_root(
        &anchor,
        (width, height),
        (monitor.width as f32, monitor.height as f32),
        bottom,
    );
    window.child(surface(rect, &theme).child(label))
}

#[cfg(test)]
mod tests {
    use super::*;
    use amane::{Size, Text, Widget};
    #[test]
    fn unicode_menu_width_uses_measured_text() {
        let label = "設定 · café · 🦊";
        let Size::Fixed(text) =
            Widget::width(&Text::new(label).font(crate::fonts::BODY).size(13.0))
        else {
            panic!("text must have fixed width");
        };
        assert!(menu_width([label].into_iter()) >= text + 64.0);
    }
}
