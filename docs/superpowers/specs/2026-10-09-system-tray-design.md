# Native system tray for Kajitsu

Date: 2026-10-09

Status: the in-chat design is approved; this written spec awaits review.

## Intended outcome

Sachin wants a working app tray in his daily Artix/Mango session. Kajitsu will
display the icons exported by running applications and provide their activation,
tooltip, scrolling, and menu behavior. The tray will use the bar's compact style
and grow with its contents, following the sizing preference established for the
layout switcher.

The approved approach keeps the tray service and UI in Kajitsu. A shared tray
service in amane would serve other shells, while an external process would reduce
Kajitsu's implementation work. The local approach gives this shell control over
its placement, menus, and settings.

## Current integration points

`src/bar/system.rs` draws battery, memory, and a fixed notification/network/
Bluetooth shortcut pill. That pill is the utility launcher and recording timer;
it does not represent application tray items. `bar_tray` currently controls it.

Local amane has D-Bus helpers, image drawing, per-monitor windows, pointer input,
and keyboard input. It has no StatusNotifier or DBusMenu implementation. Its
generic method wrapper does not expose the message sender needed for registration
by object path, so the new backend will use zbus directly.

The locked dependency graph already includes zbus 5.19.0 and zvariant 5.15.0.
Declare zbus directly in Kajitsu, keeping those locked versions. Continue building
against `../amane`; preserve the current unrelated dependency and desktop changes.

## Bar presentation

Place a separate app-tray pill immediately before the existing utility launcher
on the right side of each bar. Draw 18px app icons in 26px clickable slots, with
4px between slots and 8px horizontal padding. Use the selected-surface background,
the normal bar height, and the existing hover treatment. Preserve the app's icon
colors; emphasize an attention request with the shell's accent treatment.

The pill disappears when no visible items exist. It has no placeholder icon or
fixed empty width. Keep registration order stable while properties change, using
the reported ID and registration identity to distinguish items from the same app.

Show Active and NeedsAttention items. Passive items remain tracked and appear
when their state changes. Limit the inline area to the remaining space in the
right-hand section after measuring its other controls. If all items do not fit,
reserve a slot for an overflow button and put the remaining items in an icon list
that uses the same actions and tooltips.

Add `bar_systray=true` and a **System tray** switch under Settings → Bar. Rename
the existing `bar_tray` switch to **Utility shortcuts**, retaining its key and
saved value. Turning off System tray closes its menus and tooltips but leaves
discovery running so turning it on restores the current items immediately.

## Item interaction

| Input | Result |
| --- | --- |
| Left-click | Activate the item, or open its menu when ItemIsMenu is true |
| Right-click | Open its exported menu; use ContextMenu when it exports none |
| Middle-click | Send SecondaryActivate when supported |
| Vertical/horizontal scroll | Send Scroll with the appropriate axis |
| Hover | Show the app's tooltip after a short delay |

Use a 400ms tooltip delay. Show the title and plain-text description; strip
markup and omit embedded content. A tooltip takes no pointer or keyboard input.
Close it when hover ends or a menu opens. Missing tooltip data falls back to the
item title or ID.

Derive popup anchors from the same measured right-section layout that draws the
slots. Each action captures the item identity, monitor name, and logical slot
position at the time of input. Global coordinate hints for app-owned popups come
from fresh native Mango monitor origins in the background worker. A removed
monitor cancels an action that needs that anchor.

