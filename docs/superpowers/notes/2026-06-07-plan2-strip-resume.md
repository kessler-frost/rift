# Plan 2 Strip — RESUME NOTE (window 8, 2026-06-07)
## 📖 READ `WARP.md` (repo root) FIRST — it is the canonical strip guideline (user-maintained)
WARP.md governs: "Delete use-sites; don't stub" (delete methods/fields/variants/match-arms that carry deleted
types — do NOT paper over with return-false stubs), exhaustive matching = the safety net (NO `_ =>` wildcards),
unresolved-import (E0432) suppresses use-site errors so count jumps after import sweeps are EXPECTED, remove unused
params completely (never `_`-prefix), KEEP all `rift_core` macro imports (telemetry/logging/safe_log). Build loop +
prereqs (protoc, no `cargo clean`) documented there too.
## ⚠️⚠️ COURSE CORRECTION (user directive, 2026-06-07): NUKE ALL AI + CLOUD — DELETE, DO NOT STUB ⚠️⚠️
The user wants ZERO AI and ZERO cloud. DO NOT use "recreate-minimal" stubs for AI/cloud types to make
AI-adjacent code compile — that PRESERVES AI plumbing. Instead DELETE the code that uses them.
REVERTED: the SharedSessionStatus/SharedSessionSource/AmbientAgentTaskId stubs (terminal/shared_session.rs)
and AIConversationId/AIAgentExchangeId stubs — all removed. Now DELETE their use-sites:
 - shared-session subsystem: TerminalModel fields shared_session_status/shared_session_source/
   ordered_terminal_events_for_shared_session_tx/write_to_pty_events_for_shared_session_tx + new_*_for_shared_session
   ctors + shared_session_status()/set_* accessors + the ~65 callers (gut to the no-share path) + SizeUpdateReason
   SharerSizeChanged/ViewerSizeReported variants.
 - ambient-agent subsystem: AmbientAgentTaskId + ambient_agent_view_model + pane_group/ambient_pane_restoration.
 - AI conversation/block/fork: context_menu fork code, rich_content AI metadata, view.rs AI-block rendering,
   ai_context_model/ai_input_model/agent_view_controller fields on TerminalView/Input.
KEEP (terminal, not AI/cloud): SerializedBlockListItem (block-restore, Command-only — already AI-stripped).
RECONSIDER AgentToolbarItemKind (input chip toolbar — borderline; user said nuke AI so likely drop too).
This is the DEEP CORE STRIP (god-struct AI field cascades). It is the bulk of real remaining work.

NUKE PROGRESS (628->568): DELETED (properly, not stubbed): shared-session subsystem (TerminalModel 4 fields
+20 methods + ~6 caller sweeps incl init.rs keymap predicates + input.rs viewer-state block + view.rs pending-share),
ambient_pane_restoration.rs (whole module), ServerApi AI methods (transcribe/predict/generate-suggestions/
ambient-headers/get-ai-client/get-cloud-objects) + ambient_agent_task_id/agent_source fields+ctor params (ServerApi
+ServerApiProvider+lib.rs caller). TOOLING: delfns.py corrupts methods whose body trips its brace-matcher (server_api
ambient_agent_headers_for_task) — delete those via Edit. REMAINING DEEP-NUKE TARGETS: workspace/view.rs 73,
input.rs 47 (ai_input_model/InputType/InputConfig), terminal/view.rs 42 (AI-block render), right_panel 34 (code-review),
slash_commands 51, server/graphql 22 + harness_support 15 (Phase F cloud — check wholesale-delete), rich_content 15,
install_tmux 14 (drop requested_script UI), lib.rs ai_client wiring (get_ai_client deleted — thread out ai_client var).


## ⚠️ HANDOFF — continue on LOCAL machine (no more subagents)
State at handoff: branch `plan2-strip`, **568 real compile errors (nuke-mode, deleting AI/cloud not stubbing)** (down from 4173 baseline,
~65%), all committed + pushed (latest InputEvent-cascade commit).

⚠️ **SCOPE CHANGE 2026-06-07 (user-confirmed): REMOVE ALL AI — no keep-path.** The former
"keep inline autocomplete via rift_ai→omlx" carve-out is CANCELLED. DELETE `crates/rift_ai` +
ALL of `app/src/ai/` (incl. former keep-path `predict/**` + `block_context`) + the editor's AI
inline-autosuggestion hook (`maybe_populate_intelligent_autosuggestion` / AI decorator / AI cursor
colors) + `InputType`/`InputTypeAutoDetectionSource` (input is always Shell now — DELETE, do NOT
re-home). KEEP only the NON-AI autosuggestion (fish-style history + `command_corrections`
rule-based). The strip is now uniform — no AI seam to preserve. See updated plan DECISIONS/SCOPE.

