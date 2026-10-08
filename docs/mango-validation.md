# Mango conversion validation

The source conversion is implemented on `mango-conversion`. **Kajitsu is installed and running in the daily Mango session** following the requested local setup. Mango startup, launcher, and Alt+L lock bindings now use Kajitsu. Authenticated unlock, logind/suspend, physical output changes, and recording audio/output-removal checks remain pending. See [desktop setup](desktop-setup.md) for the installed components and rollback.

## Build and framework

Validated on 2026-10-08 with the installed Mango 0.17.5. Cargo metadata resolves amane to `/home/sachin/Documents/amane/Cargo.toml`, with no registry or Git source. Kajitsu and amane must remain sibling checkouts.

The local amane branch is `kajitsu-mango-local`, tested at `fdc48eedf35dd0a83d376042de9d581709180dde`. It contains the upstream Mango commit `9d36cb9d3531c55ea44b1ee485e64e4acf1eb58f`, followed by narrow fixes for subscription recovery, bounded queries, global-client occupancy, `Terminal=true` desktop-entry metadata, backlight selection, and hidden layer lifecycle. Its tracked working tree is clean; the existing untracked `.abide/` directory is outside this conversion.

| Check | Result |
| --- | --- |
| `cargo metadata --locked --offline` | Local amane path confirmed |
| `cargo test --offline` | 25 Kajitsu tests passed, including the timer sound search regression |
| `cargo test --manifest-path ../amane/Cargo.toml -p amane --lib --offline` | 39 library tests passed, including 15 Mango tests and two backlight-selection regressions |
| `cargo fmt -- --check` | Passed, including the separate cleanup commit `eafc2f5` |
| `cargo clippy --all-targets --offline -- -D warnings` | Passed, including the separate cleanup commit `eafc2f5` |
| `cargo build --release --locked --offline` | Passed |
| `git diff --check` | Passed |
| `python3 scripts/check-mango-panels.py` | 12 rapid cycles across four panels, plus reopened-panel inside/outside pointer clicks, on two disposable outputs |

These checks used `RUSTC_WRAPPER=` because the inherited sccache wrapper cannot run in the restricted environment. Unix-socket tests ran with the necessary local socket access. Concurrent formatting and Clippy cleanup was preserved separately from the conversion and subsequently committed as `eafc2f5`; it was outside the independent conversion review.

Strict Clippy above applies to Kajitsu. Running standalone amane strict Clippy reports 13 diagnostics; an exported copy of its previous `cc4f34d` commit reproduces the same diagnostics. They are inherited framework lints, not a passing standalone check.

The exported conversion source at `d5956ec` independently passed all **24 tests**. Before cleanup, comparing exported baseline and conversion source found **no new unformatted files or Clippy lint classes/files**: the baseline had 73 unformatted files and 11 distinct lints; the conversion retained 56 and seven. Those inherited lints were `collapsible_if` in bar/system, lock_screen/password, overlay/control_center/media and settings/pages/user; `manual_range_patterns` in floating/weather; `manual_is_multiple_of` in pomodoro; and `items_after_test_module` in theme. The separate cleanup commit resolves these branch gates; applying the conversion commits without it retains the old failures.

The independent review found a recorder-panel deadlock from nested read guards and an editable audio mode during asynchronous startup. Both were fixed in one pass: a contention regression reproduced the deadlock before passing, and a startup-state regression now verifies that pending capture keeps its chosen audio. The panel snapshots state under one read guard, renders after dropping it, shows “Starting…”, and blocks audio changes until startup finishes. All 24 tests passed after these fixes. The subsequent desktop setup adds a timer sound search regression and a successful real capture check.

Tests cover exact IPC argument framing, stale socket recovery, XDG path separation, tag presentation and conservative desktop occupancy, fresh output-query failure, bounded replies, palette serialization, atomic export failure/retry, literal tmux session arguments, and bottom preference preservation. The amane tests cover monitor/tag identity, multi-selection, urgency parsing, monitor focus confirmation, subscription EOF/recovery, malformed events, bounded queries, and global windows.

## Desktop and app checks

Graphical checks used a disposable Mango compositor with two headless outputs at scales 1 and 1.25, isolated Kajitsu state/cache/runtime directories, and a separate DBus session. Virtual pointer/keyboard devices were connected only to that compositor. Read-only queries also compared the native workspace API with the daily compositor's DP-9 and eDP-1 state.

