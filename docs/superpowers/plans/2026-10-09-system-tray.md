# Native System Tray Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Display and operate applications' native system-tray icons and menus in Kajitsu's daily Artix/Mango session.

**Architecture:** Kajitsu owns a shared StatusNotifier registry, background protocol workers, and separate popup state. Existing amane widgets draw measured per-monitor icon slots and overlay menus; two additive image helpers in the local amane build give changing icons bounded ownership. Protocol decoding and geometry remain independent of rendering.

**Tech Stack:** Rust 2024, local `../amane`, zbus 5.19.0 / zvariant 5.15.0, StatusNotifierItem, DBusMenu, native Mango IPC, Python and Wayland fixture drivers.

**Spec:** [Approved system-tray design](../specs/2026-10-09-system-tray-design.md).

## Global Constraints

- Continue building against `../amane`; preserve unrelated dependency and desktop changes.
- Declare zbus 5.19.0 directly; retain locked zvariant 5.15.0 and avoid a dependency upgrade sweep.
- Support `org.kde` and `org.freedesktop` StatusNotifier interfaces; export `/StatusNotifierWatcher` with protocol version zero.
- Acquire watcher names without replacing existing owners; become a host when a watcher already exists.
- Use 18px icons, 26px slots, 4px gaps, 8px horizontal padding, selected-surface background, and registration order.
- Show Active/NeedsAttention; track Passive without displaying it; no empty pill.
- Add `bar_systray=true`, labelled **System tray**; retain `bar_tray` and label it **Utility shortcuts**.
- Tooltip delay is 400ms; descriptions are plain text and tooltips take no input.
- D-Bus method timeout is three seconds; no D-Bus/filesystem work in drawing or across service locks.
- Raw pixmaps have checked dimensions, exact byte counts, network-order ARGB conversion, and a 1024px maximum edge.
- A received menu tree permits at most 512 nodes and 16 levels; submenu fetching is lazy.
- One menu session at a time; close on owner/output removal, disabling the setting, or session locking.
- Hidden popup views take no input; hold an auto-hidden bar visible on the menu's output.
- Native validation uses an isolated bus and two headless outputs at scales 1 and 1.25.
- Install only after review and passing checks, with a binary backup and atomic replacement; restart only an unlocked session.
- XEmbed-only icons and animated attention movies are outside this implementation.

## Review Focus

1. An application restarts while properties or a menu fetch are pending: old replies cannot overwrite the new owner or popup. Tasks 2 and 4.
2. A preferred named icon is corrupt or theme inheritance cycles: resolution terminates and falls back to a usable pixmap. Task 3.
3. A dynamic menu repeats IDs or disables/removes the selected entry: reject ambiguous trees and never send an event for an invalid selection. Task 4.
4. Recording widens the utility launcher or the right group becomes narrow: recompute overflow without overlapping other controls. Task 5.
5. Repeated popup opening/dismissal changes Wayland roles: Escape works immediately and hidden views release desktop/panel input. Tasks 6 and 7.

---

## Files and responsibilities

| Files | Responsibility |
| --- | --- |
| `../amane/src/style/image.rs`, `../amane/src/graphics/image.rs`, `../amane/src/widgets/rectangle/draw.rs` | Owned images, checked construction, existing renderer integration |
| `src/tray.rs`, `src/tray/model.rs` | Service facade and shared item/anchor types |
| `src/tray/registry.rs`, `src/tray/watcher.rs`, `src/tray/backend.rs`, `src/tray/item.rs` | Pure registry, typed watcher, connection lifecycle, typed item properties |
| `src/tray/icons.rs`, `src/tray/actions.rs` | Background icon resolution and serialized application calls |
| `src/tray/menu.rs`, `src/tray/menu/client.rs` | Bounded menu model and live DBusMenu client |
| `src/tray/ui.rs`, `src/tray/popup.rs`, `src/tray/popup/geometry.rs` | Popup/tooltip state, widgets, navigation, placement |
| `src/bar/systray.rs`, `src/bar/system.rs`, `src/bar.rs`, `src/bar/reveal.rs` | Slot geometry, remaining-width calculation, per-monitor input, reveal hold |
| `src/mango.rs`, `src/main.rs` | Fresh native output geometry, service startup and popup views |
| `src/settings/store.rs`, `src/settings/pages/bar.rs`, `src/lock_screen/curtain.rs` | Settings and popup teardown hooks |
| `examples/systray-fixture.rs`, `examples/systray_fixture/menu.rs`, `examples/systray_fixture/watcher.rs` | Controlled items/menu, existing-watcher mode, call log and test controls |
| `scripts/check-mango-systray.py`, `scripts/mango-keyboard.c`, `scripts/mango-pointer.c` | Disposable native scenarios and real pointer/keyboard input |
| `Cargo.toml`, `Cargo.lock`, `README.md`, `docs/desktop-setup.md` | Direct dependency and user/validation documentation |

