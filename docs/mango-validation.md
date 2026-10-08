# Mango conversion validation

The source conversion is implemented on `mango-conversion`. **Daily-driver cutover is pending** authenticated unlock, logind/suspend, physical output changes, and real recording. The current Mango startup and rustlock binding have not been changed.

## Build and framework

Validated on 2026-10-08 with the installed Mango 0.17.5. Cargo metadata resolves amane to `/home/sachin/Documents/amane/Cargo.toml`, with no registry or Git source. Kajitsu and amane must remain sibling checkouts.

The local amane branch is `kajitsu-mango-local`, tested at `cc4f34de3ad707ec54f523c210b7d595059ec069`. It contains the upstream Mango commit `9d36cb9d3531c55ea44b1ee485e64e4acf1eb58f`, followed by narrow fixes for subscription recovery, bounded queries, global-client occupancy, and `Terminal=true` desktop-entry metadata. Its tracked working tree is clean; the existing untracked `.abide/` directory is outside this conversion.

| Check | Result |
| --- | --- |
| `cargo metadata --locked --offline` | Local amane path confirmed |
| `cargo test --offline` | 24 Kajitsu tests passed after review fixes |
| `cargo test --manifest-path ../amane/Cargo.toml -p amane --lib --offline` | 37 library tests passed, including 15 Mango tests |
| `cargo fmt -- --check` | Passed, including the separate cleanup commit `eafc2f5` |
| `cargo clippy --all-targets --offline -- -D warnings` | Passed, including the separate cleanup commit `eafc2f5` |
| `cargo build --release --locked --offline` | Passed |
| `git diff --check` | Passed |

These checks used `RUSTC_WRAPPER=` because the inherited sccache wrapper cannot run in the restricted environment. Unix-socket tests ran with the necessary local socket access. Concurrent formatting and Clippy cleanup was preserved separately from the conversion and subsequently committed as `eafc2f5`; it was outside the independent conversion review.

The exported conversion source at `d5956ec` independently passed all **24 tests**. Before cleanup, comparing exported baseline and conversion source found **no new unformatted files or Clippy lint classes/files**: the baseline had 73 unformatted files and 11 distinct lints; the conversion retained 56 and seven. Those inherited lints were `collapsible_if` in bar/system, lock_screen/password, overlay/control_center/media and settings/pages/user; `manual_range_patterns` in floating/weather; `manual_is_multiple_of` in pomodoro; and `items_after_test_module` in theme. The separate cleanup commit resolves these branch gates; applying the conversion commits without it retains the old failures.

The independent review found a recorder-panel deadlock from nested read guards and an editable audio mode during asynchronous startup. Both were fixed in one pass: a contention regression reproduced the deadlock before passing, and a startup-state regression now verifies that pending capture keeps its chosen audio. The panel snapshots state under one read guard, renders after dropping it, shows “Starting…”, and blocks audio changes until startup finishes. All 24 tests pass after these fixes; real capture remains gated below.

Tests cover exact IPC argument framing, stale socket recovery, XDG path separation, tag presentation and conservative desktop occupancy, fresh output-query failure, bounded replies, palette serialization, atomic export failure/retry, literal tmux session arguments, and bottom preference preservation. The amane tests cover monitor/tag identity, multi-selection, urgency parsing, monitor focus confirmation, subscription EOF/recovery, malformed events, bounded queries, and global windows.

## Desktop and app checks

Graphical checks used a disposable Mango compositor with two headless outputs at scales 1 and 1.25, isolated Kajitsu state/cache/runtime directories, and a separate DBus session. Virtual pointer/keyboard devices were connected only to that compositor. Read-only queries also compared the native workspace API with the daily compositor's DP-9 and eDP-1 state.

| Area | Observed result | Remaining check |
| --- | --- | --- |
| Per-output tag focus | Actual bar clicks changed only the clicked output, including identical tag numbers on two outputs | Physical display hotplug |
| Multi-selected tags | Every selected tag is marked; all selected tags participate in desktop occupancy | Live urgency indication; urgency parsing is tested |
| Global and minimized clients | Global visibility hides desktop-only widgets without changing per-tag counts; minimized clients cease to obscure the desktop | More daily window-rule combinations |
| Rendering and fonts | Bar, wallpaper, cards, launcher, settings, notifications, and lock inspected at both scales; Regular/Bold UI text and icons render | Other physical display scales |
| Keyboard and overlays | Launcher received real virtual keyboard input; utility/control/settings IPC responded | Extended daily focus/keyboard workflow |
| Fullscreen | The inherited Top layer was obscured by fullscreen clients; moving interactive panels and dismissal to Overlay fixed the launcher and keyboard input | Physical fullscreen applications |
| Overview | Mango's overview selected multiple tags; selected markers and wallpaper rendered | Longer daily overview workflow |
| Wallpaper | Temporary wallpaper selection regenerated the dynamic palette and repainted shell surfaces | Daily wallpaper collection |
| Notifications | `notify-send` on the isolated session bus appeared in the shell | Actions from daily applications |
| WezTerm palette | Actual running window reloaded an atomically replaced color file; fonts, opacity, project module, and other existing settings remained in the original config | Native Wayland behavior in the daily session |
| WezTerm fallback | The example loader was executed with missing, malformed, invalid, and valid files; invalid output preserved existing colors and other preferences | Normal daily theme-command workflow |
| bottom | Ran with the generated config in WezTerm; layout, filters, flags, and comments are preserved by regression tests | Personal layout review after cutover |
| Terminal launcher route | A temporary `Terminal=true` bottom desktop entry was selected through Kajitsu and opened `btm` in WezTerm | Install a personal entry if desired; bottom currently has no installed desktop entry |
| tmux | The full session name remains one literal argument to `wezterm start -- tmux attach-session -t ...` | Actual project attachment in the daily environment |
| Lock | Disposable session lock acquired; clock, profile, and password field rendered on both outputs and accepted input | Authenticated unlock, logind signals, suspend/resume, and recovery on hardware |
| Logout | The production Mango quit helper received success and closed only the disposable compositor | Daily logout after cutover |
| Recording | Fresh focused-output lookup is bounded; missing `wf-recorder` produces a visible error | Install/use wf-recorder, capture a playable file, stop gracefully, and test output removal/audio |
| Settings | Settings window rendered and temporary persisted state loaded after restart | Broader UI edit/restart checks |

