# Rift — Plan 2: Full Cloud Strip

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove all cloud/account/team/Drive/billing/telemetry/auto-update infrastructure from Rift, leaving a local-only terminal with local AI (rift_ai/omlx), while keeping the terminal core, blocks UI, editor/input, settings, themes, and local persistence — and localizing the last cloud-AI call (`predict_am_queries`) to omlx.

**Architecture:** A phased, **compiler-driven** strip. Cloud code is class-C coupled (server/ touched by 153 files, telemetry macros in 193 files, `AuthState` woven into the terminal core), so deletions proceed: disable flags → delete leaf modules → delete self-contained subsystems → replace coupled singletons with permissive local **stubs/shims** → delete the big coupled trees (server/auth) behind telemetry-no-op + AuthState-shim → detangle the graphql crate. Every phase ends with `cargo build --bin rift-oss` GREEN and a commit, so the terminal is always runnable.

**Tech Stack:** Rust 1.92 workspace, `rift_ai` crate (local AI, already built in Plan 1). Tooling: `rg`, `fd`, `sd`, `cargo`. Use `uv run python` (not `python3`) for any JSON/text munging; a `.venv` exists.

**Grounding:** This plan is built on the committed coupling map at `docs/superpowers/notes/2026-06-06-plan2-strip-investigation.md`. Read it before starting — it has the exhaustive per-module inventory, ref counts, and the boot-path init sequence with line numbers. This plan does NOT duplicate that inventory; it gives the method, the shims, and the gates.

---

## ⛳ EXECUTION STATUS & RESUME NOTES (updated 2026-06-06, mid-execution — READ FIRST)

**Branch:** `plan2-strip`. **Resume from clean HEAD `f2034746`** (`cargo build --bin rift-oss` is green here).

**Done & committed:**
- ✅ **Phase 0 / Task 1** — disable cloud feature flags (commit `3821be1a`). Used a new `ChannelState::with_disabled_features()` in `crates/rift_core/src/channel/state.rs`; OSS channel disables 12 cloud flags in `app/src/bin/oss.rs`.
- ✅ **Phase 1 / Task 2** — deleted usage/billing/reward/referral leaf modules + GlobalResourceHandles surgery (commit `f2034746`).

**Remaining:** Tasks 3–11 (Phases 2–7 + acceptance).

**🚨 MACRO GUARDRAIL — this killed 3 subagent attempts at Phase 2. Non-negotiable:**
Telemetry/logging macros live in **`crates/rift_core`** (KEEP code): `send_telemetry_from_ctx`/`send_telemetry_sync_from_app_ctx`/`send_telemetry_from_app_ctx` (`telemetry.rs`), `report_error`/`report_if_error` (`errors.rs`), `safe_warn`/`safe_info` (`safe_log.rs`). They reach app files via `use rift_core::{...}` imports.
- When fixing dangling refs, remove ONLY drive/cloud_object/server **type** imports. **NEVER** remove a `use rift_core::…` macro import, and never reorder `lib.rs` `mod` declarations.
- If the build ever says `cannot find macro send_telemetry_* / report_* / safe_*`, you deleted a macro import — `git diff` and revert that specific edit immediately. Do NOT "fix" it by deleting the macro call sites.

**⚠️ PHASE 2 ENTANGLEMENT (Task 3) — bigger than the spike implied:** Drive is NOT separable. Deleting `app/src/drive/` ALONE yields **165 errors** cascading into `workflows/`, `notebooks/`, `env_vars/`, `ai/`, `cloud_object/`. Drive + cloud_object + workflows + notebooks + env_vars + the `ai/` cloud-object usages are ONE feature web that must be removed together; there is **no smaller green checkpoint** (the build is all-or-nothing across the cluster). Budget for a **300+-error compiler-driven grind** in one sitting.

**EXECUTION MODE CHANGE:** subagent-driven execution FAILED for the tangled phases (they scope-crept into the rift_core macros and stalled). **Do the tangled phases (2, 5, 8) INLINE with full context**, compiler-driven, in a session with plenty of headroom — not via subagents. Phases 4, 6, 7, 9 are smaller and may still be delegated.

