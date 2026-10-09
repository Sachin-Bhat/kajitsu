use super::{
    menu::{MenuKind, MenuTree},
    model::{ItemKey, SlotAnchor},
};
use amane::Service;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use std::time::{Duration, Instant};

#[derive(Clone)]
pub(crate) struct MenuSession {
    pub request: u64,
    pub key: ItemKey,
    pub anchor: SlotAnchor,
    pub menu_path: String,
    pub tree: Option<Arc<MenuTree>>,
    pub path: Vec<i32>,
    pub selected: Option<i32>,
    pub scroll_rows: usize,
}
#[derive(Clone)]
pub(crate) struct OverflowSession {
    pub anchor: SlotAnchor,
    pub keys: Vec<ItemKey>,
    pub selected: usize,
    pub scroll_rows: usize,
}
#[derive(Clone)]
pub(crate) enum PopupSession {
    Menu(MenuSession),
    Overflow(OverflowSession),
}
#[derive(Clone)]
pub(crate) struct TooltipSession {
    pub key: ItemKey,
    pub anchor: SlotAnchor,
    pub shown: bool,
    pub deadline: Instant,
}
#[derive(Default)]
pub(crate) struct TrayUi {
    pub popup: Option<PopupSession>,
    pub tooltip: Option<TooltipSession>,
}
static REQUEST: AtomicU64 = AtomicU64::new(1);

impl Service for TrayUi {
    fn new() -> Self {
        Self::default()
    }
    fn listen() {
        loop {
            std::thread::sleep(Duration::from_millis(25));
            let enabled = crate::settings::Settings::read().flag("bar_systray");
            let state = Self::read();
            let close = !enabled && (state.popup.is_some() || state.tooltip.is_some());
            let show = state
                .tooltip
                .as_ref()
                .is_some_and(|t| !t.shown && Instant::now() >= t.deadline);
            drop(state);
            if close {
                self::close();
            } else if show {
                Self::write().show_tooltip(Instant::now());
            }
        }
    }
}

