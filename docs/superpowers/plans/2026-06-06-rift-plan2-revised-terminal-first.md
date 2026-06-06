# Rift — Plan 2 (REVISED): Terminal-First Full Strip

> **Supersedes** `2026-06-06-rift-plan2-cloud-strip.md` as of 2026-06-06 (mid-execution re-scope).
> The original plan kept Warp's AI agent product and only stripped cloud *infra*. After
> investigation + user decisions this session, the scope is now much larger and **simpler in
> shape (almost pure deletion)**: remove the entire AI agent product too, and drop
> workflows/notebooks/env-vars. Read the "DECISIONS" and "SCOPE" sections first.

**Goal:** A local-only terminal. The ONLY AI feature is **context-aware inline command
autocomplete** (via `rift_ai` → omlx). Everything else cloud/account/agent is removed.

**Branch:** `plan2-strip`. Resume baseline was clean `f2034746` (green). Tasks 1–2 of the
original plan (Phase 0 flag-disable `3821be1a`, Phase 1 leaf modules `f2034746`) are DONE.

---

## DECISIONS (2026-06-06, user-confirmed — firm)

1. **AI = autocomplete ONLY.** Keep context-aware inline completion (`rift_ai::complete` +
   `app/src/ai/predict` completion seam). Context (recent cmd history: input/output/pwd/
   git_branch/exit_code + current input + shell) is ALREADY sent — verified in `rift_bridge.rs`/
   `complete.rs`/`context.rs`. Nothing to build; just preserve the seam.
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
- Editor/input: `app/src/editor*`, `crates/rift_editor`, input.
- Settings/themes, local persistence (`app/src/persistence/` minus cloud tables), `crates/rift_core`.
- **AI autocomplete seam:** `crates/rift_ai` (keep `complete`, `context`, `client`, `config`,
  `messages`; **drop `translate`**) and `app/src/ai/predict/{mod.rs, generate_ai_input_suggestions*,
  next_command_model.rs, rift_bridge.rs}`.

### DELETE (features → infra order)
- **AI agent product:** all of `app/src/ai/` EXCEPT the keep-path above; `app/src/ai_assistant/`;
  `app/src/code_review/` (AI). The deleted `predict` siblings: `predict_am_queries*`,
  `generate_am_query_suggestions*`, `prompt_suggestions*`, `rift_nl_prefix.rs`.
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
- Do NOT touch `app/src/workspace/` (singular), the rift_ai crate, or the predict completion seam
  except the noted small fixes.
- Each phase ends GREEN + commit. Never commit a red build. If a phase won't converge, STOP and report.

### Keep-path small fixes (do when the relevant infra is deleted)
- `ai/predict/rift_bridge.rs:11` imports `crate::server::server_api::AIApiError`; `local_suggestions`
  returns `Result<_, AIApiError>`. When `server/` is deleted (Phase F), replace `AIApiError` with a
  local error (or make the fn infallible `-> GenerateAIInputSuggestionsResponseV2`).
- `ai/predict/next_command_model.rs:33` imports `server_api::{AIApiError, ServerApi}` (the Plan-1
  dead-code seam) — remove with server. Also drop the `#[allow(dead_code)] server_api` field +
  ctor param + call sites.
- `ai/predict/generate_am_query_suggestions/api/response.rs:10` uses `crate::ai::agent::FileLocations`
  — that whole subdir is deleted (am-query path), so this goes away.

---

## PHASES (each ends GREEN + commit)

### Phase A — Excise the AI agent product
Delete all of `app/src/ai/` except the keep-path; delete `ai_assistant/`, AI `code_review/`; drop
`rift_ai::translate` + `rift_nl_prefix` + `predict_am_queries`/`generate_am_query_suggestions`/
`prompt_suggestions`. De-wire terminal (AI panel, agent blocks, inline actions, conversation history,
suggestion chips, AI input footer), `pane_group` agent panes, `settings_view` AI pages, `root_view`,
command palette agent commands, and `lib.rs` agent-model boot inits. Use the de-wiring map (read-only
agent output) as the checklist. **Biggest phase; removes ~56 of 122 ServerApi consumers + most
cloud_object/auth consumers.**

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

### Phase E — Telemetry no-op + AuthState shim
Replace telemetry macro DEFINITIONS in `rift_core` with no-ops (keep names/paths/arities). Add a local
`AuthState` shim in `rift_core` exposing only the methods KEEP code still calls (e.g.
`is_anonymous_or_logged_out()->false`, `anonymous_id()->persisted UUID`). Repoint KEEP code. Prep for
Phase F.

### Phase F — Delete server/ + auth/ + their crates
Replace `lib.rs` boot inits (`AuthState::initialize`→shim; delete ServerApiProvider/AuthManager/
telemetry providers/ManagedSecretManager/NetworkLogModel/ServerExperiments/AIRequestUsageModel). Delete
`app/src/server/`, `app/src/auth/`. Apply the keep-path small fixes (rift_bridge/next_command_model
error type). Drop crates `rift_server_auth`, `rift_server_client`, `firebase`, `managed_secrets`.

### Phase G — Detangle + delete graphql crates
Stub/remove `crates/ai` rerank graphql usage if `crates/ai` still exists; handle `crates/websocket`
graphql dep; delete `crates/graphql` + `crates/rift_graphql_schema`; remove from workspace Cargo.toml.

### Phase H — Acceptance
`cargo build --bin rift-oss` 0 warnings; `cargo test -p rift_ai` (drop translate tests) + pure-logic
crates; sweep `rg`/`fd` to confirm cloud/agent gone; record binary-size delta vs Plan-1 (~721 MB);
`git tag rift-plan2-complete`; hand off GUI smoke test to user (launch, run cmds, blocks render,
settings/themes, inline autocomplete via omlx, no crashes from removed paths).

---

## SEQUENCING
Sequential — each green build gates the next; phases edit overlapping core files (`lib.rs`,
`terminal/view.rs`) so no parallel implementers. Read-only mapping/recon may run in parallel.
Order: A → B → C → D → E → F → G → H.