---

## Method: compiler-driven deletion (read once)

For class-C deletions, exact call-site fixes cannot be pre-listed (hundreds of files). The reliable, honest method per deletion is:

1. Delete the target module/dir and remove its `mod`/`pub mod` declaration from the parent (`lib.rs` or `mod.rs`).
2. `cargo build --bin rift-oss 2>&1 | rg '^error' | head -50`.
3. For each error: it names an unresolved import/path or a missing symbol in a KEEP file. Resolve by **removing the now-dead usage** (the import line, the call, the struct field, the event subscription) — NOT by re-adding the deleted code. Where a KEEP path genuinely needed a value the cloud code provided, use the **stub/shim** this plan defines for that subsystem.
4. Repeat 2–3 until the build is green.
5. `cargo build --bin rift-oss` clean (0 warnings — fix dead-code with removal, not `#[allow]`, except where this plan says otherwise), then commit.

**Hard rules for the whole plan:**
- NEVER `cargo clean` (forces a 40-min rebuild). Use long Bash timeouts (≥600000 ms) for builds.
- After EACH phase: `cargo build --bin rift-oss` is GREEN before committing. Never commit a red build.
- Do NOT touch `app/src/workspace/` (singular) — that's the window/tab container we KEEP. Only `app/src/workspaces/` (plural) is cloud teams.
- Do NOT touch `crates/rift_ai`, `app/src/ai/predict/rift_bridge.rs`, or `rift_nl_prefix.rs` (Plan 1 local AI) except where Task 6 localizes `predict_am_queries`.
- GUI smoke verification is the user's; agents stop at a green build per phase.

---

## Task 1 — Phase 0: Disable cloud feature flags (no deletion)

**Files:** `app/src/bin/oss.rs` (or the OSS channel feature-override path it uses).

**Goal:** Suppress all cloud UI/behavior via flag overrides before deleting code — a safe, reversible baseline that should already build/boot.

- [ ] **Step 1: Find the flag-override mechanism.** Run `rg -n 'with_additional_features|DEBUG_FLAGS|FeatureFlag::.*=>|feature_overrides|with_overrides' app/src crates/rift_core/src | head -30` and read how `oss.rs` sets `ChannelState` (it already calls `with_additional_features(DEBUG_FLAGS)` in debug). Identify the API to force flags off for the Oss channel.

- [ ] **Step 2: Force these flags false for the Oss channel.** Using that mechanism, set off: `CloudMode`, `CloudModeFromLocalSession`, `CloudConversations`, `CloudEnvironments`, `CloudObjects`, `DriveObjectsAsContext`, `HandoffLocalCloud`, `HandoffCloudCloud`, `BillingAndUsagePageV2`, `TeamApiKeys`, `MultiWorkspace`, `SharedWithMe`. (Exact flag list: re-verify against `rg 'FeatureFlag::' app/src | rg -i 'cloud|drive|team|workspace|billing|handoff|shared'` — use the real variants present.)

- [ ] **Step 3: Build + commit.**
  `cargo build --bin rift-oss` → GREEN.
  ```bash
  git add -A && git commit -m "chore(strip): disable cloud feature flags in OSS channel

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
  ```

---

## Task 2 — Phase 1: Delete leaf modules (A-class)

**Targets (from spike §1, §7):** `app/src/usage/`, `app/src/billing/`, `app/src/reward_view.rs`, `app/src/referral_theme_status.rs`, `app/src/server/server_api/referral.rs`, and `settings_view/referrals_page.rs` if present.

**The one piece of surgery:** `referral_theme_status` is embedded in `GlobalResourceHandles` (spike Risk 3).

- [ ] **Step 1: Delete the trivial leaves.** Remove `app/src/usage/`, `app/src/billing/`, `app/src/reward_view.rs`. Remove their `mod` declarations from `app/src/lib.rs`. Remove their `init(ctx)` calls in `lib.rs` (spike §3: `reward_view::init` ~1634, `billing::...::init` ~1638) and the `billing` import/modal in `app/src/workspace/view.rs`.