Tests live beside their Rust modules under `#[cfg(test)]`; add module declarations with each new test module so a filtered command cannot silently pass with zero tests. The native script tests the compiled binary. Shared types/fields and cross-module functions below use `pub(crate)` visibility; implementation details stay private. Existing panel/layout scripts remain the regression baseline. Check both repositories' dirty state before edits. Each commit below stages only that task's changes; use patch staging for files with existing unrelated changes, and commit amane changes in its own repository.

All shell commands below begin with `rtk`. Cargo commands clear the inherited `RUSTC_WRAPPER`. Native checks, sibling amane writes, host installation and Git mutations require the environment's escalation mechanism when sandboxed; request that mechanism using the already-authorized task scope.

### Task 1: Owned decoded images in local amane

**Files:** Modify/Test the three amane image/rendering files in the table.

**Interfaces:** Produce `Image::from_rgba(width: u32, height: u32, pixels: Vec<u8>) -> Option<Self>`, `Image::read_owned(path: impl AsRef<Path>) -> Option<Self>`, and Clone/PartialEq for Image. Owned-image equality compares dimensions/pixels and drawing options, so repeated identical updates do not force redraws. Internally produce `Bitmap::from_rgba(width: u32, height: u32, pixels: Vec<u8>) -> Option<Self>` and `Image::bitmap(&self) -> Option<Arc<Bitmap>>`. Existing cover/contain/stretch/thumbnail/blurred/loaded APIs retain their behavior; owned constructors use Contain.

- [ ] Write `owned_rgba_rejects_invalid_buffers`, `owned_images_release_replaced_pixels`, and `owned_file_read_bypasses_path_cache` in the amane image modules. Build PNG/SVG fixtures in a temporary directory, and inspect the private bitmap in these internal tests:

```rust
assert!(Image::from_rgba(0, 1, vec![]).is_none());
assert!(Image::from_rgba(u32::MAX, u32::MAX, vec![]).is_none());
assert!(Image::from_rgba(1, 1, vec![1, 2, 3]).is_none());
assert_eq!(&*image.bitmap().unwrap().pixels, &[1, 2, 3, 4]);
assert_eq!(&*reread.bitmap().unwrap().pixels, &[5, 6, 7, 8]);
assert!(same_pixels_in_a_new_allocation == image);
assert!(reread != image);
assert!(old_bitmap_weak.upgrade().is_none());
```

- [ ] Run `rtk proxy env RUSTC_WRAPPER= cargo test --manifest-path ../amane/Cargo.toml --locked owned_`. Expect compile failure for the missing constructors.
- [ ] Implement the interfaces in `style/image.rs` and `graphics/image.rs`, retaining either a path source or an `Arc<Bitmap>`. Resolve that source in rectangle drawing; validate dimensions with checked arithmetic before allocation. Reuse the synchronous PNG/JPEG/SVG decoder without adding entries to `LOADED`. Apply owned thumbnail/blur transformations once, outside drawing, when requested. Confirm the existing GPU forget path releases unshown owned images.
- [ ] Run the targeted tests, `rtk proxy env RUSTC_WRAPPER= cargo test --manifest-path ../amane/Cargo.toml --locked`, `rtk proxy env RUSTC_WRAPPER= cargo clippy --manifest-path ../amane/Cargo.toml --locked --all-targets --all-features -- -D warnings`, and `rtk proxy env RUSTC_WRAPPER= cargo fmt --manifest-path ../amane/Cargo.toml --all --check`; expect success without changed path-image behavior.
- [ ] Commit only the three amane files: `rtk git -C ../amane commit -m "feat: support owned decoded images"` after scoped staging.

### Task 2: Typed registry, watcher ownership, and discovery