The protocol's item properties and actions define these behaviors:
[StatusNotifierItem specification](https://specifications.freedesktop.org/status-notifier-item/latest/status-notifier-item.html).

## Backend and ownership

Start one tray service before the first bar frame. It maintains a snapshot of
live items for all bars and a separate UI state for the currently open popup.
Display the same registered items on both outputs, with a popup only on the
output where the user invoked it.

Use typed zbus interfaces for the watcher and typed property decoding for items.
Support the deployed `org.kde` names and the `org.freedesktop` variant. Export the
watcher at `/StatusNotifierWatcher`, with registration methods, the registered
item list, host-registration state, protocol version zero, and lifecycle signals.
The compatibility interface is documented in
[KDE's watcher XML](https://github.com/KDE/plasma-workspace/blob/master/xembed-sni-proxy/org.kde.StatusNotifierWatcher.xml).

Accept registration by bus name or object path. For object-path registration,
use the message's unique sender; for name registration, resolve its owner. Track
unique owner plus object path, keeping the advertised registration identifier for
the watcher. Duplicate registration updates the existing item. Owner loss removes
all items from that owner and closes any associated popup. An app restarting with
the same well-known name becomes a new registration generation.

Acquire watcher names without replacing an existing owner. If a watcher already
exists, register Kajitsu as a host, enumerate its items, and follow its lifecycle
signals. On watcher replacement or session-bus loss, discard stale state,
reconnect with bounded backoff, and resume ownership or host registration.
The protocol's registry lifecycle is defined by the
[watcher specification](https://specifications.freedesktop.org/status-notifier-item/latest/status-notifier-watcher.html).

Subscribe before taking initial snapshots, then reconcile queued events. Follow
icon, status, title, tooltip, and property changes. Publish a service write only
when the visible snapshot changes. Keep all D-Bus calls and filesystem work
outside the drawing thread and outside service locks. Set a three-second method
timeout, bound and serialize user actions, and coalesce refresh requests per item
so one unresponsive app cannot fill the work queue. Stale replies must not update
a replacement owner or a newer popup request.

## Icons and image lifetime

Resolve IconName through the app's IconThemePath, the selected desktop icon
theme, inherited themes, hicolor, and the standard pixmap directory. Accept an
absolute PNG or SVG icon path. Cache theme lookup work outside rendering.
If no usable named image exists, choose a suitable IconPixmap resolution.

Convert pixmap bytes from network-order ARGB to RGBA, validating dimensions and
the exact byte count before allocation. Prefer a resolution large enough for the
display scales in use. Use the attention image when supplied and composite a
usable OverlayIconName or OverlayIconPixmap over the main icon. A missing or invalid image uses a small neutral fallback mark
while retaining the app's actions and title. The encoding is defined in the
[pixmap specification](https://specifications.freedesktop.org/status-notifier-item/latest/icon-pixmap.html);
theme inheritance follows the
[icon-theme specification](https://specifications.freedesktop.org/icon-theme/latest/).

Add two narrow, additive helpers to local amane's Image API: an owned image from
validated RGBA pixels and a synchronous file read returning an owned decoded
image. The latter is used only from the tray's background work. Internally, Image
can retain an Arc to a decoded bitmap as an alternative to its existing file
source. Existing file-image constructors keep their behavior. Images owned by
items can then be replaced and released without creating a new global cached
file entry for every update. Reuse the existing PNG/SVG decoders and renderer.

Limit retained images to current live-item and open-menu generations. Use checked
dimensions with a 1024px maximum edge for raw tray pixmaps. Drop old owned images
when items disappear or change their icon. Test the new amane helpers separately
from the tray service.

## Menus

Implement a DBusMenu client for the item-reported Menu path. Notify the app before
showing a root or submenu, fetch its current layout, and follow layout/property
updates while open. Send the selected row's event back to the exporting app;
menu text never becomes a shell command. These exchanges follow the
[DBusMenu interface XML](https://sources.debian.org/src/libdbusmenu-qt/0.9.3%2B16.04.20160218-2/src/com.canonical.dbusmenu.xml).

Render ordinary entries, separators, disabled/hidden entries, checkbox/radio
state, menu icons, and submenus. Strip mnemonic underscores for display, retaining
escaped literal underscores. Dynamic menu updates preserve selection by item ID
when possible. Removed or disabled rows cannot dispatch an action.

Use themed rectangular popup surfaces with modest rounding and enough contrast
for the text. Measure row labels and clamp popup width and height to the monitor.
Long menus scroll. Submenus open beside their parent and flip inward near an
output edge. Use lazy submenu fetches and a bounded parse budget of 512 nodes and
16 levels for a received tree.

Provide Up/Down navigation, Left/Right submenu navigation, Enter/Space activation,
Home/End, and Escape dismissal. Outside-click closes the menu. Only one tray menu
session is open at a time. Opening another tray item replaces it; item removal,
settings disable, output removal, and session locking close it.

Use a dedicated per-monitor LayerWindow for menus and another click-through view
for tooltips. A menu uses overlay-layer input while open, giving Escape immediate
keyboard focus and catching outside clicks. Hidden views take no pointer or
keyboard input. Keep an auto-hidden bar visible while its tray menu is open.
Position menus below a top bar or above a bottom bar, with clamped geometry.
Reuse amane's existing role lifecycle rather than adding Wayland popup protocols.

## Code boundaries

| Area | Responsibility |
| --- | --- |
| `src/tray.rs` and `src/tray/` | Registry, item snapshots, icons, actions, DBusMenu models, reconnects |
| `src/bar/systray.rs` | Compact icon slots, width/overflow calculation, pointer actions |
| `src/tray/popup.rs` | Menu and tooltip UI, placement, keyboard navigation |
| `src/main.rs` | Start the service and register popup views |
| `src/bar.rs`, `src/bar/system.rs`, `src/bar/reveal.rs` | Per-monitor integration and auto-hide behavior |
| `src/settings/store.rs`, `src/settings/pages/bar.rs` | Defaults and separate tray/utility switches |
| Local amane image modules | Owned decoded-image helpers and their validation |
| `scripts/check-mango-systray.py` plus a fixture | Disposable native tray regression |

Keep protocol decoding and menu models independent of widget construction. Keep
the UI snapshot cheap to read, and never hold its lock across network or file IO.
Document startup behavior, shortcuts, and validation in README and desktop setup.

## Verification and completion

Use a small controlled StatusNotifier/DBusMenu fixture on an isolated session bus
and a two-output headless Mango compositor at scales 1 and 1.25. It must expose
named and pixel icons, menu-only behavior, attention state, nested menus, disabled
rows, check/radio state, and dynamic updates. Its recorded method calls provide
evidence of actual interactions rather than only screenshots.

Verify registration by both service name and sender/object path, duplicates,
multiple items per owner, owner disappearance/re-registration, existing-watcher
hosting, stale-response rejection, icon updates, and malformed pixmap/menu data.
Verify real pointer activation, secondary activation, both scroll axes, menu
selection/submenus, tooltip dismissal, outside-click, keyboard navigation,
overflow, the setting toggle, and top/bottom placement on both outputs. Confirm
hidden popup input does not block ordinary desktop or existing panel interactions.

Run meaningful protocol/model tests, Kajitsu tests, strict Clippy, formatting,
release builds, local amane image checks, and the existing panel/layout native
regressions after integration. Review the final implementation before installing.
Back up the previous Kajitsu binary, atomically install the tested build, restart
the unlocked shell, and check bar rendering and process health on physical DP-9
and eDP-1. Avoid driving real applications' menu actions during host verification.

Completion means the requested interactions work through native fixture calls,
the tray appears in the running daily shell, and the documented checks accurately
describe what was exercised. XEmbed-only legacy icons and animated attention
movies are outside this first native StatusNotifier implementation.