The standalone native-Wayland WezTerm trial failed with an NVIDIA explicit-sync error (`Buffer attached but no acquire point set`). An XWayland trial using a temporary `enable_wayland = false` override succeeded, including palette reload. The later desktop-entry launch also opened a working WezTerm/bottom window in the disposable session. This does not establish reliable native-Wayland operation on the daily displays; no user WezTerm config was changed.

No authentication secret was requested or submitted. The disposable lock was recovered by quitting its compositor. The test processes were closed afterward. The daily Mango compositor, gpui-shell startup, rustlock command, user application configs, and enabled integrations were left intact. New integrations start disabled; GTK/tmux/Vesktop/Spotify/cava remain unconfirmed opt-ins.

## Prepared cutover and rollback

After the remaining checks pass and cutover is requested, build Kajitsu and make its command available:

```sh
cd ~/Documents/kajitsu
cargo build --release --locked
install -Dm755 target/release/kajitsu ~/.local/bin/kajitsu
```

Back up `~/.config/mango/config.conf`. The current startup line is:

```ini
exec-once=/home/sachin/Documents/gpui-shell/target/debug/gpuishell
```

Replace that one line with:

```ini
exec-once=env KAJITSU_CONFIG_DIR=/home/sachin/Documents/kajitsu /home/sachin/.local/bin/kajitsu
```

Replace the existing fuzzel launcher binding and add the optional shell bindings from [the Mango example](../examples/mango/kajitsu.conf). Keep the complete existing rustlock command, including its screenshot/clock/effect options, until authenticated lock validation succeeds. Session startup should launch exactly one desktop shell and one notification service. Stop gpui-shell before a manual Kajitsu launch; `exec-once` is normally applied on the next login rather than by reloading config.

For rollback, exit Kajitsu while the session is unlocked, restore the previous startup and launcher bindings, and start the original gpui-shell (or log in again). Keep rustlock available throughout. Remove the WezTerm loader call to restore the original theme; launch bottom without the generated `--config_location` to use its original config. Disable integrations before removing generated files; source app configs remain authoritative.

## Implementation decisions

1. Keep a dedicated branch in the shared checkout to preserve the requested `../amane` layout and visible user files. Cost if wrong: less filesystem isolation than a worktree.
2. Query global-client visibility alongside native monitor events because per-tag counts omit global windows. Failed queries conservatively hide desktop-only widgets. Cost if wrong: widgets can stay hidden during query failure.
3. Add a local `TagState.global` presentation field while retaining exact per-tag counts. Cost if wrong: an additional presentation field.
4. Preserve `Terminal=true` in amane and launch those entries through WezTerm. Cost if wrong: incorrectly marked desktop entries open in a terminal.
5. Group graphical/app validation after the source changes build together. Cost if wrong: earlier commits alone are not evidence of a completed desktop port.
6. Use Inter Nerd Font Propo because the installed non-Propo family resolves Regular/Bold requests to Thin. Cost if wrong: a different UI family from the initial recommendation; no font installation was needed.
7. Recover only demonstrably stale native IPC sockets, while rejecting live sockets, symlinks, and unrelated files. Cost if wrong: ambiguous socket paths prevent startup rather than being deleted.
8. Place interactive panels and dismissal above fullscreen clients using Layer::Overlay, leaving the ordinary bar at Top. Cost if wrong: an open panel intentionally covers fullscreen content.
9. Re-grade editable startup audio from Minor to Important because a displayed “No sound” choice must not leave the captured mic mode active. Cost if wrong: a small additional state fix and regression test.

The reviewer set aside the following operational areas. Each remains an explicit limit rather than a claim of support:

| Decision | Cost if the limit is insufficient |
| --- | --- |
| Keep rustlock and gate authenticated unlock/logind/suspend/recovery on hardware validation | Uncovered authentication or suspend bugs |
| Gate physical hotplug and broader fullscreen/display combinations on hardware validation | Uncovered output or layer-order bugs |
| Keep live urgency, daily tmux and personal bottom layout checks pending | Workflow or layout mismatch |
| Gate playable recording, audio mixing and output removal during capture on wf-recorder validation | Uncovered capture or audio failure |
| Leave native-Wayland WezTerm reliability unresolved after the NVIDIA failure; preserve user config | Native terminal startup can still fail |
| Preserve concurrent cleanup separately and report conversion-only lint/format limitations; cleanup has now landed | Strict gates fail if the cleanup is omitted |
| Leave inherited integrations disabled and certify only converted paths/export behavior and terminal metadata | Latent bugs in optional inherited behavior |
| Target the installed Mango protocol; do not claim other compositor or future-version support | Future protocol or portability changes can break behavior |
