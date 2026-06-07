# Rift — Plan 2 (REVISED): Terminal-First Full Strip

> **Supersedes** `2026-06-06-rift-plan2-cloud-strip.md` as of 2026-06-06 (mid-execution re-scope).
> The original plan kept Warp's AI agent product and only stripped cloud *infra*. After
> investigation + user decisions this session, the scope is now much larger and **simpler in
> shape (almost pure deletion)**: remove the entire AI agent product too, and drop
> workflows/notebooks/env-vars. Read the "DECISIONS" and "SCOPE" sections first.

**Goal:** A local-only terminal with **NO AI features whatsoever**. Everything
cloud/account/agent/AI is removed. (User may re-add a local AI integration later as a clean
greenfield feature; nothing AI is preserved now.)

**Branch:** `plan2-strip`. Resume baseline was clean `f2034746` (green). Tasks 1–2 of the
original plan (Phase 0 flag-disable `3821be1a`, Phase 1 leaf modules `f2034746`) are DONE.

---

## ⚠️ SCOPE UPDATE (2026-06-07, user-confirmed): REMOVE ALL AI — no keep-path

The earlier plan preserved a single AI feature (context-aware inline command autocomplete via
`rift_ai`→omlx). **That carve-out is CANCELLED.** Delete ALL AI, including the former keep-path:
- DELETE `crates/rift_ai` entirely (complete/context/client/config/messages — all of it).
- DELETE all of `app/src/ai/` with NO exceptions (including the former keep-path
  `predict/{mod,generate_ai_input_suggestions*,next_command_model,rift_bridge}` and `block_context`).
- The editor's AI inline-autosuggestion integration is removed too (`maybe_populate_intelligent_
  autosuggestion`, AI input decorator/indicator, the AI cursor colors). The editor's NON-AI
  autosuggestion (fish-style history + `command_corrections` rule-based corrections) STAYS — those
  are terminal features, not AI.
- `InputType` (Shell vs AI classifier) collapses: input is always Shell now. Delete
  `InputType`/`InputTypeAutoDetectionSource` usage rather than re-homing it.
- The keep-path small fixes (rift_bridge/next_command_model `AIApiError`) are MOOT — those files
  are deleted.
- Phase H acceptance no longer includes any omlx / autocomplete test.
This makes the whole strip uniform — no AI seam to tiptoe around.

---

## DECISIONS (2026-06-06, superseded by the SCOPE UPDATE above for #1/#3)

1. ~~**AI = autocomplete ONLY.**~~ **SUPERSEDED 2026-06-07: remove ALL AI** (see SCOPE UPDATE).
   `crates/rift_ai` + all of `app/src/ai/` (incl. predict + block_context) are deleted.
2. **DELETE the entire AI agent product** ("Blocklist AI" = `ai/blocklist`, plus `ai/agent*`,
   conversations, ambient agents, `ai_assistant/`, artifacts, skills, facts, execution_profiles,
   code-review, command-error explanations, agent-mode workflows, MCP). Full excision, not facade.
3. **DELETE** NL→command (`rift_ai::translate`, `ai/predict/rift_nl_prefix.rs`, the `# ` prefix)
   and `predict_am_queries` / agent-mode query suggestions (don't localize — remove).
4. **DELETE MCP entirely** — `app/src/ai/mcp/`, `settings_view/mcp_servers/`, agent_sdk MCP
   config, AND `crates/mcp` (+ its rmcp deps if unused elsewhere). ⇒ the original "extract MCP
   transport types from cloud_object_models" step is MOOT (both crates get deleted).
5. **DROP workflows** (`app/src/workflows/`) — user doesn't want them. De-wire terminal.
6. **DROP notebooks** (`app/src/notebooks/` — cloud editor AND local file viewer). De-wire.
7. **DROP env-vars** (`app/src/env_vars/`) — it's a cloud-only feature (no local store exists).
   De-wire terminal's shell-injection call sites. (Future option: add a tiny local TOML-backed
   env-var feature as a clean enhancement — NOT in this plan.)
8. ⇒ **No POD-type relocation, no new crate.** With workflows/notebooks/env-vars all dropped,
   `cloud_object_models`/`cloud_objects`/etc. are deleted wholesale; nothing salvaged from them.

---

## SCOPE

### KEEP (must stay compiling/working)
- Terminal core: `app/src/terminal/` (minus agent/workflow/env-var/notebook wiring), blocks UI,
  `app/src/workspace/` (singular = window/tabs), `pane_group/` (minus cloud/agent/feature panes).