- [ ] **Step 2: Remove referral from GlobalResourceHandles.** Read `app/src/global_resource_handles.rs`. Remove the `referral_theme_status: ModelHandle<ReferralThemeStatus>` field, its constructor argument, and all initializers. Delete `app/src/referral_theme_status.rs`, its `mod` decl, the `ReferralThemeStatus::new` call in `lib.rs` (~1193), the `workspace/view.rs` subscriber, and `settings_view/referrals_page.rs` (+ its route/mod) if present. Update any `global_resource_handles` tests the compiler flags.

- [ ] **Step 3: Compiler-driven cleanup** per the Method until green.

- [ ] **Step 4: Build clean + commit.**
  ```bash
  git add -A && git commit -m "feat(strip): remove usage/billing/reward/referral leaf modules

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
  ```

---

## Task 3 — Phase 2: Strip Warp Drive + cloud objects

**Targets (spike §1, §2, §7):** `app/src/drive/` (22.6k), `app/src/cloud_object/`, `app/src/server/cloud_objects/`, `app/src/settings/cloud_preferences_syncer.rs`, `app/src/settings/cloud_preferences.rs`; lib.rs cloud-object sync init (`SyncQueue` ~1737, `CloudModel` ~1729, `Listener` ~1892, `UpdateManager` ~1817); Drive UI in `terminal/view.rs`, `pane_group/mod.rs`, `pane_group/pane/workflow_pane.rs`, `workspace/view.rs`; crates `cloud_object_client`, `cloud_object_models`, `cloud_object_persistence`, `cloud_objects` from `app/Cargo.toml`.

**Coupling note (spike §6):** `crates/mcp` imports `StaticEnvVar`/`TransportType` from `cloud_object_models`. Before removing that crate, **move those two types** into `crates/mcp` (or `rift_core`) and repoint `mcp`'s imports. Do this as the first step of this phase.

- [ ] **Step 1: Extract MCP transport types.** In `crates/cloud_object_models`, locate `StaticEnvVar` and `TransportType`. Move their definitions into a new `crates/mcp/src/transport_types.rs` (or an existing suitable module), `pub use` them, and repoint `crates/mcp` imports from `cloud_object_models::...` to the local path. Build `cargo build -p rift_mcp` (verify package name via `rg '^name' crates/mcp/Cargo.toml`) GREEN.

- [ ] **Step 2: Delete Drive + cloud-object app modules** and their `mod` decls (`drive/`, `cloud_object/`, `server/cloud_objects/`, the two `settings/cloud_preferences*` files).

- [ ] **Step 3: Remove cloud-object init from lib.rs** (`SyncQueue`, `CloudModel`, `Listener`, `UpdateManager`, `initialize_cloud_preferences_syncer`, `drive::index::init`).

- [ ] **Step 4: Remove Drive UI couplings** in `terminal/view.rs` (WarpDriveSettings/ShareableObject/CloudObjectTypeAndId, spike lines 333-335), `pane_group/mod.rs` (WarpDriveItemId/OpenWarpDriveObjectArgs), `pane_group/pane/workflow_pane.rs`, `workspace/view.rs` — remove the Drive panel/sharing actions and their dispatch arms.

- [ ] **Step 5: Drop cloud-object crates** from `app/Cargo.toml` (`cloud_object_client/models/persistence/objects`). Compiler-driven cleanup until green.

- [ ] **Step 6: Build clean + commit.**
  ```bash
  git add -A && git commit -m "feat(strip): remove Warp Drive and cloud-object sync

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
  ```

---

## Task 4 — Phase 3: Strip cloud workspaces/teams + pricing (with stubs)

**Targets:** `app/src/workspaces/` (plural, cloud teams), `app/src/pricing/`; lib.rs `TeamUpdateManager` (~1809), `TeamTesterStatus` (~1807), `UserWorkspaces::new` (~1317).

**Why stubs:** `UserWorkspaces` (160 KEEP refs) and `PricingInfoModel` (17 refs) are queried by terminal/pane_group/settings for permissive policy checks. Rather than delete 160 call sites, replace each singleton with a **local permissive stub** so call sites keep compiling.