view.rs is down to 82 errors; the next cluster is **input.rs (124)** — same shape as the view.rs
cleanup, now SIMPLER (delete the AI autosuggestion path too, no InputType re-homing). Dead imports
at input.rs top (L96 handoff_compose, L115 universal_developer_input, L118 ambient_agent, L125
queued_prompts_panel, L193 buy_credits_banner, L201-224 conversations/models/plans/profiles/prompts/
rewind/skills/user_query) + emit sites for the removed InputEvent variants + agent methods. Then
workspace/view.rs (252), pane_group/pane/terminal_pane.rs (119), then Phases C/D/F/G wholesale
deletes (incl. `crates/rift_ai` + `app/src/ai/` wholesale + drop rift_ai from app/Cargo.toml).
WINDOW 18: honest grind 631->592. TWO KEYSTONE RECREATES (recover exact code from git commit fe469743^,
strip AI/network/graphql parts):
 - SharedSessionStatus + SharedSessionSource -> new file terminal/shared_session.rs (recovered from deleted
   terminal/shared_session/mod.rs; dropped active_viewer_keymap_context[0 callers]/IsSharedSessionCreator[tests];
   Role from session_sharing_protocol::common, SessionSourceType from ::sharer; as_keymap_context->&'static str).
   Wired `pub mod shared_session;` in terminal/mod.rs + imports in terminal_model/input/view/view::init/
   session_management. Cleared ~24.
 - AmbientAgentTaskId(NonNilUuid) -> appended to terminal/shared_session.rs (recovered from deleted
   ai/ambient_agents/mod.rs; dropped the cynic::Id graphql impl; kept Display/FromStr). Wired imports in
   workspace/view + pane_group/ambient_pane_restoration + terminal_model + server_api + harness_support. Cleared ~15.
 RECREATE-MINIMAL now proven 4x (SerializedBlockListItem/AgentToolbarItemKind/SharedSessionStatus/AmbientAgentTaskId)
 — recover exact def from git fe469743^, keep non-AI parts, wire imports.

NEXT CLUSTER (terminal_model 2 remaining + spreads): AgentInteractionMetadata is NOT a clean recreate (it's
genuinely AI-coupled: requested_command_action_id->AIAgentActionId, ai::agent::{conversation,task} deps, 616-line
ai-coupled interaction_mode.rs). GUT it instead: drop agent_metadata params from terminal_model
start_command_execution_for_shared_session + start_command_execution_with_ai_metadata (+ delete
ai_metadata_to_protocol + uncalled send_agent_response_for_shared_session + remove AgentInteractionMetadata
from the super::block import L37), then fix CommandExecutionSource enum (drop AI{metadata}/SharedSession{ai_metadata}
variants — check CommandExecutionSource def) + pty_controller.rs write_command match arms (562/568) +
block.rs set_agent_interaction_mode. ~4-file coordination.

REMAINING after that: hub god-files view.rs ~70 / input.rs ~50 (InputType/InputConfig/ai_input_model) /
terminal/view.rs ~44 (render agent subsystem — scattered, EDITS) + right_panel 34 + slash_commands 51 +
Phase F server_api/graphql/harness_support ~50 + install_tmux 14 (requested_script — simplify, drop script UI).

WINDOW 17:

WINDOW 17: confirmed NO contained wins remain — every remaining file needs fresh-context care:
 - install_tmux.rs (14): SSH (KEEP) reused AI component `ai/blocklist/inline_action/requested_script.rs`
   (deleted). Its TYPES (TitledScript/RequestedScriptStatus/RequestedScriptMouseStates) are trivial but its
   render fns (render_requested_script) depend on AI-blocklist UI (requested_action/block::view_impl). FIX:
   either recreate the 3 trivial types + reimplement a SIMPLE script-display (new code), OR simplify
   install_tmux to drop the expandable-script UI. Judgment call — recommend the latter (simpler).
 - SharedSessionStatus keystone: shared_session module FULLY deleted (was terminal/shared_session/); callers
   have NO explicit imports (relied on deleted globs) — recreate needs a reachable home + nested
   SharedSessionSource (recreate) + Role (available from session_sharing_protocol::common). 65 callers.
 - The rest funnels through hub god-files (view.rs/input.rs/terminal/view.rs) — scattered single-site refs,
   EDITS only (deleter corrupts them), OR Phase F wholesale server/graphql deletes.
HONEST STATE: 631 real errors, all committed/pushed. ~57 commits. The grind from here is a coordinated
hub-centered + keystone-recreate effort best done with fresh context — no safe pick-off files remain.

WINDOW 16:

WINDOW 16: honest grind 663->631. CAREFUL-EDIT approach validated for entangled hub-adjacent files
(the /tmp/delfns.py + reverse-range deleters CORRUPT pane_impl-style files with closures — use Edits there).
Cleared: pane_impl.rs 27->0 (gut update_pane_configuration + delete selected_conversation chrome chain +
agent_view_shareable_object + render_shared_session_header_content + parent-card + overflow shared-session
items + Viewer import), ui_components/agent_icon.rs ->0 (gut terminal_view_agent_icon_variant to None +
delete dead helpers — SEVERED the conversation-status icon cluster across pane_impl/vertical_tabs),
auth/mod.rs 16->0 (gut log_out agent-model resets + quit-warning unsaved + delete remove_cloud_persisted_settings),
terminal/history.rs ->0 + rich_history.rs ->0 (delete linked_workflow x2 + serialized_block_is_agent_executed
+ render_ai_query_rich_history + gut callers).

NEXT KEYSTONE CLUSTER — SharedSessionStatus RECREATE-MINIMAL (unblocks terminal_model.rs 15 + ~10 files,
65 callers of .shared_session_status()). SharedSessionStatus is DELETED but heavily used. RECREATE it
(like SerializedBlockListItem/AgentToolbarItemKind) in terminal/model/terminal_model.rs or a shared spot:
  enum SharedSessionStatus { NotShared, ActiveSharer, ViewPending, ActiveViewer{role:Role},
    SharePending, SharePendingPreBootstrap{source:SharedSessionSource}, FinishedViewer }
  (also need minimal SharedSessionSource + Role/role enum + SharedSessionSource::source_type/orchestrator_task_id).
  Methods called: is_viewer/is_reader/is_sharer/is_executor/is_active_viewer/is_active_sharer/is_view_pending/
    is_sharer_or_viewer/is_finished_viewer/as_keymap_context/clone (all can be `false`/None except clone).
  TerminalModel::shared_session_status() should always return NotShared (sharing removed); set_shared_session_*
    become no-ops; shared_session_source()->None; ambient_agent_task_id->None.
  Simpler ALT: recreate SharedSessionStatus with ONLY NotShared + the is_* methods returning false, then fix
    the ~25 callers that match other variants (grep 'SharedSessionStatus::' for arms) — but full-variant
    recreate is less caller-churn. SharedSessionSource also used in terminal_pane.rs/gql/etc.

REMAINING after that: hub god-files view.rs 73 / input.rs 51 (InputType/InputConfig/ai_input_model AI-input
subsystem) / terminal/view.rs 44 (render agent subsystem) — all EDITS not scripts. Plus right_panel.rs 34
(CodeReviewView panel — likely wholesale-deletable + remove RightPanelView field/render from workspace/view),
slash_commands mod 26 + data_source 25 (execute_slash_command agent arms), server_api 21 + graphql/schema 22
(Phase F server), rich_content 15 (RichContentMetadata enum -> block_list_element), ssh/install_tmux 14.

WINDOW 15:

WINDOW 15: honest grind 759->697 (859 honest baseline). DONE this window: AgentToolbarItemKind recreate
(toolbar cluster), rich_content.rs partial (agent structs/field/accessors), vertical_tabs.rs 35->0
(TypedPane keep-only Terminal/Settings/Other + resolve_pane_type + drive-object icon colors gutted +
ConversationStatus summary sub-cluster + AgentNotifications/cloud-env/status-pill), working_directories.rs
22->0 (removed DiffStateModelMap + comment/code_review/file_tree fields + storage/cleanup methods).
PANE_IMPL CAUTION: /tmp/delfns.py CORRUPTED pane_impl.rs (left orphan braces — likely a method with a
complex closure). pane_impl needs EDIT-based work, not scripts. Its agent chrome is ENTANGLED: keep method
update_pane_configuration -> selected_conversation_display_title -> selected_conversation_for_user_facing_chrome
-> chrome helpers + default_agent_conversation_title; gut the conversation-title logic in update_pane_configuration
first, then delete the chain. pane_impl also: agent_view_shareable_object(ShareableObject/BlocklistAIHistoryModel),
render_shared_session_header_content(SharedSessionKind), render_parent_conversation_header_card, the
selected_conversation_* accessors, is_in_cloud_agent_setup_phase, super::Viewer import.
Remaining top files: view.rs 73, input.rs 51, terminal/view.rs 44, right_panel 34, pane_impl 27,
slash_commands mod 26 + data_source 25, graphql/schema 22, server_api 21, auth 16, rich_content 15.

WINDOW 14: honest grind continues 859->759. Validated RECREATE-MINIMAL pattern again: AgentToolbarItemKind
recreated as enum{ContextChip(ContextChipKind),RichInput} in terminal/session_settings.rs + imports wired in
chip_configurator/view (resolved toolbar cluster ~43 errs). chip_configurator match had `control=>` catch-all so
safe. Next clusters by yield: pane_group panes (~118: vertical_tabs/pane_impl/right_panel/working_directories),
input AI-mode subsystem (~51), terminal/view.rs render (~43), slash_commands (~51), rich_content (21,
agent_view_conversation_id field + InitStepBlock/PendingUserQueryBlock/InitEnvironmentBlock types).

WINDOW 13 (honest grind from re-baselined 859): user APPROVED re-baseline to honest count.
Committed 859 honest baseline (removed masking command_search-view glob from view.rs). Then ground
859->802 via SAFE deleter /tmp/delfns.py (single-pass, finds ALL fn ranges first, deletes in REVERSE
order, brace-count only AFTER first '{' — avoids delbyname's multi-name line-shift + orphan-'}' bug).
DONE: workspaces/user_workspaces.rs 37->0 (cloud spaces/drive/team methods), gql_convert.rs 26->6
(cloud GraphQL conversion fns + dead From impls). 

KEY INSIGHT: remaining ~802 are NOT independent per-file deletions — they're INTERCONNECTED AGENT-TYPE
CLUSTERS. Each needs a delete-whole-thing vs recreate-minimal-stub decision (cf. SerializedBlockListItem
which I recreated Command-only). Map of clusters:
 1. AgentToolbarItemKind (DELETED type) — terminal/session_settings.rs (27, ToolbarChipSelection trait +
    AgentToolbarChipSelection enum) + header_toolbar_editor.rs + tab_settings.rs + view.rs +
    context_chips/{current_prompt,prompt_type}.rs + terminal/view.rs. Header toolbar is KEEP; the trait's
    left_chips/right_chips filter ContextChip(ContextChipKind=KEEP). LIKELY recreate AgentToolbarItemKind
    minimally as enum { ContextChip(ContextChipKind) } (drop agent variants) — verify use-sites first.
 2. WorkspaceSettings.ai_autonomy_settings: AiAutonomySettings (fields are deleted ActionPermission/
    WriteToPtyPermission/ComputerUsePermission) — gql_convert.rs (6 remaining) + workspaces/workspace.rs
    type def. Remove ai_autonomy_settings field from WorkspaceSettings + the conversion block.
 3. pane_group agent panes (CodePane/CodeDiffPane/AIFactPane/NotebookPane/WorkflowPane/
    EnvironmentManagementPane/ExecutionProfileEditorPane/ChildAgentOrigin + IPaneType agent variants +
    TypedPane agent variants) — view/vertical_tabs.rs (35, resolve_pane_type/matching_tab_indices),
    view/pane_impl.rs (27, Viewer), view/right_panel.rs (34, CodeReviewView), working_directories.rs (22),
    home.rs, view.rs. Coordinated: trim IPaneType/TypedPane enums + delete the pane structs + use-sites.
 4. input AI-input-mode subsystem: InputType/InputConfig/InputTypeAutoDetectionSource/ai_input_model field
    woven through terminal/input.rs (51) handle_editor_event/input_enter/handle_action/ctor +
    suggestions_mode_model.rs. Remove ai_input_model field + InputType/InputConfig + AI-input branches.
 5. terminal/view.rs (44) render agent subsystem (ambient_agent/Viewer/agent_view_bg_fill/
    BlocklistAIHistoryModel/struct fields FileLocations/AIConversationId/FinishReason etc).
 6. Phase F server/: server_api.rs (21, ai::predict use-sites), network_log_view, graphql/schema (22).
 7. slash_commands mod (26) + data_source (25): FuzzyMatchWorkflowResult, input::models, saved_prompts.
 8. rich_content.rs (21), block_list_element.rs (shared_session), rich_history.rs (AIQueryHistoryEntryDetails).
APPROACH: hub files (view.rs/input.rs/terminal/view.rs) — use EDITS not scripts (broke view.rs 3x w/ scripts).
For each cluster, decide recreate-minimal (if KEEP code needs the type's non-agent part) vs delete-whole.

WINDOW 12 — ⚠️ CRITICAL DISCOVERY: THE ERROR COUNT WAS MASKED.
A FAILING GLOB IMPORT (`use CommandSearchEvent::*;` at workspace/view.rs ~L11903, inside
handle_command_search_event; CommandSearchEvent/CommandSearchView are FULLY DELETED — no `view`
module under search/command_search/) POISONS name resolution and SUPPRESSES E0433/E0412 for ALL
unresolved names crate-wide. PROOF: view.rs uses `CodeSource` 14x with NO import yet shows 0 errors
for it; baseline shows "20 errors" but removing the masking glob surfaces the TRUE count ≈ 859.
So the "813->38->20" drops were LARGELY ILLUSORY — clearing module-level broken imports flipped
rustc into suppression mode. REAL remaining ≈ 859, distributed: view.rs ~75, terminal/input.rs ~51,
terminal/view.rs ~44, workspaces/user_workspaces ~37, view/vertical_tabs ~35, view/right_panel ~34,
view/pane_impl ~27, terminal/session_settings ~27, + long tail. GENUINELY DONE (real, verified):
events.rs 178->0 (Phase E telemetry nuke), terminal_pane.rs 119->0, root_view.rs 49->0,
SerializedBlockListItem recreated, WorkspaceAction cascade, Workspace struct field linchpin.
TO RE-BASELINE HONESTLY (next session, PENDING USER OK on the never-commit-higher rule):
complete the command_search-view removal in view.rs (delete fns show_command_search +
handle_command_search_event by BRACE-MATCHED range [verified: count braces only AFTER first '{',
skip sig parens; delbyname's multi-name pass shifts line numbers & leaves orphan '}' — use a
single-pass reverse-order range deleter], remove field `command_search_view: ViewHandle<CommandSearchView>`,
its ctor let+subscribe block, the `command_search_view,` Self line, and the
`if self.current_workspace_state.is_command_search_open {...}` render block). That compiles cleanly
to ~859 REAL errors -> then grind file-by-file (Edits NOT scripts on the fragile view.rs hub).
HARD-RULE CONFLICT: unmasking 20->859 violates "never commit higher than prior" — the masked 20 was
never a real floor. Recommend re-baselining to the honest count and applying strict-decrease FORWARD.
TOOLING: /tmp/delbyname.py is UNSAFE for multiple names at once (line-shift bug) and can leave orphan
braces — prefer single-pass reverse-range deletion with the brace-after-first-'{' matcher.

WINDOW 11 done (MASSIVE drop 1179->38). Cleared terminal_pane.rs (119->0: agent free-fns + attach/detach
subscription gut + snapshot rewrite + recreated SerializedBlockListItem enum Command-only), root_view.rs
(49->0: agent free-fns/action-registrations/NewWorkspaceSource agent variants/CloudPreferencesSyncer),
recreated SerializedBlockListItem (block-restore pipeline KEEP type, was deleted) in serialized_block.rs +
wired imports. PHASE E DONE: events.rs 178->0 — TelemetryEvent macros are FULLY no-op (=>{{}}, args
discarded/never type-checked) so removed 77 agent variants + agent metadata structs/enums/impls
(CloudObject/Space/Workflow/Notebook/EnvVar/MCP/NotificationAgentVariant/AIAgentInput/QueuedQueryOrigin)
with ZERO call-site breakage. BREAKTHROUGH: workspace/view.rs hub had dead imports (crate::ai, crate::code::*,
pane_group agent-panes, command_search::view, handoff) POISONING crate-wide symbol resolution — clearing them
collapsed 813->38. Remaining 38 = scattered dead imports, mostly the `code::` IDE-strip cluster
(code::editor/global_buffer_model/buffer_location/editor_management/file_tree refs in auth/mod, lib.rs,
server/server_api+network_log_view, remote_server/*, pane_group/working_directories) + leftover agent type
imports (AgentInteractionMetadata, EnvVar/WorkflowTelemetryMetadata, Viewer, InputConfig, command_search::view
CommandSearchEvent/View which is FULLY DELETED so workspace command_search_view field+handler+show must go,
conversation_list view, onboarding_agentic_suggestions_block). NOTE: CommandSearchView/Event no longer exist
(search/command_search has no view module). LocalOrRemotePath re-homed crate::code->rift_util::local_or_remote_path.
Next: clear the 38 import errors (watch for use-site exposure), then 0 errors, then 256 warnings.

WINDOW 10 done (WorkspaceAction cascade + workspace/view.rs + Workspace struct linchpin, 1387->1179):
excised 102 agent variants from WorkspaceAction enum (action.rs) + rewrote From/blocked_for_anonymous_user/
should_save_app_state_on_action keep-only + deleted ~1200 lines of handle_action arms in workspace/view.rs;
deleted ~25 pure-agent methods (file-tree/shared-session/drive/handoff/code-review/fork/restore-conversation/
warp-drive/env-vars/mcp/ai-warm-welcome); de-agented handle_palette_event + handle_command_search_event
(gutted workflow/notebook/drive/AI arms, kept history/dir); removed 14 agent view-handle fields from the
Workspace struct + their ctor let-bindings/Self-settings/factory+handler methods/render overlay blocks
(import/ai-assistant/workflow/agent-toolbar/suggested-workflow-rule/ai-fact/agent-mgmt/notification-mailbox/
handoff/auth-secret/rewind/delete-conversation modals). REMAINING workspace/view.rs (~70, mostly
error-SUPPRESSED self.<removed-field> uses in focus methods 3640-3963 + notification render + a few
methods — clearing them un-suppresses then drops the count). TOOLING LESSON: balanced-delete scripts
(enum-variant / match-arm / let-statement removers) work well BUT (a) start at the statement's FIRST line
not a continuation line (else they over-run — ate FileModel in lib.rs + adjacent agent methods in
workspace/view.rs), (b) derive(Debug) enums need no manual-Debug fix. Next: finish workspace/view.rs
field-use tail, then pane_group/pane/terminal_pane.rs (119), root_view (49), panel enum-definers
(right_panel 34/vertical_tabs 32/left_panel 12), then Phase F wholesale (server/+auth/+events.rs 178
TelemetryEvent enum — note PaletteSource/AgentModeEntrypoint in server/telemetry are used by KEEP code,
relocate or keep), then C/D/G. Also the `crate::code::` IDE-strip surfaces across auth/remote_server/server.

WINDOW 9 done (all AI removed + lib.rs boot inits, 1456->1387): deleted app/src/ai + rift_ai dep
+ editor/input AI-autosuggestion (kept completer-based fallback); de-agented input.rs
(handle_editor_event AI-input-detection/context-menu, agent accessor methods, dead imports);
removed lib.rs agent boot-inits (blocklist/drive/ai_assistant/env_vars/workflows/voltron/
agent-todos + EnvVarCollectionManager/WorkflowManager/AITipModel/CodebaseIndexManager/
ProjectContextModel/AIExecutionProfiles/shared_session/BonusGrant singletons) + ::ai::index
import + determine_agent_source/daemon_codebase_index_snapshot_storage fns. Accurate top files
now: workspace/view.rs 238, server/telemetry/events.rs 178 (Phase F), pane_group/pane/
terminal_pane.rs 119, terminal/input.rs ~45, terminal/view.rs ~56, root_view 49. Remaining input.rs
needs input_enter/handle_action de-agent (entangled w/ cli_agent_sessions+inline menus) + the
lib.rs `code::` IDE-strip imports. NOTE delbyname-style balanced deletes BREAK on multi-line
statements whose marker is on a continuation line (ate FileModel + left orphan `add_singleton_model(`
in lib.rs — hand-fixed); start balanced deletes at the STATEMENT's first line.

To pick up locally:
`cd /Users/fimbulwinter/dev/rift && git fetch origin plan2-strip && git checkout plan2-strip`
(needs `protoc`: `brew install protobuf`). Pairs with local `project_rift.md` memory.
Iterate with `cargo check --bin rift-oss > /tmp/rb.log 2>&1; grep -c '^error' /tmp/rb.log`.
Recreate `/tmp/delbyname.py` from the helper at the top of window-7 if the container is fresh
(it was recreated this window).

## TELEMETRY — macro no-op ALREADY DONE (Phase E macros complete)
Verified this window: the 4 telemetry macros are ALREADY no-op'd —
`send_telemetry_from_ctx!`/`send_telemetry_from_app_ctx!` in `crates/rift_core/src/telemetry.rs`
(L150/L161) and `send_telemetry_sync_from_ctx!`/`send_telemetry_sync_from_app_ctx!`/
`send_telemetry_on_executor!` in `app/src/server/telemetry/macros.rs`. So all ~67 call sites
compile away (tokens discarded, not type-checked). Phase E's REMAINING work is only: (a) the
`AuthState` shim, and (b) Phase F deletion of `server/telemetry/events.rs` (the `TelemetryEvent`
enum DEFINITION — 176 errors there are its variant payloads referencing deleted types; leave
until the server delete). Do NOT delete the 67 call sites.

## WINDOW 8b — what's DONE (TerminalView struct/ctor/Event-enum/agent-methods linchpin; 1663 → 1479, −184)
The window-5 "linchpin" is now largely DONE. All committed + pushed, each a verified drop.
- **Struct + ctor de-agent (`0fcbfca1`, 1647→1612)**: removed remaining agent fields from the
  `TerminalView` struct def + `Self {}` block (scroll_position_before_entering_agent_view,
  pending_user_query*/queued_prompt_callback/usage_footer_view_ids/onboarding_agentic*/
  auto_stop_sharing/conversation_ended_tombstone/is_todo_popup_visible/conversation_completed_callbacks/
  agent_view_back_button/orchestration/conversation_details_panel[the broken-type field that was
  tainting the struct + suppressing E0560]/ambient_agent_cancel/env-setup/cloud-mode-start/aws-login/
  viewer-driven-size + Self-only extras ai_*/agent_view_controller/ambient/cli_subagent/use_agent_footer/
  agent_todos/ephemeral). Deleted the orphaned ctor agent subscriptions (the 310-line
  `agent_view_controller` block, ai_controller, ai_status_bar, ai_context/input/action, CLIAgentSessions,
  executors, ambient) + agent let-bindings. NOTE the ctor takes NO agent params (trimmed earlier) — the
  agent vars were all undefined (E0425), so the subscriptions were orphaned.
- **Event enum (`4cf70ce7`, 1612→1580)**: trimmed view.rs `Event` to terminal-only (dropped
  AskAIAssistant/OpenWorkflowModal*/Drive/AI-doc/agent-toolbar/code-review/sharing/role/MCP/
  agent-profile/cloud-capacity/StartAgentConversation/child-agents/ShareModalOpened/AnonymousUserSignup);
  deleted `ExecuteAIRequestedCommandEvent` struct.
- **Agent accessor/helper methods (`1546`+`1509`)**: deleted ~60 pure-agent methods via
  `/tmp/delbyname.py` (ai_controller/ai_context_model/ai_input_model/agent_view_controller/ambient
  accessors; rewind-dialog/fork/cloud-mode callbacks; codebase-index/cloud-env-setup/agent-mode-setup/
  code-diff/ai-block/init-environment/env-var-collection/exchange-scroll helpers; git-status cluster;
  telemetry-banner/onboarding-callout/lsp/file-tree/input-config).
- **More ctor residuals (`1493`)**: fixed the ctor `Input::new` CALL (dropped 8 agent args to match the
  de-agented `Input::new` signature in input.rs L1798 — current params: model, tips_completed, server_api,
  sessions, size_info, menu_positioning_provider, current_prompt, terminal_view_id, current_repo_path,
  model_events, active_session, ctx); removed AgentConversationsModel/cli_subagent_controller/
  maa_passive_suggestions/legacy_passive_suggestions subscriptions.
- **Caller fixups (`3f16a99c`, 1479)**: dropped calls to deleted update_git_status_subscription/
  hide_telemetry_banner_permanently; `HideTelemetryBannerPermanently` handle_action arm → `{}`.

## WINDOW 8b — REMAINING view.rs (95 errors, the long tail + 3 dispatch matches)
- **`handle_input_event` (15)** — the **InputEvent cascade**: `terminal/input.rs` `pub enum Event`
  (L846-980) has agent variants (ExecuteAIQuery/SendAgentPrompt/SubmitCloudFollowup/
  CancelSharedSessionConversation/UnhandledCmdEnter/CtrlEnter/SignupAnonymousUser/OpenCodeInWarp/
  OpenCodeReviewPane/AttachDiffSetContext/OpenConversationHistory/OpenViewMCPPane/OpenAddMCPPane/
  OpenProjectRulesPane/OpenEnvironmentManagementPane/OpenFilesPalette/TryHandlePassiveCodeDiff/
  ToggleAIDocumentPane/SubmitCLIAgentInput/OpenAIDocumentPane/OpenAutoReloadModal/AuthSecretDelete/
  EnterAgentView/EnterCloudAgentView/CreateDockerSandbox/ExitCloudMode/ScrollToExchange/
  TriggerEnvironmentSetup/RegisterPluginListener/OpenPluginInstructionsPane/OpenShareSessionModal/
  StartRemoteControl/OpenHandoff*/OpenCloudModeV2*). KEEP terminal Events (Autosuggestion/Clear*/Page*/
  SelectRecentBlocks/Copy/UnhandledModifierKey/ClearSelectionsWhenShellMode/InputStateChanged/
  InputEmptyStateChanged/Escape/SyncInput/ShowCommandSearch/CtrlD/CtrlC/Enter/ExecuteCommand/
  EmacsBindingUsed/InputFocusedFromMiddleClick/EditorFocused/OpenSettings/ShowToast). Trim the enum +
  remove the matching `handle_input_event` arms in view.rs TOGETHER. Also `InputAction` (L988+) has
  agent variants. This unblocks a big chunk of input.rs's 128 errors too.
- **`handle_terminal_event` (9)** — the **ModelEvent cascade** (`terminal/model_events.rs`): agent
  ModelEvent variants (AgentViewEntryOrigin etc.). Trim ModelEvent + remove arms together.
- **`render` (7)** + **`keymap_context` (4)**: agent render branches / keymap flags — edit in place
  (drop agent_view_controller.is_fullscreen / use_agent_footer / ambient branches; agent keymap flags).
- Mixed 1-ref methods (edit in place, drop the agent branch): handle_ctrl_c_input_event,
  on_user_block_completed, handle_session_bootstrapped, context_menu_items, ctrl_c_to_active_block,
  is_input_box_visible, viewport_state, apply_block_metadata_update, render_remote_server_loading_footer
  (missing fn `shimmering_warp_loading_text` — renamed?), begin_block_text_selection, clear_buffer,
  session_command_context, render_alt_screen_element (`Viewer` struct), drop.
- Dead IMPORTS (top of file, lines ~53/56/213/306/339/402: block_onboarding::onboarding_agentic*/
  drive_sharing, CodeReviewPanelArg, AgentInteractionMetadata, PromptSuggestionBannerState,
  AIAgentActionResultType/AIRequestUsageModel) — remove LAST, after their use-sites are gone.

## WINDOW 8 — what's DONE (ContextMenuAction + TerminalAction enum cascade; 1736 → 1663, −73)
All committed + pushed on `plan2-strip`, each a verified net error drop.
1. **ContextMenuAction / InputContextMenuAction cluster (commit `3d924b13`, 1737→1710)**
   `terminal/view.rs`: dropped agent variants from both enums + their `impl Debug` arms
   (AskAI/OpenWorkflowModal/OpenShareBlockModal/sharing/AI-block-copy/fork/rewind/debugging-link/
   EditAgentToolbar/EditCLIAgentToolbar + AskAISource enum; ShowAICommandSearch/AskWarpAI/
   SaveAsWorkflow). De-wired the menu-builder construction sites (block/text/input/alt-screen
   context-menu builders — removed the AI/sharing/Drive `if`-gated item blocks + the
   `RichContentBlockRightClick` AI-block menu section + `prompt_context_menu_items` agent-toolbar
   branches), the handlers `context_menu_action` + `handle_input_context_menu_action` (keep-only),
   and `terminal/view/init.rs` ask_ai keybinding registrations.
2. **TerminalAction enum (commit `d9d9e3c1`, 1710→1669)** `terminal/view/action.rs`: rewrote the
   `TerminalAction` enum + `impl Debug` to terminal-only (~75 agent/cloud/sharing/workflow/code/
   MCP variants removed). De-wired `terminal/view.rs` `action_accessibility_contents` (the
   `|`-chains + dedicated AI arms) and `handle_action` (deleted ~75 agent arms incl. the big runs
   AI-block-menu / SetInputMode* / DeleteAttachment..StartNewAgentConversation / child-agents).
   **OnboardingFlow + ShowInitializationBlock kept as variants with NO-OP `handle_action` arms**
   (pending struct/ctor cleanup — their bodies were agent: add_agentic_suggestions_block /
   start_agent_onboarding_tutorial / show_initialization_block). KEPT terminal variants
   (Scroll/Block*/Copy*/Split*/Paste/Find/Vim/SSH/Subshell/Warpify/MiddleClick/DragDrop/
   ToggleBlockFilter/ToggleSnackbar/ToggleSessionRecording/OpenInlineHistoryMenu/HideTelemetryBanner/
   ImportSettings/ShowWarpifySettings/OnboardingFlow[no-op]/ShowInitializationBlock[no-op]).
3. **TerminalAction keybindings (commit `a3d400ce`, 1669→1663)** `terminal/view/init.rs`: removed
   all agent keybinding registrations (Resume/Fork/ToggleAIDocumentPane/SetInputMode*/
   ToggleCLIAgentRichInput/ResolvePromptSuggestion/JumpToLatestAgentMessage/OpenWorkflowModal/
   OpenShareModal/share-session/autoexecute/queue/codebase-index/agentic-suggestion/
   WriteCodebaseIndex/LoadAgentModeConversation/InitProject/AddProject/ConversationDetails/
   EnterCloudAgentView); emptied `register_input_mode_bindings` to a no-op.

## WINDOW 8 — REMAINING TerminalAction dispatch sites (≈46, NOT isolated — embedded in OTHER cascades)
These still reference removed TerminalAction variants but are NOT standalone — they live inside
agent subtrees that are their own cascades and will clear when those are removed. DO NOT
one-line-patch them (entangled). Locations:
- `terminal/view.rs` `handle_input_event` (~L16418-16445): `InputEvent::{OpenProjectRulesPane,
  OpenViewMCPPane,OpenAddMCPPane,...}` arms call removed TerminalAction — part of the **InputEvent
  enum cascade** (InputEvent defined in `input.rs`; de-agent InputEvent + these arms together).
  Also `terminal/view.rs:3676` `TerminalAction::ExitAgentView` (in the agent ctor block).
- `terminal/input/slash_commands/mod.rs` (12) + `data_source/mod.rs` (25): agent slash commands
  (/add-mcp,/add-prompt,/add-rule,/conversations,/index,/open-rules,/open-project-rules,/edit-skill,
  /summarize,/agent,/new,/cloud-agent,/create-docker-sandbox). The whole agent-slash-command set
  (incl. `search/slash_command_menu/static_commands` `commands::*` defs) should be removed; the file
  also references deleted Input fields (ai_context_model/ephemeral_message_model/agent_view_controller/
  suggestions_mode_model/cloud_mode_v2_history_menu_view) + deleted Events (EnterAgentView/
  EnterCloudAgentView/CreateDockerSandbox). Do with the Input/Event cleanup.
- Agent banner files — candidates for WHOLESALE deletion (delete file + mod decl + inline_banner
  render dispatch): `terminal/view/inline_banner/{aws_bedrock_login,aws_cli_not_installed,
  anonymous_user_ai_sign_up,agent_mode_setup}.rs`, `terminal/view/zero_state_block.rs`.
- Isolated keep-file singletons (can patch when convenient): `workspace/view/right_panel.rs:1803`
  (PickRepoToOpen), `pane_group/pane/view/header/mod.rs:416` (DismissCodeToolbeltTooltip),
  `terminal/view/context_menu.rs:458` (RewindAIConversation), `terminal/view/pane_impl.rs` (4),
  `terminal/block_list_element.rs` (6), `terminal/alt_screen/alt_screen_element.rs` (1),
  `workspace/view.rs:21300` (ExecuteRewindAIConversation), `terminal/view_tests.rs` (2).

## WINDOW 8 — NEXT BIG CLUSTERS (in suggested order)
1. **view.rs `Event` enum** (def ~L1359-1734 pre-window-8; now shifted up ~130 lines): remove agent
   variants (AskAIAssistant/OpenWorkflowModal*/OpenWarpDriveObjectInPane/AI-doc/CodeReview/sharing/
   role/MCP/agent-profile/cloud-capacity/StartAgentConversation/child-agents/EnterAgentView/
   EnterCloudAgentView/CreateDockerSandbox/etc.) TOGETHER with their `ctx.emit(Event::X)` sites
   (scattered in view.rs methods + workspace/view.rs handle_event) and the handle_event match arms.
   KEEP terminal Events (AppStateChanged/Escape/Exited/BlockListCleared/SendNotification/
   BlockCompleted/Pane/OpenSettings/SyncInput/ShowCommandSearch/CtrlD/ShutdownPty/WriteBytesToPty/
   Resize/ExecuteCommand/BlockStarted/CloseRequested/FocusSession/Onboarding*Completed/
   Selected*Changed/PendingCommandCompleted/SessionBootstrapped/ShellSpawned/file-upload/
   RunNativeShellCompletions/RemoteServer*/OpenThemeChooser/ToggleLeftPanel/SlowBootstrap/ShowToast/
   PluggableNotification + OpenFileInWarp[verify]).
2. **TerminalView struct + ctor (the window-5 CORRECTION linchpin, STILL pending)**: the struct
   (~L2294) still has ~30 agent fields (ai_controller/ai_context_model/ai_input_model/
   ai_action_model/agent_view_controller/ambient_agent_view_model/cli_subagent_controller/
   use_agent_footer/agent_todos_popup/onboarding_agentic_suggestions_block/pty_recorder[KEEP]/
   is_conversation_details_panel_open/is_todo_popup_visible/etc.) referenced by ~40 agent methods +
   the ~1100-line ctor + the `render` method (also agent-laden: agent_view_controller.is_fullscreen,
   use_agent_footer, ambient_agent_view_model, etc.). This is the from-scratch struct+ctor+render
   rewrite. Removing the fields will surface/clear hundreds of method-body errors.
3. **InputEvent cascade** (input.rs enum + view.rs handle_input_event) — see remaining-dispatch above.
4. Then workspace/view.rs (252) + workspace/action.rs (26) WorkspaceAction + left/right_panel/
   vertical_tabs enum-definers + pane_group/pane/terminal_pane.rs (119) — the rest of the contract
   cluster. Then Phase F (server/auth/events.rs) etc.

Error distribution at window-8 handoff (1663): terminal/view.rs 264, workspace/view.rs 252,
server/telemetry/events.rs 176 (Phase F), terminal/input.rs 128, pane_group/pane/terminal_pane.rs
119, root_view.rs 50, lib.rs 46, workspaces/user_workspaces.rs 37 (Phase C), right_panel 34,
vertical_tabs 33, slash_commands/mod 28, view/pane_impl 27, session_settings 27, gql_convert 26
(Phase C), workspace/action 26, slash_commands/data_source 25, server/graphql 23 (Phase G),
view/rich_content 21, pane_group/working_directories 21.

**DO NOT USE SUBAGENTS from here on.** The remaining work is the tightly-interconnected
contract cluster (Event/action enum cascade across `terminal/view.rs` ↔ `workspace/view.rs` ↔
`pane_group/pane/terminal_pane.rs` ↔ action/enum-definer files — removing one enum variant
means removing all its match arms across every file *in one pass*) plus the wholesale
`server/`+`auth/`+`workspaces/` deletion (needs the shims below). These cannot be safely
parallelized; work INLINE, compiler-driven, one coherent pass. SPEED TIP: iterate with
`cargo check --bin rift-oss` (skips codegen/linking; same errors), only `cargo build` for the
final green check. Tool: `/tmp/delbyname.py` (fixed: ignores braces in //-comments & strings)
— recreate from git if the container is fresh.

## TELEMETRY — full removal (NOT done yet; Phase E + F)
Currently telemetry is still WIRED but runtime-OFF in OSS (`oss.rs` has
`telemetry_config: None` + `crash_reporting_config: None`, so nothing reaches Rudderstack).
Code is NOT yet removed. To fully nuke it (no code, no events, nothing leaves the machine):
- **Phase E (sanctioned `rift_core` edit):** replace the telemetry macro DEFINITIONS in
  `crates/rift_core/src/telemetry.rs` (`send_telemetry_from_ctx!`, `send_telemetry_from_app_ctx!`,
  `send_telemetry_sync_from_app_ctx!`, `send_telemetry_on_executor!`) with NO-OPs (keep
  names/paths/arities) so all ~67 remaining call sites in `app/src` compile away to nothing.
  This is the ONLY allowed edit to the guarded rift_core macros. Do NOT delete the 67 call sites.
- **Phase F (with the server delete):** delete `server/telemetry/` (`TelemetryCollector`,
  `AppTelemetryContextProvider`), remove the `lib.rs` boot inits (`AppTelemetryContextProvider::
  new_context_provider` ~L1096, `TelemetryCollector::new(server_api)` ~L1530, the
  `TelemetryCollector::handle` close-flush ~L1913) and the `TelemetryEvent::*` emissions
  (AppStartup ~L1401, LoggedOutStartup ~L1434, UserInitiatedClose ~L1971/2003); drop the
  `TelemetryEvent` + `SettingsTelemetryEvent` enums. Result: zero telemetry.

## WINDOW 7 — what's DONE (inline_history + workflows-structs + editor AI-menu + context_chips; 1815 → 1736, −79)
All committed + pushed on `plan2-strip`, each step a verified net error reduction.
1. **inline_history 3-file cluster (DONE)** `terminal/input/inline_history/{data_source,search_item,view}.rs`
   + `input/inline_menu/{view,message_bar}.rs`: dropped `agent_view_controller` field/param +
   subscriptions from InlineMenuView::new/new_with_tabs/new_inner + InlineMenuMessageBar(+Args);
   removed `AcceptHistoryItem::Conversation`/`AIConversationId`/`ConversationStatus` thread
   (the Conversation variant, conversation()/build_conversation_entries/interleave_conversations/
   build_agent_view_results, MenuItem::Conversation, render_status_element, name_match_result, the
   Conversations QueryFilter source, NavigateToConversation/SelectConversation events,
   HistoryItemIdentity::Conversation). Defined a local `STATUS_ELEMENT_PADDING` const (was from
   deleted ai::conversation_status_ui). Also fixed pre-existing `agent_view_bg_color` ref in
   `input/message_bar/common.rs`.
2. **Dropped-workflows structs in input.rs (DONE)**: removed `WorkflowsState`/`SelectedWorkflowState`/
   `CommandMatchesWorkflowTemplate`/`EnvVarCollectionState` + the `workflows_state`/
   `env_var_collection_state` fields + ctor bindings + all usages. Deleted the pure-workflow helper
   methods `get_text_style_ranges_for_workflow`/`build_text_run_ranges_for_workflows`/
   `get_current_argument`; hand-edited `input_shift_tab` (dropped workflow-arg branch),
   `input_enter`/`handle_editor_event`/key-context (dropped workflow blocks), and the
   `start_block_and_write_command_to_pty` telemetry block (now emits ExecuteCommandEvent with
   workflow_id/command = None). NOTE `crate::server::ids::SyncId` import in input.rs now unused
   (warning only — clean in import sweep). `input_enter` STILL has a `WorkflowAliases`/`CloudModel`/
   `insert_workflow_into_input`/`WorkflowSelectionSource::Alias` block (the `# WorkflowAliases` flag
   path) + `submit_ai_query`/`ai_input_model`/`ai_controller` agent bits — deferred to the
   input.rs core-method pass (task below).
3. **editor AI-context-menu de-wire (DONE)** `editor/view/mod.rs`: removed `with_context_model`
   (deleted BlocklistAIContextModel), `ai_context_menu()`, `render_ai_context_menu()`,
   `render_at_context_menu_button()`, `set_is_ai_input`, the `include_ai_context_menu` EditorOption
   (+2 default setters), the `AIContextMenuState` struct, the `context_model`/`ai_context_menu_state`/
   `is_ai_input` EditorView fields + Self entries + ctor block, the EditorAction::SetAIContextMenuOpen
   variant+arm, the Event::{SetAIContextMenuOpen,AcceptAIContextMenuItem,SelectAIContextMenuCategory}
   variants, the search::ai_context_menu imports, the at-context-menu render. ALSO de-agented (these
   were among the editor's ~13 pre-existing errors, all genuine agent code not the KEEP autosuggestion
   path): `handle_ctrl_c` (dropped is_agent_responding/is_pending_passive_ai_block/BlocklistAIHistoryModel
   checks), `process_and_attach_images_as_ai_context` future (dropped ImageContext construction →
   counts only), `process_non_image_files` (no-op), `is_ai_input` keymap flag. **Re-homed
   `InputType` import to `input_classifier::InputType`** (was from deleted ai::blocklist) — keeps the
   KEEP autosuggestion path (matches_input_type / maybe_populate_intelligent_autosuggestion) working.
   NOTE: input.rs has MANY now-dangling callers of the removed editor methods (`editor.ai_context_menu()`
   ~10 sites, EditorEvent::SetAIContextMenuOpen/SelectAIContextMenuCategory/AcceptAIContextMenuItem
   handlers, set_is_ai_input) + input/common.rs:302 `render_ai_context_menu` — these are agent code,
   clean them in the input.rs core-method pass.
4. **context_chips de-wire (DONE)** `context_chips/{display,display_chip}.rs`: dropped agent params/
   fields from `PromptDisplay::new` (ai_input_model/ai_context_model/agent_view_controller/
   is_shared_session_viewer→now default false) and `DisplayChipConfig` (ai_input_model/ai_context_model/
   agent_view_controller/ambient_agent_view_model). Removed `DisplayChip::new_for_agent_view`, the
   `AgentPlanAndTodoList` DisplayChipKind variant + all arms (PlanAndTodoListView is deleted), the
   code-review chip click (ToggleCodeReview action + supports_code_review branch → diff-stats chip now
   non-interactive), the ambient-agent directory-chip gating, and the agent
   PromptDisplay(Chip)Event variants (OpenCodeReview/OpenConversationHistory/RunAgentQuery/
   OpenAIDocument). Kept prompt rendering: cwd/git-branch/git-diff-stats/node-version/ssh/subshell/
   venv chips. NodeVersion InstallNvm now runs the nvm curl-install command directly (was RunAgentQuery).
   `is_in_agent_view`/`is_shared_session_viewer` kept as plain bool fields = false (not agent types,
   read by ~40 render branches — keeping them avoids a render refactor). Removed
   `GitLineChanges::from_diff_stats` (deleted DiffStats) + fixed its caller
   `terminal/view/tab_metadata.rs:current_diff_line_changes` (from_model path → None; the
   git_status_metadata/GitStatusMetadata/stats_against_head/DiffStats cluster is STILL broken —
   GitRepoStatusModel re-homing, separate task).

## WINDOW 7 — REMAINING (the big Event/action-enum cascade — task 5, NOT started)
This is the multi-thousand-line interlocked agent web. Error distribution (of 1736): terminal/view.rs
307, workspace/view.rs 252, terminal/input.rs 128, pane_group/pane/terminal_pane.rs 119, root_view.rs
50, lib.rs 46, workspace/view/right_panel.rs 34, vertical_tabs.rs 33, input/slash_commands 28+25,
view/pane_impl.rs 27, session_settings.rs 27, workspace/action.rs 26, view/action.rs 24,
view/rich_content.rs 21, pane_group/working_directories.rs 21, pane_group/mod.rs 18,
model/terminal_model.rs 17. **Top error TYPES** (deleted tokens referenced pervasively in METHOD
BODIES, not just enum variants): AIConversationId(93), AgentViewEntryOrigin(69),
BlocklistAIHistoryModel(51), AgentToolbarItemKind(30), BlocklistAIHistoryEvent(26),
AmbientAgentTaskId(25), Space(24), DriveObjectType(21), AIAgentExchangeId(20), SharedSessionStatus(19),
CloudModel(19), InputTypeAutoDetectionSource(18), FullAIAgentInput(18), ConversationStatus(15),
ServerOutputId(14), InputType(13), WarpDriveItemId(12). DO THIS AS ONE COORDINATED PASS:
- `terminal/view/action.rs` `TerminalAction` enum (~93-460): drop agent variants (JumpToLatestAgentMessage,
  OpenAIBlock*Menu, Rewind*AIConversation, ExecuteRewind*, SelectAIAttachedBlock, OpenWorkflowModal*,
  AskAIAssistant, SetInputMode{Agent,Terminal}, StopSharing*/OpenSharedSession*/CopySharedSession*/
  MakeAllParticipantsReaders/Request*Role, Generate/WriteCodebaseIndex, LoadAgentModeConversation,
  Toggle{Autoexecute,QueueNextPrompt,AIDocumentPane,Todo,CodeReviewPane,ConversationDetailsPanel,
  UsageFooter,LongRunning,HideCliResponses,SessionRecording,CLIAgentRichInput}, Resume/Summarize/Fork/
  Init/IndexProject*/AddProject/Open{Project,View/Add}MCP/Open*Pane/OpenConversationsPalette/Setup*Cloud*/
  TriggerEnvironment*/Enter/Exit/StartNew*AgentView/Cancel/Reveal/Switch/Open/Stop/KillAgent*/
  ResolvePromptSuggestion/Anonymous*/Aws*Banner/CodebaseIndex*/AgentModeSetup* etc.) AND their
  `impl Debug` arms (same file) AND every `handle_action` arm in `terminal/view.rs`. KEEP terminal
  variants (Scroll/Block*/Copy*/Split*/Paste/Find/Vim/SSH/Subshell/Warpify/etc.).
- `terminal/view.rs` Event enum + 31 agent struct fields/Self + handle_action/handle_input_event/
  handle_terminal_event agent arms (the `## ⚠️ CORRECTION` window-5 section below still applies —
  the struct + ctor are still agent-laden).
- `workspace/view.rs` handle_action arms + `workspace/action.rs` WorkspaceAction agent variants
  (~398-700) + `workspace/view/{left_panel,right_panel,vertical_tabs}.rs` enum-definers
  (ToolPanelView/LeftPanelEvent/RightPanelEvent) — remove each variant + ALL arms across the cluster
  TOGETHER. `pane_group/pane/terminal_pane.rs` snapshot must match trimmed TerminalPaneSnapshot.
- TIP: many AIConversationId/AIAgentExchangeId/ConversationStatus refs live in enum-variant payloads
  AND method bodies; removing the variant clears the body arms. Re-home `InputType` to
  `input_classifier::InputType` wherever it appears (precedent set in editor/view/mod.rs this window).
- DO NOT touch server/auth/workspaces(plural)/pricing/autoupdate — their errors (events.rs 176,
  user_workspaces 37, gql_convert 26, server_api 19, graphql 23, auth 16, ~290 total) are Phase C/F.

## WINDOW 6 — what's DONE (Input ctor/struct de-agented; 1987 → 1815, −172)
The window-5 note claimed `terminal/view.rs` ctor was already de-fielded but framed the
`Input::new` work as a "straggler trim." **CORRECTION: it was NOT a straggler trim — it was a
~1450-line full ctor rewrite plus a multi-file contract cluster.** What I did this window
(all committed + pushed on `plan2-strip`, each step a net error reduction):
- **`terminal/input.rs` `Input` struct**: removed agent fields (ai_controller/ai_context_model/
  ai_input_model/ai_action_model/ai_follow_up_icon_mouse_state, shared_session_input_state/
  presence_manager, latest_buffer_operations, deferred_remote_operations, prompt_suggestions_*,
  has_prompt_suggestion_banner, predict_am_queries_future_handle, debounce_ai_query_prediction_tx,
  cached_agent_mode_hint_text, universal_developer_input_button_bar [UDI struct is DELETED],
  agent_input_footer, prompt_suggestions_view, handoff_compose_state, all inline_* agent menus
  [conversation/plan/repos/model/profile/skill/prompts/user_query/rewind/history-as-cloud],
  slash_command_data_source/model, cloud_mode_v2_*, buy_credits_banner, agent_status_view,
  queued_prompts_panel, agent_view_controller, agent_shortcut_view_model, ambient_agent_view_state,
  ephemeral_message_model, voltron_view/is_voltron_open). Deleted the `AmbientAgentViewState`
  struct+impl.
- **`Input::new` signature**: dropped 8 agent params (ai_controller/ai_context_model/ai_input_model/
  ai_action_model/cli_subagent_controller/agent_view_controller/ambient_agent_view_model/
  ephemeral_message_model). REWROTE the ctor body: de-agented the `editor` EditorOptions block
  (dropped AI cursor colors → default_cursor_colors, AI decorator/ai_input_indicator, agent
  keymap flags [AGENT_VIEW_ENABLED/CTRL_ENTER_ENTERS_AGENT_VIEW/CLI_AGENT_RICH_INPUT_OPEN/
  CTRL_ENTER_ACCEPTS_PROMPT_SUGGESTION], `with_context_model`, set `include_ai_context_menu:false`);
  deleted all agent subscription blocks (agent_view_controller, ambient, ai_controller,
  ai_input_model, BlocklistAIHistoryModel, ai_context_model, LLMPreferences, CLIAgentSessionsModel)
  and all inline-menu/slash-command/prompt-suggestions/agent-status-bar/queued-prompts/buy-credits/
  ai-req-usage let-bindings; removed the workflows/voltron/predict_am_queries machinery.
- **Deleted ~70 pure-agent methods** from input.rs via `/tmp/delbyname.py` (handoff/conversation/
  queued-prompts/shared-session/ambient/inline-menu accessors/workflow/env-var/voltron-event/
  predict_am_query). NOTE: delbyname.py's brace matcher MISCOUNTS when a method body has a `{`
  inside a `//` comment (it nuked the rest of the file twice on `command_matches_workflow_template`
  which has `// if let ... {`). I hand-deleted that one. ALWAYS check the printed line ranges; if
  any range is absurdly large, `git checkout` and hand-delete that method.
- **Helper files de-wired (part of the cluster, all done)**: `prompt_render_helper.rs` (dropped
  ai_input_model field/param + simplified should_render_prompt_using_editor_decorator_elements);
  `input/inline_menu/positioning.rs` (dropped agent_view_controller from InlineMenuPositioner::new
  +field+the is_active branches); `input/terminal_message_bar.rs` (dropped ai_input_model/
  context_model; gutted the agent message producers + transformers + TerminalMessageArgs +
  message_magenta; render now shows ONLY the inline-history hint, else Empty).

## WHAT REMAINS in the Input cluster (next window — these are why input.rs still has ~137 errs)
The Input STRUCT/ctor/Self are now de-agented, but residual errors are in CORE methods that
still reference deleted tokens, PLUS a few helper-view ctors I could not finish (they're each
their own deep agent subtree). In priority order:
1. **`input/inline_history/` cluster** (BLOCKS the ctor's `inline_history_menu_view` call which
   still passes `agent_view_controller`): strip agent_view_controller from
   `inline_history/view.rs` (InlineHistoryMenuView::new/new_with_tab_configs/new_inner — pass
   `false` to build_tab_configs, drop the agent_view_controller subscription),
   `inline_history/data_source.rs` (drop agent_view_controller field/param AND the
   `AcceptHistoryItem::Conversation { conversation_id: AIConversationId }` variant + its arms +
   the Conversations QueryFilter source), and `input/inline_menu/view.rs` InlineMenuView::
   new_with_tabs (drop agent_view_controller field/param + its subscription). This is a coupled
   3-file change; `AcceptHistoryItem::Conversation`/`AIConversationId` thread through all three.
2. **`crate::editor` de-wire** (the note's "NEVER touch editor" means don't RESTRUCTURE — but
   editor ALREADY has ~13 errors from deleted AI types): `editor/view/mod.rs` `with_context_model`
   (takes deleted BlocklistAIContextModel), `ai_context_menu()`, `set_is_ai_input`,
   `include_ai_context_menu` option, `maybe_populate_intelligent_autosuggestion` (takes InputType).
   These are referenced by the (now-removed) input ctor; clean them out of the editor.
3. **Dropped-workflows de-wire in input.rs**: `WorkflowsState`/`SelectedWorkflowState`/
   `CommandMatchesWorkflowTemplate`/`EnvVarCollectionState` structs (lines ~1144-1187) reference
   deleted workflow types (WorkflowsMoreInfoView/WorkflowArgumentIndex/WorkflowSource/WorkflowType/
   WorkflowDisplayData/EnumVariants/SyncId-for-env-vars). Remove these structs + the `workflows_state`/
   `env_var_collection_state` fields + their ctor bindings + all method refs.
4. **Core-method hand-edits** (incidental deleted-token refs; do NOT delete the whole method —
   edit in place): `handle_editor_event`, `input_enter`, `handle_action`, `menu_position`,
   `populate_enum_suggestions_menu`, `set_input_mode_terminal`, `buffer_contains_attachment_patterns`,
   `apply_external_input_config_update`, `as_str`/`Event` enum (846+ has agent variants —
   coordinate with view.rs handlers), `to_telemetry_mode`, `remove_excess_images`, `select_image`.
5. **Clean input.rs imports LAST** (lines ~28-256): many `use` of deleted modules (ai::skills,
   model::block::AgentInteractionMetadata, universal_developer_input, view::ambient_agent,
   view::inline_banner, view::queued_prompts_panel, code::editor_management, context_chips::
   display_chip::DisplayChipConfig, search::ai_context_menu, cli_agent_sessions,
   cloud_mode_v2_history_menu, conversations, models/plans/profiles/prompts/repos/rewind/skills/
   user_query, CodeDiffAction, CLIAgent, session_sharing_protocol). Remove these only AFTER the
   methods/bindings using them are gone, else you create "undeclared" errors.

## STILL-PENDING contract-cluster file: `context_chips/display.rs` + `display_chip.rs`
I simplified the `PromptDisplay::new` CALL in input.rs (now 7 args: current_prompt,
terminal_view_id, menu_positioning_provider, initial_session_context, current_repo_path,
model_events, ctx) but did NOT yet change `PromptDisplay::new`'s DEFINITION. So
`context_chips/display.rs` `PromptDisplay::new` still takes ai_input_model/ai_context_model/
agent_view_controller/is_shared_session_viewer — MUST be trimmed to match (drop those 4 params,
the struct fields, the AI/agent subscriptions, the agent chip kinds, the PromptDisplayEvent
agent variants OpenAIDocument/RunAgentQuery/OpenConversationHistory/OpenCodeReview). And
`display_chip.rs` `DisplayChipConfig` (lines ~437-448) has ai_input_model/ai_context_model/
agent_view_controller/ambient_agent_view_model fields woven through DisplayChip — de-wire jointly.
This is the NEXT thing to do after #1 above (PromptDisplay is the prompt renderer, terminal-essential).

---
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
4173 (baseline) → ... → 2254 (window 3) → 2081 (window 4) → 1987 (window 5) → 1815 (window 6) → **1736** (window 7).
~58% reduced. All checkpoints committed + pushed (RED intermediate commits are expected
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

## DELETED vs KEEP reference (for de-wiring) — UPDATED 2026-06-07 (all AI deleted)
DELETED (remove all refs): **`crate::ai::*` ENTIRELY** (no exceptions — `predict` + `block_context`
are NO LONGER kept) + **`rift_ai` crate**; `crate::code` (whole IDE);
`crate::{drive,workflows,notebooks,env_vars,cloud_object,ai_assistant}`;
agent block types (AgentViewVisibility, SerializedAgentViewVisibility, SerializedAIMetadata,
AgentInteractionMetadata); `terminal::shared_session`; `terminal::view::{ambient_agent,
conversation_list,queued_prompts_panel}`; inline_banner ZeroStatePromptSuggestion*/
PromptSuggestionBannerState; cloud-object id types (ObjectIdType/NotebookId/WorkflowId/…);
`remote_server::{codebase_index_model,diff_state_proto,diff_state_tracker}`;
the editor AI-autosuggestion hook + `InputType`/`InputTypeAutoDetectionSource`.
STILL EXISTS — DO NOT remove yet (deleted in later phases): `crate::server`, `crate::auth`,
`crate::workspaces` (plural=cloud teams), `crate::pricing`, `crate::autoupdate`.
NEVER TOUCH: `rift_core` macros (send_telemetry_*, report_error/report_if_error,
safe_warn/info/error — keep `use rift_core::...`); `crate::editor` (terminal input editor, distinct
from deleted code/) EXCEPT removing its AI-autosuggestion hook; the NON-AI autosuggestion
(fish-style history + `command_corrections`) is a KEEP terminal feature.
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