- Editor/input: `app/src/editor*`, `crates/rift_editor`, input (minus the AI autosuggestion hook).
- Settings/themes, local persistence (`app/src/persistence/` minus cloud tables), `crates/rift_core`.
- NON-AI autosuggestion: fish-style history completion + `command_corrections` rule-based
  corrections (terminal features — NOT AI; keep).

### DELETE (features → infra order)
- **ALL AI:** the WHOLE of `app/src/ai/` (no exceptions — incl. former keep-path `predict/**` and
  `block_context`) + `crates/rift_ai`; `app/src/ai_assistant/`; `app/src/code_review/`. The editor's
  AI inline-autosuggestion hook (`maybe_populate_intelligent_autosuggestion` + AI decorator/indicator/
  cursor colors) and `InputType`/`InputTypeAutoDetectionSource` usage.
- **Cloud objects / Drive:** `app/src/drive/`, `app/src/cloud_object/`, `app/src/server/cloud_objects/`,
  `app/src/settings/cloud_preferences*.rs`; crates `cloud_object_client/models/persistence/objects`.
- **Dropped features:** `app/src/workflows/`, `app/src/notebooks/`, `app/src/env_vars/`,
  `app/src/search/{workflows,notebooks}/`, MCP (`ai/mcp`, `settings_view/mcp_servers`, `crates/mcp`).
- **Cloud teams/billing:** `app/src/workspaces/` (plural), `app/src/pricing/`, `app/src/billing/` (if any remnants).
- **Updater/voice:** `app/src/autoupdate/`, `app/src/changelog_model.rs`, `ServerVoiceTranscriber`/`ai/voice`.
- **Server/auth/telemetry:** `app/src/server/`, `app/src/auth/`; crates `rift_server_auth`,
  `rift_server_client`, `firebase`, `managed_secrets`.
- **GraphQL:** `crates/graphql`, `crates/rift_graphql_schema`.

---

## METHOD (compiler-driven deletion — unchanged from original)

Per deletion: remove module + its `mod`/`pub mod` decl → `cargo build --bin rift-oss 2>&1 | rg '^error' | head -50`
→ fix each error by **removing the now-dead usage** (import/call/field/event/UI render), NOT by
re-adding deleted code → repeat to green → `cargo build --bin rift-oss` clean (0 warnings, fix
dead code by removal not `#[allow]`) → commit.

### GUARDRAILS (non-negotiable — these caused prior failures)
- **MACRO GUARDRAIL:** telemetry/logging macros live in `crates/rift_core` (KEEP):
  `send_telemetry_*` (telemetry.rs), `report_error`/`report_if_error` (errors.rs),
  `safe_warn`/`safe_info` (safe_log.rs). Reach app via `use rift_core::{...}`. When fixing dangling
  refs, remove ONLY drive/cloud_object/server/agent **type** imports; **NEVER** remove a
  `use rift_core::…` macro import; never reorder `lib.rs` `mod` decls. If build says "cannot find
  macro …", you broke an import — `git diff` and revert that edit; do NOT delete the macro call sites.
- **NEVER `cargo clean`** (40-min rebuild). Use Bash timeouts ≥600000 ms for builds.
- **Tangled phases inline**, compiler-driven, with full context (subagents failed here before).
  Read-only recon/mapping MAY be delegated. Smaller leaf deletions may be delegated.
- Do NOT touch `app/src/workspace/` (singular). The former "NEVER TOUCH rift_ai/predict/block_context"
  guard is CANCELLED (2026-06-07) — those are now DELETED like everything else AI.
- Each phase ends GREEN + commit (RED intermediate commits OK mid-phase as long as the error count
  strictly drops). If a phase won't converge, STOP and report.

### Keep-path small fixes — CANCELLED (2026-06-07)
The `ai/predict/{rift_bridge,next_command_model}` `AIApiError` fixes are MOOT: those files (and all
of `app/src/ai/` + `crates/rift_ai`) are deleted, not preserved.

---

## PHASES (each ends GREEN + commit)

