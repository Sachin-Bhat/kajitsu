<div align="center">
  <img src="assets/fruit-basket.svg" alt="A woven basket filled with colorful fruit, including a golden mango" width="240">
  <h1>果実 · kajitsu</h1>
  <p><strong>A hard fork of suzuha.</strong><br>
  A personal Wayland desktop shell written in Rust with amane, being adapted for my daily-driver compositor, Mango.</p>

  <p>
    <img alt="amane 0.1" src="https://img.shields.io/badge/AMANE-0.1-89b4fa?style=flat-square&labelColor=181825">
    <img alt="Target compositor: Mango" src="https://img.shields.io/badge/TARGET_COMPOSITOR-MANGO-89b4fa?style=flat-square&labelColor=181825">
    <img alt="Rust" src="https://img.shields.io/badge/LANGUAGE-RUST-89b4fa?style=flat-square&labelColor=181825">
  </p>
</div>

## Preview

need to record again. soon.

## Overview

Kajitsu is a **hard fork of [Suzuha](https://github.com/MystiaFin/suzuha)**, MystiaFin's personal amane configuration. Suzuha provides the original Rust desktop shell and began as a rewrite of their [JAQC-shell](https://github.com/MystiaFin/shell) Quickshell config.

This fork follows its own direction: building a config for **Mango**, my daily-driver compositor. Application integration targets follow the programs I use: **WezTerm** for the terminal and **bottom** for system monitoring. UI text uses **Inter Nerd Font Propo** and icons use **GeistMono Nerd Font Mono**.

The power-menu button uses the **Artix logo**. Power actions use the login1 DBus interface supported by elogind and systemd-logind.

The conversion uses native Mango workspace support from my local amane checkout. Per-output tags, multiple selected tags, urgency, workspace recovery, recording-output queries, and Mango logout replace the inherited compositor assumptions. Kajitsu is installed in my daily Mango session; see [the desktop setup](docs/desktop-setup.md) for shortcuts and rollback, and [validation status](docs/mango-validation.md) for the remaining hardware, authentication, and recording checks.

### Included

- Status bar with workspaces, an interactive Mango layout indicator, system information, clock, media, and a pomodoro pill
- Liquid shader that lets the bar and panels flow into each other, with rounded screen corners
- Application launcher with a `>` command mode and a tmux project picker
- Control center with media controls and a cava visualizer
- Utility panel with Wi-Fi, Bluetooth, brightness, calendar, notifications, and a screen recorder
- Wallpaper picker with animated transitions and shuffle
- Wallpaper-derived dynamic color palette, plus fixed Gruvbox and Catppuccin schemes
- Floating desktop widgets that place themselves around the wallpaper
- Pomodoro timer
- File converter for images, video, audio, and documents
- Power menu
- logind-driven Wayland lock screen
- Built-in settings window
- Optional theme integrations for WezTerm, bottom, GTK, tmux, Vesktop, Spotify, and cava

### Layout switcher

Each monitor's bar shows its current Mango layout beside the workspace strip.
An outlined pane diagram accompanies the name, with rotated patterns for vertical
layouts. The pill sizes itself to the icon and measured text with consistent padding.
Left-click or scroll down to select the next layout; right-click or scroll up to
select the previous one. The control cycles through the layouts reported by
`mmsg get layouts` and wraps at either end. It follows layout changes made through
keybindings or other Mango clients, including changes on inactive monitors.

The indicator uses native Mango IPC. It shows **Layout —** with input disabled
while layout state is unavailable, and reconnects when IPC returns. Control calls
run off the drawing thread and confirm the clicked output before changing its
layout. **Settings → Bar → Layout switcher** controls its visibility.

Mango 0.17.5 [applies `setlayout` to the currently focused monitor](https://github.com/mangowm/mango/blob/0.17.5/src/dispatch/bind.c#L828-L845). Focus confirmation
and layout dispatch are separate IPC calls, so moving focus during a switch can
apply the change to the newly focused monitor.

### System tray

Running StatusNotifier applications appear in a compact pill beside the utility
shortcuts on each monitor. It grows with its icons, disappears when empty, and
puts excess items in an overflow list. Passive items stay hidden until active;
attention icons receive an accent outline and retain their application colors.

Left-click activates an app, right-click opens its exported menu, and middle-click
sends secondary activation. Menu-only items open their menu on left-click.
Applications without an exported menu receive ContextMenu on right-click.
Both scroll axes are forwarded; hovering shows a plain-text tooltip after 400ms.
Menus support check/radio states, icons, live updates, lazy submenus, and scrolling.
Use arrow keys, Home/End, Enter/Space, Escape, or an outside click.

**Settings → Bar → System tray** controls application icons independently of
**Utility shortcuts**, which retains the existing `bar_tray` preference. The tray
holds an auto-hidden bar open on the clicked monitor. Removal, disabling the tray,
output removal, and starting the session lock close its popup.

Kajitsu starts a native watcher on the session bus or joins an existing watcher
as a host without replacing it. It follows owner changes and reconnects after
bus loss. Local amane's owned-image helpers let updated tray images be released
without growing its file cache. XEmbed-only legacy icons and animated attention
movies are unsupported. See [native tray verification](docs/desktop-setup.md#system-tray)
for the controlled fixture and regression commands.

## Installation

Kajitsu builds directly against the local amane checkout in `~/Documents/amane`. Keep the two repositories as siblings:

```text
~/Documents/amane/     local library, including Mango workspace support
~/Documents/kajitsu/   this configuration
```

```sh
cd ~/Documents/kajitsu
cargo build --release --locked
KAJITSU_CONFIG_DIR="$PWD" ./target/release/kajitsu
```

To make the IPC and Mango binding examples available in `PATH`:

```sh
install -Dm755 target/release/kajitsu ~/.local/bin/kajitsu
```

The tracked manifest uses `amane = { path = "../amane" }`, so changes in the local library are used on the next build. The tested revision is [1b07d1f](https://github.com/Sachin-Bhat/amane/commit/1b07d1f04b4782e9583e60b158ebe22900ef53cb) on [kajitsu-mango-local](https://github.com/Sachin-Bhat/amane/tree/kajitsu-mango-local). It includes the owned-image helpers required by the native tray, desktop lifecycle fixes, and Mango support originating from [add-mango-workspace-support](https://github.com/Sachin-Bhat/amane/tree/add-mango-workspace-support). Cargo.lock locks external dependencies; local amane changes are deliberately not pinned. See [validation](docs/mango-validation.md) for the tested revision and remaining desktop checks.

Use native Cargo commands here. The amane CLI's compile/dev workflow generates a different manifest and embedded library snapshot. No amane CLI is required to launch Kajitsu or send IPC.

Configuration defaults to `$XDG_CONFIG_HOME/kajitsu` (or `~/.config/kajitsu`). Set `KAJITSU_CONFIG_DIR` to this checkout so shaders and `cava.conf` are found. State and cache use `$XDG_STATE_HOME/kajitsu` and `$XDG_CACHE_HOME/kajitsu`, with the usual home-directory fallbacks. Kajitsu starts with fresh settings; it does not import upstream enabled integrations.

## Dependencies

### Required

- **Local amane source** in `~/Documents/amane`, with Mango workspace support and owned-image helpers
- **Mango** (JSON IPC, tested against installed 0.17.5)
- **Inter Nerd Font Propo** for UI text
- **GeistMono Nerd Font Mono** for monospace and icons
- **Rust/Cargo**, a C toolchain, `pkg-config`, Wayland, libxkbcommon, fontconfig, libpulse, Vulkan/EGL development libraries, and PAM runtime
- **curl**
- **systemd-logind or elogind**, with a session bus and a polkit agent for session/power actions

bottom inherits its font from WezTerm. The installed font files cover all 87 icon codepoints used by the shell; graphical checks are recorded separately.

### Desktop font defaults

The desktop font choices are **GeistMono Nerd Font Mono** for monospace,
**Inter Nerd Font Propo** for sans-serif, and **Tinos Nerd Font Propo** for serif.
Install these families before applying the [fontconfig example](examples/fontconfig/60-kajitsu-fonts.conf).
Tinos is available from the [official Nerd Fonts downloads](https://www.nerdfonts.com/font-downloads)
or the Artix `galaxy/ttf-tinos-nerd` package.

```sh
mkdir -p ~/.config/fontconfig/conf.d
install -m644 examples/fontconfig/60-kajitsu-fonts.conf ~/.config/fontconfig/conf.d/
fc-cache -f
fc-match monospace
fc-match sans-serif
fc-match serif
```

These preferences apply when an application requests a generic font family.
Applications with explicit font settings need their own configuration. Restart
applications that have already loaded their fonts. Kajitsu uses Inter for UI text
and GeistMono for icons; bottom uses the font selected in WezTerm.

### Optional

These are only needed for their corresponding features:

| Package | Used for |
| --- | --- |
| `cava` | Media visualizer |
| `ffmpeg` / `ffprobe` | Image, video, and audio conversion |
| `ImageMagick 7` | Image conversion |
| `LibreOffice` (`soffice`) | Document conversion |
| `pw-play` / PipeWire and the freedesktop sound theme | Pomodoro sounds, found through the XDG data search path |
| `libnotify` (`notify-send`) | Pomodoro notifications |
| `wezterm` | Terminal applications, tmux attachment, and optional palette loader |
| `bottom` (`btm`) | Optional generated system-monitor configuration |
| `dconf` | GTK theme switching |
| `tmux` | Project launcher and generated tmux palette |
| `xdg-desktop-portal` | Picking a profile picture |
| `wf-recorder` | Screen recording |
| `pactl` | Recording desktop sound and mic together |

## IPC

Everything is driven through `kajitsu ipc call`. Mango bindings can invoke it directly:

```ini
bind=ALT,Space,spawn,kajitsu ipc call launcher toggle
bind=ALT+SHIFT,w,spawn,kajitsu ipc call wallpaper toggle
```

<details>
<summary><strong>All IPC targets</strong></summary>

```sh
kajitsu ipc call launcher toggle     # also show, hide, showTmux
kajitsu ipc call utility toggle      # also show, hide, then a page: notifications, wifi, bluetooth, record
kajitsu ipc call control toggle      # also show, hide
kajitsu ipc call wallpaper toggle    # also show, hide
kajitsu ipc call settings
kajitsu ipc call record              # start or stop a screen recording
kajitsu ipc call converter
kajitsu ipc call lock
```

</details>

## Launcher

The launcher handles normal application search and also acts as a small command palette.

Type `>` to switch into command mode. Commands can expose things such as:

- Settings
- Wallpapers
- Tmux sessions
- Other shell actions

The available command entries can be enabled or disabled from **Settings → Launcher**.

`showTmux` opens the launcher straight into the tmux project picker:

```ini
bind=ALT+SHIFT,p,spawn,kajitsu ipc call launcher showTmux
```

## Wallpapers & colors

By default the wallpaper picker reads images from:

```text
~/Pictures/Wallpapers
```

The folder can be changed from **Settings → Wallpaper**.

The selected wallpaper is persisted in:

```text
~/.local/state/kajitsu/wallpaper-selection
```

The wallpaper can also drive the shell's dynamic palette. Wallpaper transitions, shuffle, light/dark mode, and color schemes are configurable from the settings window.

## Lock screen

**Alt+L** uses Kajitsu's native Wayland session lock in the local Mango setup. The backend authenticates through the system's PAM `login` service. Kajitsu inherits Suzuha's lock listener, so logind lock requests (`loginctl lock-session`, an idle daemon, closing the lid) also bring it up. It reuses the active wallpaper and palette, and slides the password field up once you start typing. See [validation](docs/mango-validation.md) for the checks performed.

Lock it from the power menu, or through IPC:

```sh
kajitsu ipc call lock
```

The name and profile picture shown on the lock screen can be changed under **Settings → User**; they do not change the system account used for authentication.

> [!CAUTION]
> A Wayland session lock deliberately stays locked if the locker dies. If Kajitsu crashes while the session is locked, the compositor will not reveal the desktop; recover from another TTY if necessary.

## Pomodoro & converter

Two small tools that live inside the shell instead of being separate apps:

- **Pomodoro** — a focus timer with a wavy progress ring, also shown as a pill on the bar
- **Converter** — converts and compresses images, video, audio, and documents through `ffmpeg`, ImageMagick, and LibreOffice

```sh
kajitsu ipc call converter
```

## Settings

The shell includes its own settings window, so normal day-to-day preferences don't need code changes.

Current sections include:

- Appearance
- User
- Colors
- Wallpaper
- Bar
- Launcher
- Behavior
- Floating widgets
- Weather
- Integrations
- About

Settings are stored in:

```text
~/.local/state/kajitsu/settings
```

## Theme integrations

External theme integrations are **opt-in**. Enabling one may generate configuration files or update a running application, so the shell does not enable them automatically.

**WezTerm** and **bottom** are the primary targets. Both start disabled, as do the inherited GTK, tmux, Vesktop, Spotify/Spicetify, and cava entries.

For WezTerm, copy [the loader](examples/wezterm/kajitsu-colors.lua) beside your `wezterm.lua`, then apply it **after** your existing color/theme selection:

```lua
require('kajitsu-colors').apply(config)
```

Enable WezTerm in Kajitsu settings. The loader watches `${XDG_STATE_HOME:-$HOME/.local/state}/kajitsu/wezterm-colors.lua` for changes and applies only colors. Missing or invalid output keeps your existing color configuration. Your fonts, opacity, key bindings, projects, and other preferences remain in your own config. Remove the loader call to restore your existing theme selection. See [WezTerm's watch-list API](https://wezterm.org/config/lua/wezterm/add_to_config_reload_watch_list.html).

For bottom, enable its integration and launch with the generated configuration:

```sh
btm --config_location "${XDG_STATE_HOME:-$HOME/.local/state}/kajitsu/bottom.toml"
```

Kajitsu copies the source `bottom/bottom.toml` under XDG config and replaces only `[styles]`. It preserves flags, layout, filters, and non-style comments. A malformed source leaves the last valid output intact. Source changes trigger regeneration even if the shell palette is unchanged. **New launches** use the new styles; running bottom instances are not recolored. Avoid `--theme`, which takes precedence over custom config colors. Remove `--config_location` to return to your original configuration. See [the example](examples/bottom/README.md) and [bottom's styling reference](https://bottom.pages.dev/stable/configuration/config-file/styling/).

Other opt-in exporters write `tmux-colors.conf` under Kajitsu state, `spotify.css` under Kajitsu cache, a managed Vesktop CSS block, and `~/.config/cava/themes/amane` (an inherited theme filename). GTK creates themes and adjusts dconf/GTK CSS; it does not restart a GNOME portal service. Review each settings confirmation before enabling it.

For tmux, new sessions can load the generated palette with:

```tmux
source-file -q ~/.local/state/kajitsu/tmux-colors.conf
```

The paths shown with `~` use default XDG directories. Exports are atomic, and failed WezTerm/bottom writes are retried without overwriting the last valid output.

## Weather

The floating weather widget's location is set under **Settings → Weather** with a place name, latitude, and longitude.

Using approximate city-center coordinates is enough. Weather and air quality data are fetched directly from Open-Meteo; no IP geolocation service is used.

## Project structure

```text
src/bar/            status bar, workspaces, pills, and the pomodoro ring
src/overlay/        launcher, control center, utility panel, power menu
src/floating/       floating desktop widgets and their placement
src/wallpaper/      wallpaper, picker, transitions, and shuffle
src/lock_screen/    lock screen and the logind listener
src/settings/       settings window and its pages
src/theme/          palette generation and color schemes
src/integrations/   external theme/application integrations
src/converter/      file converter
src/motion/         springs, glides, and shared animation helpers
shaders/            the liquid shader
src/main.rs         root configuration
```

## Notes

This is a personal config being built for my Mango daily-driver setup. Its defaults, app integrations, and font requirements will follow what I use. Expect opinions, occasional breakage, and features that exist because I want them on my own desktop.

If you use it as a base for your own setup, reading and modifying the Rust is very much part of the experience.