- [ ] **Step 1: Stub `UserWorkspaces`.** Replace the cloud `UserWorkspaces` model with a minimal local singleton exposing the SAME methods KEEP code calls, returning permissive constants. First enumerate the methods actually used: `rg -n 'UserWorkspaces::as_ref|UserWorkspaces::' app/src | rg -o '\.[a-z_]+\(' | sort -u`. Implement a stub at `app/src/local_workspace_stub.rs` (or keep the `UserWorkspaces` name to minimize churn) where e.g. `is_ai_allowed_in_remote_sessions(&self,_) -> bool { true }`, `is_byo_api_key_enabled(&self,_) -> bool { true }`, etc., for every method the grep found. Register it as the singleton in `lib.rs` in place of the cloud one. Delete `app/src/workspaces/`.

- [ ] **Step 2: Stub `PricingInfoModel`.** Replace `app/src/pricing/` with a stub `PricingInfoModel` (keep the type name + the ~methods the 17 consumers call, returning "no limits / allowed"). Enumerate via `rg -n 'PricingInfoModel|pricing::' app/src`.

- [ ] **Step 3: Remove team init** from lib.rs (`TeamUpdateManager`, `TeamTesterStatus`, cloud `UserWorkspaces::new`). Compiler-driven cleanup until green.

- [ ] **Step 4: Build clean + commit.**
  ```bash
  git add -A && git commit -m "feat(strip): remove cloud workspaces/teams + pricing (local stubs)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
  ```

---

## Task 5 — Phase 4: Strip autoupdate + changelog + cloud voice

**Targets:** `app/src/autoupdate/`, `app/src/changelog_model.rs`, `ServerVoiceTranscriber`; lib.rs inits (`AutoupdateState::register` ~1929, `check_and_report_update_errors` ~1352, `ChangelogModel::new` ~1666, `ServerVoiceTranscriber::new` ~1688).

- [ ] **Step 1: Delete autoupdate + changelog** modules + `mod` decls + lib.rs inits + any `root_view`/`debug_dump`/`settings_view` references (spike §2 lists 9 ref files).

- [ ] **Step 2: No-op the voice transcriber.** Voice input is KEEP-ish UI but its transcriber is cloud. Find the `VoiceTranscriber` trait/usage (`rg -n 'VoiceTranscriber|voice' app/src/lib.rs app/src/voice* crates/voice_input`). Replace `ServerVoiceTranscriber` with a no-op impl that yields no transcription (so the voice button is inert, not crashing). If voice is entirely cloud-dependent and behind a flag, removing the feature wholesale is acceptable — prefer the no-op if it's less surgery.

- [ ] **Step 3:** Compiler-driven cleanup until green.

- [ ] **Step 4: Build clean + commit.**
  ```bash
  git add -A && git commit -m "feat(strip): remove autoupdate/changelog, no-op cloud voice

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
  ```

---

## Task 6 — Phase 5a: Localize `predict_am_queries` to rift_ai

**This is feature work, done BEFORE deleting server/ so the AM-query suggestion keeps working locally.**

**Files:** `crates/rift_ai/src/queries.rs` (new), `crates/rift_ai/src/lib.rs`, `app/src/ai/predict/rift_bridge.rs` (extend), `app/src/terminal/input.rs` (rewire call site ~13319).

- [ ] **Step 1: Recon the existing call.** Read `terminal/input.rs` around the `server_api.predict_am_queries(...)` call (~13319): capture the argument it builds and the return type/shape (likely `Vec<String>` of suggested agent-mode queries, or a typed response). Read `server_api`'s `predict_am_queries` signature.

- [ ] **Step 2: Add `rift_ai::queries::predict_queries` (TDD).** New file `crates/rift_ai/src/queries.rs`, mirroring `complete.rs`:

