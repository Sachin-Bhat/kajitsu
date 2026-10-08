# Kajitsu conversion to Mango

Status: proposed design for planning; implementation has not started.

## Intent and scope

Kajitsu is a hard fork of Suzuha for Sachin's daily-driver Mango desktop. Preserve the existing shell, wallpaper-derived palette, panels, animations, launcher, widgets, and utilities while replacing compositor assumptions and application defaults. WezTerm and bottom are the confirmed application targets. Font requirements should reflect this desktop.

Planning assumptions: Mango is the only supported compositor; there is no requirement to preserve Niri support. Keep the existing visual design. Additional application integrations remain disabled until selected. Treat the currently installed Inter Nerd Font and GeistMono Nerd Font Mono as proposed font defaults, subject to the user's preference.

Use the local amane checkout/build in `~/Documents/amane`, as requested. Development and validation must consume that checkout's current source, including local changes.

Use the Mango backend from [Sachin-Bhat/amane's add-mango-workspace-support branch](https://github.com/Sachin-Bhat/amane/tree/add-mango-workspace-support), inspected at `9d36cb9d3531c55ea44b1ee485e64e4acf1eb58f`. The remote revision matches the local branch. Kajitsu should consume its existing workspace API.

Success means a reproducible build, working per-monitor Mango tags, correct desktop-widget visibility, working shell actions and recording, readable text and complete icons, and usable WezTerm/bottom themes without losing existing application preferences.

## Findings

| Area | Evidence | Consequence |
| --- | --- | --- |
| Build | `Cargo.toml` is absent and explicitly ignored; `Cargo.lock` records amane 0.1.1 | This checkout currently relies on amane's generated build project |
| Local checkout | `~/Documents/amane` currently has `format-with-rustfmt` checked out at `da45e21c96f10c0470a5a6255892a241aade17c2`; the Mango branch exists locally but is not checked out | Prepare the local build to include the Mango changes while preserving current work |
| Framework support | Branch `add-mango-workspace-support` adds Mango detection and a native socket backend at `9d36cb9d3531c55ea44b1ee485e64e4acf1eb58f` | Reuse native workspace support instead of adding a Kajitsu workspace service |
| Workspace consumers | `src/bar/workspaces.rs` and `src/floating.rs` already use `amane::Workspaces` | Preserve those APIs; adapt their single-active-workspace assumptions |
| Direct commands | Logout invokes `niri msg action quit`; recording parses `niri msg focused-output` | Replace both with Mango state/actions |
| Mango | Installed compositor reports `0.17.5(release)`; `mmsg --help` and read-only queries confirm JSON `get`/`watch` IPC | Target this protocol; do not implement the old flag-based mmsg interface |
| Tag semantics | The live response contains per-monitor tags with `index`, `is_active`, `is_urgent`, and `client_count` | Tag identity must include monitor name; multiple tags may be selected |
| Fonts | Shell constants use Poppins, JetBrainsMono Nerd Font, Symbols Nerd Font, and Material Design Icons | Font names and glyph coverage need explicit conversion |
| Personal fonts | WezTerm uses GeistMono Nerd Font Mono; fontconfig lists Inter Nerd Font, GeistMono Nerd Font Mono, and Symbols Nerd Font | These provide grounded candidate families |
| Integrations | Terminal exporter writes kitty/foot formats; monitor exporter writes a btop theme | Neither format can simply be renamed for the target applications |
| Paths | Settings, wallpaper selection, profile, exports, About, and cava assume amane directories | Separate Kajitsu's files from upstream state |
| Existing session | Mango config starts gpui-shell and binds fuzzel and rustlock | Cutover must prevent duplicate shell surfaces and preserve a working lock route during validation |
| Portal behavior | GTK exporter restarts `xdg-desktop-portal-gnome` | That restart must not be assumed appropriate for Mango |

Framework claims were checked against the [Mango branch's compositor routing](https://github.com/Sachin-Bhat/amane/blob/9d36cb9d3531c55ea44b1ee485e64e4acf1eb58f/src/compositor.rs), [Mango backend](https://github.com/Sachin-Bhat/amane/blob/9d36cb9d3531c55ea44b1ee485e64e4acf1eb58f/src/compositor/mango.rs), local Git objects, [Cargo manifest](/home/sachin/Documents/amane/Cargo.toml), and [generated build project](/home/sachin/Documents/amane/cli/src/project.rs). Mango commands are documented in its [IPC manual](https://github.com/mangowm/mango/wiki/ipc); live queries confirmed the installed protocol.

## Recommended architecture

Own a small Cargo project in Kajitsu and continue using the local amane checkout as the rendering/service library, with the Mango branch's changes included. With the current sibling layout in `~/Documents`, declare `amane = { path = "../amane" }` in Kajitsu's manifest. Cargo reads the local library source directly and builds or reuses compatible artifacts. Keep `amane::Workspaces` for workspace state and tag switching. A small Kajitsu helper handles logout and fresh recording-output queries; it does not maintain another workspace registry or event listener.

The inspected local HEAD and Mango branch revision are provenance for this plan, not Git dependency pins. Record the actual build checkout's HEAD and working-tree state with validation results; `Cargo.lock` locks resolved external dependencies but cannot freeze changes to a path dependency. During implementation, prepare a local integration branch that includes the Mango support commit and preserves the current formatting work. No branch switch or merge is part of this planning update. If Kajitsu is moved, update the dependency path to resolve to `~/Documents/amane` and document the new layout.

Use Kajitsu's native Cargo workflow. The local amane CLI generates a manifest beside the config and unpacks an embedded library snapshot, so running its compile/dev workflow on Kajitsu would replace the custom manifest and select that snapshot instead of the direct local dependency.

The native Cargo project provides the extra dependencies needed for application configuration parsing and small session-action queries. The existing amane backend owns workspace parsing and switching; implementing those again in Kajitsu would duplicate the branch's work.

The executable is `kajitsu`. With no arguments it runs the shell; `kajitsu ipc call <target> [arguments...]` forwards the existing shell IPC targets using amane's public `IpcCall` and `ipc_socket()` APIs. Keep the framework's socket location for this conversion. Do not install an amane CLI merely to send IPC calls.

## Mango state and shell behavior

The amane branch connects directly to `MANGO_INSTANCE_SIGNATURE`, subscribes to `watch all-monitors`, and translates newline-delimited monitor snapshots into ordinary `Workspace` values. No mmsg child is required for workspace events. Each monitor retains an allocated slot, and tag IDs combine that slot with the tag index, so IDs remain stable across output reordering and reconnection.

Consume `Workspace::output()`, `index()`, `active()`, `focused()`, `urgent()`, `windows()`, and `id()`. The backend permits several active/focused tags on one output. Tag identity is `(output name, tag index)`, never a globally unique tag number. It does not expose the JSON monitor's `active_client` field; validate global-client occupancy rather than assuming that field is available through the workspace API.

The branch already skips malformed event lines and tests stable IDs, multi-tag state, exact monitor selectors, and confirmed monitor focus before a tag change. Its listener currently ends on connection failure or EOF without clearing stale state or reconnecting; socket reads also have no explicit timeout. Task 2 validates and hardens those narrow gaps inside local amane. Workspace state must clear on stream loss, reconnect with a bounded delay, and avoid blocking UI threads. At startup or while disconnected, an empty workspace list yields unavailable tag state and hides desktop-only widgets.

Each bar displays its monitor's tags, including empty tags. Highlight every selected tag and show every urgent tag. Keep the existing sliding highlight when exactly one tag is selected; use explicit selected markers for a multi-tag view. Labels use `Tag 1` or `Tags 1, 3` rather than suggesting a single active Niri workspace.

Clicking a tag calls `Workspaces::focus(id)`. The backend rechecks the ID in a fresh snapshot, escapes and anchors the monitor's PCRE2 selector, dispatches `focusmon`, confirms the selected monitor, and then dispatches `viewcrossmon`. Preserve this behavior. The public focus API is asynchronous and returns no result; verify observable changes rather than treating a call as confirmation of success. The small logout helper dispatches Mango's `quit` action and checks the reply.

Desktop-only widgets are eligible only when the workspace list supplies at least one selected tag on the output and every selected tag has zero windows. Check all selected tags instead of the first match. Validate this against global and minimized clients in the live session; if Mango's per-tag counts omit a visible global client, extend the backend's occupancy metadata narrowly before claiming desktop-only parity. Preserve the exact per-tag `windows()` count contract.

Recording obtains the focused output from Mango immediately before starting wf-recorder. If focus is unavailable or the output has disappeared, recording does not start and displays an explanation. Preserve audio mixing and graceful SIGINT shutdown.

Validate layer order, exclusive zones, keyboard focus, click-through surfaces, Mango overview, fullscreen windows, output changes, and fractional scaling. Adapt only the surfaces that show a problem; do not redesign the shell during this conversion.

## Paths and defaults

Use `${XDG_CONFIG_HOME:-$HOME/.config}/kajitsu` for configuration, `${XDG_STATE_HOME:-$HOME/.local/state}/kajitsu` for persistent state and generated palettes, and `${XDG_CACHE_HOME:-$HOME/.cache}/kajitsu` for disposable output. An optional `KAJITSU_CONFIG_DIR` overrides the config directory when developing a checkout elsewhere.

Update About, cava's configuration lookup, settings, profile, wallpaper selection, and integration writers to use shared path helpers. Start with fresh Kajitsu state. Do not automatically import enabled Suzuha integrations or rewrite upstream files. Manual preference import can be documented if wanted.

Replace the kitty/foot and btop entries with named WezTerm and bottom entries. Other inherited integrations stay disabled pending a confirmed app list. External palette exports remain opt-in: choosing the project's default app targets does not silently enable third-party configuration changes. This preserves the existing opt-in setting behavior.

## Fonts

Proposed UI family: `Inter Nerd Font`. Proposed monospace/primary Nerd Font family: `GeistMono Nerd Font Mono`. Keep `Symbols Nerd Font` only for glyphs that need it after an audit. Replace the Material Design Icons font dependency only after verifying the actual battery, memory, and other glyphs in use.

Keep font roles centralized in `src/fonts.rs`. Match family names through fontconfig, check the glyphs actually used, then adjust sizes and alignment where the new metrics require it. Font-family selection does not require a new settings subsystem in this conversion. bottom inherits its font from WezTerm.

## Application integrations

WezTerm gets a generated Lua color table under Kajitsu state. Reuse the current ANSI palette calculation; replace kitty/foot serialization and process-wide recoloring with WezTerm fields for foreground, background, cursor, selection, ANSI, and bright colors. A documented loader in the user's existing WezTerm configuration applies those colors and registers the generated file for reload. Missing or invalid generated data falls back to the existing configuration. Preserve fonts, key bindings, opacity, projects, and manual theme commands. See [WezTerm color fields](https://wezterm.org/config/lua/config/colors.html) and [reload watch lists](https://wezterm.org/config/lua/wezterm/add_to_config_reload_watch_list.html).

bottom gets an effective TOML configuration under Kajitsu state, generated from the existing bottom configuration with only its styling replaced. Preserve flags, layout, filters, and all non-style settings. Use a TOML parser, not string concatenation into a possibly existing table. Launch it with `btm --config_location <generated-file>`; the first version makes new launches use the new palette and does not promise live recoloring. Recheck the source configuration when it changes. A malformed source leaves the last valid generated file intact and reports an error. See [bottom styling](https://bottom.pages.dev/stable/configuration/config-file/styling/).

Generated output is written atomically. An unsuccessful export must not be cached as successful; the next update retries. Disabling an integration stops exports; removing its loader or generated-config launch option restores the app's previous configuration. No application process scanning, blanket signals, or full configuration replacement is required.

## Validation and delivery

Reuse amane's existing Mango tests for JSON parsing, stable monitor/tag identity, and safe focus routing. Add meaningful tests for backend recovery, Kajitsu's multi-tag rendering/desktop rules, session-action IPC framing, generated color formats, and preservation of bottom preferences. Use fixture data with unrelated titles removed. Font metrics and Wayland surface behavior require visual checks on Mango.

The desktop cutover follows a successful build and validation in a nested/test session. First keep rustlock as the operational lock command. Validate Kajitsu's lock/unlock, logind request, suspend, and output changes in a test session before choosing it for daily use. Change startup and launcher bindings only after there is a tested replacement. Record how to restore the existing startup entry.

Completion requires a real session on Mango 0.17.5 with mixed display scales, working actions and palettes, no unintended Niri commands, documented dependencies and IPC examples, and no duplicate bars or notification services. Newer Mango releases are supported only after their JSON protocol passes the same checks.
