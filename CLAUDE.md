# CLAUDE.md

Guidance for working with code in this repository.

## What this project is

**Rift** is a personal, **local-only** fork of Warp. Everything cloud is being removed —
AI/agents, Warp Drive, account/auth, telemetry, billing, sharing, cloud workspace sync. The goal
is a fast, fully local terminal that keeps Warp's good parts (the command **blocks** UI, GPU
rendering, editor-style command input) and nothing that phones home.

**Do not reintroduce or extend** cloud, accounts, AI, or telemetry. When in doubt, the local
option wins.

## Upstream sync

Rift tracks `warpdotdev/warp` as the `upstream` remote and ports fixes by hand (the `warp→rift`
rename means cherry-picks don't apply cleanly).

**Last reviewed/synced against upstream: 2026-07-20.**

To sync again, start from that date, not earlier:

```bash
git fetch upstream
git log upstream/master --since=2026-07-20 --date=short --pretty='%h %ad %s'
```

### Notes from the 2026-07-20 review

Reviewed 5 upstream commits (2026-07-19 … 2026-07-20). **Nothing in that window
was portable.** The real work of this pass was landing the 2026-07-19 review's
*uncommitted* remainder, and **fixing two commits that review missed** — one of
which it had actively broken Rift by half-porting.

- **The 2026-07-19 pass shipped a regression: it took warp #13515 (promote async
  find to stable) without its prerequisite #13521.** Upstream deliberately split
  the behavioral fix out of the enablement PR; Rift took only the flag flip. With
  `async_find` on, four integration tests failed
  (`test_case_sensitive_find`, `test_find_bar_autoselects_text`,
  `test_block_filtering_filter_then_find`,
  `test_block_filtering_toggle_filter_while_find_active`) — the find bar reported
  0 matches in tests and, in the app, counted/focused/highlighted matches in rows
  an active block filter hides. Both #13521 and the other miss below **were
  present in the `--since` listing**; this was a *triage* failure, not an
  enumeration one, and it would have been caught by running the integration
  tests. **Lesson: when porting a feature-flag promotion, check that flag's other
  upstream commits, and always run `-p integration` afterwards** — a flag flip is
  never a "2-line, can't-break-anything" port.