**Files:** Create `src/tray.rs`, `src/tray/model.rs`, `src/tray/registry.rs`, `src/tray/watcher.rs`, `src/tray/backend.rs`, `src/tray/item.rs`; modify `src/main.rs`, `Cargo.toml`, `Cargo.lock`. Test registry/item modules and isolated watcher tests in `src/tray/backend.rs`.

**Interfaces:** Task 1 supplies cloneable/comparable images. Define these shared types in `model.rs`, with Clone/PartialEq on item data, equality/hash on `ItemKey`, and Default for `ItemIcon`:

```rust
struct ItemKey { owner: String, path: String, generation: u64 }
enum ItemStatus { Active, Passive, NeedsAttention }
type RawPixmap = (i32, i32, Vec<u8>);
struct IconSpec { name: String, pixmaps: Vec<RawPixmap> }
struct IconSources { theme_path: Option<PathBuf>, normal: IconSpec, attention: IconSpec, overlay: IconSpec }
struct ItemIcon { normal: Option<Image>, attention: Option<Image>, overlay: Option<Image> }
struct TooltipText { title: String, description: String }
struct ItemProperties { id: String, title: String, status: ItemStatus, item_is_menu: bool,
    menu: Option<String>, tooltip: TooltipText, icons: IconSources }
struct TrayItem { key: ItemKey, properties: ItemProperties, icon: ItemIcon }
struct TraySnapshot { revision: u64, items: Vec<Arc<TrayItem>> }
struct SlotAnchor { output: String, x: f32, y: f32, width: f32, height: f32 }
```

Produce `Registry::register(&mut self, owner: String, path: String, advertised: String) -> ItemKey`, `Registry::remove_owner(&mut self, owner: &str) -> Vec<ItemKey>`, `Registry::update(&mut self, key: &ItemKey, properties: ItemProperties, icon: ItemIcon) -> bool`, and `Registry::snapshot(&self) -> TraySnapshot`. Produce `Tray: Service` with `Tray::snapshot(&self) -> &TraySnapshot`, and `backend::connection() -> Option<(u64, zbus::blocking::Connection)>`; the integer identifies a connection generation. `backend::is_current(key: &ItemKey, connection_generation: u64) -> bool` protects subsequent workers. Define `item::read(connection: &Connection, key: &ItemKey) -> zbus::Result<ItemProperties>` using the blocking Connection type.

Export watcher methods `RegisterStatusNotifierItem(s)` and `RegisterStatusNotifierHost(s)`, properties `RegisteredStatusNotifierItems: as`, `IsStatusNotifierHostRegistered: b`, `ProtocolVersion: i`, and item registered/unregistered and host registered lifecycle signals. Advertised item IDs retain their bus-name/object-path representation; internal deduplication uses unique owner/path.

- [ ] Write `registry_deduplicates_and_rejects_stale_owners`, `passive_items_restore_in_registration_order`, and isolated `watcher_owns_or_hosts_without_replacement`. Test both registration forms, both interface names, two paths per owner, host/item lifecycle signals, subscribe-before-snapshot reconciliation, and bus/watcher replacement. Local fixtures supply decoded properties and an independent bus:

```rust
assert_eq!(first_key, duplicate_key);
assert_ne!(first_key, second_path_key);
assert_ne!(first_key, restarted_key);
assert_ne!(before_bus_restart_key, after_bus_restart_key);
assert!(!registry.update(&first_key, old_reply, ItemIcon::default()));
assert_eq!(registry.snapshot().items.len(), 2); // Active + NeedsAttention, not Passive
assert_eq!(watcher.protocol_version(), 0);
assert_eq!(external_watcher_owner_before, external_watcher_owner_after);
assert_eq!(unchanged_visible_revision_before, unchanged_visible_revision_after);
```

