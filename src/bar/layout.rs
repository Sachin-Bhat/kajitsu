use std::cell::RefCell;
use std::collections::HashMap;

use amane::{Button, Center, Full, Pointer, Rectangle, Row, Service, Size, Text, Weight, Widget};

use crate::fonts;
use crate::mango::layouts::{Layouts, ScrollSteps};
use crate::theme::Theme;

mod icon;

thread_local! {
    static SCROLL: RefCell<HashMap<String, ScrollSteps>> = RefCell::new(HashMap::new());
}

pub fn view(output: &str, theme: &Theme) -> Rectangle {
    let layouts = Layouts::read();
    let current = layouts.current(output);
    let enabled = current.is_some();
    let label = current.map(label).unwrap_or_else(|| "Layout —".into());
    let color = if enabled {
        theme.accent
    } else {
        theme.muted_text
    };
    let text = Text::new(label)
        .size(12.0)
        .font(fonts::BODY)
        .weight(Weight::Medium)
        .color(color);
    let content = Row::new(vec![Box::new(icon::view(current, color)), Box::new(text)])
        .gap(6.0)
        .align(Center);
    let width = match Widget::width(&content) {
        Size::Fixed(width) => Size::Fixed(width.ceil() + 20.0),
        Size::Parent => Size::Parent,
    };
    let control = Rectangle::new()
        .width(width)
        .height(super::ITEM_HEIGHT)
        .radius(Full)
        .fill(theme.selected_surface)
        .align_child(Center, Center)
        .child(content);
    if !enabled {
        return control;
    }
    let clicked = output.to_string();
    let scrolled = output.to_string();
    let hovered = output.to_string();
    control
        .cursor(Pointer)
        .on_click(move |button| {
            crate::tray::ui::close();
            let steps = match button {
                Button::Left => 1,
                Button::Right => -1,
                Button::Middle => return,
            };
            Layouts::cycle(clicked.clone(), steps);
        })
        .on_scroll(move |scroll| {
            let steps = SCROLL.with_borrow_mut(|states| {
                states.entry(scrolled.clone()).or_default().push(scroll.y)
            });
            Layouts::cycle(scrolled.clone(), steps);
        })
        .on_hover(move |inside| {
            if !inside {
                SCROLL.with_borrow_mut(|states| {
                    states.remove(&hovered);
                });
            }
        })
}

fn label(name: &str) -> String {
    let (prefix, name) = name
        .strip_prefix("vertical_")
        .map_or(("", name), |name| ("V. ", name));
    let mut name = name.replace('_', " ");
    if let Some(first) = name.get_mut(..1) {
        first.make_ascii_uppercase();
    }
    format!("{prefix}{name}")
}
