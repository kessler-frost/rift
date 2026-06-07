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

## Error trajectory
4173 (baseline) → ... → 2254 (window 3 HEAD a2e8b90c) → 2081 (window 4) → **1987** (window 5).
~52% reduced. All checkpoints committed + pushed (RED intermediate commits are expected
mid-Phase-A). Re-baseline each window: rebuild → `/tmp/rb.log` (use
`cargo build --bin rift-oss > /tmp/rb.log 2>&1` — NOTE: `>/tmp/rb.log` ordering after
`2>&1` truncates the log; use the form here).

## WINDOW 5 — what's DONE (linchpin PART 1: the create_model trio is GREEN-relative)
- **`terminal/local_tty/terminal_manager.rs`** (was 91 errors → 0 real errors): `create_model`
  signature dropped is_shared_session_creator/restored_blocks/conversation_restoration/
  initial_input_config; the entire session-sharing body + tail DELETED (the restored_blocks/
  conversation merge logic, IsSharedSessionCreator match, the warp-prompt session_sharer
  observe, the `#[cfg(test)] attempt_to_share_session` block, the restoration-separator block,
  the LLMPreferences/ai_input_model/agent_view_controller/ai_context_model/ai_controller/
  BlocklistAIHistory subscriptions, the ActiveAgentViewsModel.register, and the
  Self::wire_up_session_sharer_with_view + Self::handle_network_status_events calls).
  KEPT: channels, Sessions, ModelEventDispatcher, ApiKeyManager.register, preferred_shell/
  ShellStarter, create_terminal_model (restored_blocks→None), pty_controller,
  remote_server_controller, current_prompt/prompt_type, the trimmed `TerminalView::new` call,
  wire_up_pty_controller_with_view, wire_up_remote_server_controller_with_view. DELETED the
  whole session-sharing METHOD CLUSTER (stream_historical_agent_conversations,
  send_selected_conversation_update_for_sharer, start_sharing_session, log_shared_session_lifecycle,
  cleanup_shared_session, shared_session_terminated, end_shared_session,
  wire_up_session_sharer_with_view, handle_network_status_events, session_sharer accessor) and
  the should_skip_sharer_op helper + ACL_UPDATE_FAILURE_RESPONSE const. Removed the
  `session_sharer` struct field + Self{} entry. `on_view_detached` is now a no-op. Pruned the
  session_sharing_protocol::* / network / agent imports; re-added `use crate::send_telemetry_on_executor`.
  KEPT `pid()` (integration_tests-only).
- **`terminal/mock_terminal_manager.rs`** (was 4 → 0): create_model dropped restored_blocks/
  conversation_restoration params (→None into create_terminal_model + trimmed TerminalView::new);
  on_view_detached → no-op (was ActiveAgentViewsModel.unregister); test helper
  create_new_terminal_view_window_for_test dropped its SerializedBlockListItem param; fixed its
  8 callers in view_tests.rs.
- **`terminal/remote_tty/terminal_manager.rs`** (was 1 → 0): create_model dropped
  initial_input_config param + the two extra TerminalView::new args.
- Net: 2081 → 1987 (-94). Committed `7b08bbb6`, pushed.

## ⚠️ CORRECTION to window-4 note (IMPORTANT for next window)
The window-4 note claimed `terminal/view.rs` had its `TerminalView::new` ctor agent let-bindings
AND its 31 agent struct fields removed. **THIS IS NOT TRUE on disk.** As of window 5:
- The `TerminalView` STRUCT (def starts ~line 2294) STILL has all agent fields:
  scroll_position_before_entering_agent_view, enter_agent_view_after_pending_commands,
  agent_view_back_button, is_orchestration_split_off, conversation_details_panel,
  ambient_agent_cancel_mouse_state, and (in the Self{} block ~3835-3993):
  ai_controller, ai_action_model, ai_render_context, get_relevant_files_controller,
  shared_session, pending_share_source, auto_stop_sharing_on_cli_end, ai_input_model,
  ai_context_model, agent_todos_popup, cli_subagent_controller, use_agent_footer(=use_agent_button_bar),
  agent_view_controller, agent_view_back_button, orchestration_pill_bar, ambient_agent_view_model,
  conversation_details_panel, pending_cloud_followup_task_id, first_time_cloud_agent_setup_view,
  environment_setup_mode_selector, ephemeral_message_model, passive_suggestions_models, etc.
