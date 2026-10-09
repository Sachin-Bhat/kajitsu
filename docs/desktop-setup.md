# Local desktop setup

Installed on 2026-10-08 for the Artix Mango session. Kajitsu replaced GPUi Shell in the running session and in Mango's `exec-once` configuration. The installed binary is `~/.local/bin/kajitsu`, built against the sibling local amane checkout. Runtime assets are copied into `~/.config/kajitsu`.

## Components

The bar, launcher, panels, wallpaper picker, desktop widgets, settings, pomodoro, converter, recorder, and lock UI are part of the same binary. Networking, Bluetooth, brightness, audio, and the profile file chooser use the existing NetworkManager, BlueZ, elogind, PipeWire, and desktop portal services.

Installed the missing Artix packages `world/tmux`, `galaxy/cava`, `world/imagemagick`, and `world/wf-recorder`. FFmpeg, LibreOffice, WezTerm, bottom, the two Nerd Font families, and the freedesktop sound theme were already present.

Enabled GTK, WezTerm, bottom, tmux, and Cava palette integrations. The WezTerm loader preserves the existing fonts, opacity, keybindings, and project configuration. `kajitsu-bottom` and its desktop entry open bottom using the generated configuration; `~/.config/bottom/bottom.toml` remains its source. Tmux sources the generated palette through `~/.tmux.conf`; detached `kajitsu` and `amane` sessions start in their respective repositories. Cava has both its terminal theme and the shell's visualizer configuration.

The selected wallpaper is the existing `artix-black-4k.png` from `~/Pictures/Wallpapers`. The picker can select the rest of that collection. Weather and air quality need a location under **Settings → Weather**. Spotify and Vesktop are absent from this machine and their optional exports are disabled.

The power-menu button uses the Artix logo. Power actions use the login1 DBus interface provided by elogind on this dinit system. Startup selects `AMANE_BACKLIGHT_DEVICE=amdgpu_bl1` so brightness controls the laptop panel rather than the separate NVIDIA backlight device.

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
Standalone amane strict Clippy still has the 13 confirmed pre-existing diagnostics;
all other lint categories are checked strictly. Physical authentication, power
actions, and hardware hotplug remain the manual checks listed below.

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
install -Dm755 target/release/kajitsu ~/.local/bin/kajitsu
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

A disposable two-output Mango session verified panel IPC, Cava output, and native recording start/stop. FFprobe confirmed a playable H.264 MP4 at 1280×720. Converter readiness checks produced nonempty outputs for all 16 configured FFmpeg formats, an ImageMagick WebP, and a LibreOffice PDF. The timer sound opened through PipeWire at zero volume. Kajitsu passes 25 tests and strict Clippy; amane passes 39 library tests. Formatting and the release build pass. Standalone amane strict Clippy retains 13 diagnostics also present at its prior committed baseline.

Rapid panel toggles exposed an inherited Wayland configure-serial failure. Amane now retires and recreates hidden layer roles after dispatching the current event batch and exits on fatal protocol errors. Kajitsu keeps dismissal mapped with inactive input disabled, preserving clicks on reopened panels. The regression check covers 12 rapid show/hide cycles across four panels on two isolated outputs at scales 1 and 1.25, plus inside/outside pointer clicks. Run it after building:

```sh
python3 scripts/check-mango-panels.py
```

It needs Mango/mmsg, grim, `cc`, `pkg-config`, `wayland-scanner`, and Wayland client development files. It uses the virtual-pointer protocol cached by Cargo when building Kajitsu. It launches and cleans up its own headless Mango compositor, DBus session, and temporary state without changing the daily desktop. Role recreation initializes a fresh rendering surface when a layer hides; longer daily performance checks remain useful.

Authenticated lock/unlock, suspend/resume, physical hotplug, recording audio modes/output removal, and extended daily workflows remain manual validation items. See [Mango validation](mango-validation.md).