impl TrayUi {
    pub fn show_tooltip(&mut self, now: Instant) -> bool {
        if let Some(t) = self
            .tooltip
            .as_mut()
            .filter(|t| !t.shown && now >= t.deadline)
        {
            t.shown = true;
            return true;
        }
        false
    }
    pub fn holds_bar(&self, output: &str) -> bool {
        self.popup.as_ref().is_some_and(|p| match p {
            PopupSession::Menu(m) => m.anchor.output == output,
            PopupSession::Overflow(o) => o.anchor.output == output,
        })
    }
    pub fn navigate(&mut self, key: amane::Key, visible_count: usize) -> Navigation {
        if key == amane::Key::Escape {
            self.popup = None;
            self.tooltip = None;
            return Navigation::None;
        }
        if let Some(PopupSession::Overflow(o)) = &mut self.popup {
            if o.keys.is_empty() {
                return Navigation::None;
            }
            o.selected = o.selected.min(o.keys.len() - 1);
            match key {
                amane::Key::Home => o.selected = 0,
                amane::Key::End => o.selected = o.keys.len() - 1,
                amane::Key::Up => o.selected = (o.selected + o.keys.len() - 1) % o.keys.len(),
                amane::Key::Down => o.selected = (o.selected + 1) % o.keys.len(),
                amane::Key::Enter | amane::Key::Space => {
                    return Navigation::Overflow(o.keys[o.selected].clone(), o.anchor.clone());
                }
                _ => return Navigation::None,
            }
            if o.selected < o.scroll_rows {
                o.scroll_rows = o.selected;
            } else if o.selected >= o.scroll_rows + visible_count {
                o.scroll_rows = o.selected + 1 - visible_count.max(1);
            }
            return Navigation::None;
        }
        let Some(menu) = self.menu_mut() else {
            return Navigation::None;
        };
        let Some(tree) = &menu.tree else {
            return Navigation::None;
        };
        if key == amane::Key::Left && menu.path.len() > 1 {
            menu.selected = menu.path.pop();
            menu.scroll_rows = 0;
            return Navigation::None;
        }
        let Some(parent) = menu.path.last().and_then(|id| tree.nodes.get(id)) else {
            return Navigation::None;
        };
        let rows: Vec<_> = parent
            .children
            .iter()
            .filter_map(|id| tree.nodes.get(id))
            .filter(|n| n.visible)
            .collect();
        let actions: Vec<_> = rows
            .iter()
            .filter(|n| n.enabled && n.kind != MenuKind::Separator)
            .collect();
        if actions.is_empty() {
            return Navigation::None;
        }
        let index = actions
            .iter()
            .position(|n| Some(n.id) == menu.selected)
            .unwrap_or(0);
        let target = match key {
            amane::Key::Home => 0,
            amane::Key::End => actions.len() - 1,
            amane::Key::Up => (index + actions.len() - 1) % actions.len(),
            amane::Key::Down => (index + 1) % actions.len(),
            amane::Key::Right | amane::Key::Enter | amane::Key::Space => {
                let row = actions[index];
                return if row.has_submenu {
                    Navigation::Open(menu.request, row.id)
                } else if key != amane::Key::Right {
                    Navigation::Activate(menu.request, row.id)
                } else {
                    Navigation::None
                };
            }
            _ => return Navigation::None,
        };
        menu.selected = Some(actions[target].id);
        if let Some(index) = rows.iter().position(|n| Some(n.id) == menu.selected) {
            if index < menu.scroll_rows {
                menu.scroll_rows = index;
            } else if index >= menu.scroll_rows + visible_count {
                menu.scroll_rows = index + 1 - visible_count.max(1);
            }
        }
        Navigation::None
    }
    pub fn menu(&self) -> Option<&MenuSession> {
        match &self.popup {
            Some(PopupSession::Menu(menu)) => Some(menu),
            _ => None,
        }
    }
    pub fn menu_mut(&mut self) -> Option<&mut MenuSession> {
        match &mut self.popup {
            Some(PopupSession::Menu(menu)) => Some(menu),
            _ => None,
        }
    }
    pub fn begin_menu(&mut self, key: ItemKey, anchor: SlotAnchor, menu_path: String) -> u64 {
        let request = REQUEST.fetch_add(1, Ordering::Relaxed);
        self.popup = Some(PopupSession::Menu(MenuSession {
            request,
            key,
            anchor,
            menu_path,
            tree: None,
            path: vec![0],
            selected: None,
            scroll_rows: 0,
        }));
        self.tooltip = None;
        request
    }
    pub fn accept_layout(&mut self, request: u64, parent: i32, tree: MenuTree) -> bool {
        let Some(menu) = self.menu_mut().filter(|m| m.request == request) else {
            return false;
        };
        if tree.root != parent {
            return false;
        }
        let mut merged = if parent == 0 {
            tree.clone()
        } else {
            let Some(old) = &menu.tree else {
                return false;
            };
            if !old.nodes.contains_key(&parent) {
                return false;
            }
            let mut merged = old.as_ref().clone();
            fn remove(tree: &mut MenuTree, id: i32) {
                if let Some(node) = tree.nodes.remove(&id) {
                    for child in node.children {
                        remove(tree, child);
                    }
                }
            }
            remove(&mut merged, parent);
            if tree.nodes.keys().any(|id| merged.nodes.contains_key(id)) {
                return false;
            }
            merged.nodes.extend(tree.nodes);
            merged.revision = tree.revision;
            merged
        };
        if merged.nodes.len() > 512 {
            return false;
        }
        // Retain already fetched branches only when the refreshed parent still contains their IDs.
        if parent == 0
            && let Some(old) = &menu.tree
        {
            for node in merged.nodes.values_mut() {
                if node.has_submenu
                    && node.children.is_empty()
                    && let Some(previous) = old.nodes.get(&node.id)
                {
                    node.children = previous.children.clone();
                }
            }
            let mut pending: Vec<_> = merged
                .nodes
                .values()
                .flat_map(|n| n.children.iter().copied())
                .collect();
            while let Some(id) = pending.pop() {
                if !merged.nodes.contains_key(&id)
                    && let Some(node) = old.nodes.get(&id)
                {
                    if merged.nodes.len() >= 512 {
                        return false;
                    }
                    pending.extend(node.children.iter().copied());
                    merged.nodes.insert(id, node.clone());
                }
            }
        }
        fn valid_depth(tree: &MenuTree, id: i32, depth: usize) -> bool {
            depth <= 16
                && tree.nodes.get(&id).is_some_and(|node| {
                    node.children
                        .iter()
                        .all(|child| valid_depth(tree, *child, depth + 1))
                })
        }
        if !valid_depth(&merged, merged.root, 1) {
            return false;
        }
        let keep = menu
            .path
            .iter()
            .enumerate()
            .take_while(|(i, id)| {
                merged.nodes.contains_key(id)
                    && (*i == 0
                        || merged
                            .nodes
                            .get(&menu.path[*i - 1])
                            .is_some_and(|n| n.children.contains(id)))
            })
            .count();
        menu.path.truncate(keep);
        if menu.path.is_empty() {
            menu.path.push(0);
        }
        let rows = merged
            .nodes
            .get(menu.path.last().unwrap_or(&0))
            .map(|n| n.children.as_slice())
            .unwrap_or(&[]);
        let valid = |id: &i32| {
            merged
                .nodes
                .get(id)
                .is_some_and(|n| n.enabled && n.visible && n.kind != MenuKind::Separator)
        };
        menu.selected = menu
            .selected
            .filter(|id| rows.contains(id) && valid(id))
            .or_else(|| rows.iter().find(|id| valid(id)).copied());
        menu.tree = Some(Arc::new(merged));
        true
    }
    pub fn row_valid(&self, request: u64, row_id: i32) -> bool {
        self.menu()
            .filter(|m| m.request == request)
            .and_then(|m| m.tree.as_ref())
            .and_then(|t| t.nodes.get(&row_id))
            .is_some_and(|n| n.enabled && n.visible && n.kind != MenuKind::Separator)
    }
    pub fn reconcile(&mut self, live: &[ItemKey], outputs: &[String], enabled: bool) {
        if !enabled {
            self.popup = None;
            self.tooltip = None;
            return;
        }
        if let Some(PopupSession::Overflow(overflow)) = &mut self.popup {
            overflow.keys.retain(|k| live.contains(k));
        }
        let invalid = self.popup.as_ref().is_some_and(|p| match p {
            PopupSession::Menu(m) => !live.contains(&m.key) || !outputs.contains(&m.anchor.output),
            PopupSession::Overflow(o) => o.keys.is_empty() || !outputs.contains(&o.anchor.output),
        });
        if invalid {
            self.popup = None;
        }
        if self
            .tooltip
            .as_ref()
            .is_some_and(|t| !live.contains(&t.key) || !outputs.contains(&t.anchor.output))
        {
            self.tooltip = None;
        }
    }
}

