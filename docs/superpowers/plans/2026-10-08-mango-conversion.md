# Kajitsu Mango Conversion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Convert the Suzuha hard fork into a usable personal Mango shell with WezTerm, bottom, and fonts suited to Sachin's desktop.

**Architecture:** Use the local amane checkout at `~/Documents/amane` through a Cargo path dependency, including the existing `add-mango-workspace-support` changes. Keep amane's workspace service and add only small Kajitsu session-action helpers; app exporters preserve existing app preferences.

**Tech Stack:** Rust 2024, local amane 0.1.1 at `~/Documents/amane` with [Mango support](https://github.com/Sachin-Bhat/amane/tree/add-mango-workspace-support) from inspected commit `9d36cb9d3531c55ea44b1ee485e64e4acf1eb58f`, serde_json, toml_edit 0.25.15, Mango 0.17.5 JSON socket IPC, WezTerm Lua, bottom TOML.

**Spec:** [Conversion design](../specs/2026-10-08-mango-conversion-design.md).

## Global constraints

- Mango is the only supported compositor; there is no requirement to preserve Niri support.
- Keep the existing visual design.
- WezTerm and bottom are the confirmed application targets.
- Treat the currently installed Inter Nerd Font and GeistMono Nerd Font Mono as proposed font defaults, subject to the user's preference.
- External palette exports remain opt-in.
- Do not automatically import enabled Suzuha integrations or rewrite upstream files.
- Tag identity is `(output name, tag index)`, never a globally unique tag number.
- Use the local amane checkout/build in `~/Documents/amane`, as requested. Development and validation must consume that checkout's current source, including local changes.
- Include the existing `add-mango-workspace-support` changes in that local build and consume `amane::Workspaces`; do not add a duplicate Kajitsu workspace service.
- Declare `amane = { path = "../amane" }` for the current sibling layout. Record the local amane HEAD and working-tree state during validation; the inspected commit is provenance, not a required reset or Git dependency pin.
- Track `Cargo.lock` for resolved external dependencies. A path dependency remains tied to local source changes.
- Use the native Cargo workflow for Kajitsu; the local amane CLI's compile/dev workflow generates a replacement manifest and selects an embedded library snapshot.
- Use Kajitsu's XDG config/state/cache directories and optional `KAJITSU_CONFIG_DIR`.
- Preserve the framework's IPC socket and all existing IPC target names.
- Keep live session changes separate from source implementation until the desktop checks pass.
- Prefix shell commands with `rtk`, per the repository's RTK instruction.

## Review focus

1. Two monitors with the same tag numbers: a click must affect the monitor whose bar was clicked.
2. Multiple selected tags or a global client: desktop widgets must not treat the first empty tag as an empty desktop.
3. Mango restarts, stream EOF, malformed JSON, and output removal: no UI hangs, stale recording target, or retry spin.
4. Existing app preferences or invalid configuration: preserve original files and the last valid generated output.
5. Fractional scaling, keyboard focus, and session lock: rendering and interaction need actual Wayland checks, including a test-session lock/unlock.

## Work sequence

`1. Local build and paths → 2. Native Mango backend checks → 3. Shell behavior → 4. Fonts → 5. WezTerm → 6. bottom → 7. Desktop validation`

Tasks 4–6 can be reviewed independently after the build foundation. Complete them in sequence in a native session by default; this scope does not require parallel agents. Make one focused commit for each completed deliverable, without folding the existing README change into an unrelated commit.

## Task 1: Own a reproducible Kajitsu build and paths

**Files:** Create `Cargo.toml`, `src/cli.rs`, `src/paths.rs`; modify `.gitignore`, `Cargo.lock`, `src/main.rs`, `src/settings/store.rs`, `src/profile.rs`, `src/wallpaper/state.rs`, `src/integrations.rs`, `src/integrations/spotify.rs`, `src/overlay/control_center/cava.rs`, `src/settings/pages/about.rs`, `README.md`.

**Interfaces:**
- `paths::config_dir() -> PathBuf`, `state_dir() -> PathBuf`, `cache_dir() -> PathBuf` honor the spec's directories; `KAJITSU_CONFIG_DIR` affects config only.
- `cli::run(args: &[String]) -> Result<(), String>` handles `ipc call <target> [arguments...]`; main runs the shell only when there are no arguments. Help exits successfully; malformed commands exit nonzero without opening Wayland surfaces.
- IPC uses `amane::IpcCall::new`, `IpcCall::write`, and `amane::ipc_socket()`; close the write side before reading the reply.

- [ ] Add the tracked Rust 2024 manifest with binary name `kajitsu`, `amane = { path = "../amane" }`, and a direct serde_json dependency. Remove only `Cargo.toml` from `.gitignore`; preserve `target/`.
- [ ] Record the local amane branch, HEAD, and working-tree state. The planning audit found `format-with-rustfmt` checked out, while Mango support exists on a separate local branch. Prepare a local integration branch preserving the formatting work and incorporating Mango commit `9d36cb9d3531c55ea44b1ee485e64e4acf1eb58f`; resolve any routine merge conflicts and verify the resulting source includes the Mango backend.
- [ ] Build the existing shell against that local `~/Documents/amane` source. Resolve only API differences needed for this baseline; record native library requirements and the resulting checkout's HEAD/working-tree state.
- [ ] Inspect `rtk cargo metadata --locked --format-version 1` after initial dependency resolution. The resolved amane package's manifest must be `/home/sachin/Documents/amane/Cargo.toml` and its source must be local. Document the sibling layout and native Cargo commands so the build follows changes in this checkout.
- [ ] Add the native CLI and path helpers, then route the listed config/state consumers through them. About should identify Kajitsu and credit Suzuha; its checkout path must honor the configured directory.
- [ ] Test path resolution with explicit environment maps rather than mutating process-global environment in concurrent tests. Test IPC against a temporary Unix listener: the target/arguments have exact line framing, the writer finishes, and the reply is returned. A missing socket and an unknown CLI command must produce a useful nonzero result.
- [ ] Run `rtk cargo test` and `rtk cargo build --release --locked`. Verify `rtk proxy target/release/kajitsu --help`; then start a baseline in a test session. The amane CLI must not be required for build, run, or IPC.
- [ ] Update build/launch instructions and commit the independently working foundation.

## Task 2: Validate and harden amane's existing Mango backend

**Files:** Inspect and, for demonstrated gaps, modify local `~/Documents/amane/src/compositor/mango.rs`; preserve `~/Documents/amane/src/compositor.rs`, `src/services/workspace.rs`, and `src/services/workspaces.rs` API contracts. No Kajitsu workspace-service files are created.

**Interfaces:**
- Consume `Workspaces::read().list() -> &[Workspace]` and `Workspaces::focus(id: i64)` from the branch.
- Preserve the backend's stable IDs, tag indexes 1–31, urgency/count fields, exact PCRE2 monitor selectors, and focus verification before tag switching.
- Backend `listen(on_change: impl FnMut(Vec<Workspace>))` owns direct Unix-socket subscription and recovery. An empty callback on stream loss clears the public list; malformed individual lines retain the last good list.
- One-shot socket requests need bounded reads/writes; watch reads stay open until an event or stream loss. Preserve `Workspaces::focus`'s asynchronous public contract.

- [ ] Run the existing Mango backend tests with `rtk cargo test --manifest-path ../amane/Cargo.toml -p amane compositor::mango::tests`. They already cover per-monitor tags, multiple active tags, stable IDs, invalid replies/counts, exact monitor matching, and disabled-monitor focus verification. Do not duplicate them in Kajitsu.
- [ ] Compare a sanitized live `mmsg get all-monitors` response with the public workspace list and verify initial state, focus, selected tags, urgency, and client-count updates. mmsg is a diagnostic tool; the backend's event connection is direct socket IPC.
- [ ] Add a regression test for connection failure, EOF, and successful reconnect using injected/test Unix streams. Implement a single subscription loop that clears stale state on loss and reconnects with delays starting at 250 ms and capped at 5 s; reset the delay after a successful connection. Keep blocking socket work outside the service write lock.
- [ ] Add a stalled one-shot reply test. Set read/write timeouts of 3 s on requests used during focus changes and handle timeout/error replies without issuing the tag switch. Keep the existing exact selector and monitor-confirmation tests intact.
- [ ] Verify global/minimized client occupancy against the live tag counts. Preserve `Workspace::windows()` as the per-tag count. If a visible global client is omitted, record a failing occupancy case and add only the backend metadata required by Task 3 before calling desktop-only visibility complete.
- [ ] Run the backend tests, then Kajitsu's tests and release build through the local dependency. Record any narrow amane changes in that repository separately from Kajitsu commits. The shell continues to use its existing `Workspaces` consumers.

## Task 3: Make shell behavior correct on Mango

**Files:** Modify `src/bar/workspaces.rs`, `src/floating.rs`, `src/overlay/power_menu.rs`, `src/recorder.rs`, `src/main.rs`, `src/bar.rs`, `src/wallpaper.rs`, `src/overlay.rs`, `src/screen_mask.rs`, `src/overlay/dismiss.rs`, `src/wallpaper/picker.rs`; create `src/mango.rs`, `examples/mango/kajitsu.conf`.

**Interfaces:** Consume the existing `Workspace` getters and `Workspaces::focus(id)`. Add `mango::focused_output() -> Result<String, String>` and `mango::quit() -> Result<(), String>` for fresh recording-output selection and logout; their one-shot JSON socket requests have 3 s read/write timeouts and validate replies. For testable presentation logic, define an ephemeral `TagState { index: u32, selected: bool, windows: u32 }` in `src/mango.rs`, with `tag_states(workspaces: &[Workspace], output: &str) -> Vec<TagState>`, `desktop_empty(tags: &[TagState]) -> bool`, and `active_label(tags: &[TagState]) -> String`. There is no new persistent Mango service or tag registry.

- [ ] Keep both `amane::Workspaces` consumers and their native focus API. Adapt rendering to all selected tags and urgent markers per output; use `Tag N`/`Tags N, M` labels. Keep the sliding highlight for a single selection and explicit markers for multiple selections.
- [ ] Add behavior tests for label/selection calculation and widget visibility using `TagState`; this avoids constructing amane's private workspace fields in Kajitsu tests. Selected empty tag 1 plus occupied selected tag 3 must not count as empty; two selected empty tags do. No selected tags must hide desktop-only widgets; `always` and `hidden` widget modes keep their existing meaning. Add the global-client case established in Task 2.
- [ ] Route logout through Mango dispatch. Make recording choose the currently focused output using a fresh bounded query before spawning wf-recorder; propagate unavailable focus, query failure, and output removal to the existing record UI. Preserve audio handling and graceful stop.
- [ ] Test fresh recording output selection for either monitor, no focused output, a removed monitor, malformed JSON, and a timed-out request. Verify a recording uses the intended output and a stopped recording produces a readable file in the test session. Leave logout's actual session termination for the nested session.
- [ ] Check wallpaper, rounded mask, bar reservation, overlays, fullscreen, overview, outside-click dismissal, and keyboard grabs on Mango. Change only surfaces/rules that fail. Add layer rules only where they prevent duplicate compositor/shell effects or fix demonstrated behavior.
- [ ] Write a Mango startup/binding fragment using `kajitsu ipc call` and the current config's launcher convention. Document that it replaces the existing shell startup entry during cutover; keep the existing rustlock binding until Task 7's lock checks pass.
- [ ] Run tests and a release build. Search `src` for executable Niri commands; none may remain. Confirm `Workspaces` reads and focus calls still route through local amane's Mango backend. Commit the usable Mango shell behavior.

## Task 4: Set the personal font baseline

**Files:** Modify `src/fonts.rs`, font-dependent widgets where metrics require it, and `README.md`.

**Interfaces:** Preserve centralized font roles. Default recommendation is `Inter Nerd Font` for body text and `GeistMono Nerd Font Mono` for monospace/primary Nerd glyphs; update this choice if the user supplies different families before implementation.

- [ ] Match candidate family names with fontconfig and audit codepoints used by Nerd, Symbols, and Material roles. Check battery states, memory, tray, media, launcher, weather, recording, and lock icons.
- [ ] Replace Poppins/JetBrains defaults. Use the Nerd family for material glyphs where coverage is verified; keep Symbols Nerd Font only for uncovered glyphs. Remove a font dependency from the README only when no runtime glyph requires it.
- [ ] Visually inspect bar, launcher, settings, notifications, widgets, and lock at scale 1 and 1.25. Adjust clipping, alignment, and fixed widths where the new family changes metrics. No new font-picker subsystem is needed.
- [ ] Build and verify the dependency list matches the selected families. Commit the font change; do not add tests that merely assert constant strings.

## Task 5: Replace kitty/foot theming with WezTerm

**Files:** Create `src/integrations/wezterm.rs`, `examples/wezterm/kajitsu-colors.lua`; modify `src/integrations/terminal.rs`, `src/integrations.rs`, `src/settings/store.rs`, `src/settings/pages/integrations.rs`, `src/overlay/launcher/results.rs`, `README.md`.

**Interfaces:**
- Preserve the existing ANSI color calculation as a shared terminal palette helper; remove kitty/foot writers and process recoloring.
- `wezterm::render(theme: &Theme) -> String` returns a Lua color table; `export(theme: &Theme) -> Result<(), String>` atomically writes `paths::state_dir()/wezterm-colors.lua`.
- `integrations::write_owned(path: &Path, text: &str) -> Result<(), String>` is the checked atomic writer used by the new WezTerm and bottom exporters.
- `integration_wezterm` replaces the generic terminal entry, initially false.
- Integration success is recorded only after a successful write. An error is reported and retried on a later update.

- [ ] Test dark/light serialization: foreground/background, cursor foreground/background/border, selection foreground/background, and exactly eight regular plus eight bright ANSI values must match the palette helper. Validate the generated Lua using Lua parsing or WezTerm's own configuration loader.
- [ ] Implement the exporter and update the settings title, effect description, and catalog. Add an atomic-write failure test: the old valid palette remains and the failed export does not advance its success fingerprint.
- [ ] Document a small loader applied after the user's existing color selection. It uses a guarded load, preserves existing config when the generated file is absent/invalid, and registers the palette path through `wezterm.add_to_config_reload_watch_list`.
- [ ] Fix tmux attachment to launch `wezterm start -- tmux attach-session -t <session>` directly, replacing the generic `$TERMINAL -- ...` assumption. Test that the entire session name stays one argument. Include a terminal application such as bottom in the launcher check so its desktop-entry route also reaches WezTerm.
- [ ] Verify generated colors in new and existing WezTerm windows, with the current fonts, opacity, project setup, and theme commands. Check fallback after removing the loader. Confirm no kitty processes, foot signals, or terminal-device writes occur.
- [ ] Run the affected tests and release build, update usage/dependencies, and commit.

## Task 6: Replace btop theming with bottom

**Files:** Create `src/integrations/bottom.rs`, `examples/bottom/README.md`; retire `src/integrations/btop.rs`; modify `Cargo.toml`, `Cargo.lock`, `src/integrations.rs`, `src/settings/store.rs`, `src/settings/pages/integrations.rs`, `README.md`.

**Interfaces:**
- Add `toml_edit = "=0.25.15"` and use its [DocumentMut API](https://docs.rs/toml_edit/0.25.15/toml_edit/struct.DocumentMut.html). Lock dependency resolution and keep generated syntax compatible with bottom's installed parser.
- `bottom::render(base: &str, theme: &Theme) -> Result<String, String>` replaces the generated styling in a copy of the base document, preserving every non-style setting.
- `bottom::export(theme: &Theme) -> Result<(), String>` reads the user's `bottom/bottom.toml` under XDG config and writes `paths::state_dir()/bottom.toml` atomically. Missing base configuration means an empty/default base; invalid base configuration is an error.
- `integration_bottom` replaces `integration_btop`, initially false. Source-config changes are part of the export invalidation check.

- [ ] Test preservation using a base config containing flags, a custom layout, process/network filters, and comments. After rendering, those values remain; generated styles use the intended shell colors. Assert the rendered document parses as TOML without duplicate tables.
- [ ] Map styles for widgets, selection, borders, text, tables, graphs, CPU, memory, network, and battery to the shell palette. Verify field names against the installed bottom 0.14.9 format, including both light and dark palettes.
- [ ] Test missing source, malformed source, repeated generation, source changes with an unchanged shell theme, and a failed output write. Invalid input must not replace the previous valid generated file or mutate the original configuration.
- [ ] Add the settings entry and document `btm --config_location <Kajitsu-state>/bottom.toml`. Explain that the first version applies changes on the next launch. Do not claim config inclusion or hot reload without verifying support.
- [ ] Run bottom against the generated configuration in WezTerm and confirm preferences/layout survive. Run targeted tests and a release build, update documentation, and commit.

## Task 7: Validate and prepare the daily-driver cutover

**Files:** Update `README.md`, `examples/mango/kajitsu.conf`, `src/settings/pages/about.rs`; modify `src/integrations/gtk.rs` and its settings copy only as needed to remove the inappropriate GNOME-portal restart; add `docs/mango-validation.md`.

**Interfaces:** Preserve every existing shell IPC target. Examples consistently use `kajitsu ipc call`. Read-only app inventories do not authorize adding new enabled integrations.

- [ ] Review the remaining GTK/tmux/Vesktop/Spotify/cava entries against the user's actual app list. Leave unconfirmed entries disabled. Remove Niri-specific runtime notes, obsolete kitty/foot/btop setup instructions, and incorrect font requirements from the active setup guide while retaining Suzuha attribution.
- [ ] Audit GTK/portal behavior for Mango. Remove the unconditional GNOME-portal restart; only add a portal-specific action if a live check establishes that it is required for this setup.
- [ ] Record the local amane HEAD and working-tree state, and verify Cargo metadata still resolves its package to `~/Documents/amane`. Run `rtk cargo fmt -- --check`, `rtk cargo test`, `rtk cargo clippy --all-targets -- -D warnings`, `rtk cargo build --release --locked`, and `rtk git diff --check`. Document pre-existing warnings separately; do not hide warnings introduced by this work.
- [ ] Complete a live matrix: tag clicks on each display; multiple selected/urgent tags; empty/occupied/global-client desktop behavior; monitor focus and hotplug; scale 1/1.25; overlays/keyboard input; fullscreen/overview; wallpaper changes; launcher and tmux; settings persistence; notifications; recording; both app palettes.
- [ ] Validate lock/unlock and logind/suspend behavior in a nested or disposable session with a recovery route. Keep rustlock as the daily lock command until these checks succeed. Test the logout action in that test session.
- [ ] Prepare the concrete startup change that replaces gpui-shell with Kajitsu, including rollback instructions. Verify only one shell/notification service is intended to run. Apply the daily session change only when implementation and validation are complete and the user requests the cutover.
- [ ] Record results and any failed cases in `docs/mango-validation.md`. Do not mark Mango or an app integration supported on the basis of compilation alone. Commit the verified delivery and its documentation.

## Plan validation and remaining decisions

The task mapping covers compositor state, direct commands, Wayland behavior, native build/IPC, paths, fonts, application defaults, theme generation, recording, session lock, and documentation. The five review risks each have an owning task and an explicit check.

The outstanding personal choices are the preferred UI/icon font families and any additional app integrations. The proposed fonts are available locally; if no different preference is supplied, use those recommendations. The scope of the first working conversion remains Mango plus WezTerm and bottom.

This document is a plan, not evidence of a completed port. No build, graphical, lock, or application-reload check has been performed for the proposed changes. The planning audit used source inspection, installed CLI help/version output, fontconfig inventory, and read-only Mango queries. The linked Mango branch was inspected through local Git objects, and a read-only GitHub API request confirmed its remote revision matches `9d36cb9d3531c55ea44b1ee485e64e4acf1eb58f`. Its tests were inspected but have not been run in this planning update. No branch switch or source change was made in the local amane checkout.