- [ ] Run `rtk proxy env RUSTC_WRAPPER= cargo test --locked tray::`. Expect missing module/interfaces until the module is declared; then new tests must fail for absent behavior.
- [ ] Add `zbus = "=5.19.0"` and refresh only Kajitsu's lockfile dependency entry with `rtk proxy env RUSTC_WRAPPER= cargo check --offline`; verify transitive versions remain unchanged.
- [ ] Implement registry and typed watcher/item interfaces, including sender-header resolution for path registration and unique-owner resolution for named registration. Start `Tray::read()` before the first frame in `main.rs`. Subscribe before enumeration and follow NewIcon/NewAttentionIcon/NewOverlayIcon/NewStatus/NewTitle/NewToolTip and PropertiesChanged. Coalesce per-item refreshes, publish only changed visible snapshots, remove all paths on owner loss, and reject generation-mismatched replies; generation IDs remain monotonic across reconnection even when a new bus reuses unique names. Use bounded refresh work, three-second calls, and reconnect backoff starting at 250ms and capped at five seconds. If ownership differs between compatibility names, follow/deduplicate each registry without taking another process's name. Initially publish default icon values; Task 3 supplies decoded icons.
- [ ] Run `rtk proxy env RUSTC_WRAPPER= cargo test --locked tray::`; expect all new registry/protocol tests to pass, including timeout and reconnect cases on disposable buses.
- [ ] Commit only Task 2 paths with `rtk git commit -m "feat: discover native tray items"`; retain all pre-existing lockfile/dependency changes.

### Task 3: Icon resolution and bounded item actions

**Files:** Create/Test `src/tray/icons.rs`, `src/tray/actions.rs`; modify `src/tray.rs`, `src/tray/backend.rs`, `src/tray/item.rs`, `src/mango.rs`.

**Interfaces:** Consume Task 2 item/anchor types and connection/current-generation helpers. Produce `icons::argb_to_rgba(width: i32, height: i32, bytes: &[u8]) -> Option<Vec<u8>>`, `icons::resolve(sources: &IconSources, target_px: u32) -> ItemIcon`, `icons::read_menu_icon(bytes: &[u8]) -> Option<Image>`, and `item::plain_text(input: &str) -> String`. Produce `OutputGeometry { name: String, x: i32, y: i32, width: u32, height: u32, scale: f32 }` and `mango::output_geometries() -> Result<Vec<OutputGeometry>, String>`.

Produce `ItemAction::{Activate, ContextMenu, SecondaryActivate, Scroll { delta: i32, axis: ScrollAxis }}`, `ScrollAxis::{Horizontal, Vertical}`, and `actions::submit(item: ItemKey, action: ItemAction, anchor: SlotAnchor) -> bool`. Produce the shared serialized worker entry `actions::enqueue(job: Box<dyn FnOnce() + Send + 'static>) -> bool`; use a 64-job bounded queue and nonblocking submission. Task 4 adds menu work through this same queue. Fractional wheel input accumulates per item/axis; one line sends a 120-unit protocol step, positive right/down and negative left/up.

- [ ] Write `pixmaps_validate_before_conversion`, `theme_cycles_and_bad_named_icons_fall_back`, `menu_icons_are_owned_and_scratch_files_are_removed`, `item_calls_use_captured_output_and_generation`, and `scroll_axes_and_fractional_steps`. Use temporary theme trees and a controlled D-Bus item; test attention/overlay selection, 1x/1.25x resolution choice, unreadable files, tooltip entities/markup, missing methods, and a queue filled by a stalled app:

```rust
assert_eq!(argb_to_rgba(1, 1, &[0x80, 0x11, 0x22, 0x33]), Some(vec![0x11, 0x22, 0x33, 0x80]));
assert!(argb_to_rgba(1025, 1, &[]).is_none());
assert!(argb_to_rgba(-1, 1, &[]).is_none());
assert!(argb_to_rgba(1, 1, &[0; 3]).is_none());
assert!(resolved.normal.is_some()); // corrupt preferred name, cyclic theme, valid pixmap
assert!(read_menu_icon(&vec![0; 4 * 1024 * 1024 + 1]).is_none());
assert_eq!(scratch_file_count_after_success_or_error, 0);
assert_eq!(plain_text("<b>Mail</b> &amp; news"), "Mail & news");
assert_eq!(recorded_coordinates, (output_origin_x + slot_center_x, output_origin_y + slot_center_y));
assert_eq!(removed_output_call_count, 0);
assert!(!submit_when_queue_is_full);
```