pub(crate) enum Navigation {
    None,
    Open(u64, i32),
    Activate(u64, i32),
    Overflow(ItemKey, SlotAnchor),
}

pub(crate) fn select_row(request: u64, row_id: i32) {
    let state = TrayUi::read();
    if !state.row_valid(request, row_id) || state.menu().is_some_and(|m| m.selected == Some(row_id))
    {
        return;
    }
    drop(state);
    let mut state = TrayUi::write();
    if !state.row_valid(request, row_id) {
        return;
    }
    if let Some(menu) = state.menu_mut() {
        if let Some(depth) = menu.path.iter().position(|id| {
            menu.tree
                .as_ref()
                .and_then(|t| t.nodes.get(id))
                .is_some_and(|n| n.children.contains(&row_id))
        }) {
            menu.path.truncate(depth + 1);
        }
        menu.selected = Some(row_id);
    }
}
pub(crate) fn enter_submenu(request: u64, row_id: i32) {
    let state = TrayUi::read();
    if !state.row_valid(request, row_id)
        || !state
            .menu()
            .and_then(|m| m.tree.as_ref())
            .and_then(|t| t.nodes.get(&row_id))
            .is_some_and(|n| n.has_submenu)
    {
        return;
    }
    let Some(menu) = state.menu().cloned() else {
        return;
    };
    drop(state);
    select_row(request, row_id);
    let mut state = TrayUi::write();
    if let Some(current) = state.menu_mut().filter(|m| m.request == request) {
        if current.path.contains(&row_id) {
            return;
        }
        current.path.push(row_id);
        current.selected = None;
        current.scroll_rows = 0;
    } else {
        return;
    }
    drop(state);
    super::menu::client::show(menu.key, menu.menu_path, request, row_id);
}