- The `TerminalView::new` ctor (2863-3993, ~1100 lines) is FULLY agent-laden, NOT just
  "subscription stragglers". The agent vars (agent_view_controller/ai_controller/ai_context_model/
  ai_input_model/ai_action_model/ambient_agent_view_model/ephemeral_message_model/
  cli_subagent_controller) are referenced (undefined) by MANY blocks + the `Input::new` call
  (~3385) + the `Self{}` block. Only a FEW vars are still let-defined (suggestions_mode_model,
  ai_status_bar, conversation_details_panel — all derived from `input`, which itself needs the
  Input::new strip first). So this is a from-scratch ctor+struct rewrite, not a straggler trim.
- `Input::new` (input.rs ~1975) STILL takes the 8 agent params and its body builds
  DisplayChipConfig/PromptDisplay/etc. all over agent models — it is its own large agent-strip.

## WINDOW 4 — what's DONE (the pane_group contract sub-cluster is now GREEN)
- **`pane_group/pane/mod.rs`**: IPaneType trimmed to Terminal/Settings/GetStarted/
  NetworkLog/Welcome/DeferredPlaceholder/Dummy; removed all `from_*_pane_*`/`is_*_pane`/
  render arms for deleted panes; PaneEvent lost NewPaneInAIMode/ReplaceWith{Code,File}Pane;
  `LocalOrRemotePath` repointed to `rift_util::local_or_remote_path`.
- **Sharing subsystem GUTTED** (cloud `ShareableObject`/`SharingDialog`/`ContentEditability`
  were deleted types): `pane_group/pane/view/header/sharing.rs` rewritten to no-op shims
  (SharedPaneContent + no-op header methods); removed `set_shareable_object`/
  `ShareableObjectChanged` from PaneConfiguration + view/mod.rs + header add_overlays arm.
- **`pane_group/mod.rs`** (was 139 prim errors → ~0): removed `pub use` of 8 deleted panes,
  AmbientAgentViewModelHandle alias+trait, dead imports; trimmed the `Event` enum (~30
  agent/cloud variants), `NewTerminalOptions` (is_shared_session_creator/
  conversation_restoration), `PanesLayout::AmbientAgent`, and ~12 agent PaneGroup struct
  fields (share_session_modal, role_change_modal, active_file_model, child_agent_*,
  pending_ambient_*, transitively_shared_*); deleted ~64 pure-agent methods via the
  name-based brace-matcher (`/tmp/delbyname.py`); SURGICALLY simplified the CORE methods:
  `restore_pane_leaf`/`restore_pane_tree` (now only Terminal/Settings/NetworkLog/Welcome/
  GetStarted arms; dropped block_lists/conversation_restoration/pending_ambient params),
  `create_session` (dropped is_shared_session/restored_blocks/conversation_restoration/
  initial_input_config), `new_with_panes_layout` (dropped block_lists + deferred/ambient
  machinery), `add_session`/`add_session_with_default_session_mode_behavior`/
  `create_terminal_pane_data`/`add_session_in_directory` (dropped conversation_restoration +
  IsSharedSessionCreator + agent-view entry), `new_internal` ctor (removed agent
  subscriptions + Self{} fields), `selected_text_from_focused_pane`/`discard_pane`/
  `close_active_pane_with_confirmation`/`close_overlays`/`render` (terminal-only),
  `pane_tree_from_template_recursive` (dropped PaneMode::Cloud/Agent branches —
  launch_config::PaneMode STILL has Agent/Cloud variants, treated as Terminal here).
- **`app_state.rs`**: removed `use crate::ai::blocklist::{InputConfig, SerializedBlockListItem}`,
  AppState.block_lists/running_mcp_servers fields, TerminalPaneSnapshot.input_config field.
  (`std::sync::Arc`/`HashMap` imports now unused → warnings only, leave for warning-sweep.)
