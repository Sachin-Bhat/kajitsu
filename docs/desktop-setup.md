# Local desktop setup

Installed on 2026-10-08 for the Artix Mango session. Kajitsu replaced GPUi Shell in the running session and in Mango's `exec-once` configuration. The installed binary is `~/.local/bin/kajitsu`, built against the sibling local amane checkout. Runtime assets are copied into `~/.config/kajitsu`.

## Components

The bar, launcher, panels, wallpaper picker, desktop widgets, settings, pomodoro, converter, recorder, and lock UI are part of the same binary. Networking, Bluetooth, brightness, audio, and the profile file chooser use the existing NetworkManager, BlueZ, elogind, PipeWire, and desktop portal services.

Installed the missing Artix packages `world/tmux`, `galaxy/cava`, `world/imagemagick`, and `world/wf-recorder`. FFmpeg, LibreOffice, WezTerm, bottom, the two Nerd Font families, and the freedesktop sound theme were already present.

Enabled GTK, WezTerm, bottom, tmux, and Cava palette integrations. The WezTerm loader preserves the existing fonts, opacity, keybindings, and project configuration. `kajitsu-bottom` and its desktop entry open bottom using the generated configuration; `~/.config/bottom/bottom.toml` remains its source. Tmux sources the generated palette through `~/.tmux.conf`; detached `kajitsu` and `amane` sessions start in their respective repositories. Cava has both its terminal theme and the shell's visualizer configuration.

The selected wallpaper is the existing `artix-black-4k.png` from `~/Pictures/Wallpapers`. The picker can select the rest of that collection. Weather and air quality need a location under **Settings → Weather**. Spotify and Vesktop are absent from this machine and their optional exports are disabled.

The power-menu button uses the Artix logo. Power actions use the login1 DBus interface provided by elogind on this dinit system. Startup selects `AMANE_BACKLIGHT_DEVICE=amdgpu_bl1` so brightness controls the laptop panel rather than the separate NVIDIA backlight device.

## Fonts

The desktop font defaults are **GeistMono Nerd Font Mono** for monospace,
**Inter Nerd Font Propo** for sans-serif, and **Tinos Nerd Font Propo** for serif.
Tinos's regular, bold, italic, and bold-italic Propo styles were installed from
the official Nerd Fonts v3.5.1 release into `~/.local/share/fonts/TinosNerdFontPropo`.
The archive's published SHA-256 was verified; its release metadata and Apache
license are stored with the installed fonts.

`~/.config/fontconfig/conf.d/60-kajitsu-fonts.conf` matches the
[repository example](../examples/fontconfig/60-kajitsu-fonts.conf). It applies
the three choices to generic font requests while preserving language fallbacks
and the existing font rendering settings. GTK 3, GTK 4, and the saved xsettingsd
configuration use Inter at size 11. Desktop interface settings use Inter 11,
Tinos 11 for documents, and GeistMono 11 for monospace. WezTerm continues to use
its existing GeistMono size 13 configuration; bottom inherits that font.

All three generic families resolved to their selected fonts in regular, bold,
italic, and bold-italic styles. Explicit Tinos requests resolved to the installed
Propo files. Restart applications that have already loaded their fonts.

Original configuration files and desktop font settings are recorded in
`~/.local/state/kajitsu/setup-backups/fonts-20261009-125314/manifest.json`.
`~/.local/state/kajitsu/font-setup-backup-path` records that backup directory.

## Layout switcher

Added on 2026-10-09. Each output's bar shows its current Mango layout beside the
workspace strip, with a vector pane diagram and its name. The pill width follows
the icon and actual font metrics, with 10px padding on each side. Vertical layouts
use rotated diagrams. Left-click or scroll down cycles forward; right-click or scroll
up cycles backward. The control wraps through all layouts from `mmsg get layouts`
and updates after external keybindings or IPC commands. Disable it under
**Settings → Bar → Layout switcher**.

The control shows **Layout —** and disables input while Mango state is unavailable.
IPC calls run outside the drawing thread. Mango 0.17.5's layout dispatch targets
the focused monitor: focus is checked before dispatch, but a focus change between
those separate requests can send the change to another monitor.