pub(crate) fn open_menu(key: ItemKey, anchor: SlotAnchor) {
    let path = super::Tray::read()
        .snapshot()
        .items
        .iter()
        .find(|i| i.key == key)
        .and_then(|i| i.properties.menu.clone());
    let Some(path) = path else {
        super::actions::submit(key, super::actions::ItemAction::ContextMenu, anchor);
        return;
    };
    let request = TrayUi::write().begin_menu(key.clone(), anchor, path.clone());
    super::menu::client::show(key, path, request, 0);
}
pub(crate) fn open_overflow(anchor: SlotAnchor, keys: Vec<ItemKey>) {
    let mut ui = TrayUi::write();
    ui.popup = Some(PopupSession::Overflow(OverflowSession {
        anchor,
        keys,
        selected: 0,
        scroll_rows: 0,
    }));
    ui.tooltip = None;
}
pub(crate) fn close() {
    let mut ui = TrayUi::write();
    ui.popup = None;
    ui.tooltip = None;
}
pub(crate) fn close_request(request: u64) {
    let mut ui = TrayUi::write();
    if ui.menu().is_some_and(|m| m.request == request) {
        ui.popup = None;
        ui.tooltip = None;
    }
}
pub(crate) fn hover(item: Option<(ItemKey, SlotAnchor)>) {
    let ui = TrayUi::read();
    if matches!(ui.popup, Some(PopupSession::Menu(_)))
        || match &item {
            Some((key, _)) => ui.tooltip.as_ref().is_some_and(|t| &t.key == key),
            None => ui.tooltip.is_none(),
        }
    {
        return;
    }
    drop(ui);
    TrayUi::write().tooltip = item.map(|(key, anchor)| TooltipSession {
        key,
        anchor,
        shown: false,
        deadline: Instant::now() + Duration::from_millis(400),
    });
}
pub(crate) fn keep_bar_visible(output: &str) -> bool {
    TrayUi::read().holds_bar(output)
}
pub(crate) fn reconcile(live: &[ItemKey], outputs: &[String], enabled: bool) {
    let state = TrayUi::read();
    if state.popup.is_none() && state.tooltip.is_none() {
        return;
    }
    let invalid = !enabled
        || state
            .tooltip
            .as_ref()
            .is_some_and(|t| !live.contains(&t.key) || !outputs.contains(&t.anchor.output))
        || state.popup.as_ref().is_some_and(|p| match p {
            PopupSession::Menu(m) => !live.contains(&m.key) || !outputs.contains(&m.anchor.output),
            PopupSession::Overflow(o) => {
                o.keys.iter().any(|k| !live.contains(k)) || !outputs.contains(&o.anchor.output)
            }
        });
    if !invalid {
        return;
    }
    drop(state);
    TrayUi::write().reconcile(live, outputs, enabled);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tray::menu::{decode_layout, tests::node};
    use zbus::zvariant::OwnedValue;

    fn key(owner: &str) -> ItemKey {
        ItemKey {
            owner: owner.into(),
            path: "/Item".into(),
            generation: 1,
        }
    }
    fn anchor() -> SlotAnchor {
        SlotAnchor {
            output: "DP-9".into(),
            x: 10.0,
            y: 7.0,
            width: 26.0,
            height: 26.0,
        }
    }

    #[test]
    fn dynamic_selection_cannot_activate_removed_rows() {
        let mut ui = TrayUi::default();
        let request = ui.begin_menu(key(":1.1"), anchor(), "/Menu".into());
        ui.accept_layout(
            request,
            0,
            decode_layout(
                1,
                node(0, "", vec![node(1, "One", vec![]), node(2, "Two", vec![])]),
            )
            .unwrap(),
        );
        ui.menu_mut().unwrap().selected = Some(2);
        ui.accept_layout(
            request,
            0,
            decode_layout(
                2,
                node(0, "", vec![node(2, "Two", vec![]), node(1, "One", vec![])]),
            )
            .unwrap(),
        );
        assert_eq!(ui.menu().unwrap().selected, Some(2));
        let mut disabled = node(2, "Two", vec![]);
        disabled.1.insert("enabled".into(), OwnedValue::from(false));
        ui.accept_layout(
            request,
            0,
            decode_layout(3, node(0, "", vec![disabled, node(1, "One", vec![])])).unwrap(),
        );
        assert!(!ui.row_valid(request, 2));
        ui.accept_layout(
            request,
            0,
            decode_layout(4, node(0, "", vec![node(1, "One", vec![])])).unwrap(),
        );
        assert!(!ui.row_valid(request, 2));
        assert!(ui.row_valid(request, 1));
    }

    #[test]
    fn stale_menu_replies_cannot_replace_new_popup() {
        let mut ui = TrayUi::default();
        let old = ui.begin_menu(key(":1.1"), anchor(), "/OldMenu".into());
        let new = ui.begin_menu(key(":1.2"), anchor(), "/NewMenu".into());
        assert!(!ui.accept_layout(
            old,
            0,
            decode_layout(1, node(0, "", vec![node(1, "stale", vec![])])).unwrap()
        ));
        assert_eq!(ui.menu().unwrap().request, new);
        assert_eq!(ui.menu().unwrap().menu_path, "/NewMenu");
        assert!(!ui.row_valid(old, 1));
        ui.reconcile(&[], &["DP-9".into()], true);
        assert!(ui.popup.is_none());
    }

    #[test]
    fn lazy_submenus_cannot_replace_ancestors_or_exceed_depth() {
        let mut ui = TrayUi::default();
        let request = ui.begin_menu(key(":1.1"), anchor(), "/Menu".into());
        assert!(ui.accept_layout(
            request,
            0,
            decode_layout(1, node(0, "", vec![node(1, "branch", vec![])])).unwrap()
        ));
        assert!(!ui.accept_layout(
            request,
            1,
            decode_layout(2, node(1, "branch", vec![node(0, "cycle", vec![])])).unwrap()
        ));
        for id in 1..15 {
            assert!(ui.accept_layout(
                request,
                id,
                decode_layout(3, node(id, "branch", vec![node(id + 1, "branch", vec![])])).unwrap()
            ));
        }
        assert!(!ui.accept_layout(
            request,
            15,
            decode_layout(4, node(15, "branch", vec![node(16, "too deep", vec![])])).unwrap()
        ));
    }

    #[test]
    fn keyboard_navigation_skips_non_actions() {
        let mut ui = TrayUi::default();
        let request = ui.begin_menu(key(":1.1"), anchor(), "/Menu".into());
        let mut disabled = node(2, "disabled", vec![]);
        disabled.1.insert("enabled".into(), OwnedValue::from(false));
        ui.accept_layout(
            request,
            0,
            decode_layout(
                1,
                node(
                    0,
                    "",
                    vec![node(1, "first", vec![]), disabled, node(3, "last", vec![])],
                ),
            )
            .unwrap(),
        );
        ui.navigate(amane::Key::Down, 10);
        assert_eq!(ui.menu().unwrap().selected, Some(3));
        ui.navigate(amane::Key::Home, 10);
        assert_eq!(ui.menu().unwrap().selected, Some(1));
        ui.navigate(amane::Key::Escape, 10);
        assert!(ui.popup.is_none());
    }
    #[test]
    fn tooltip_delay_and_teardown() {
        let mut ui = TrayUi::default();
        let now = Instant::now();
        ui.tooltip = Some(TooltipSession {
            key: key(":1.1"),
            anchor: anchor(),
            shown: false,
            deadline: now + Duration::from_millis(400),
        });
        assert!(!ui.show_tooltip(now + Duration::from_millis(399)));
        assert!(ui.show_tooltip(now + Duration::from_millis(400)));
        assert!(ui.tooltip.as_ref().unwrap().shown);
        ui.begin_menu(key(":1.1"), anchor(), "/Menu".into());
        assert!(ui.tooltip.is_none());
        ui.reconcile(&[key(":1.1")], &["DP-9".into()], false);
        assert!(ui.popup.is_none());
    }
    #[test]
    fn menu_holds_only_its_output_bar() {
        let mut ui = TrayUi::default();
        ui.begin_menu(key(":1.1"), anchor(), "/Menu".into());
        assert!(ui.holds_bar("DP-9"));
        assert!(!ui.holds_bar("eDP-1"));
        ui.reconcile(&[key(":1.1")], &["eDP-1".into()], true);
        assert!(ui.popup.is_none());
    }
}
