use crate::{
    fonts,
    theme::Theme,
    tray::{
        actions::{self, ItemAction},
        model::{ItemKey, ItemStatus, SlotAnchor, TrayItem},
        ui,
    },
};
use amane::{Button, Center, Monitor, Pointer, Rectangle, Row, Service, Stack, Text};
use std::sync::Arc;

const SLOT: f32 = 26.0;
const GAP: f32 = 4.0;
const PAD: f32 = 8.0;

#[derive(Clone, Copy)]
pub(crate) struct InlineLayout {
    pub width: f32,
    pub inline: usize,
    pub overflow: usize,
}
pub(crate) fn layout(count: usize, available: f32) -> InlineLayout {
    if count == 0 || !available.is_finite() || available < PAD * 2.0 + SLOT {
        return InlineLayout {
            width: 0.0,
            inline: 0,
            overflow: count,
        };
    }
    let slots = (((available - PAD * 2.0 + GAP) / (SLOT + GAP)).floor() as usize).min(count);
    let inline = if slots < count { slots - 1 } else { slots };
    InlineLayout {
        width: PAD * 2.0 + slots as f32 * SLOT + (slots - 1) as f32 * GAP,
        inline,
        overflow: count - inline,
    }
}

pub(crate) fn placement(
    output: &str,
    monitor_size: (f32, f32),
    bar: (f32, bool),
    others: &[f32],
    insertion: usize,
    count: usize,
) -> (SlotAnchor, InlineLayout) {
    let occupied = others.iter().sum::<f32>() + others.len().saturating_sub(1) as f32 * 10.0;
    let layout = layout(count, monitor_size.0 / 3.0 - occupied - 10.0);
    let trailing =
        others[insertion..].iter().sum::<f32>() + (others.len() - insertion) as f32 * 10.0;
    let y = (bar.0 - SLOT) / 2.0 + if bar.1 { monitor_size.1 - bar.0 } else { 0.0 };
    (
        SlotAnchor {
            output: output.into(),
            x: monitor_size.0 - trailing - layout.width,
            y,
            width: layout.width,
            height: SLOT,
        },
        layout,
    )
}

pub(crate) fn view(
    monitor: &Monitor,
    theme: &Theme,
    items: &[Arc<TrayItem>],
    placement: SlotAnchor,
    layout: InlineLayout,
) -> Rectangle {
    debug_assert_eq!(monitor.name, placement.output);
    let mut slots: Vec<Box<dyn amane::Widget>> = Vec::new();
    for (index, item) in items.iter().take(layout.inline).enumerate() {
        let mut anchor = placement.clone();
        anchor.x += PAD + index as f32 * (SLOT + GAP);
        anchor.width = SLOT;
        slots.push(Box::new(slot(item.clone(), theme, anchor)));
    }
    if layout.overflow > 0 {
        let mut anchor = placement;
        anchor.x += PAD + layout.inline as f32 * (SLOT + GAP);
        anchor.width = SLOT;
        let keys: Vec<ItemKey> = items
            .iter()
            .skip(layout.inline)
            .map(|i| i.key.clone())
            .collect();
        slots.push(Box::new(
            Rectangle::new()
                .width(SLOT)
                .height(SLOT)
                .cursor(Pointer)
                .align_child(Center, Center)
                .child(
                    Text::new("󰇘")
                        .font(fonts::NERD)
                        .size(17.0)
                        .color(theme.text),
                )
                .on_hover(|_| ui::hover(None))
                .on_click(move |_| ui::open_overflow(anchor.clone(), keys.clone())),
        ));
    }
    super::pill::view(layout.width, theme.selected_surface)
        .align_child(Center, Center)
        .child(Row::new(slots).gap(GAP).align(Center))
}

