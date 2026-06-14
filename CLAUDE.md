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

**Last reviewed/synced against upstream: 2026-06-14.**

To sync again, start from that date, not earlier:

```bash
git fetch upstream
git log upstream/master --since=2026-06-14 --date=short --pretty='%h %ad %s'
```

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
- **Never remove `use rift_core::...` macro imports.** The telemetry/logging macros (`send_telemetry_*`, `report_error`/`report_if_error`, `safe_warn`/`safe_info`) live in `crates/rift_core` (`telemetry.rs`, `errors.rs`, `safe_log.rs`) and are **KEEP**. Deleting their imports during cleanup causes a "cannot find macro" cascade. Only remove *type* imports for deleted modules.
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
