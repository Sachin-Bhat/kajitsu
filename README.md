<div align="center">
  <h1>kajitsu</h1>
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

This fork follows its own direction: building a config for **Mango**, my daily-driver compositor. Default application integrations will be based on the programs I use, including **WezTerm** for the terminal and **bottom** for system monitoring. Font choices and requirements will also be tailored to my setup, rather than retaining Suzuha's defaults.

The Mango migration, WezTerm and bottom integrations, and font changes are planned work. The current code still contains Suzuha's Niri-specific behavior, kitty/foot and btop integrations, and original font requirements; the implementation details below describe that inherited state.

### Included

- Status bar with workspaces, system information, clock, media, and a pomodoro pill
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
- Optional theme integrations for GTK, terminals, tmux, Vesktop, Spotify, btop, and cava

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

The tracked manifest uses `amane = { path = "../amane" }`, so changes in the local library are used on the next build. Its Mango support comes from [add-mango-workspace-support](https://github.com/Sachin-Bhat/amane/tree/add-mango-workspace-support). Cargo.lock locks external dependencies; local amane changes are deliberately not pinned. See [validation](docs/mango-validation.md) for the tested revision and remaining desktop checks.

Use native Cargo commands here. The amane CLI's compile/dev workflow generates a different manifest and embedded library snapshot. No amane CLI is required to launch Kajitsu or send IPC.

Configuration defaults to `$XDG_CONFIG_HOME/kajitsu` (or `~/.config/kajitsu`). Set `KAJITSU_CONFIG_DIR` to this checkout so shaders and `cava.conf` are found. State and cache use `$XDG_STATE_HOME/kajitsu` and `$XDG_CACHE_HOME/kajitsu`, with the usual home-directory fallbacks. Kajitsu starts with fresh settings; it does not import upstream enabled integrations.

## Dependencies

### Required

- **Local amane source** in `~/Documents/amane`, with Mango workspace support
- **Mango** (JSON IPC, tested against installed 0.17.5)
- **Inter Nerd Font** for UI text
- **GeistMono Nerd Font Mono** for monospace and icons
- **Rust/Cargo**, a C toolchain, `pkg-config`, Wayland, libxkbcommon, fontconfig, libpulse, Vulkan/EGL development libraries, and PAM runtime
- **curl**

bottom inherits its font from WezTerm. The installed font files cover all 87 icon codepoints used by the shell; graphical checks are recorded separately.

### Optional

These are only needed for their corresponding features:

| Package | Used for |
| --- | --- |
| `cava` | Media visualizer |
| `ffmpeg` / `ffprobe` | Image, video, and audio conversion |
| `ImageMagick 7` | Image conversion |
| `LibreOffice` (`soffice`) | Document conversion |
| `pw-play` / PipeWire | Pomodoro sounds |
| `libnotify` (`notify-send`) | Pomodoro notifications |
| `kitty` remote control | Live kitty palette updates |
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

Kajitsu inherits Suzuha's Wayland session lock that listens to logind, so anything that asks logind to lock (`loginctl lock-session`, an idle daemon, closing the lid) brings it up. It reuses the active wallpaper and palette, and slides the password field up once you start typing.

Lock it from the power menu, or through IPC:

```sh
kajitsu ipc call lock
```

The name and profile picture shown on the lock screen can be changed under **Settings → User**; they do not change the system account used for authentication.

> [!CAUTION]
> A Wayland session lock deliberately stays locked if the locker dies. If amane crashes while the session is locked, the compositor will not reveal the desktop; recover from another TTY if necessary.

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

Kajitsu's planned default integration targets include **WezTerm** and **bottom**, reflecting the applications I use. They are not implemented yet. The inherited integrations currently include:

- GTK 3 / GTK 4
- kitty
- foot
- tmux
- Vesktop
- Spotify / Spicetify
- btop
- cava

<details>
<summary><strong>Generated files and side effects</strong></summary>

Depending on which integrations are enabled, Kajitsu may write files like:

```text
~/.local/state/kajitsu/terminal-colors-kitty.conf
~/.local/state/kajitsu/terminal-colors-foot.ini
~/.local/state/kajitsu/tmux-colors.conf
~/.config/btop/themes/amane.theme
~/.config/cava/themes/amane
~/.cache/kajitsu/spotify.css
```

GTK integration also generates light and dark wallpaper-derived themes under `~/.local/share/themes/` and updates the active color-scheme preference through `dconf`.

For tmux, add this to `~/.tmux.conf` so new sessions load the generated palette:

```tmux
source-file -q ~/.local/state/kajitsu/tmux-colors.conf
```

For kitty, include the generated colors and let running windows be recolored live:

```conf
allow_remote_control socket-only
listen_on unix:@amane-kitty
include ~/.local/state/kajitsu/terminal-colors-kitty.conf
```

</details>

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