pub(crate) fn icon(item: &TrayItem, theme: &Theme) -> Rectangle {
    let attention = item.properties.status == ItemStatus::NeedsAttention;
    let image = if attention {
        item.icon.attention.as_ref().or(item.icon.normal.as_ref())
    } else {
        item.icon.normal.as_ref()
    };
    let base = if let Some(image) = image {
        Rectangle::new()
            .width(18.0)
            .height(18.0)
            .fill(image.clone())
    } else {
        Rectangle::new()
            .width(18.0)
            .height(18.0)
            .align_child(Center, Center)
            .child(
                Text::new("󰀻")
                    .font(fonts::NERD)
                    .size(16.0)
                    .color(if attention { theme.accent } else { theme.text }),
            )
    };
    let mut pictures: Vec<Box<dyn amane::Widget>> = vec![Box::new(base)];
    if let Some(overlay) = &item.icon.overlay {
        pictures.push(Box::new(
            Rectangle::new()
                .width(9.0)
                .height(9.0)
                .translate(9.0, 9.0)
                .fill(overlay.clone()),
        ));
    }
    let icon = Rectangle::new()
        .width(SLOT)
        .height(SLOT)
        .radius(5.0)
        .align_child(Center, Center)
        .child(Stack::new(pictures).width(18.0).height(18.0));
    if attention {
        icon.border(1.0, theme.accent)
    } else {
        icon
    }
}

pub(crate) fn slot(item: Arc<TrayItem>, theme: &Theme, anchor: SlotAnchor) -> Rectangle {
    let hover_key = item.key.clone();
    let hover_anchor = anchor.clone();
    let scroll_key = item.key.clone();
    let scroll_anchor = anchor.clone();
    let hovered = ui::TrayUi::read()
        .tooltip
        .as_ref()
        .is_some_and(|t| t.key == item.key);
    icon(&item, theme)
        .fill(if hovered {
            theme.hover_surface
        } else {
            amane::Color::TRANSPARENT
        })
        .cursor(Pointer)
        .on_hover(move |inside| {
            ui::hover(inside.then(|| (hover_key.clone(), hover_anchor.clone())))
        })
        .on_scroll(move |scroll| actions::scroll(scroll_key.clone(), scroll, scroll_anchor.clone()))
        .on_click(move |button| click(item.clone(), button, anchor.clone()))
}

pub(crate) fn click(item: Arc<TrayItem>, button: Button, anchor: SlotAnchor) {
    ui::close();
    match button {
        Button::Left if item.properties.item_is_menu => {
            ui::open_menu(item.key.clone(), anchor.clone())
        }
        Button::Left => {
            actions::submit(item.key.clone(), ItemAction::Activate, anchor.clone());
        }
        Button::Right => ui::open_menu(item.key.clone(), anchor.clone()),
        Button::Middle => {
            actions::submit(
                item.key.clone(),
                ItemAction::SecondaryActivate,
                anchor.clone(),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inline_layout_reserves_overflow_and_empty_has_no_width() {
        assert_eq!(layout(0, 200.0).width, 0.0);
        assert_eq!(layout(1, 200.0).width, 42.0);
        assert_eq!(layout(2, 200.0).width, 72.0);
        assert_eq!((layout(5, 72.0).inline, layout(5, 72.0).overflow), (1, 4));
        assert_eq!((layout(5, 42.0).inline, layout(5, 42.0).overflow), (0, 5));
        assert_eq!(layout(5, 41.0).width, 0.0);
    }
    #[test]
    fn recording_growth_repositions_slots_without_overlap() {
        let small = placement("DP-9", (1920.0, 1080.0), (40.0, false), &[70.0, 5.0], 0, 5);
        let large = placement("DP-9", (1920.0, 1080.0), (40.0, false), &[370.0, 5.0], 0, 5);
        assert!(large.0.x < small.0.x);
        for ((anchor, layout), utility) in [(small, 70.0), (large, 370.0)] {
            assert_eq!(anchor.output, "DP-9");
            assert!(anchor.x + layout.width <= 1920.0 - 5.0 - 10.0 - utility - 10.0);
        }
    }
}