- [ ] Run `rtk proxy env RUSTC_WRAPPER= cargo test --locked tray::`; expect unresolved icon/action interfaces or failing new assertions.
- [ ] Implement background theme lookup (app path, desktop theme, inheritance, hicolor, standard pixmaps, absolute PNG/SVG) with a visited-theme set and cached paths. Decode to owned images, selecting pixmaps for `ceil(18 * max_output_scale)` physical pixels, and expose attention/overlay images for widget compositing. Replace/drop old images on update. For DBusMenu's encoded icon-data, reject buffers over 4MiB, decode through `Image::read_owned` using an exclusively created private scratch file, then unlink it; no permanent file cache or additional image API. Resolve fresh Mango output geometry in action work and use the captured slot center as the coordinate hint, then revalidate owner/connection generation before dispatch. A timed-out or unsupported method returns a bounded failure; no retry storm or UI lock. Feed icon resolution into coalesced item refreshes. Strip tooltip markup/entities as text without executing or loading embedded content.
- [ ] Run the targeted tray tests and `rtk proxy env RUSTC_WRAPPER= cargo test --locked mango::`; expect correct calls, bounded stalls, released image generations, and no Mango layout regression.
- [ ] Commit Task 3 paths with `rtk git commit -m "feat: render tray icons and dispatch item actions"`.

### Task 4: Bounded live DBusMenu model and popup state

**Files:** Create/Test `src/tray/menu.rs`, `src/tray/menu/client.rs`, `src/tray/ui.rs`; modify `src/tray.rs`, `src/tray/actions.rs`, `src/tray/backend.rs`.

**Interfaces:** Consume Tasks 2–3 identities, icons, anchors and serialized jobs. Define RawMenuNode/MenuKind/MenuNode/MenuTree in `menu.rs`; define session and TrayUi types in `ui.rs`:

```rust
type RawMenuNode = (i32, HashMap<String, zbus::zvariant::OwnedValue>, Vec<zbus::zvariant::OwnedValue>);
enum MenuKind { Entry, Separator, Check { state: i32 }, Radio { state: i32 } }
struct MenuNode { id: i32, label: String, enabled: bool, visible: bool, kind: MenuKind,
    icon: ItemIcon, children: Vec<i32>, has_submenu: bool }
struct MenuTree { revision: u32, root: i32, nodes: BTreeMap<i32, MenuNode> }
struct MenuSession { request: u64, key: ItemKey, anchor: SlotAnchor, tree: Option<Arc<MenuTree>>,
    path: Vec<i32>, selected: Option<i32>, scroll_rows: usize }
struct OverflowSession { anchor: SlotAnchor, keys: Vec<ItemKey> }
enum PopupSession { Menu(MenuSession), Overflow(OverflowSession) }
struct TooltipSession { key: ItemKey, anchor: SlotAnchor, shown: bool, deadline: Instant }
struct TrayUi { popup: Option<PopupSession>, tooltip: Option<TooltipSession> }
```

Produce `menu::decode_layout(revision: u32, root: RawMenuNode) -> Result<MenuTree, MenuError>` and `menu::display_label(label: &str) -> String`; define explicit parse errors for malformed structure, duplicate IDs and exceeded budgets. `TrayUi` implements Service. Produce `ui::open_menu(key: ItemKey, anchor: SlotAnchor)`, `ui::open_overflow(anchor: SlotAnchor, keys: Vec<ItemKey>)`, `ui::close()`, `ui::hover(item: Option<(ItemKey, SlotAnchor)>)`, `ui::keep_bar_visible(output: &str) -> bool`, and `ui::reconcile(live_items: &[ItemKey], outputs: &[String], enabled: bool)`. Produce `menu::client::show(key: ItemKey, path: String, request: u64, parent: i32)` and `menu::client::activate(request: u64, row_id: i32, timestamp_ms: u32)`; these enqueue work and never block callers.

- [ ] Write `menu_decode_bounds_and_mnemonics`, `dynamic_selection_cannot_activate_removed_rows`, `stale_menu_replies_cannot_replace_new_popup`, and `about_to_show_precedes_layout_and_event`. Exercise separators/hidden/disabled/check/radio rows, icon data, nested layouts, LayoutUpdated/ItemsPropertiesUpdated, and request replacement:

```rust
assert_eq!(display_label("_Open __folder"), "Open _folder");
assert!(decode_layout(1, tree_with_513_nodes).is_err());
assert!(decode_layout(1, tree_with_17_levels).is_err());
assert!(decode_layout(1, duplicate_ids).is_err());
assert_eq!(selection_after_reorder, Some(original_row_id));
assert_eq!(disabled_or_removed_row_event_count, 0);
assert_eq!(active_popup_request_after_old_reply, new_request);
assert_eq!(call_order, ["AboutToShow", "GetLayout", "Event"]);
```