```rust
use crate::client::{send_messages, AiError};
use crate::config::RiftAiConfig;
use crate::context::RiftContext;
use crate::messages::MessagesRequest;

const QUERIES_SYSTEM: &str = "You suggest up to 5 concise natural-language tasks the user might \
ask an AI agent next, based on recent shell history. One per line, most likely first. Output ONLY \
the suggestions, no prose, no numbering, no backticks.";

const QUERIES_MAX_TOKENS: u32 = 256;

/// Suggest natural-language agent-mode queries from recent context.
pub async fn predict_queries(ctx: &RiftContext, cfg: &RiftAiConfig) -> Result<Vec<String>, AiError> {
    let user = format!("history:\n{}\nshell: {}", ctx.history_as_jsonl(), ctx.shell.as_deref().unwrap_or("unknown"));
    let req = MessagesRequest::single_user(&cfg.model, QUERIES_SYSTEM, &user, QUERIES_MAX_TOKENS);
    let resp = send_messages(cfg, &req).await?;
    Ok(parse_lines(&resp.text()))
}

fn parse_lines(text: &str) -> Vec<String> {
    text.lines()
        .map(|l| l.trim().trim_matches('`').trim())
        .map(|l| l.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.' || c == ')').trim())
        .filter(|l| !l.is_empty())
        .take(5)
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn parse_lines_strips_noise() {
        assert_eq!(parse_lines("1. deploy the app\n`check logs`\n\n"), vec!["deploy the app", "check logs"]);
    }

    #[tokio::test]
    async fn predict_queries_parses_response() {
        let server = MockServer::start().await;
        Mock::given(method("POST")).and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "content": [ { "type": "text", "text": "deploy the app\ncheck logs" } ]
            }))).mount(&server).await;
        let cfg = RiftAiConfig::from_toml_str(&format!("[ai]\nendpoint = \"{}\"\nmodel = \"m\"\n", server.uri())).unwrap();
        let out = predict_queries(&RiftContext::default(), &cfg).await.unwrap();
        assert_eq!(out, vec!["deploy the app", "check logs"]);
    }
}
```
Add `pub mod queries;` to `crates/rift_ai/src/lib.rs`. Run `cargo test -p rift_ai queries::` → 2 passed.

- [ ] **Step 3: Bridge function.** In `app/src/ai/predict/rift_bridge.rs` add:
```rust
/// Local replacement for the cloud AM-query prediction. Empty on error/missing config.
pub async fn local_am_queries(ctx: &NextCommandContext, current_input: &str) -> Vec<String> {
    let Ok(cfg) = RiftAiConfig::load_from(&RiftAiConfig::default_path()) else { return Vec::new(); };
    let rctx = to_rift_context(ctx, current_input);
    rift_ai::queries::predict_queries(&rctx, &cfg).await.unwrap_or_default()
}
```
(If the call site lacks a `NextCommandContext`, add an overload taking the raw history slice + input, reusing `map_history` — mirror what the real call site has in scope, verified in Step 1.)

- [ ] **Step 4: Rewire `terminal/input.rs`** — replace the `server_api.predict_am_queries(...)` call with `crate::ai::predict::rift_bridge::local_am_queries(...)`, adapting the result to whatever the downstream handler expects (map to the same shape; if it expected a typed response, construct it from the `Vec<String>`).

- [ ] **Step 5: Build clean + commit.**
  ```bash
  git add -A && git commit -m "feat: localize predict_am_queries to rift_ai/omlx

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
  ```

---

## Task 7 — Phase 5b: Telemetry no-op + AuthState shim

**The two coupling risks that make server/auth deletable. Do these BEFORE Task 8.**

**Telemetry (spike Risk 1):** macros `send_telemetry_from_ctx!`, `send_telemetry_from_app_ctx!`, `send_telemetry_on_executor!` are in 193 files. Replace the macro DEFINITIONS with no-ops so all call sites compile away — far safer than editing 260+ call sites.

- [ ] **Step 1: Find the macro definitions.** `rg -n 'macro_rules! send_telemetry_from_ctx|macro_rules! send_telemetry_from_app_ctx|macro_rules! send_telemetry_on_executor' app/src crates`.

- [ ] **Step 2: Replace each macro body with a no-op** that still "uses" its args to avoid unused warnings, e.g.:
```rust
#[macro_export]
macro_rules! send_telemetry_from_ctx {
    ($ctx:expr, $event:expr $(, $rest:tt)*) => {{ let _ = (&$ctx, &$event); }};
}
```
Mirror the arms/arities of the originals (check each macro's match arms first). Keep the macro names/paths identical so call sites are untouched. Build GREEN (call sites now expand to nothing).

**AuthState (spike Risk 2):** `terminal/view.rs` holds `Arc<AuthState>` and uses it to gate AI display + telemetry identity.

- [ ] **Step 3: Add a local AuthState shim in `rift_core`.** Create a minimal `AuthState` (or `LocalIdentity`) in `crates/rift_core` exposing exactly the methods KEEP code calls (enumerate: `rg -n 'auth_state\.\|AuthState::' app/src | rg -o '\.[a-z_]+\(' | sort -u`), e.g. `is_anonymous_or_logged_out(&self) -> bool { false }`, `anonymous_id(&self) -> &str { &self.id }` backed by a persisted UUID. No firebase/server deps.

- [ ] **Step 4: Repoint KEEP code** (`terminal/view.rs`, `terminal/local_tty/terminal_manager.rs`, `recorder.rs`, persistence) to the shim. The AI-display gate `is_anonymous_or_logged_out()` now returns false → suggestions show. Build GREEN.

- [ ] **Step 5: Commit.**
  ```bash
  git add -A && git commit -m "refactor(strip): no-op telemetry macros + local AuthState shim

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
  ```

---

## Task 8 — Phase 5c: Delete server/ + auth/ + their crates

**Now unblocked by Task 6 (predict localized) + Task 7 (telemetry/auth shimmed).**

**Targets:** `app/src/server/` (39.5k), `app/src/auth/` (6.5k); lib.rs inits `AuthState::initialize` (~1128 → replace with shim init), `ServerApiProvider::new` (~1148), `AuthManager::new` (~1162), `AppTelemetryContextProvider`/`TelemetryCollector` (~1159/1602), `ManagedSecretManager::new` (~1380), `NetworkLogModel` (~1137), `ServerExperiments` (~1312), `AIRequestUsageModel` (~1314); crates `rift_server_auth`, `rift_server_client`, `firebase`, `crates/managed_secrets` from `app/Cargo.toml`.

- [ ] **Step 1: Replace boot init.** In `lib.rs run_internal()`, replace `AuthState::initialize(...)` with the Task-7 shim constructor; delete `ServerApiProvider::new`, `AuthManager::new`, telemetry providers/collector, `ManagedSecretManager`, `NetworkLogModel`, `ServerExperiments`, `AIRequestUsageModel`, and any remaining `server_api.clone()` passes. Remove the now-dead `#[allow(dead_code)] server_api` field in `next_command_model.rs` and its constructor param + call sites (the field deferred from Plan 1).

