# Plan 2 Strip — RESUME NOTE (window 3, 2026-06-07)

**Branch:** `plan2-strip` (in this container at `/home/user/rift`; the harness default
branch `claude/rift-plan2-excision-0dyb7` is just an empty init commit — IGNORE it, all
work + the real codebase live on `plan2-strip`). The local memory `project_rift.md` is
NOT in the container; this note replaces its "PROGRESS (window N)" section.

**Goal (unchanged):** local-only terminal, AI = inline command autocomplete ONLY
(`crates/rift_ai` + `app/src/ai/predict` + `app/src/ai/block_context`). FULL EXCISION of
the AI agent product + cloud. See `docs/superpowers/plans/2026-06-06-rift-plan2-revised-terminal-first.md`.

## Build / env
- `cargo build --bin rift-oss 2>&1 >/tmp/rb.log; grep -c '^error' /tmp/rb.log`
- REQUIRED once per fresh container: `apt-get install -y protobuf-compiler` (build needs `protoc`).
- Never `cargo clean` (40-min dep rebuild). App-crate incremental rebuild ≈ a few minutes.

## Error trajectory (this session)
4173 (baseline) → 3081 → 2505 → 2427 → 2398 → 2412 → **2254** (HEAD a2e8b90c).
~46% reduced. All checkpoints committed + pushed (RED intermediate commits are expected
mid-Phase-A, matching prior windows). Re-baseline each window: rebuild → `/tmp/rb.log`.

## Precise cross-scope emit/consume sites to fix (from persistence de-wire)
When finishing the contract cluster, these now-dangling producers of removed persistence
contracts must be de-wired:
- Deleted `ModelEvent` variants are EMITTED by `pane_group/pane/terminal_pane.rs` (3 sites)
  and `terminal/view.rs` (1 site).
- Deleted `PersistedData` fields (cloud_objects, object_actions,
  time_of_next_force_object_refresh, ai_queries, multi_agent_conversations,
  mcp_server_installations, mcp_servers_to_restore) are CONSUMED in `lib.rs` ~1177–1194.
- Removed `StartedCommandMetadata` fields (cloud_workflow_id, workflow_command,
  is_agent_executed) are SET in `terminal/writeable_pty/command_history.rs`.
- Removed `LeafContents`/`TerminalPaneSnapshot` agent/cloud variants+fields are CONSTRUCTED
  across `workspace/**` and `pane_group/**`.
- `context_chips/{display.rs,display_chip.rs}` are deeply parameterized over deleted agent
  models (BlocklistAIInputModel/AIContextModel, AgentViewController, AmbientAgentViewModel,
  AIDocumentId) — their ctors are called from terminal code, so de-wire needs coordinated
  signature changes (do with the contract cluster). `current_prompt.rs` only blocks on
  GitRepoStatusModel (re-home target).
- `integration_testing/**` is behind `feature="integration_tests"` (NOT in default build);
  its agent/cloud helpers are consumed by `crates/integration` — coordinate before deleting.

## What's DONE this window
- `terminal/model/blocks.rs`: agent_view_state field + conversation/AI method cluster +
  AIBlock helpers excised. (DONE/committed)
- `terminal/view.rs`: deleted 43 agent methods (~3700 lines) + 31 agent struct fields +
  agent let-bindings/Self-entries in `TerminalView::new` ctor. (committed; subscription
  stragglers + Event/action enums + dispatchers REMAIN)
- `workspace/view.rs`: deleted 35 cloud/agent methods (~2100 lines). (dispatcher/enum REMAIN)
- `terminal/input.rs`: deleted agent/cloud methods (~2000 lines). (ctor/dispatcher REMAIN;
  note `maybe_launch_cloud_handoff_request` has TWO defs — handle manually)