- [ ] Run `rtk proxy env RUSTC_WRAPPER= cargo test --locked tray::`; expect failures for missing menu/UI interfaces or their behavior.
- [ ] Implement bounded typed layout decoding, underscore display conversion, lazy root/submenu fetching and live property/layout updates. Fetch AboutToShow before each opening, merge fetched submenu nodes into the active tree, subscribe/reconcile updates and preserve selected IDs. Send `Event(id, "clicked", data, timestamp)` only after checking the current enabled/visible row and generation. Retain menu images only for the current request. Implement popup replacement, delayed tooltip state, and removal reconciliation; no labels become commands. When no Menu is exported, right-click routes to ContextMenu; ItemIsMenu left-click follows the same menu path/fallback.
- [ ] Run tray tests; expect correct method order, boundary acceptance at 512 nodes/16 levels, and rejection of stale replies/actions. Isolated fixture tests must show that another app's menu cannot receive an old row event.
- [ ] Commit Task 4 paths with `rtk git commit -m "feat: support live application tray menus"`.

### Task 5: Adaptive per-monitor tray pill, overflow and settings

**Files:** Create/Test `src/bar/systray.rs`; modify/Test `src/bar/system.rs`; modify `src/bar.rs`, `src/settings/store.rs`, `src/settings/pages/bar.rs`.

**Interfaces:** Consume `TraySnapshot`, `SlotAnchor`, Task 3 actions and Task 4 popup/hover functions. Produce `InlineLayout { width: f32, inline: usize, overflow: usize }` and `systray::layout(item_count: usize, available: f32) -> InlineLayout`. Produce `systray::view(monitor: &Monitor, theme: &Theme, items: &[Arc<TrayItem>], placement: SlotAnchor, layout: InlineLayout) -> Rectangle`. Change `system::view(theme: &Theme, width: f32)` to `system::view(monitor: &Monitor, theme: &Theme, width: f32) -> Row` and update its caller.

- [ ] Write `inline_layout_reserves_overflow_and_empty_has_no_width` and `recording_growth_repositions_slots_without_overlap`. Measure the actual other widgets once and derive anchors from those same measurements, including current animated recording width:

```rust
assert_eq!(layout(0, 200.0).width, 0.0);
assert_eq!(layout(1, 200.0).width, 42.0);
assert_eq!(layout(2, 200.0).width, 72.0);
assert_eq!((layout(5, 72.0).inline, layout(5, 72.0).overflow), (1, 4));
assert_eq!((layout(5, 42.0).inline, layout(5, 42.0).overflow), (0, 5));
assert_eq!(layout(5, 41.0).width, 0.0); // not enough room for even one complete slot
assert!(last_slot_right <= utility_left - 10.0);
assert_eq!(captured_anchor.output, clicked_output_name);
```

- [ ] Run `rtk proxy env RUSTC_WRAPPER= cargo test --locked bar::`; expect failures for missing layout/interface behavior.
- [ ] Implement measured width and fixed slot geometry. Insert the tray immediately before utility shortcuts, preserving right alignment and the existing 10px inter-control gap. When less than 42px remains, omit the inline pill until room returns rather than creating an overlapping target. Draw normal/attention base images and a usable overlay with Stack, retaining app colors and a neutral missing-image mark. Capture logical slot anchors in handlers; route left/right/middle, accumulated scroll and hover to existing interfaces. The overflow button opens remaining items with identical behavior. Add the default and the two exact setting labels, retaining saved `bar_tray` semantics; no extra font dependency.
- [ ] Run `rtk proxy env RUSTC_WRAPPER= cargo test --locked bar::` and `rtk proxy env RUSTC_WRAPPER= cargo clippy --locked --all-targets --all-features -- -D warnings`. Expect widths/anchors to remain valid while battery, memory, utility settings and recording width change, and stable item ordering across icon/status updates.
- [ ] Commit Task 5 changes with `rtk git commit -m "feat: add adaptive system tray to the bar"` using patch staging on already-dirty files.

### Task 6: Menu and tooltip windows, navigation and teardown

**Files:** Create/Test `src/tray/popup.rs`, `src/tray/popup/geometry.rs`; modify/Test `src/tray/ui.rs`; modify `src/main.rs`, `src/bar.rs`, `src/bar/reveal.rs`, `src/settings/store.rs`, `src/lock_screen/curtain.rs`.