- [ ] **Step 2: Delete `app/src/server/` and `app/src/auth/`** + `mod` decls + `auth::init(ctx)` (~1633). Compiler-driven cleanup (this is the largest error wave; work through it methodically, removing dead usages and routing any survivors to shims).

- [ ] **Step 3: Drop crates** `rift_server_auth`, `rift_server_client`, `firebase`, `managed_secrets` from `app/Cargo.toml`. (Leave `graphql` for Task 10.) Build GREEN.

- [ ] **Step 4: Commit.**
  ```bash
  git add -A && git commit -m "feat(strip): delete warp-server client and auth subsystem

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
  ```

---

## Task 9 — Phase 6: Strip cloud-agent AI bits

**Targets:** `app/src/ai/cloud_agent_config/`, `app/src/ai/cloud_agent_settings.rs`, `app/src/ai/cloud_environments/`, `app/src/ai/connected_self_hosted_workers.rs`; `settings/init.rs` (`CloudAgentSettings`), lib.rs `ConnectedSelfHostedWorkersModel::new` (~1935), cloud-agent modals in `workspace/view.rs`.

- [ ] **Step 1:** Delete the four AI cloud modules + `mod` decls + the settings/init + lib.rs + workspace/view references. Compiler-driven cleanup until green.

- [ ] **Step 2: Commit.**
  ```bash
  git add -A && git commit -m "feat(strip): remove cloud-agent AI modules

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
  ```