- **`terminal/view.rs`**: `TerminalView::new` signature lost initial_input_config/
  conversation_restoration/is_cloud_mode params (BODY de-wire NOT done — see linchpin below).

## LINCHPIN STATUS (window 5)
- **PART 1 DONE**: `local_tty`/`mock`/`remote_tty` `create_model` + `TerminalManager` impls are
  de-agented and contribute 0 real errors (see WINDOW 5 above). The `TerminalView::new`
  CALL sites now match the trimmed 11-arg signature
  `(resources, wakeups_rx, model_events_handle, model, sessions, size_info, colors,
  model_event_sender, current_prompt, inactive_pty_reads_rx, ctx)`.
- **PART 2 REMAINS (the big one)**: `terminal/view.rs` `TerminalView` struct + `TerminalView::new`
  BODY/`Self{}` + `terminal/input.rs` `Input::new` signature+body + `context_chips::display*`
  ctors. This is a ~several-thousand-line interlocked agent web (NOT a straggler trim — see the
  CORRECTION section above). Recommended order for next window:
  1. Strip `Input` struct + `Input::new` (input.rs, 278 errs): drop the 8 agent params
     (ai_controller/ai_context_model/ai_input_model/ai_action_model/cli_subagent_controller/
     agent_view_controller/ambient_agent_view_model/ephemeral_message_model); remove agent body
     (DisplayChipConfig agent fields, PromptDisplay agent args, agent footer/status-bar). Keep
     completer/prompt/editor terminal essentials. `suggestions_mode_model`/`agent_status_bar`/
     `inline_terminal_menu_positioner` accessors are read by view.rs ctor — keep or remove jointly.
  2. Strip `TerminalView` struct agent fields (view.rs ~2294-2660) + the `Self{}` block
     (~3835-3993) + the ctor agent let/subscription blocks (2876-3834) TOGETHER (compiler-driven).
  3. Then the cascade: Event/action enums (view/action.rs 24) + handle_action/handle_input_event/
     handle_terminal_event arms; view/pane_impl.rs (27); view/rich_content.rs (21); view/init.rs (13);
     pane_group/pane/terminal_pane.rs (119, snapshot must match trimmed TerminalPaneSnapshot:
     uuid/cwd/shell_launch_data/is_active/is_read_only/active_profile_id ONLY); workspace/view.rs
     handle_action; workspace/action.rs WorkspaceAction (~398-700); workspace/view/{left_panel,
     right_panel,vertical_tabs}.rs enum-definers — remove each variant + ALL its match arms together.
- NOTE: `parent` `terminal/terminal_manager.rs` `create_terminal_model` still has 2 pre-existing
  errors NOT from window 5: `SerializedBlockListItem` param type (line ~79) and free-fn
  `should_collect_ai_ugc_telemetry` call (line ~101, the free fn was deleted; it's a method on
  PrivacySettings now). These are coupled to `terminal/model/terminal_model.rs` (TerminalModel::new
  still threads restored_blocks: Option<&[SerializedBlockListItem]> + SharedSessionStatus) — a
  separate later sub-task. Counted in the 1987.

## LINCHPIN remaining (THE thing blocking the rest; do it as ONE coordinated change)
`TerminalView::new` + `Input::new` + `local_tty::TerminalManager::create_model` (+ the
`remote_tty`/`MockTerminalManager` create_model variants behind cfg) form a single
signature contract that I simplified at the `create_session` CALL site (window 4) but NOT at
the DEFINITIONS. They all still take/thread the deleted agent params and have huge agent
bodies. Required joint change:
- `terminal/local_tty/terminal_manager.rs` `create_model` (lines ~187–832, ~645 lines, 91
  errors): drop params is_shared_session_creator/restored_blocks/conversation_restoration/
  initial_input_config; DELETE the entire session-sharing tail (session_sharer Network,
  LLMPreferences agent-mode subs, agent_view_controller subs, BlocklistAIHistory subs,
  ai_context/ai_controller sharer-update subs, ActiveAgentViewsModel.register). KEEP:
  channels, Sessions, ModelEventDispatcher, ApiKeyManager.register, preferred_shell/
  ShellStarter, create_terminal_model (drop restored_blocks arg → None), pty_controller,
  remote_server_controller, current_prompt/prompt_type, the simplified `TerminalView::new`
  call, wire_up_pty_controller_with_view, wire_up_remote_server_controller_with_view.
  The `#[cfg(test)] attempt_to_share_session` block + restoration-separator block go.