- **Ported warp #13521 — async find respects block-filter-hidden rows.**
  `AbsoluteMatch` gains `is_filtered`, recomputed by `recompute_filtered_for_block`
  (`update_matches_for_filtered_block`'s async branch was a no-op);
  `match_count`, focus traversal, and `BlockFindRenderData::from_async` all skip
  filtered matches; `from_range` stops routing through `AbsolutePoint::from_point`,
  whose displayed→original translation corrupted match rows when scanning under an
  active filter. Rift's `async_find.rs` was byte-identical to upstream's pre-fix
  state (modulo warp→rift and one rustfmt import ordering), so this is a
  zero-divergence port.

- **Ported warp #10738/#9098 — group header for every multi-pane vertical tab.**
  Also missed by the 2026-07-19 pass. The header was gated on
  `has_custom_title || is_being_renamed`, so an un-renamed split tab rendered with
  no tab-level label at all. Reachable on the **default** path:
  `VerticalTabsDisplayGranularity::Panes` is `#[default]` and the granularity is a
  plain user setting with no feature-flag guard. Upstream blames its AI/CLI
  session-naming flow (removed in Rift), but ordinary un-renamed splits hit the
  same gate, so the comments were reworded. 5 upstream unit tests ported as-is.

- **Committed the 2026-07-19 leftovers.** The prior pass left its last two
  changes in the working tree, unverified and uncommitted: the warp #13696
  alt-screen scroll port and the `CommandXRayTriggered` /
  `TabSingleResultAutocompletion` / `ImageReceived` telemetry deletions. Both are
  now verified (`cargo check --bin rift-oss` and `--tests -p rift` at 0/0;
  `rift_terminal` 110 passed; `rift terminal::*` 759 passed) and committed
  separately.
- **Followed the `ImageReceived` deletion through.** Removing that call site left
  `ImageProtocol` — a *telemetry* type — threading through the terminal core
  (`ansi_handler` → `Event` → `ModelEvent`) purely to be discarded at the view.
  Per the strip guardrails the field is now gone from both enums and its three
  `ansi_handler` producers, and the unreferenced enum is deleted. Note the old
  call site was one of the **bit-rotted** ones CLAUDE.md warns about: it bound
  `image_protocol: _` while the macro body referenced `*image_protocol`, and
  compiled only because the shim discards its tokens.
- **Fixed a warp→rift rename leftover in `.gitignore`.** Line 24 still read
  `crates/warp_files/test_data/test_write`, so `rift_files`' `lib_tests.rs`
  (`WRITE_TEST_PATH = "test_data/test_write/"`) leaked its scratch output into
  `git status` on every test run. Also ignored `command-signatures-v2/js/bun.lock`,
  the local bun-install artifact from the clippy unblock step (upstream's
  `yarn.lock` stays the tracked lockfile).

- **Deliberately NOT ported (verified N/A, not skipped blindly):**
  - **Rust 2024 edition migration (warp #13990)** — 1893 files, +22.7k/−21.4k, and
    **not a bug fix**. Upstream generated it with `cargo fix --edition` +
    `cargo clippy --fix` + `./script/format`, so for Rift this is *regenerable
    work*, not a hand-port — which is the only reason it's even tractable. Skipped
    for now on the same grounds as #13523 and #13483 (mass mechanical churn is not
    taken as a unit), plus two Rift-specific risks: edition 2024 changes
    **`if let` scrutinee temporary drop order**, and Rift has an explicit
    deadlock hazard around `TerminalModel::lock()` (see "Terminal Model Locking")
    — exactly the kind of code where a silent drop-order change matters; and the
    migration forces a full-workspace rebuild, which is expensive here. **This one
    has a real cost to deferring**, unlike the other churn skips: every commit
    upstream now lands on 2024-edition context lines, so hand-porting drifts
    further each pass. Take it as one deliberate project — run the three fix-up
    commands, then audit every `if let` holding a lock guard — not as part of a
    routine sync.
  - **Runner config for orchestration (warp #13896)** — its only Rift-existing file
    is `pane_group/pane/terminal_pane.rs`, but all three touched items
    (`dispatch_start_agent_conversation`, `launch_remote_child`,
    `RemoteLaunchFields`) are absent from Rift's copy. Pure AI orchestration.
  - **PR url + repository host in `InputContext.Git` (warp #13936)** — adds a
    `host` field to `RepositoryInfo` in the kept `app/src/util/git.rs`, but
    `RepositoryInfo`/`get_repository_info` have **zero consumers** anywhere in
    Rift outside `util/git.rs` itself; the field exists to feed the removed AI
    input context. Dead island.
  - **Partial `read_files` results (warp #12054)** — an agent tool; its only
    non-AI hunk is a `warp_multi_agent_api` proto rev bump, and that dep is not in
    Rift's `Cargo.toml`.
  - **TUI `/view-logs` slash command (warp #13932)** — TUI surface; touches only
    `Cargo.lock` outside `tui/`.
  - **`getent` user resolution (warp #13384)** — a follow-up to #13382, which Rift
    *did* port, so it fit the miss pattern above and was re-checked. Genuinely
    unported (Rift still has `struct Passwd<'a>` / `get_pw_entry` / the raw
    `libc::getpwuid_r` block), but **skipped on merit**: it exists so a static
    musl remote binary can resolve SSSD/LDAP/AD accounts it can't reach through
    glibc NSS plugins. Rift is macOS-only, where `getpwuid_r` resolves via
    Directory Services and upstream's tier 1 always succeeds. The Rift-side gain
    is only removing an `unsafe` block and deduplicating two call sites, and the
    port needs hand-adaptation because #13483 (the `report_error!` migration Rift
    deliberately skipped) sits between #13382 and #13384. Optional cleanup, not a
    bug fix.

### Notes from the 2026-07-19 review

Reviewed 142 upstream commits (2026-07-10 … 2026-07-19). Ported 12; the rest were
the ratatui **TUI** surface (roughly half the window), AI/agents/orchestration/MCP,
VA video-recording, Oz/cloud/auth/billing, telemetry, or the removed code editor.

Two ports fixed bugs that were **already live in Rift**, not just upstream:

- **Render-test logging was not idempotent (warp #13889).** `crates/editor`'s
  `render/model/test_utils.rs` called `env_logger::…init()` while
  `content/find_tests.rs` already called `try_init()`. Whichever lost the race
  panicked with `SetLoggerError`, and because the guard is a **`parking_lot::Once`**
  the panic *poisoned* it, cascading into 8 failures. `cargo test -p rift_editor
  --lib` was 419 passed / **8 failed** before, 427 / 0 after. CI never saw it
  because nextest forks per test — but CLAUDE.md's own port bar says
  `cargo test -p <crate>`, so it burned anyone verifying an editor port.
- **Zero-delta alt-screen scrolls wrote to the PTY (warp #13696, partial).**
  `accumulate_lines_to_scroll` truncates to whole lines and returns 0 on sub-cell
  trackpad ticks, but both branches still built an empty byte vec and passed it to
  `write_user_bytes_to_pty` — which is *not* a no-op on empty input
  (`mark_received_user_input`, `clear_selected_blocks`, scroll-position locking,
  `emit_non_editor_typed_event`). Gently trackpad-scrolling in vim/htop cleared the
  user's block selection and forced redraws while sending nothing. Reachable by
  default. Only the GUI + shared-encoder slice was taken; the TUI half and the
  `pub(crate) mod alt_screen` widening (which exists only to feed `tui_export`)
  were dropped.

- **Also ported:**
  - **Terminal/links:** exclude fullwidth punctuation from detected links (warp
    #13085) — minus upstream's `app/src/util/link_detection.rs` hunk, which is the
    removed AI rich-text detector; the terminal-facing fix is complete without it.
  - **Markdown:** strip the full GFM autolink trailing-punctuation set `?!.,:*_~`
    so `**https://example.com**.` stops swallowing the closing `**` (warp #13747).
  - **Workspace:** tab-configs menu capped to window height + scrollable (warp
    #13224).
  - **CLI:** `args_conflicts_with_subcommands` → `subcommand_precedence_over_arg`
    (warp #13816). Reproduced in Rift: `rift --debug completions zsh` failed with
    `invalid value 'completions' for '[URLS]...'`. `--api-key` is gone from Rift but
    `--debug`/`--finish-update`/`--parent-pid` trigger it just as well. Added
    `crates/rift_cli/src/lib_tests.rs` (Rift had none) — 4 of its 10 tests fail
    before the fix.
  - **Log severity (warp #13905/#13923, partial).** See the caveat below.
  - **Feature flag:** async find promoted to stable (warp #13515) — upstream
    shipped it in `default`, so Rift's stable-only rule is satisfied.
  - **riftui_core:** table body layer `set_active_layer_click_through()` (warp
    #12841). Unreachable today (`Table` is used only by `riftui/examples/table-sample`;
    the agent-output markdown-table surface is removed and the editor's own table
    renderer already sets it) — taken as a 1-line zero-divergence keep.
  - **Deps/security:** serde_with 2.3.3 → 3.21.0 for GHSA-7gcf-g7xr-8hxj, which
    **requires** jumping command-signatures to `29cd61c3` (not merely #13673's
    `ec1ae8e8`) — the older rev pins serde_with `^2`, so bumping only the workspace
    dep leaves the vulnerable 2.3.3 in the lock. Rift has no `serde_as`/`KeyValueMap`
    use, so this is audit hygiene, not a live crash.

- **Caveat on the `report_error!` demotion series (warp #13905/#13923/#13922/#13910/#13924).**
  This series is upstream *walking back* #13483 ("migrate `log::error!` →
  `report_error!`"), which Rift deliberately never ported. Rift is at the
  pre-#13483 state, so for Rift the net delta is only a severity change
  `log::error!` → `log::warn!`. Only #13905 (7 OSC-parser sites — worth it, that
  parser consumes untrusted program output and is trivially spammable) and #13923
  (7 sites, incl. the one real `report_error!` demotion and a message-correctness
  fix) were taken. #13922/#13910/#13924 are cosmetic-only in Rift or land on
  AI/Windows code. The organizing premise — "reduce Sentry noise" — doesn't apply:
  Rift's `report_error!` is purely local logging. **Don't re-litigate these.**
  The two `secrets.rs` sites were converted to `safe_warn!` rather than `log::warn!`
  because a regex-compile error embeds the user's secret-detection pattern.

- **Deliberately NOT ported (verified N/A, not skipped blindly):**
  - **OSC 8 hyperlinks (warp #9850).** `FeatureFlag::OscHyperlinks` is in upstream's
    **`DOGFOOD_FLAGS` only** — `PREVIEW_FLAGS` is empty and it's absent from
    `RELEASE_FLAGS` — so the stable-only rule applies, one rung *below* the
    tab-grouping precedent. Also 2 days old with zero follow-ups, ~1900 lines
    through hot paths (per-cell storage, FlatStorage RLE, reflow), and **not a bug
    fix**: Rift already renders OSC 8 text correctly, it's just not clickable.
    Technically the port is clean (the storage layer is byte-identical after
    warp→rift), so revisit on graduation — **but add a scheme allow-list**:
    upstream's `open_hyperlink_uri` validates only `Url::parse`, so `javascript:`
    and `file://` open, from URIs and link text that untrusted terminal output
    fully controls. Its own spec lists an allow-list as an invariant; it shipped
    without one.
  - **Procedural box-drawing glyphs (warp #13453)** — `BoxDrawingGlyphs` is
    DOGFOOD-only. Rift's `grid_renderer.rs` is barely diverged, so it's easy later.
  - **Pinned tabs (warp #13767)** — see the 2026-06-14 update above. The flag
    condition fired, but Rift forked one day before `view/tab_grouping.rs` was
    created, and 6 of the 9 pinning functions live in it; Rift's `tabs` table also
    has no `tab_group_id`/`pinned` columns. Pinning is ~1.5k LOC on top of a
    ~2.5–3.5k LOC tab-groups prerequisite (11 unported commits incl. persistence).
    ~4–5k LOC total. Only take it as one deliberate project, in order:
    grouped-tabs + migration → `tab_grouping.rs` → pinning + migration → flags.
  - **Command-palette file search (warp #13433/#13801).** #13801 would be *actively
    harmful* in Rift: it hides directories because upstream "has no way to open
    them", but in Rift `OpenFile` is a no-op (`workspace/view.rs`, code editor
    removed) while `OpenDirectory` works (it `cd`s the terminal) — so it would leave
    the palette entirely inert. #13433 can't apply either: it refactors a
    query-in-traversal design (`get_repo_contents(query, app)`) Rift doesn't have.
    Rift's palette is more degraded than upstream's was (no query filter pushed
    into the walk, so only the first ~100 entries are ever seen) — fixing that is
    Rift-original work, not a port.
  - **OSC 7 drive paths (#13920), PowerShell history (#13803), ConPTY kill (#13802)** —
    all Windows. Rift is macOS-only (CI is `macos-14`; `local_tty::windows` is
    `#[cfg(windows)]`). #13920's one non-cfg-gated hunk is a provable macOS no-op.
    #13803's regression came from #13526, which Rift never took.
  - **`crates/editor` render/model changes (#13591/#13655/#13817)** — `render/model`
    is a **dead island** in Rift: zero consumers outside `crates/editor`, and Rift's
    command input is the separate `app/src/editor/`. Rift has none of the
    `char_cell_*` machinery these depend on. #13764 (markdown images) is the removed
    file-viewer and needs an unported `LocalFileContentVersion`.
  - **repo_metadata teardown (#12328)** — compiled but unreachable: `get_repo_contents`
    is the only API with callers; nothing in Rift ever indexes or tears down a repo,
    so the cancelled-walk bug has no trigger. 3-way apply conflicts in 3 files.
  - **Terminal lifecycle recovery (#13788)** — zero matches in Rift; it's the
    #12853–#12859 groundwork still deferred.
  - **Synthetic mouse (#13644)** — the GUI hunk only widens visibility for the TUI;
    no behavior change. **Long-running command rendering (#13774)** — adds a `pub`
    wrapper with no non-TUI caller. **keymap (#13781) / app.rs (#13545)** — would add
    dead public API. **StandardizedPath (#13552)** — wasm-only `debug_assert`.
    **keychain (#13692)** — scopes storage per `LaunchMode::Tui`; Rift has only
    App/Test. **headless HTTP server (#13715)** — Rift's `is_headless()` is
    hardcoded `false`. **cmov (#13423)** — no applicable advisory.
  - **TUI conversation persistence (#13590)** — despite touching many kept
    blocks/selection files, every hunk is an `AgentViewState` → `TranscriptScope`
    parameter swap; Rift deleted that parameter outright, so there is nothing to
    rename and no selection-correctness content.
  - **Telemetry-removal commits (#13847/#13713/#13659/#13772/#13621)** — not ports
    (Rift's telemetry macros are already no-op shims), but they pinpoint **strip
    leftovers**; #13713's were cleaned up here (see below).

- **Strip work done in this pass:** deleted the dead `CommandXRayTriggered` (×2),
  `TabSingleResultAutocompletion`, and `ImageReceived` telemetry call sites and
  their `events.rs` variants/match arms. Note `ImageReceived` was already dead
  upstream but **live in Rift**, so Rift needed one deletion upstream didn't.

- **Known remaining strip debt (not this pass).** ~225 `send_telemetry_*!` call
  sites remain; `app/src/server/telemetry/` is ~3.5k lines of parked dead code
  (`events.rs` alone ~3.3k) with no live consumer. Dead AI flags still declared:
  `PRCommentsSlashCommand`, `PRCommentsV2`, `PRCommentsSkill`,
  `OrchestrationViewerStreamer`, `OwnerOrchestrationAncestorStreamer`. Also a
  `PrintTelemetryEvents` CLI subcommand (`rift_cli/src/lib.rs` + `app/src/lib.rs`
  dispatch + `events.rs::print_telemetry_events_json`). **Several call sites are
  bit-rotted** — their macro bodies reference identifiers the strip already renamed
  (e.g. `view.rs`'s `send_telemetry_on_executor!(auth_state, block_event, executor)`
  where all three are now `_`-prefixed locals); they compile only because the shim
  discards its tokens. That is latent breakage, not just dead weight.

### Notes from the 2026-07-10 review

Reviewed 24 upstream commits (2026-07-09, after the prior review's 10:10 EDT
cutoff … 2026-07-10). **Nothing ported — no commit touched a Rift-kept
subsystem in a reachable way.** The window was almost entirely the ratatui
**TUI** surface and AI/agents/computer-use/VA/MCP, plus cloud/Oz/auth/billing
and onboarding. Verified each candidate that *looked* like it might apply and
confirmed it doesn't:

- **Deliberately NOT ported (verified N/A, not skipped blindly):**
  - **Hidden-section bar: double-click to fully expand (warp #11621) and its
    follow-up "scope unmodified-lines hover to the label text" (warp #13556).**
    These modify the collapsed-unmodified-lines UI of the removed code
    editor / diff viewer. `crates/editor` still carries the `hidden_ranges` /
    `RenderableHiddenSection` machinery, but nothing in Rift calls
    `set_hidden_lines`, so the bar is never rendered, and #11621's interactive
    double-click/hover wiring lived in the removed `app/src/code/editor/view/`.
    Rift's `hidden_section.rs` is at the pre-#11621 state (static bar, no
    `Hoverable`/`on_click`). Revisit only if a hidden-lines consumer is ever
    added to Rift.
  - **Orchestration pill bar infinite-width panic on pane drag (warp #13528).**
    The fix (`ConstrainedBox::with_max_width(400.)` around the drag-preview
    header) lands in the kept `pane_group/pane/view/mod.rs`, but the panic is
    specifically the orchestration pill bar's *horizontal scrollable* reporting
    an infinite/NaN viewport to `Scene::validate_rect`. That pill bar is a
    removed AI feature; Rift's header uses only `MainAxisSize::Max` + `Clipped`
    + `Shrinkable` (no horizontal `Scrollable`), and is daily-driven without
    pane-drag crashes, so the panic isn't reachable. Mechanically portable
    (`ConstrainedBox::with_max_width` exists in `riftui_core`) — revisit only if
    a horizontal scrollable is ever added to the pane header.
  - **macOS computer-use keycode-cache main-thread crash (warp #13547)** —
    `crates/computer_use` doesn't exist in Rift.
  - **Terminal theming probe (warp #13542)** — TUI-only (detects the ambient
    host terminal's background when Warp runs *inside* another terminal); Rift is
    a standalone GUI terminal, so there is no ambient terminal to probe.
  - **Separate persistence scope for TUI and GUI (warp #13500)** — gives the TUI
    its own `warp.sqlite`; Rift has no TUI, so there is only one (GUI) scope and
    the separation is a no-op. The `terminal/input.rs` touch is pure scope
    plumbing, no standalone GUI fix.
  - **`import warp_errors directly` (warp #13523)** — mechanical import-path
    churn across ~hundreds of files, ~95% in removed AI subsystems; no
    functional change. Same call as the skipped #13483 log→report migration.
  - **macOS protoc install CI resilience (warp #13516)** — patches
    `.github/actions/prepare_environment/action.yml`, which Rift doesn't have
    (Rift's CI is `.github/workflows/{test,release}.yml`, no composite actions).
  - The remaining 18 were AI/agents/MCP/computer-use/VA, TUI (zero-state,
    warping indicator, inline diff, slash mixer, credits footer, editor
    extraction), cloud/Oz/auth/billing/subscribe, feature-intro/onboarding
    popovers, or the Slack-community label rename.

### Notes from the 2026-07-09 review

Reviewed 231 upstream commits (2026-06-23 … 2026-07-09). Ported 20 PRs across 17
commits; the rest were AI/agents/MCP, the new ratatui **TUI** surface,
tab-groups/pinning (PREVIEW-only), onboarding, cloud/Drive/auth/billing/
telemetry, file-viewer/notebook/voice, or Windows/Linux-only.

- **Ported:**
  - **Crash/panic fixes:** RowIterator crash when clear-resize truncates a
    wide-char spacer (warp #12726); `get_pw_entry`/fallback-shell panic when the
    current uid has no passwd entry (warp #13382, #13367); cross-window tab-drag
    RefCell-borrow panic on fullscreen preview creation + stale-index bounds
    guards (warp #13409).
  - **Terminal/blocks:** detect file paths across soft-wrapped lines (warp
    #12968/#9193); exclude trailing sentence period from file links (warp
    #12965); dedupe multiline command-block prefix (warp #12619); include PS1 in
    block copy (warp #13076); make the completions "not working" banner
    permanently dismissible (warp #12969).
  - **Editor/input:** visual-line Home/End + macOS document nav (warp #13195);
    preserve vim visual selection across history recall (warp #13152); stop
    semantic drag selection over-running non-word ranges (warp #12985).
  - **Workspace/themes:** activate tab below after closing active vertical tab +
    escape exe path in dump-debug-info (warp #13142, #13188); keep header-toolbar
    chip labels readable on light themes (warp #13209); "Copy current path"
    command-palette action, minus the removed file-viewer branch (warp #13148).
  - **macOS/build/shell:** don't block system-initiated termination (warp
    #12480); resolve rich-text glyph identity via CGFont (warp #12923/#13317);
    stop auto-claiming file types via LSHandlerRank (warp #13121); standalone-CLI
    `bundled_resources_dir` (warp #13312); `CARGO_FULL_PROFILE`→`PROFILE` build
    fallback (warp #13342); fall back to plain SSH when ssh_config sets
    RemoteCommand (warp #13219/#13315).
  - Port caveats: #12726's exact flat-storage crash-repro test needs the
    `enable_full_grid_clear_behavior` scaffolding Rift lacks, so the reset is
    verified at the GridStorage level + upstream's reflow test instead. #12985
    was completed here — a prior WIP had landed only the inert `word_boundaries`
    refactor (1 of 7 files), so the fix wasn't actually delivered.
- **Deliberately NOT ported (notable):**
  - Horizontal cross-window tab-drag "fuzzy shake" (warp #13007) — depends on the
    un-ported prerequisite #12746 (`reordering_in_source`); Rift is at the
    pre-#12746 cross-window-drag state, so the bug isn't reachable. Revisit only
    if #12746 lands.
  - Faster `FxHashMap`/`FxHashSet` for EntityId maps (warp #13058) — 31-file
    churn, most of it in the removed `tui/` surface; perf-only, deferred.
  - Mass `log::error!`→`report_error!` migration (warp #13483) — 300+ files,
    almost entirely in removed subsystems (drive/code/auth/ai/voice); impractical
    as a unit.
  - Run tab-level commands from launch-config URIs (warp #13103), vertical-tabs
    Summary PR-chip clickable (warp #12945, emits telemetry), heap-profile CLI
    action (warp #13107), PowerShell linter fixes (warp #13242) — low value or
    Windows-only; deferred.
  - Precmd-metadata / terminal-lifecycle series (warp #12853–#12859) — still
    deferred as premature groundwork (per the 2026-06-23 note).

### Notes from the 2026-06-23 review

Reviewed 128 upstream commits (2026-06-15 … 2026-06-23). Ported 7 fixes; the
rest were AI/agents/Oz-cloud/remote/MAA/LRC/telemetry/auth/billing/onboarding/
file-tree/code-editor/pinning/tab-grouping changes (out of scope), already
present, or incomplete upstream features.

- **Ported:**
  - OSC 1337 inline-image crash fix (warp #12889) — bare `OSC 1337` with no
    second param indexed `params[1]` and panicked; added the `params.len() < 2`
    guard + regression test.
  - markdown_parser delimiter-run `u8`→`usize` overflow fix (warp #12644) — a
    run of 256+ `*`/`_`/`~` panicked (debug) / corrupted (release).
  - rift_files reversed line-range fix (warp #12642) — truncation on the first
    in-range line produced a reversed `start..0` range.
  - CRLF command-paste normalization per shell family (warp #12446) — POSIX
    shells now keep `\n` (CRLF→`\n` only on Windows); PowerShell still →`\r`.
  - block_list `mouse_down` now guarded by `if !handled` (warp #12079) — it was
    the only mouse arm missing the already-handled guard its siblings have.
  - search mixer: late async results placed at the low-priority edge instead of
    appended after established results (warp #12600).
  - Bumped `warp-command-signatures` to rev `a937ae35` for eza completions
    (warp #12798) — pure rev bump, source-compatible.
- **Deliberately NOT ported (notable):**
  - Vertical-tabs traffic-light padding (warp #12660) — its fix and test hang
    off the removed AI "tools panel" (`HeaderToolbarItemKind::AgentManagement/
    ToolsPanel/CodeReview`); doesn't apply to Rift.
  - PTY-spawn E2BIG fail-fast (warp #12663) — entangled with the AI agent driver,
    Oz-secret error text, and telemetry call sites.
  - Precmd `exit_code`/`next_block_id` metadata (warp #12853) — bootstrap scripts
    emit the fields but the parser only no-ops them; preparatory groundwork for
    the in-progress block-lifecycle-hardening series, no realized benefit yet.
  - tree-sitter `MAX_PARSE_BYTES` guard (warp #12696) — primarily for the removed
    code editor; marginal for Rift's command-input syntax highlighting.
  - Headless integration-test render loop (warp #12703) — functional no-op
    without the removed `agent_mode_evals` feature.
  - emitter_handle subscription refactor (warp #12914/#12767/#12763) — risky
    UI-framework change whose churn is almost entirely AI call sites.
  - pane_leaves sqlite delete (warp #12672) — already present in Rift.
  - openssl 0.10.80 bump (warp #11479) — Rift doesn't depend on openssl.

### Notes from the 2026-06-14 review

- **Ported:** DCS hook-integrity checks (warp #25395). The session-viewer
  validation exception (warp #25607) was skipped — Rift has no shared-session
  subsystem, so it always validates.
- **Deliberately NOT ported — tab grouping / pinning refinements.** Rift carries
  vertical tab grouping up to warp #12000 but with `grouped_tabs` off in the
  default Cargo features. That matches Warp's own stable release, where
  `GroupedTabs` is **PREVIEW-only** (not in `RELEASE_FLAGS`) and `PinnedTabs`
  (#12453/#12534) is still on an unmerged Warp dev branch. The rule: if a feature
  isn't in Warp's stable release, don't add it to Rift yet. The horizontal
  tab-group view (warp #12089's `view/tab_grouping.rs`) never existed in Rift, so
  the horizontal-tab commits (#12089/#12110/#12432) are N/A. Revisit only if/when
  these graduate to Warp's `RELEASE_FLAGS`.
  **UPDATE (2026-07-19): this revisit condition has fired** — warp #13767
  promoted pinned tabs to stable (upstream `PREVIEW_FLAGS` is now empty and
  `pinned_tabs` is in `app/Cargo.toml`'s `default`). It was still skipped, but
  now on cost grounds rather than the stable-release rule; see the 2026-07-19
  note for the sizing. Don't re-litigate the flag status — re-decide on scope.

Only port changes to subsystems Rift keeps (terminal core, blocks, wgpu rendering, command
input/editor, themes, tabs/vertical tabs, command search, completions, history autosuggestion,
vim input, syntax highlighting, shell integration/bootstrap, SSH/riftify, secret redaction,
macOS platform, perf, security/crash fixes). Skip anything touching the stripped subsystems
(AI/agents/MCP, cloud/Drive/teams/sharing, auth, telemetry, billing, auto-update, the code
editor/LSP/file-tree, workflows, notebooks, voice, onboarding). After porting, bump the date
above.

**Verify every port** before committing — porting renamed/diverged code is exactly where things
silently break, so don't stop at "it compiles". Run the full bar (0 errors AND 0 warnings each):

```bash
cargo check --bin rift-oss
cargo check --tests -p rift          # tests must still compile
cargo test -p <touched_crate>        # run tests for crates you changed; port the upstream tests too
```

If a port touched a crate with tests (e.g. `rift_terminal`), run that crate's tests and make sure
the upstream regression test you ported actually passes. Don't commit a port on a red or
warning-emitting build.

## Development Commands

### Build / run / iterate
- `cargo run --bin rift-oss` — build and run Rift (the local, cloud-free binary; `app/src/bin/oss.rs`).
- Fast iterate loop: `cargo check --bin rift-oss > /tmp/rb.log 2>&1; grep -c '^error' /tmp/rb.log`
  - `cargo check` skips codegen/linking (same errors, much faster). Use `cargo build` only for the final green check.
- Build prereq: **`protoc`** (`brew install protobuf`) — required for the `crates/remote_server` protos. `./script/install_cargo_build_deps` installs the rest.
- Don't `cargo clean` casually — it forces a ~40-min full dependency rebuild. App-crate incremental rebuilds are a few minutes.

### Testing
- `cargo nextest run --no-fail-fast --workspace` — run tests (parallel).
- `cargo test --doc` — doc tests.
- Unit tests live in a sibling `${filename}_tests.rs` (or `mod_test.rs`), included at the end of the module:
  ```rust
  #[cfg(test)]
  #[path = "filename_tests.rs"]
  mod tests;
  ```

### UI integration tests are NOT in CI — run them before calling work "done"
The `integration` crate's `ui_tests::*` (plus `test_up_arrow_history`, and the
cloud `*ssh*` tests) drive the **rendered UI** — windows, menus, snapshots — which
needs a real display the hosted CI runner lacks, so CI skips them (they fail there
for environmental, not code, reasons). They run fine locally. **Definition of
done:** before considering any UI / terminal / blocks / input / menu work
complete, run the relevant UI integration tests locally and confirm they pass:
```bash
cargo nextest run -p integration -E 'test(/ui_tests::/)'   # whole module, or a specific test(name)
```
CI covers fmt + clippy + all unit/doc tests + the non-UI integration tests; the
rendered-UI tests are on you to run locally.

### Linting / formatting (the "done" bar = 0 errors AND 0 warnings)
- `./script/format` — format the code.
- `cargo clippy --workspace --all-targets --tests -- -D warnings` — `-D warnings` is exactly the "no warnings" gate.
- `./script/presubmit` — runs fmt + clippy + tests together.

## Architecture Overview

A Rust terminal emulator with a custom UI framework, **RiftUI**.

**RiftUI** (`crates/riftui`, `crates/riftui_core`):
- Entity-Component-Handle pattern. A global `App` object owns all views/models (entities).
- Views hold `ViewHandle<T>` references to other views; `AppContext` gives temporary handle access during render/events.
- Elements describe visual layout (Flutter-inspired). An Actions system handles events.
- `MouseStateHandle` must be created **once** during construction and then referenced/cloned wherever mouse input is used. An inline `MouseStateHandle::default()` during render breaks all mouse interaction.

**Main app** (`app/`):
- Terminal emulation and shell management (`terminal/`)
- Tabs / windows / pane layout (`workspace/`, `pane_group/`)
- Settings and preferences (`settings/`)
- (AI, Drive, auth, cloud sync, GraphQL are being **removed** — do not build on them.)

**Core crates:**
- `crates/rift_core` — core utilities + platform abstractions. **Also home of the telemetry/logging macros** (see guardrail below).
- `crates/editor` (package `rift_editor`) — the command-input editor.
- `crates/riftui`, `crates/riftui_core` — the UI framework.
- `crates/persistence` — local SQLite (Diesel).
- `crates/rift_features` — the `FeatureFlag` enum + default-on flag lists.
- `crates/ipc` — inter-process communication.

This is a Cargo workspace; platform-specific code is conditionally compiled.

## Coding Style

- Avoid unnecessary type annotations, especially in closure params.
- Prefer imports at the top of the file over long path qualifiers. Exception: inside `cfg`-guarded branches, a scoped import or a one-off absolute path is fine.
- A context param (`AppContext`, `ViewContext`, or `ModelContext`) is named `ctx` and goes **last** — unless the function takes a closure, in which case the closure is last.
- **Remove unused parameters completely** — never prefix with `_`. Update the signature and every call site. (This is most of the strip's work.)
- Inline format args in macros: `eprintln!("{message}")`, not `eprintln!("{}", message)` (Clippy `uninlined_format_args`).
- Don't pass `Itertools::format` results to logging macros (`log::*`, `safe_*`) — it's a single-use formatter and loggers may format twice. Use a reusable `String` (`iter.join(", ")`) for logs. Direct use in `format!`/`write!` is fine.
- Don't remove existing comments for unrelated changes — only when the logic they describe has actually changed.

## Exhaustive Matching (critical during the strip)

Avoid the wildcard `_` in `match` whenever possible. Exhaustive matching is what makes the
compiler flag every site that still references a variant — it is the **safety net for the
AI/cloud excision**. Silencing a `match` with `_ =>` *hides* leftover agent/cloud variants
instead of forcing their deletion. Match every variant explicitly.

## Strip guardrails (while removing AI/cloud)

- **Delete use-sites; don't stub.** The decision is full excision — remove the fields, enum variants, methods, and match arms that carry deleted types. Do not paper over them with stubs.
- **Remove ALL telemetry / cloud / AI code — including no-op call sites.** The goal is full removal, not preservation. When you encounter a `send_telemetry_*!` call site (or any cloud / Drive / account / auth / sharing / AI-agent remnant), **delete it** along with whatever dead code it leaves behind — the empty `if`, the now-unused local, the `if`/`else` whose branches became identical, the stale explanatory comment. Don't leave parked dead code (`if false && …`) either. The no-op telemetry macro shims in `crates/rift_core/telemetry.rs` (`send_telemetry_from_ctx!` / `send_telemetry_from_app_ctx!` expand to `{}`) exist **only** so not-yet-removed call sites still compile during the strip — they are scaffolding to delete against, not a pattern to keep. Once a subsystem's call sites are all gone, remove the shim too.
- **Keep the LOCAL logging/error macros.** `report_error`/`report_if_error`/`safe_warn`/`safe_info` (`crates/rift_core` `errors.rs`, `safe_log.rs`) are real *local* logging — **KEEP** these macros *and* their call sites. This is the one exception to the rule above; don't confuse local logging with telemetry.
- **Never remove `use rift_core::...` macro imports while call sites remain.** Deleting a macro import while call sites still reference it causes a "cannot find macro" cascade. Order matters: delete the call sites first, then the now-unused import/shim. Only remove *type* imports for deleted modules.
- An unresolved import (`E0432`) makes rustc **suppress** all of that symbol's use-site errors. Removing the dead import un-masks the real work, so the error count jumping up after an import sweep is expected — not new breakage.

## Terminal Model Locking

- Be extremely careful calling `model.lock()` on `TerminalModel`. Acquiring multiple locks on the same model from different call sites can deadlock → UI freeze (macOS beachball).
- Before adding a new `model.lock()`, verify no caller already up the stack holds the lock.
- Prefer passing an already-locked reference down the stack. If you must lock, keep the scope as short as possible and don't call other functions that might also lock.

## Feature Flags

Compile-time flags with a small runtime plumbing layer.
- Add a variant to the `FeatureFlag` enum in `crates/rift_features/src/lib.rs`; gate code with `FeatureFlag::YourFlag.is_enabled()`. Default-on lists (`DOGFOOD_FLAGS`/`PREVIEW_FLAGS`/`RELEASE_FLAGS`) are in the same file.
- **Prefer runtime `is_enabled()` checks over `#[cfg(...)]`** so flags toggle without recompiling and are easy to remove later. Use `#[cfg(...)]` only when the code can't compile otherwise (platform-specific or optional-dep code).
- As the server/experiment system is removed, flags must resolve from **local defaults** — there is no remote config.

## Database

- Diesel ORM over local SQLite. Migrations in `crates/persistence/migrations/`; schema in `crates/persistence/src/schema.rs`.