The verified release was installed and Kajitsu restarted in the unlocked session.
The indicator rendered **Tile** on DP-9 and **V. Scroller** on eDP-1, matching their
native monitor state at scales 1 and 1.25. The initial install's previous binary
is backed up in `~/.local/state/kajitsu/setup-backups/layout-20261009-132924`.
The icon and adaptive-width update was also installed on 2026-10-09; all 14
diagrams were visually checked at both scales, and **Tile** measured about 65px
wide, including its icon and padding. It rendered correctly on both physical
outputs. This update's previous binary is backed up in
`~/.local/state/kajitsu/setup-backups/layout-20261009-133813`; the manifest records
both release hashes. `~/.local/state/kajitsu/layout-setup-backup-path` records
the latest backup directory.

## System tray

The native application tray uses StatusNotifier and DBusMenu on the session bus.
It acquires free KDE/freedesktop watcher names and hosts existing watchers without
replacing their owner. Both bars share a registry; popups belong to the clicked
output. Active and attention items appear in registration order, with adaptive
width and an overflow list. Empty trays take no space.

Left/right/middle clicks activate, open a menu, and send secondary activation;
both wheel axes are forwarded. Menu-only items open on left-click. Tooltips show
plain text after 400ms and take no input. Menus support disabled/hidden entries,
separators, checkbox/radio state, icons, live updates, and lazy side submenus.
Arrow keys, Home/End, Enter/Space, Escape, and outside-click work in the menu;
overflow also supports keyboard selection. Long content scrolls within the output.
Each submenu keeps its parent's scroll position. Clicking the bar dismisses the
menu while preserving the clicked control's action. Refreshed menus validate
unique row IDs and reject actions beneath hidden or disabled ancestors.

Settings → Bar has separate **System tray** (`bar_systray=true`) and
**Utility shortcuts** (`bar_tray`) switches. Disabling the tray closes its popup
while discovery continues. An open popup holds only its own auto-hidden bar.
Owner loss, output removal, and lock startup also close it. Calls run off the
drawing thread with three-second D-Bus timeouts; queued jobs validate item and
popup generations before sending actions. XEmbed-only icons and animated
attention movies are outside this implementation.

Build and run the controlled native regression:

```sh
env RUSTC_WRAPPER= cargo build --release --locked --bin kajitsu --example systray-fixture
python3 scripts/check-mango-systray.py --binary target/release/kajitsu --fixture target/release/examples/systray-fixture
```

The runner creates its own session bus, HOME/XDG directories, and two headless
Mango outputs at scales 1 and 1.25. It records actual fixture calls and screenshots
under the printed `/tmp/kajitsu-systray-check-*` path. It verifies both registration
forms and duplicates, pointer/keyboard menus, tooltips, both scroll axes, icon/status
updates, malformed data, overflow, watcher hosting/replacement, owner restart,
method stalls, queued-request cancellation, bus reconnection, settings visibility,
top/bottom placement, auto-hide hold, and output-removal teardown. It needs the
panel-check dependencies plus dbus-daemon, busctl, and xkbcommon development files.
Its output driver rejects physical output names. Real applications and the daily
compositor are not used.

Local amane includes commit `1b07d1f` for owned RGBA/file images. Kajitsu's tray
checks and the existing panel/layout regressions complement amane's image tests.
The final tray build passes 60 Kajitsu tests, 42 amane tests, strict Kajitsu Clippy,
formatting, and the release/fixture build. The complete native tray runner,
12-cycle panel regression, and both-output layout regression pass.
Standalone amane strict Clippy still has the 13 confirmed pre-existing diagnostics;
all other lint categories are checked strictly. Physical authentication, power
actions, and hardware hotplug remain the manual checks listed below.

The reviewed release is installed at `~/.local/bin/kajitsu`. Cropped bar captures
confirmed tray icons and the retained Artix/layout controls on DP-9 (scale 1) and
eDP-1 (scale 1.25). A controlled fixture registered four icons, then disappeared
after exit while the existing application's icon remained. The running executable
matches the tested release; IPC and watcher host registration respond.

The prior binary is backed up at
`~/.local/state/kajitsu/setup-backups/systray-20261009-165440/kajitsu`.
Its manifest records both hashes; `~/.local/state/kajitsu/systray-setup-backup-path`
points to the backup directory. To restore, stop Kajitsu, copy that backup binary
to `~/.local/bin/kajitsu`, and run `~/.local/bin/kajitsu-start`.

## Shortcuts