**Interfaces:** Consume Task 4 UI/menu state and Task 5 anchors. Produce `popup::view(monitor: &Monitor) -> LayerWindow`, `popup::tooltip_view(monitor: &Monitor) -> LayerWindow`, `popup::key_pressed(key: amane::Key)`, and `ui::select_row(request: u64, row_id: i32)`. Define `PopupRect { x: f32, y: f32, width: f32, height: f32 }`; produce `geometry::place_root(anchor: &SlotAnchor, desired: (f32, f32), monitor_size: (f32, f32), bottom: bool) -> PopupRect` and `geometry::place_submenu(parent_row: PopupRect, desired: (f32, f32), monitor_size: (f32, f32)) -> PopupRect`.

- [ ] Write `popup_geometry_clamps_and_flips`, `keyboard_navigation_skips_non_actions`, `tooltip_delay_and_teardown`, and `menu_holds_only_its_output_bar`. Use deterministic time in the tooltip state tests and real Unicode labels in measurement tests:

```rust
assert!(root.x >= 0.0 && root.x + root.width <= monitor_width);
assert!(bottom_root.y + bottom_root.height <= anchor.y);
assert!(flipped_submenu.x < parent_row.x);
assert_eq!(selection_after_down, next_enabled_visible_row);
assert_eq!(selection_after_home, first_enabled_visible_row);
assert!(!tooltip_visible_at_399_ms);
assert!(tooltip_visible_at_400_ms);
assert!(!tooltip_visible_after_menu_or_hover_end);
assert!(keep_bar_visible(clicked_output));
assert!(!keep_bar_visible(other_output));
assert!(popup_after_disable_or_lock_or_removal.is_none());
```

- [ ] Run `rtk proxy env RUSTC_WRAPPER= cargo test --locked tray::`; expect missing popup/placement interfaces or failing new state tests.
- [ ] Implement themed measured rows, bounded/scrolling menu and overflow contents, side submenus, row icons/toggles and hover selection. Bind Up/Down/Left/Right, Enter/Space, Home/End and Escape to current row IDs; row clicks use the same validated event path. The menu's per-monitor overlay window uses Zone::Ignore, immediate Exclusive keyboard focus and transparent outside-click dismissal. Tooltip windows are click-through and keyboard-free. Hidden windows are invisible with no input; other outputs remain hidden. Register both views after existing overlays in `main.rs` and use the existing amane role lifecycle. Hold reveal while that output owns a popup. After releasing Settings' write guard, close on systray disable; close at the start of `curtain::close()`. Reconcile outputs in background while popup/tooltip state exists, without writes or IO from view functions.
- [ ] Run `rtk proxy env RUSTC_WRAPPER= cargo test --locked`, `rtk proxy env RUSTC_WRAPPER= cargo fmt --all --check` and `rtk proxy env RUSTC_WRAPPER= cargo clippy --locked --all-targets --all-features -- -D warnings`; expect geometry/navigation/teardown assertions to pass without service-write redraw loops or nested-lock deadlocks.
- [ ] Commit Task 6 paths with `rtk git commit -m "feat: add interactive tray popups and tooltips"`.

### Task 7: Native fixture, full validation, documentation and installation

**Files:** Create/Test `examples/systray-fixture.rs`, `examples/systray_fixture/menu.rs`, `examples/systray_fixture/watcher.rs`, `scripts/check-mango-systray.py`, `scripts/mango-keyboard.c`; modify `scripts/mango-pointer.c`, `README.md`, `docs/desktop-setup.md`. Fix feature files only when a failing scenario or final review identifies a defect.

**Interfaces:** Fixture CLI: `systray-fixture --mode items|watcher --log PATH`, with stdin JSON commands `{"op":"status","id":"NAME","value":"Passive|Active|NeedsAttention"}`, `{"op":"icon","id":"NAME","generation":N}`, `{"op":"menu_update","id":"NAME"}`, `{"op":"remove","id":"NAME"}`, `{"op":"add","id":"NAME"}`, and `{"op":"quit"}`. It logs flushed JSON lines with `item`, `method`, `args` and `generation`. The native runner accepts `--binary PATH --fixture PATH`. Extend pointer stdin with `middle` and `scroll-x`, retaining existing commands/acknowledgement; keyboard stdin accepts `up|down|left|right|enter|space|home|end|escape` with a `keyed` acknowledgement. Generate the cached virtual-keyboard protocol via wayland-scanner; use xkbcommon to provide a keymap. No host applications participate.