---

## Task 10 — Phase 7: Detangle + delete the graphql crates

**Targets:** `crates/graphql` (9.1k), `crates/rift_graphql_schema`; dependents `crates/ai` (reranking) and `crates/managed_secrets` (already removed in Task 8) and `crates/websocket`.

- [ ] **Step 1: Stub AI reranking.** In `crates/ai`, find the `rift_graphql::queries::rerank_fragments::FragmentLocationInput` usage (codebase-index reranking). Replace the cloud rerank call with a local no-op that returns the input fragments unranked (or remove the rerank step), and drop the `graphql` import. Build `cargo build -p <ai pkg>` GREEN (verify pkg name).

- [ ] **Step 2: Handle `crates/websocket`** graphql dep — if it only used graphql types for the cloud subscription, remove that path; else move the minimal types local.

- [ ] **Step 3: Delete `crates/graphql` and `crates/rift_graphql_schema`** and remove from workspace `Cargo.toml` `[workspace.dependencies]` and any remaining `app/Cargo.toml` ref. Compiler-driven cleanup until green.

- [ ] **Step 4: Commit.**
  ```bash
  git add -A && git commit -m "feat(strip): detangle and remove graphql crates

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
  ```

---

## Task 11 — Plan-2 acceptance checkpoint

- [ ] **Step 1: Clean build.** `cargo build --bin rift-oss` → Finished, 0 warnings.

- [ ] **Step 2: Workspace tests still green.** `cargo test -p rift_ai` (16 tests now, incl. queries) and the pure-logic crates from Plan 1 (`rift_completer`, `rift_util`, `sum_tree`, `markdown_parser`, `command`, `rift_editor` — run `rift_editor` with `--test-threads=1` due to the known env_logger flake). Plus `cargo test -p rift rift_bridge:: rift_nl_prefix::`.

- [ ] **Step 3: Confirm cloud is gone.** All return nothing meaningful:
  `rg -l 'crate::server::|crate::auth::|warp_server|cloud_object|ServerApi' app/src | rg -v 'rift_bridge|next_command_model'` ; `fd -t d 'server|auth|drive|workspaces|pricing|billing' app/src` ; `rg 'graphql|firebase' Cargo.toml`.

- [ ] **Step 4: Binary size delta.** Record `ls -lh target/debug/rift-oss` vs the Plan-1 size (~721 MB) as the shrink metric.

- [ ] **Step 5: Tag.** `git tag rift-plan2-complete`.

- [ ] **Step 6: HANDOFF — user GUI smoke test (required, not agent-runnable):**
  - `cargo run --bin rift-oss` launches, no login wall.
  - Terminal runs commands; blocks render; settings/themes open.
  - Local AI completion + `# ` prefix still work (omlx running).
  - No crashes from removed cloud paths (open settings, panes, command palette).

---

## Parallelization & sequencing

Phases are mostly **sequential** (each green build gates the next), and they edit overlapping core files (`lib.rs`, `terminal/view.rs`), so parallel implementer agents would conflict — run one at a time. The exception: Task 6's `rift_ai::queries` crate work (Step 2) is isolated and could be built by a separate agent while another finishes Task 5. Tasks 7 (telemetry/auth shims) MUST precede Task 8 (server/auth deletion). Recommended order: 1 → 2 → 3 → 4 → 5 → 6 → 7 → 8 → 9 → 10 → 11.

## Risks called out

- **Task 8 is the high-risk phase** (largest error wave). If the compiler-driven loop isn't converging or a survivor genuinely needs server data, stop and shim rather than reintroducing cloud code.
- **Stubs over deletion** for `UserWorkspaces`/`PricingInfoModel`/`AuthState`/`VoiceTranscriber` is deliberate — it keeps 160+ call sites compiling. Don't try to delete those call sites.
- If any phase's build can't be made green within a reasonable loop, that phase's targets are more coupled than the spike showed — report it; we re-scope rather than leave a red build.