| Shortcut | Action |
| --- | --- |
| Alt+Space | Launcher |
| Alt+Shift+P | Tmux projects |
| Alt+Shift+W | Wallpapers |
| Alt+Shift+C | Control center |
| Alt+Shift+U | Utility panel |
| Alt+Shift+S | Settings |
| Alt+Shift+V | File converter |
| Alt+Shift+E | Start/stop recording |
| Alt+Shift+Space | Fuzzel fallback |
| Alt+L | Kajitsu native lock screen |

Pomodoro is available from the launcher's `>` command mode. Alt+L, the power-menu lock action, and `kajitsu ipc call lock` use Kajitsu's native lock screen. Authentication uses the system's PAM `login` service and the system account password; the profile name does not change that account. The installed PAM policy and helper were checked, but authenticated unlock and suspend/resume remain manual checks.

## Restart, update, and rollback

`kajitsu-start` is the Mango startup command. Its supervisor prevents duplicate launches, records the actual shell PID, and rotates `~/.local/state/kajitsu/shell.log` at 1 MiB, retaining one previous log. `kajitsu-restart` stops only Kajitsu's recorded process, waits for the supervisor to finish, and starts it again. Run it while the session is unlocked. Both duplicate startup and restart were verified.

After rebuilding the local source, update the installed binary and runtime assets, then restart:

```sh
cd ~/Documents/kajitsu
cargo build --release --locked
install -Dm755 target/release/kajitsu ~/.local/bin/kajitsu.new
mv ~/.local/bin/kajitsu.new ~/.local/bin/kajitsu
install -Dm644 shaders/liquid.wgsl ~/.config/kajitsu/shaders/liquid.wgsl
install -Dm644 cava.conf ~/.config/kajitsu/cava.conf
kajitsu-restart
```

The setup backed up all modified user files and the GTK dconf values. The backup directory is recorded in `~/.local/state/kajitsu/setup-backup-path`. Its `manifest.json` lists each original file, and `rollback.py` restores those configs and restarts GPUi Shell. Run rollback from an unlocked Mango session:

```sh
python3 "$(cat ~/.local/state/kajitsu/setup-backup-path)/rollback.py"
```

Rollback restores the setup's original configuration snapshots. It leaves installed packages, tmux sessions, and generated cache/state files available.

## Verification

Kajitsu rendered on both physical outputs, DP-9 and eDP-1, at their existing scales. It owns the desktop notification service, responds to native IPC, and generated each enabled app palette. WezTerm loaded the updated Lua configuration, and a temporary tmux server loaded the generated status colors. The file chooser portal exposes its expected interface; elogind reports poweroff, reboot, and suspend available. Those power actions were not executed.

A disposable two-output Mango session verified panel IPC, Cava output, and native recording start/stop. FFprobe confirmed a playable H.264 MP4 at 1280×720. Converter readiness checks produced nonempty outputs for all 16 configured FFmpeg formats, an ImageMagick WebP, and a LibreOffice PDF. The timer sound opened through PipeWire at zero volume. Kajitsu passes 60 tests and strict Clippy; amane passes 42 library tests. Formatting and the release build pass. Standalone amane strict Clippy retains 13 diagnostics also present at its prior committed baseline.

Rapid panel toggles exposed an inherited Wayland configure-serial failure. Amane now retires and recreates hidden layer roles after dispatching the current event batch and exits on fatal protocol errors. Kajitsu keeps dismissal mapped with inactive input disabled, preserving clicks on reopened panels. The regression check covers 12 rapid show/hide cycles across four panels on two isolated outputs at scales 1 and 1.25, plus inside/outside pointer clicks. Run it after building:

```sh
python3 scripts/check-mango-panels.py
```

It needs Mango/mmsg, grim, `cc`, `pkg-config`, `wayland-scanner`, and Wayland client development files. It uses the virtual-pointer protocol cached by Cargo when building Kajitsu. It launches and cleans up its own headless Mango compositor, DBus session, and temporary state without changing the daily desktop. Role recreation initializes a fresh rendering surface when a layer hides; longer daily performance checks remain useful.

The layout regression uses the same isolated setup to verify left/right clicks,
wheel scrolling, wraparound, external layout updates, rendered labels, and the
visibility setting on two outputs at scales 1 and 1.25:

```sh
python3 scripts/check-mango-layouts.py
```

It confirms that switching one output leaves the other unchanged during ordinary
interaction. The inter-request focus race described above remains possible.

Authenticated lock/unlock, suspend/resume, physical hotplug, recording audio modes/output removal, and extended daily workflows remain manual validation items. See [Mango validation](mango-validation.md).