| Area | Observed result | Remaining check |
| --- | --- | --- |
| Per-output tag focus | Actual bar clicks changed only the clicked output, including identical tag numbers on two outputs | Physical display hotplug |
| Multi-selected tags | Every selected tag is marked; all selected tags participate in desktop occupancy | Live urgency indication; urgency parsing is tested |
| Global and minimized clients | Global visibility hides desktop-only widgets without changing per-tag counts; minimized clients cease to obscure the desktop | More daily window-rule combinations |
| Rendering and fonts | Bar, wallpaper, cards, launcher, settings, notifications, and lock inspected in disposable sessions; the installed shell renders on physical DP-9 and eDP-1 at scales 1 and 1.25 | Other physical display scales |
| Keyboard and overlays | Launcher received real virtual keyboard input; utility/control/settings IPC responded | Extended daily focus/keyboard workflow |
| Fullscreen | The inherited Top layer was obscured by fullscreen clients; moving interactive panels and dismissal to Overlay fixed the launcher and keyboard input | Physical fullscreen applications |
| Overview | Mango's overview selected multiple tags; selected markers and wallpaper rendered | Longer daily overview workflow |
| Wallpaper | Temporary wallpaper selection regenerated the dynamic palette and repainted shell surfaces | Daily wallpaper collection |
| Notifications | Isolated `notify-send` worked; Kajitsu owns the daily notification service and a Clipcat notification appeared | More daily application actions |
| WezTerm palette | Actual running window reloaded an atomically replaced color file; fonts, opacity, project module, and other existing settings remained in the original config | Native Wayland behavior in the daily session |
| WezTerm fallback | The example loader was executed with missing, malformed, invalid, and valid files; invalid output preserved existing colors and other preferences | Normal daily theme-command workflow |
| bottom | Ran with the generated config in WezTerm; layout, filters, flags, and comments are preserved by regression tests | Personal layout review after cutover |
| Terminal launcher route | A temporary `Terminal=true` bottom entry opened `btm` in WezTerm; local setup now installs a personal entry and generated-config wrapper | Extended daily launcher use |
| tmux | Literal session arguments are tested; installed tmux loaded generated colors, with `kajitsu` and `amane` project sessions initialized | Actual project attachment in the daily environment |
| Lock | Disposable session lock acquired; clock, profile, and password field rendered on both outputs and accepted input | Authenticated unlock, logind signals, suspend/resume, and recovery on hardware |
| Logout | The production Mango quit helper received success and closed only the disposable compositor | Daily logout after cutover |
| Recording | Installed wf-recorder; native IPC start/stop produced a playable H.264 MP4, confirmed by FFprobe, on a disposable Mango output | Audio modes and output removal during capture |
| Settings | Settings window rendered and temporary persisted state loaded after restart | Broader UI edit/restart checks |

The standalone native-Wayland WezTerm trial failed with an NVIDIA explicit-sync error (`Buffer attached but no acquire point set`). An XWayland trial using a temporary `enable_wayland = false` override succeeded, including palette reload. The later desktop-entry launch also opened a working WezTerm/bottom window in the disposable session. This does not establish reliable native-Wayland operation on the daily displays. The trial left the user's WezTerm config unchanged; the subsequent local setup added its palette loader while preserving other preferences.

During the source-conversion checks, no authentication secret was requested or submitted. The disposable lock was recovered by quitting its compositor, and test processes were closed afterward. That phase left daily configuration intact. The later requested local setup replaced GPUi Shell, backed up modified configs, and enabled GTK/WezTerm/bottom/tmux/Cava. Spotify and Vesktop remain disabled because those apps are absent. At the user's request, Alt+L now uses Kajitsu's native lock instead of the initially retained Rustlock command.

Artix setup exposed two inherited assumptions: `systemctl` is absent on dinit/elogind, and the NixOS timer sound path is absent. Power actions now call the shared login1 DBus interface; read-only capability checks returned `yes`. The timer locates the freedesktop alarm through XDG data directories; a failing search-path regression now passes, and zero-volume PipeWire playback succeeded. No shutdown, reboot, or suspend was executed.

The power-menu button now uses the Artix glyph, visually checked on both physical outputs. The laptop exposes two backlights; `AMANE_BACKLIGHT_DEVICE=amdgpu_bl1` selects its actual panel, showing 80% instead of the unrelated NVIDIA device's 100%. Device selection and unknown-device behavior have regression tests. Elogind accepted a same-value brightness request without changing the display level.

Rapid panel toggles reproduced a Wayland configure-serial failure. Amane retires hidden layer roles after dispatching the current batch, creates fresh roles for later display, and exits on fatal protocol errors. Kajitsu keeps its transparent dismissal surface mapped and switches its input region so it remains below interactive panels. The regression runner enables outside-click dismissal and uses a virtual pointer bound only to its headless compositor. Startup supervision also prevents duplicate launches, records the shell's actual PID, and bounds logs; duplicate launch and restart passed in the daily session.

The setup review identified the recreated dismissal role taking input above reopened panels. A real pointer check reproduced the issue before the mapped-surface fix. The new regression checks inside clicks retain the panel and outside clicks dismiss it. The review found no other material issue in the setup changes; the earlier conversion review remains separate.

## Cutover and rollback

The requested local setup completed cutover with copied runtime assets and backup/rollback helpers. Mango now starts:

```ini
exec-once=/home/sachin/.local/bin/kajitsu-start
```

GPUi Shell is stopped. Kajitsu owns notifications, the launcher binding, and the Alt+L lock shortcut. Its native lock uses PAM `login`; the installed policy includes the system authentication stack and the local shell is allowed. A snapshot of the setup's Mango configuration was saved before switching the lock binding. The updated configuration passed Mango's parser and was reloaded; the daily desktop was not locked for an authentication test. See [desktop setup](desktop-setup.md) for the installed shortcuts, update/restart commands, and restoration procedure. The original user configs and GTK dconf values are backed up with a manifest and rollback script. Optional integrations preserve their source app preferences.

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
| Use Kajitsu's native lock as requested; authenticated unlock/logind/suspend/recovery remain unverified on hardware | Uncovered authentication or suspend bugs |
| Gate physical hotplug and broader fullscreen/display combinations on hardware validation | Uncovered output or layer-order bugs |
| Keep live urgency, daily tmux and personal bottom layout checks pending | Workflow or layout mismatch |
| Playable recording and graceful stop now pass; audio mixing and output removal still need validation | Uncovered capture or audio failure |
| Leave native-Wayland WezTerm reliability unresolved after the NVIDIA failure; preserve user config | Native terminal startup can still fail |
| Preserve concurrent cleanup separately and report conversion-only lint/format limitations; cleanup has now landed | Strict gates fail if the cleanup is omitted |
| Enable the five installed app integrations after backing up configs; leave absent Spotify/Vesktop integrations disabled | Latent bugs in inherited integration behavior beyond the performed export checks |
| Target the installed Mango protocol; do not claim other compositor or future-version support | Future protocol or portability changes can break behavior |