- [ ] Implement the controlled fixture exporting named/pixmap/menu-only/attention items, multiple paths under one owner, nested/dynamic/check/radio/disabled menus, plus a watcher mode. Its only effects are private-bus calls and the specified log/control files; fixture labels include long Unicode text and ordinary underscores.
- [ ] Write the native script using temporary HOME/XDG directories, a dedicated session bus shared by fixture and shell, and headless Mango at scales 1/1.25. Extend the drivers and assert real interactions on both outputs:

```python
assert calls("Activate", output) == 1
assert calls("SecondaryActivate", output) == 1
assert scroll_orientations(output) == {"horizontal", "vertical"}
assert selected_menu_ids(output) == expected_enabled_ids
assert disabled_menu_event_count() == 0
assert popup_absent_on_other_output()
assert desktop_and_existing_panels_receive_input_after_dismissal()
assert shell_is_alive_and_log_is_bounded()
```

  Each helper above belongs to this script and reads fixture logs, screenshots or controlled input evidence. Also exercise named/path duplicate registration, owner restart/removal, external-watcher hosting/replacement, isolated bus reconnection, malformed icons/menus, passive/attention changes, icon updates, overflow, 400ms tooltip show/dismissal, settings disable/re-enable, top/bottom placement, keyboard/submenu/outside-click dismissal and auto-hide hold. Observe output-removal teardown using disposable outputs. Model tests cover locking teardown; do not lock the real session.
- [ ] Run `rtk proxy env RUSTC_WRAPPER= cargo build --release --locked --examples` and `rtk proxy python scripts/check-mango-systray.py --binary target/release/kajitsu --fixture target/release/examples/systray-fixture`. Expect every native assertion to pass; repair demonstrated integration defects and rerun affected checks if any fail.
- [ ] Document actual watcher/host startup, interactions, separate settings, local amane image helpers, protocol limitations and reproducible validation in README/desktop setup. Keep the hard-fork/Mango/Artix/font documentation and unrelated user changes.
- [ ] Run `rtk proxy env RUSTC_WRAPPER= cargo test --locked`, `rtk proxy env RUSTC_WRAPPER= cargo clippy --locked --all-targets --all-features -- -D warnings`, `rtk proxy env RUSTC_WRAPPER= cargo fmt --all --check`, `rtk git diff --check`, and `rtk proxy env RUSTC_WRAPPER= cargo build --release --locked`. Run amane tests/Clippy/formatting from Task 1, then `rtk proxy python scripts/check-mango-panels.py --binary target/release/kajitsu` and `rtk proxy python scripts/check-mango-layouts.py --binary target/release/kajitsu`. Expect all checks to pass; save native evidence under `/tmp` and record precisely what passed.
- [ ] Obtain the final independent whole-change review required by the chosen execution method, including the sibling amane diff. Fix actionable findings and rerun only checks affected by fixes. Commit Task 7 paths with `rtk git commit -m "test: verify native tray interactions on Mango"`.
- [ ] Back up the installed Kajitsu binary and write old/new hashes to a manifest under `~/.local/state/kajitsu/setup-backups/`. Verify `loginctl` reports the session unlocked, atomically replace `~/.local/bin/kajitsu` with the tested release build and run the existing `kajitsu-restart` helper. Restore the backup if restart fails; do not bypass a locked session.
- [ ] Check the installed hash, supervisor/process health and bars on physical DP-9/eDP-1; use a controlled fixture if a visible sample is needed. Report implemented interactions, successful checks and backup path. Keep real applications' menu actions untouched during this host check.

## Inline self-review

- Spec coverage: Tasks 1–3 cover image ownership, discovery, compatibility, updates, reconnects, icons and item actions; Tasks 4–6 cover menus, tooltips, measured bars, settings and lifecycle; Task 7 covers native evidence, documentation, review and installation.
- Interface consistency: Shared identity/image/anchor types originate in Task 2; action work in Task 3, menu/UI state in Task 4 and slot placement in Task 5 are consumed with the same names by subsequent tasks.
- Review Focus: All five conditions are assigned explicit tests above, including owner/popup generations, cyclic themes, invalid selection, shrinking width and repeated input-role teardown.
- Lock discipline: Background calls and decodes operate on owned copies; publish/reconcile locks are brief; view functions only read services and construct widgets.
- Scope: One native tray subsystem and its required additive amane helpers; no legacy-tray bridge, unrelated refactor or dependency sweep.
