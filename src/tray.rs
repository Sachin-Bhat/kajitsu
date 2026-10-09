pub(crate) mod actions;
pub(crate) mod menu;
pub(crate) mod ui;
mod backend;
mod icons;
mod item;
pub(crate) mod model;
mod registry;
mod watcher;

use amane::Service;
use model::TraySnapshot;

#[derive(Default)]
pub(crate) struct Tray {
    snapshot: TraySnapshot,
}

impl Service for Tray {
    fn new() -> Self {
        Self::default()
    }
    fn listen() {
        backend::run();
    }
}

impl Tray {
    pub fn snapshot(&self) -> &TraySnapshot {
        &self.snapshot
    }
}

fn publish(snapshot: TraySnapshot) {
    if Tray::read().snapshot != snapshot {
        Tray::write().snapshot = snapshot;
    }
}