- `pane_group/mod.rs`: 18 agent methods deleted (more REMAINS — see cluster below).
- Subagents (reviewed): full excision of `app/src/search/**`, `app/src/settings_view/**`
  (NB: `settings_view/code_page.rs` deleted wholesale — was 95% AI codebase-indexing; a
  minimal non-AI Code settings page could be re-added later if wanted), `app/src/code/**`
  (IDE editor — USER DECISION: strip entirely), `remote_server/` agent bits, partial
  persistence agent-table removal.

## METHODOLOGY THAT WORKS (reuse it)
1. **Structural method-deletion via python brace-matcher** (huge wins, low risk for
   clearly-agent private methods): see `/tmp/delmethods*.py` pattern — match
   `^    (pub..)?(async )?fn NAME(`, brace-count to close, delete incl. preceding
   doc/attr lines; guard against MULTI/NOTFOUND/OVERLAP. Used on view/workspace/input.
2. **Struct-field removal**: parse struct body, collapse multiline fields, remove fields
   whose type matches a deleted-token regex (verify ambiguous types exist in app+crates
   first — the COMPILER is authority; e.g. GitRepoStatusModel/GitDeltaPreference/
   EphemeralMessageModel are DELETED, EnvVar lives in cloud crate).
3. **Ctor de-wire**: remove agent `let NAME = ...;` (balanced to `;`) + Self{} entries.
4. **Parallel subagents** for SELF-CONTAINED leaf subtrees only (search, settings_view,
   code, misc). They CANNOT build (shared target) so review diffs + rely on the next
   consolidated build. They hit session limits ~after 70-90 tool uses — partial work is
   on disk; commit it when git settles.

## KEY SEQUENCING INSIGHT (why remaining work resists parallelism)
The core cluster is contract-coupled and MUST be de-wired jointly by ONE agent:
`workspace/view.rs` + `pane_group/mod.rs` (the `IPaneType` enum + `pub use` of deleted
panes: CodePane/NotebookPane/WorkflowPane/AIDocumentPane/CodeDiffPane/FilePane/
EnvironmentManagementPane/ExecutionProfileEditorPane) + `terminal/input.rs` + the
workspace leaf enum-definers (`workspace/view/{left_panel,right_panel,vertical_tabs}.rs`
defining ToolPanelView/LeftPanelEvent/RightPanelEvent + `workspace/action.rs` WorkspaceAction).
Removing an enum variant requires removing ALL its match arms across this cluster at once.
`LocalOrRemotePath` re-points from `crate::code::buffer_location` → `rift_util::local_or_remote_path`.

## DELETED vs KEEP reference (for de-wiring)
DELETED (remove all refs): `crate::ai::*` EXCEPT `block_context`+`predict`; `crate::code`
(whole IDE); `crate::{drive,workflows,notebooks,env_vars,cloud_object,ai_assistant}`;
agent block types (AgentViewVisibility, SerializedAgentViewVisibility, SerializedAIMetadata,
AgentInteractionMetadata); `terminal::shared_session`; `terminal::view::{ambient_agent,
conversation_list,queued_prompts_panel}`; inline_banner ZeroStatePromptSuggestion*/
PromptSuggestionBannerState; cloud-object id types (ObjectIdType/NotebookId/WorkflowId/…);
`remote_server::{codebase_index_model,diff_state_proto,diff_state_tracker}`.
STILL EXISTS — DO NOT remove yet (deleted in later phases): `crate::server`, `crate::auth`,
`crate::workspaces` (plural=cloud teams), `crate::pricing`, `crate::autoupdate`.
NEVER TOUCH: `crates/rift_ai`, `app/src/ai/predict/**`, `app/src/ai/block_context`,
`rift_core` macros (send_telemetry_*, report_error/report_if_error, safe_warn/info/error —
keep `use rift_core::...`), `crate::editor` (terminal input editor, distinct from deleted code/).
`context_chips/` is MIXED-KEEP (prompt rendering: PromptSnapshot/CurrentPrompt/GitLineChanges/
PromptDisplay/PromptType/ContextChipKind) — de-wire agent chips only, don't delete subtree.

