use std::path::PathBuf;
use std::sync::Arc;

use amane::Image;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct ItemKey {
    pub owner: String,
    pub path: String,
    pub generation: u64,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ItemStatus {
    #[default]
    Active,
    Passive,
    NeedsAttention,
}

pub(crate) type RawPixmap = (i32, i32, Vec<u8>);

#[derive(Default, Clone, PartialEq)]
pub(crate) struct IconSpec {
    pub name: String,
    pub pixmaps: Vec<RawPixmap>,
}

#[derive(Default, Clone, PartialEq)]
pub(crate) struct IconSources {
    pub theme_path: Option<PathBuf>,
    pub normal: IconSpec,
    pub attention: IconSpec,
    pub overlay: IconSpec,
}

#[derive(Default, Clone, PartialEq)]
pub(crate) struct ItemIcon {
    pub normal: Option<Image>,
    pub attention: Option<Image>,
    pub overlay: Option<Image>,
}

#[derive(Default, Clone, PartialEq)]
pub(crate) struct TooltipText {
    pub title: String,
    pub description: String,
}

#[derive(Default, Clone, PartialEq)]
pub(crate) struct ItemProperties {
    pub id: String,
    pub title: String,
    pub status: ItemStatus,
    pub item_is_menu: bool,
    pub menu: Option<String>,
    pub tooltip: TooltipText,
    pub icons: IconSources,
}

#[derive(Clone, PartialEq)]
pub(crate) struct TrayItem {
    pub key: ItemKey,
    pub properties: ItemProperties,
    pub icon: ItemIcon,
}

#[derive(Default, Clone, PartialEq)]
pub(crate) struct TraySnapshot {
    pub revision: u64,
    pub items: Vec<Arc<TrayItem>>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SlotAnchor {
    pub output: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}