### Phase A — Excise ALL AI (agent product + the former autocomplete keep-path)
Delete the WHOLE of `app/src/ai/` (no keep-path) + `crates/rift_ai`; delete `ai_assistant/`, AI
`code_review/`. De-wire terminal (AI panel, agent blocks, inline actions, conversation history,
suggestion chips, AI input footer, the editor's AI inline-autosuggestion hook + `InputType`),
`pane_group` agent panes, `settings_view` AI pages, `root_view`, command palette agent commands, and
`lib.rs` agent-model + AI-predict boot inits. Drop `rift_ai` from `app/Cargo.toml`. **Biggest phase.**

### Phase B — Drive + cloud objects + dropped features
Delete Drive, `cloud_object/`, `server/cloud_objects/`, `cloud_preferences*`; delete `workflows/`,
`notebooks/`, `env_vars/`, `search/{workflows,notebooks}/`, MCP (`ai/mcp`, `settings_view/mcp_servers`,
`crates/mcp`). Remove cloud-object boot inits from `lib.rs` (SyncQueue/CloudModel/Listener/UpdateManager/
cloud_preferences_syncer). De-wire terminal/pane_group/search/settings of all of the above. Drop
`cloud_object_*` crates + `crates/mcp` from `app/Cargo.toml`.

### Phase C — Cloud workspaces/teams + pricing + billing
Delete `app/src/workspaces/` (plural), `pricing/`, any `billing/` remnants; remove team/pricing boot
inits from `lib.rs`. With the agent product gone, many consumers are already removed — prefer deletion
over stubs; add a permissive stub ONLY where a KEEP call site genuinely remains.

### Phase D — Autoupdate + changelog + voice
Delete `autoupdate/`, `changelog_model.rs`, cloud voice (`ServerVoiceTranscriber`/`ai/voice`); remove
their boot inits + refs.

### Phase E — Telemetry FULL removal (no-op) + AuthState shim
**TELEMETRY IS BEING NUKED, not retained.** No data is collected, none leaves the machine.
Mechanism (chosen over deleting ~67 scattered call sites): replace the telemetry macro DEFINITIONS in
`crates/rift_core/src/telemetry.rs` (`send_telemetry_from_ctx!`, `send_telemetry_from_app_ctx!`,
`send_telemetry_sync_from_app_ctx!`, `send_telemetry_on_executor!`) with NO-OPs (keep names/paths/arities)
so every call site compiles to nothing. This is the ONLY sanctioned edit to the guarded `rift_core` macros.
Then in Phase F delete the telemetry INFRA: `server/telemetry/` (`TelemetryCollector`,
`AppTelemetryContextProvider`), the `lib.rs` boot inits + `TelemetryEvent::*` emissions, and the
`TelemetryEvent`/`SettingsTelemetryEvent` enums. (OSS already sets `telemetry_config: None` +
`crash_reporting_config: None` in `oss.rs`, so it's runtime-off even before this.)
Also add a local `AuthState` shim in `rift_core` exposing only the methods KEEP code still calls (e.g.
`is_anonymous_or_logged_out()->false`, `anonymous_id()->persisted UUID`). Repoint KEEP code. Prep for
Phase F.

### Phase F — Delete server/ + auth/ + their crates
Replace `lib.rs` boot inits (`AuthState::initialize`→shim; delete ServerApiProvider/AuthManager/
telemetry providers/ManagedSecretManager/NetworkLogModel/ServerExperiments/AIRequestUsageModel). Delete
`app/src/server/`, `app/src/auth/`. Drop crates `rift_server_auth`, `rift_server_client`, `firebase`,
`managed_secrets`. (No keep-path error-type fixes needed — `ai/predict` is deleted.)

### Phase G — Detangle + delete graphql crates
Stub/remove `crates/ai` rerank graphql usage if `crates/ai` still exists; handle `crates/websocket`
graphql dep; delete `crates/graphql` + `crates/rift_graphql_schema`; remove from workspace Cargo.toml.

### Phase H — Acceptance
`cargo build --bin rift-oss` 0 warnings; `cargo test` pure-logic crates; sweep `rg`/`fd` to confirm
cloud/agent/AI gone (`rg -i 'rift_ai|BlocklistAI|AgentView' app/src` should be empty); record
binary-size delta vs Plan-1 (~721 MB); `git tag rift-plan2-complete`; hand off GUI smoke test to user
(launch, run cmds, blocks render, splits, SSH/warpify, settings/themes, history autosuggestion +
command corrections work, no crashes from removed paths). No AI/omlx test — AI is gone.

---

## SEQUENCING
Sequential — each green build gates the next; phases edit overlapping core files (`lib.rs`,
`terminal/view.rs`) so no parallel implementers. Read-only mapping/recon may run in parallel.
Order: A → B → C → D → E → F → G → H.