## NEXT STEPS (in order)
1. Finish the contract cluster (single agent, compiler-driven): view.rs Event+action enums +
   handle_action/handle_input_event/handle_terminal_event arms; pane_group/mod.rs IPaneType +
   pane re-exports + pane/mod.rs; workspace/view.rs dispatchers; workspace/action.rs
   WorkspaceAction; left_panel/right_panel/vertical_tabs enum defs; input.rs ctor/dispatchers.
   Also terminal/local_tty/terminal_manager.rs (91), terminal/view/pane_impl.rs (27).
2. Finish remaining leaf de-wiring (root_view 50, util/link_detection 28, slash_commands,
   session_settings, persistence/sqlite cloud tables + ModelEvent arms, lib.rs inits).
   NB lib.rs init lines to drop: ai::blocklist*, drive::*, ai_assistant::panel,
   settings_view::update_environment_form, env_vars::*, ai::agent::todos::popup,
   coding_entrypoints::project_buttons (verify context_chips::display_menu/node_version_popup
   keep-vs-delete first). determine_agent_source fn (AgentSource) → delete + its callers.
3. Phase C/D/E/F (wholesale): telemetry macro no-op + AuthState shim in rift_core (Phase E,
   ALLOWED here only); then DELETE server/, auth/, workspaces/, pricing/, autoupdate/,
   changelog_model, voice; drop crates rift_server_auth/rift_server_client/firebase/
   managed_secrets + cloud_object_* + mcp from Cargo. Keep-path fix: rift_bridge.rs/
   next_command_model.rs AIApiError → local error when server/ goes.
4. Phase G: delete graphql crates (handle crates/ai rerank + websocket dep first).
5. Phase H: 0 warnings, tests, tag.

## REIMPLEMENTATION needed (NOT mechanical deletion — design decisions)
These KEEP features depended on primitives that lived in deleted agent modules:
- Rich-content rendering for KEEP terminal/CLI features — `ssh/install_tmux.rs`
  (`requested_script`/`RequestedScriptStatus`/`TitledScript`), `warpify/render.rs` +
  `ssh/warpify.rs` (`inline_action_icons`/`RenderableAction`), `view/plugin_instructions_block.rs`
  + `warpify/success_block.rs` (`CodeSnippetButtonHandles`/`render_code_block_plain`/
  `CodeBlockOptions`/`render_runnable_code_snippet`) — all from deleted
  `app/src/ai/blocklist/{inline_action,code_block}.rs`. Reimplement with non-deleted
  riftui primitives (or salvage those two files' non-agent rendering helpers à la the
  `redact_secrets`→`terminal::model::secrets` salvage precedent).
- `model/session/command_executor/{wsl,remote}_command_executor.rs` use
  `serialize_variables_for_shell` + `EnvVarValue` (deleted `crates/cloud_object_models/
  src/env_vars.rs`) for functional PATH passing — add a terminal-local helper or drop
  that PATH-passing behavior.
- `model/secrets.rs` `RichContentSecretTooltipInfo.location: TextLocation` — `TextLocation`
  also broken in `util/link_detection.rs`; decide its home jointly.

## CONTRACT-CLUSTER details (do these together, single agent, compiler-driven)
Shared types whose variant removal must be coordinated across the excluded/parent files:
`InputConfig`, `InputSuggestionsMode`, `AgentViewState`/`BlockList::agent_view_state()`,
`AgentViewController(Event)`, `ConversationStatus`, `BlocklistAIRenderContext`,
`AIAgentActionId`/`requested_command_action_id`, `is_agent_executed`,
`SharedSessionStatus/Source/ActionSource`, `session_settings.rs ToolbarChipSelection`/
`AgentToolbarItemKind` (deleted). Note `terminal/model/session/active_session.rs
ai_execution_environment()` already removed.