- `terminal/view.rs` `TerminalView::new` BODY (ctor ~2876–3835 before `let mut terminal_view
  = Self {`): the agent let-bindings were removed in window 3, leaving SUBSCRIPTION
  STRAGGLERS that reference now-undefined vars (agent_view_controller, ai_controller,
  ai_context_model, ai_input_model, ai_action_model, suggestions_mode_model,
  ambient_agent_view_model, ephemeral_message_model, cli_subagent_controller). Delete those
  subscription blocks (2883 agent_view_controller block is ~430 lines; 3313 ai_controller;
  3327 AgentConversationsModel; 3424 ai_status_bar; 3434 ambient; 3441/3446/3447 ai_*;
  3448 CLIAgentSessions stop_sharing; 3463/3468 ai_action_model executors). KEEP the
  legit terminal subs (model_events_handle→handle_terminal_event, inline_menu_positioner,
  input→handle_input_event, find_bar, block_filter_editor, context_menu, sessions,
  windowing_state_handle, ligature_handle, block_list_settings_handle, UserWorkspaces,
  cli_subagent_controller IFF kept). **`Input::new` (3385) is the crux**: it takes 8 agent
  params (ai_controller, ai_context_model, ai_input_model, ai_action_model,
  agent_view_controller, ambient_agent_view_model, ephemeral_message_model,
  cli_subagent_controller) — must simplify `Input::new` signature (in `terminal/input.rs`,
  278 errors) in the same pass, which is itself a large agent-strip.
- Then the Event/action enums + handle_action/handle_input_event/handle_terminal_event arms
  in view.rs, and workspace/view.rs handle_action arms + workspace/action.rs WorkspaceAction
  variants + left_panel/right_panel/vertical_tabs enum-definers — all must drop the same
  agent variants TOGETHER (removing a variant requires removing all its arms across the
  cluster at once). workspace/action.rs WorkspaceAction agent variants are at lines ~398–700.

## Tooling note (reuse)
`/tmp/delbyname.py <file> <comma,sep,method,names>` — deletes whole methods by name (brace-
matched, incl. preceding doc/attr lines). SAFE for pure-agent methods; do NOT use on core
methods that merely *reference* a deleted token incidentally (edit those by hand). The
generic body-token matcher (`/tmp/delmethods.py`) is TOO BLUNT for pane_group-style files
(it nuked core restore/create/render methods) — prefer name-based.

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
0. DONE (window 4): pane_group/pane/mod.rs + pane_group/mod.rs + sharing subsystem +
   app_state.rs are GREEN. See "WINDOW 4" + "LINKPIN remaining" sections above.
1. Finish the contract cluster (single agent, compiler-driven) — START with the LINCHPIN
   (see dedicated section above): the joint `TerminalView::new` + `Input::new` +
   `create_model` signature/body simplification. THEN: view.rs Event+action enums +
   handle_action/handle_input_event/handle_terminal_event arms; workspace/view.rs
   dispatchers; workspace/action.rs WorkspaceAction; left_panel/right_panel/vertical_tabs
   enum defs; input.rs dispatchers. Also terminal/view/pane_impl.rs (27),
   pane_group/pane/terminal_pane.rs (119 — many will clear once create_model/TerminalView::new
   land since terminal_pane constructs/snapshots TerminalPaneSnapshot — note its snapshot()
   must match the trimmed TerminalPaneSnapshot fields: uuid/cwd/shell_launch_data/is_active/
   is_read_only/active_profile_id ONLY).
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
