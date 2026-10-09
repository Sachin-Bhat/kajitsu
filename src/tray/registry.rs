use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use super::model::{ItemIcon, ItemKey, ItemProperties, ItemStatus, TrayItem, TraySnapshot};

static GENERATION: AtomicU64 = AtomicU64::new(1);

struct Entry {
    advertised: String,
    item: Arc<TrayItem>,
}

#[derive(Default)]
pub(crate) struct Registry {
    entries: Vec<Entry>,
    revision: u64,
}

impl Registry {
    pub fn register(&mut self, owner: String, path: String, advertised: String) -> ItemKey {
        if let Some(entry) = self
            .entries
            .iter()
            .find(|e| e.item.key.owner == owner && e.item.key.path == path)
        {
            return entry.item.key.clone();
        }
        let key = ItemKey {
            owner,
            path,
            generation: GENERATION.fetch_add(1, Ordering::Relaxed),
        };
        self.entries.push(Entry {
            advertised,
            item: Arc::new(TrayItem {
                key: key.clone(),
                properties: ItemProperties::default(),
                icon: ItemIcon::default(),
            }),
        });
        self.revision += 1;
        key
    }

    pub fn update(&mut self, key: &ItemKey, properties: ItemProperties, icon: ItemIcon) -> bool {
        let before = self.visible();
        let Some(entry) = self.entries.iter_mut().find(|e| &e.item.key == key) else {
            return false;
        };
        if entry.item.properties == properties && entry.item.icon == icon {
            return false;
        }
        entry.item = Arc::new(TrayItem {
            key: key.clone(),
            properties,
            icon,
        });
        self.bump_if_changed(before);
        true
    }

    pub fn remove_owner(&mut self, owner: &str) -> Vec<ItemKey> {
        let removed: Vec<_> = self
            .entries
            .iter()
            .filter(|e| e.item.key.owner == owner)
            .map(|e| e.item.key.clone())
            .collect();
        let before = self.visible();
        self.entries.retain(|e| e.item.key.owner != owner);
        self.bump_if_changed(before);
        removed
    }

    pub fn remove(&mut self, key: &ItemKey) {
        let before = self.visible();
        self.entries.retain(|e| &e.item.key != key);
        self.bump_if_changed(before);
    }

    pub fn clear(&mut self) {
        if !self.visible().is_empty() {
            self.revision += 1;
        }
        self.entries.clear();
    }

    pub fn contains(&self, key: &ItemKey) -> bool {
        self.entries.iter().any(|e| &e.item.key == key)
    }

    pub fn find(&self, owner: &str, path: &str) -> Option<ItemKey> {
        self.entries
            .iter()
            .find(|e| e.item.key.owner == owner && e.item.key.path == path)
            .map(|e| e.item.key.clone())
    }

    pub fn ids(&self) -> Vec<String> {
        self.entries.iter().map(|e| e.advertised.clone()).collect()
    }

    pub fn owner_ids(&self, owner: &str) -> Vec<String> {
        self.entries
            .iter()
            .filter(|e| e.item.key.owner == owner)
            .map(|e| e.advertised.clone())
            .collect()
    }

    pub fn keys(&self) -> Vec<ItemKey> {
        self.entries.iter().map(|e| e.item.key.clone()).collect()
    }

    fn visible(&self) -> Vec<Arc<TrayItem>> {
        self.entries
            .iter()
            .filter(|e| e.item.properties.status != ItemStatus::Passive)
            .map(|e| Arc::clone(&e.item))
            .collect()
    }

    fn bump_if_changed(&mut self, before: Vec<Arc<TrayItem>>) {
        if self.visible() != before {
            self.revision += 1;
        }
    }

    pub fn snapshot(&self) -> TraySnapshot {
        TraySnapshot {
            revision: self.revision,
            items: self.visible(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tray::model::{ItemIcon, ItemProperties, ItemStatus};

    #[test]
    fn registry_deduplicates_and_rejects_stale_owners() {
        let mut registry = Registry::default();
        let first = registry.register(":1.1".into(), "/One".into(), "app/One".into());
        assert_eq!(
            first,
            registry.register(":1.1".into(), "/One".into(), "app/One".into())
        );
        assert_ne!(
            first,
            registry.register(":1.1".into(), "/Two".into(), "app/Two".into())
        );
        assert_eq!(registry.remove_owner(":1.1").len(), 2);
        let restarted = registry.register(":1.2".into(), "/One".into(), "app/One".into());
        assert_ne!(first, restarted);
        assert!(!registry.update(&first, ItemProperties::default(), ItemIcon::default()));
        registry.clear();
        let reused_bus_name = registry.register(":1.1".into(), "/One".into(), "app/One".into());
        assert_ne!(first, reused_bus_name);
        assert_ne!(restarted, reused_bus_name);
    }

    #[test]
    fn passive_items_restore_in_registration_order() {
        let mut registry = Registry::default();
        let keys: Vec<_> = (0..3)
            .map(|n| registry.register(":1.1".into(), format!("/Item{n}"), format!("app/Item{n}")))
            .collect();
        let properties = |status| ItemProperties {
            status,
            ..Default::default()
        };
        registry.update(
            &keys[0],
            properties(ItemStatus::Active),
            ItemIcon::default(),
        );
        registry.update(
            &keys[1],
            properties(ItemStatus::Passive),
            ItemIcon::default(),
        );
        registry.update(
            &keys[2],
            properties(ItemStatus::NeedsAttention),
            ItemIcon::default(),
        );
        assert_eq!(
            registry
                .snapshot()
                .items
                .iter()
                .map(|i| i.key.clone())
                .collect::<Vec<_>>(),
            [keys[0].clone(), keys[2].clone()]
        );
        let revision = registry.snapshot().revision;
        assert!(!registry.update(
            &keys[0],
            properties(ItemStatus::Active),
            ItemIcon::default()
        ));
        assert_eq!(registry.snapshot().revision, revision);
        let mut hidden = properties(ItemStatus::Passive);
        hidden.title = "a hidden change".into();
        registry.update(&keys[1], hidden, ItemIcon::default());
        assert_eq!(registry.snapshot().revision, revision);
        registry.update(
            &keys[1],
            properties(ItemStatus::Active),
            ItemIcon::default(),
        );
        assert_eq!(
            registry
                .snapshot()
                .items
                .iter()
                .map(|i| i.key.clone())
                .collect::<Vec<_>>(),
            keys
        );
    }
}
