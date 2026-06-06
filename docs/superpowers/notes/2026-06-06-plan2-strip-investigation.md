# Plan 2 Strip — Coupling Investigation

**Date:** 2026-06-06  
**Branch:** `plan2-strip`  
**Investigator:** read-only spike (no source changes)

---

## 1. Module Inventory & Size

### `app/src/` strip targets

| Module | Type | LOC | Notes |
|--------|------|-----|-------|
| `app/src/auth/` | directory | 6,486 | Login views, auth_manager, user persistence; re-exports from `rift_server_auth` |
| `app/src/drive/` | directory | 22,623 | Warp Drive index, sharing, workflow modals, export/import |
| `app/src/workspace/` | directory | 61,169 | **KEEP** — this is the app window container (tabs, pane layout, etc.), NOT cloud workspaces |
| `app/src/workspaces/` | directory | 5,693 | Cloud team/workspace management: `user_workspaces`, `team`, `team_tester`, `update_manager`, `user_profiles`, `gql_convert` |
| `app/src/billing/` | directory | 474 | One dialog: `shared_objects_creation_denied_modal` |
| `app/src/pricing/` | directory (single mod) | 85 | `PricingInfoModel` — a thin singleton |
| `app/src/server/` | directory | 39,553 | Warp-server client façade: server_api, telemetry, cloud_objects sync, experiments, network logging, IAP, voice transcriber |
| `app/src/autoupdate/` | directory | 3,756 | Auto-update check, channel version fetch via server_api |
| `app/src/usage/` | directory | 1 | Stub `mod.rs` pointing to `ai::blocklist::usage` — nearly empty |
| `app/src/referral_theme_status.rs` | single file | 179 | Referral theme model; embedded in `GlobalResourceHandles` |
| `app/src/reward_view.rs` | single file | 212 | Reward/congratulations modal view |
| `app/src/cloud_object/` | directory | 6,969 | App-side cloud object model, persistence, sync queue adapter |

### AI cloud bits (`app/src/ai/`)

| Path | LOC | Notes |
|------|-----|-------|
| `ai/cloud_agent_config/` | 58 | Thin config model for cloud Oz agent |
| `ai/cloud_agent_settings.rs` | 128 | Settings struct for cloud agent |
| `ai/cloud_environments/` | 91 | Cloud environment management |
| `ai/connected_self_hosted_workers.rs` | 125 | Connected self-hosted worker model |

### Cloud-only `crates/`

| Crate | LOC | Notes |
|-------|-----|-------|
| `crates/firebase/` | 145 | Firebase auth tokens |
| `crates/rift_server_auth/` | 1,491 | `AuthState`, credentials, anonymous ID, user persistence |
| `crates/rift_server_client/` | 1,385 | Base HTTP client, auth refresh, skip-login feature |
| `crates/graphql/` | 9,155 | GraphQL schema, queries (cloud AI, Drive, workspaces) |
| `crates/rift_graphql_schema/` | 15 | Schema codegen entrypoint |
| `crates/cloud_object_client/` | 372 | HTTP client for cloud object CRUD |
| `crates/cloud_object_models/` | 3,461 | Cloud object data types (workflows, notebooks, envs…) |
| `crates/cloud_object_persistence/` | 1,006 | SQLite persistence for cloud objects |
| `crates/cloud_objects/` | 2,401 | Domain types for cloud object ownership, revisions |

---

## 2. Reverse Dependencies — Classification

### Summary table

| Strip target | Files referencing it (total) | From genuine KEEP code | Classification |
|---|---|---|---|
| `server/` (ServerApi + telemetry) | 456 files, 939 matched lines | 153 files touch `ServerApi`; **telemetry macros** in 193 files including 260 hits in `terminal/`, 8 in `editor/` | **C — deeply coupled** |
| `auth/` (AuthState, AuthStateProvider) | 175 files, 267 matched lines | 128 files outside auth/server; terminal/view, terminal_manager, pane_group header, persistence/sqlite | **C — deeply coupled** |
| `drive/` | 125 files | 30+ outside drive/server/cloud_object: `terminal/view`, `pane_group/mod`, `workflow_pane`, settings_view | **C — coupled** (but mostly UI layers) |
| `workspaces/` (cloud teams) | 191 files | 160 outside workspaces/: `terminal/view`, `terminal/input`, `pane_group`, `workspace/view`, settings | **C — coupled** |
| `cloud_object/` (app-level) | 171 files outside cloud_object/ | terminal/*, pane_group, workspace/view, settings | **C — coupled** |
| `pricing/` | 17 files | 15 outside pricing: terminal/view, workspace modals, settings, test_util | **B — removable with shims** (PricingInfoModel → stub that returns always-allowed) |
| `billing/` | 1 file | `workspace/view.rs` only | **A — cleanly removable** (1 call site in workspace::view, itself a strip candidate) |
| `autoupdate/` | 9 files | lib.rs, changelog_model, root_view, debug_dump, terminal/view, settings_view | **B — removable with shims** (remove or always-return-no-update) |
| `referral_theme_status.rs` | 0 outside (but embedded in GlobalResourceHandles + workspace/view) | workspace/view, global_resource_handles | **B — needs minor surgery on GlobalResourceHandles** |
| `reward_view.rs` | 0 outside (called only from workspace/view) | workspace/view | **A — removable with workspace/view edits** |
| `usage/` (app/src) | 0 outside | n/a | **A — stub module, just delete** |
| AI cloud bits (cloud_agent_*, cloud_environments, connected_self_hosted) | Small (<10 refs each) | settings/init, lib.rs, workspace/view modals | **B — small, moderate coupling** |

### Key KEEP-code coupling details

#### `server/telemetry` → KEEP code
- `send_telemetry_from_ctx!` / `send_telemetry_from_app_ctx!` macros are used in **260 lines inside `terminal/`**, 8 in `editor/`, and across 193 files total.
- These macros call `ServerApiProvider::as_ref(ctx).get().send_telemetry_event(...)`.
- The events only actually reach Rudderstack when `ChannelState::is_release_bundle()` is true — dev/oss builds already no-op the network call.
- **Plan:** Replace with a no-op telemetry trait/fn; macros expand to nothing. This is mechanical but touches many files.

#### `auth/AuthState` → KEEP code
- `terminal/view.rs` uses `auth_state.is_anonymous_or_logged_out()` to gate AI suggestion display (lines 13301, 13391–13396, 26368) and to attach user_id to telemetry.
- `terminal/local_tty/terminal_manager.rs` passes `auth_state` to `send_telemetry_on_executor!` when an unsupported shell is detected.
- `terminal/local_tty/recorder.rs` uses `auth_state` for telemetry.
- **Pattern:** Almost all KEEP-code usage of `AuthState` is for (a) telemetry user_id or (b) gating cloud AI features. If telemetry is removed and auth-gated AI features are always-disabled, these calls shrink to zero or trivial stubs.

#### `workspaces/UserWorkspaces` → KEEP code
- `terminal/view.rs` subscribes to `UserWorkspacesEvent::TeamsChanged` to call `update_focused_terminal_info()` — which checks `UserWorkspaces::is_ai_allowed_in_remote_sessions()`.
- `terminal/input.rs` uses `UserWorkspaces::as_ref(ctx)` for related AI-policy checks.
- `pane_group` files use `UserWorkspaces` for billing/org-level checks.
- **Plan:** `UserWorkspaces` can be replaced by a stub that always returns permissive defaults for local-only operation.

#### `drive/` → KEEP code
- `terminal/view.rs` uses `WarpDriveSettings`, `ShareableObject`, `CloudObjectTypeAndId` (lines 333–335).
- `pane_group/mod.rs` uses `WarpDriveItemId`, `CloudObjectTypeAndId`, `OpenWarpDriveObjectArgs` (lines 87–88).
- `pane_group/pane/workflow_pane.rs` uses `WarpDriveItemId`, `OpenWarpDriveObjectSettings`.
- These are all for routing "open this Drive object" actions — they can be stripped along with drive/pane_group cloud UI.

#### `server/ServerApi` → KEEP code
- 153 files reference `ServerApi` or `ServerApiProvider`.
- In `terminal/view.rs`: `server_api.predict_am_queries(...)` (line 13319) — this is a cloud-AI suggestion call that was NOT rewired to rift_ai (only `generate_ai_input_suggestions` was). And `server_api.send_telemetry_event(...)` in `on_drop`.
- In `terminal/input.rs`: `server_api` stored in struct, passed to `NextCommandModel::new()` — already marked `#[allow(dead_code)]` in `next_command_model.rs` with comment "Retained but unused after the local-AI rewire."
- `autoupdate::AutoupdateState::register(ctx, server_api.clone())` — needs server_api for channel version fetch.
- Voice transcription: `ServerVoiceTranscriber::new(server_api.clone())` — cloud voice-to-text.

---

## 3. App Boot Path

### `app/src/bin/oss.rs`
Sets up `ChannelState` with:
- `WarpServerConfig::production()` — baked-in server URLs
- `telemetry_config: None`
- `crash_reporting_config: None`
- `autoupdate_config: None`

Then calls `rift::run()` → `run_internal()` in `app/src/lib.rs`.

### Cloud/auth init sequence in `run_internal()` (line numbers approximate)

| Step | Line range | What it does |
|------|-----------|---|
| `AuthState::initialize(ctx, api_key)` | ~1128 | Loads persisted user from Keychain; creates anonymous UUID |
| `ctx.add_singleton_model(NetworkLogModel)` | ~1137 | HTTP network log (used by server client) |
| `IapState::new()` (cfg-gated staging only) | ~1143–1146 | GCP IAP for staging — no-ops in OSS |
| `ServerApiProvider::new(auth_state, ...)` | ~1148–1152 | Creates the warp-server HTTP client + all sub-clients |
| `AuthManager::new(server_api, ...)` | ~1161–1167 | Auth token refresh manager |
| `AppTelemetryContextProvider::new(ctx)` | ~1159 | Telemetry context (user, session, etc.) |
| `ReferralThemeStatus::new` | ~1193 | Referral theme model — embedded in GlobalResourceHandles |
| `ServerExperiments::new_from_cache(...)` | ~1312 | Server-side experiment flags from SQLite |
| `AIRequestUsageModel::new(ai_client, ctx)` | ~1314 | Tracks AI usage/credits |
| `UserWorkspaces::new(team_client, workspace_client, ...)` | ~1317–1323 | Cloud team/workspace model |
| `pricing::PricingInfoModel::new()` | ~1378 | Pricing tier model |
| `ManagedSecretManager::new(...)` | ~1380–1385 | Managed secrets via server |
| `autoupdate::check_and_report_update_errors(ctx)` | ~1352 | Checks for autoupdate errors from prior launch |
| `TeamTesterStatus::new` | ~1807 | Team tester experiment status |
| `TeamUpdateManager::new(team_client, ...)` | ~1809–1815 | Cloud team sync |
| `UpdateManager::new(cloud_objects_client, ...)` | ~1817–1822 | Cloud objects sync manager |
| `initialize_cloud_preferences_syncer(...)` | ~1823–1831 | Syncs settings to/from cloud |
| `TelemetryCollector::new(server_api)` | ~1602 | Registers telemetry flush loop |
| `auth::init(ctx)` | ~1633 | Registers auth view actions |
| `reward_view::init(ctx)` | ~1634 | Registers reward modal actions |
| `billing::shared_objects_creation_denied_modal::init(ctx)` | ~1638 | Registers billing modal |
| `drive::index::init(ctx)` | ~1643 | Registers Drive index actions |
| `ChangelogModel::new(server_api.clone())` | ~1666 | Changelog fetcher |
| `SyncQueue::new(queue_items, cloud_objects_client, ...)` | ~1737 | Cloud object sync queue |
| `CloudModel::new(...)` | ~1729 | In-memory cloud object store |
| `Listener::new(cloud_objects_client, ctx)` | ~1892 | Cloud object push-event listener |
| `AutoupdateState::register(ctx, server_api.clone())` | ~1929 | Autoupdate state machine |
| `ConnectedSelfHostedWorkersModel::new` | ~1935 | Self-hosted worker discovery |
| On first frame (logged-in only): `TelemetryEvent::AppStartup` | ~1494 | Startup timing telemetry |

**Conclusion:** The app will **not boot cleanly** without auth/server infrastructure in its current form. `AuthState` and `ServerApiProvider` are created before persistence and most other subsystems. However, the OSS binary already sets `telemetry_config: None` and `autoupdate_config: None`, so those are already soft-disabled.

---

## 4. ServerApi Usage After AI Rewire

`grep -rn 'ServerApi\|server_api'` finds **1,344 occurrences in 153 files**.

Key call sites still using `ServerApi` outside the strip targets:

| Call | Location | Needed for |
|------|----------|-----------|
| `server_api.predict_am_queries(...)` | `terminal/input.rs:13319` | Cloud "AM query" suggestions (NOT rewired to rift_ai) |
| `server_api.send_telemetry_event(...)` | `terminal/view.rs:28088`, many others | Telemetry — gated on `is_release_bundle()` |
| `server_api.generate_ai_input_suggestions(...)` | `ai/predict/next_command_model.rs` (dead_code) | Rewired to rift_ai; `#[allow(dead_code)]` |
| `ServerVoiceTranscriber::new(server_api.clone())` | `lib.rs:1688` | Cloud voice transcription |
| `ChangelogModel::new(server_api.clone())` | `lib.rs:1666` | Changelog fetch |
| `autoupdate::AutoupdateState::register(ctx, server_api.clone())` | `lib.rs:1929` | Channel version fetch for updates |
| `SyncQueue::new(..., cloud_objects_client)` | `lib.rs:1737` | Drive object sync |
| `Listener::new(cloud_objects_client, ctx)` | `lib.rs:1892` | Drive object push events |
| `UpdateManager::new(cloud_objects_client, ctx)` | `lib.rs:1817` | Drive object update manager |
| `TeamUpdateManager::new(team_client, ...)` | `lib.rs:1809` | Cloud team updates |
| `UserWorkspaces::new(team_client, workspace_client, ...)` | `lib.rs:1317` | Cloud workspace/team model |
| `ManagedSecretManager::new(managed_secrets_client, ...)` | `lib.rs:1380` | Managed secrets |
| `AuthManager::new(server_api, auth_client, ctx)` | `lib.rs:1162` | Auth token refresh |

The `predict_am_queries` path in `terminal/input.rs` is the most surprising: it was NOT rewired to rift_ai (only `generate_ai_input_suggestions` was). This is a separate cloud-AI call that will break if `ServerApi` is removed without also removing or stubbing that path.

---

## 5. Feature Flags & Settings

Several cloud features are behind `FeatureFlag` that could allow a disable-first approach:

| Flag | Current scope | Relevant for |
|------|--------------|---|
| `FeatureFlag::Autoupdate` | Compile-time `#[cfg(feature = "autoupdate")]` | Autoupdate |
| `FeatureFlag::CloudObjects` | Runtime | Cloud object features |
| `FeatureFlag::DriveObjectsAsContext` | Runtime | Drive in AI context |
| `FeatureFlag::CloudEnvironments` | Runtime, `#[cfg(feature = "cloud_environments")]` | Cloud environments |
| `FeatureFlag::CloudMode` | Runtime | Cloud agent (Oz) mode |
| `FeatureFlag::CloudModeFromLocalSession` | Runtime | Cloud agent handoff |
| `FeatureFlag::CloudConversations` | Runtime | Cloud conversation sync |
| `FeatureFlag::TeamApiKeys` | Runtime | Team API key management |
| `FeatureFlag::BillingAndUsagePageV2` | Runtime | Billing UI |
| `FeatureFlag::HandoffLocalCloud` | Runtime | Local→Cloud handoff |
| `FeatureFlag::HandoffCloudCloud` | Runtime | Cloud→Cloud handoff |
| `FeatureFlag::SendTelemetryToFile` | Runtime | File-based telemetry |
| `FeatureFlag::APIKeyAuthentication` | Runtime | API key auth path |
| `FeatureFlag::MultiWorkspace` | Runtime | Multi-workspace UI |

**Key finding:** The `rift-oss` channel already has `telemetry_config: None` and `autoupdate_config: None` in `oss.rs`. Autoupdate is also gated on `#[cfg(feature = "autoupdate")]`. So those two are already soft-disabled in the OSS binary.

A "disable-first" pass (setting these flags to always-false in the OSS channel) would suppress most cloud UI before actually deleting code. This is the recommended Phase 0.

---

## 6. Workspace Crates — Cloud-Only Analysis

### Crate dependency graph

```
rift_graphql_schema → graphql
firebase → (standalone, used by rift_server_client)
rift_server_auth → graphql (for OwnerType), firebase (indirectly via rift_server_client)
rift_server_client → rift_server_auth, cloud_object_client, cloud_object_models, cloud_objects, firebase, graphql
cloud_objects → rift_server_auth, graphql
cloud_object_models → cloud_object_persistence, graphql
cloud_object_client → cloud_object_models, cloud_objects, graphql
cloud_object_persistence → cloud_objects
```

### Non-cloud crate dependencies on cloud crates

| Cloud crate | Non-cloud crate(s) that depend on it |
|-------------|--------------------------------------|
| `cloud_object_models` | `crates/mcp` (uses `StaticEnvVar`, `TransportType` — for MCP server config) |
| `graphql` | `crates/ai` (uses `FragmentLocationInput` for codebase index reranking), `crates/managed_secrets`, `crates/websocket` |

**Important:** `crates/mcp` imports `cloud_object_models` for MCP server transport config types. These will need to be moved to a non-cloud crate or duplicated if `cloud_object_models` is removed.

`crates/ai` imports `graphql` only for `rift_graphql::queries::rerank_fragments::FragmentLocationInput` in the codebase-index embedding module. This is a cloud-backed reranking call that can be removed/stubbed.

`crates/managed_secrets` uses `graphql` — managed secrets are a server-backed feature and can be stripped.

All other cloud crates are self-contained within the cloud dependency cluster.

---

## 7. Recommended Strip Order

### Phase 0 — Feature-flag disable (safe, reversible, `cargo build` stays GREEN)
1. In `oss.rs` or a new `features.rs` override, set all cloud `FeatureFlag`s to `false` for the OSS channel: `CloudMode`, `CloudConversations`, `CloudEnvironments`, `DriveObjectsAsContext`, `HandoffLocalCloud`, `HandoffCloudCloud`, `BillingAndUsagePageV2`, `TeamApiKeys`, `MultiWorkspace`, `CloudObjects`, `SharedWithMe`.
2. This suppresses all cloud UI paths without deleting code. Verify boot is clean.

### Phase 1 — Delete leaf modules with zero or trivial KEEP refs (A-class)
Order within phase doesn't matter much:
1. `app/src/usage/` — 1-line stub, just delete.
2. `app/src/billing/` — 1 consumer (`workspace/view.rs`); remove the import and the modal init call.
3. `app/src/reward_view.rs` — used only in `workspace/view.rs`; remove the reward_modal field and handlers.
4. `app/src/referral_theme_status.rs` + `settings_view/referrals_page.rs` — remove from `GlobalResourceHandles`, remove from `workspace/view.rs` subscriber. **Surgery on GlobalResourceHandles required.**
5. `app/src/server/server_api/referral.rs` — leaf of server_api.

### Phase 2 — Strip Drive (22k LOC directory) + cloud preferences
1. Delete `app/src/drive/`.
2. Delete `app/src/settings/cloud_preferences_syncer.rs` and `cloud_preferences.rs`.
3. Remove drive imports from `terminal/view.rs`, `pane_group/mod.rs`, `pane_group/pane/workflow_pane.rs`, `workspace/view.rs` (remove the Drive panel and sharing UI).
4. Remove drive-related init calls from `lib.rs` (~lines 1643–1645).
5. Remove `SyncQueue`, `UpdateManager`, `Listener` (cloud object sync) from `lib.rs`.
6. Delete `app/src/cloud_object/` and `app/src/server/cloud_objects/`.
7. Remove `cloud_object_*` crates from `app/Cargo.toml` and `Cargo.lock`.

### Phase 3 — Strip workspaces (cloud teams) + pricing
1. Delete `app/src/workspaces/` (cloud team management).
2. Replace `UserWorkspaces` singleton with a stub that returns permissive defaults (`is_ai_allowed_in_remote_sessions() → true`, etc.) OR delete the checks entirely from terminal/view.
3. Delete `app/src/pricing/` and replace `PricingInfoModel` with a stub (always-no-limit) — 17 consumers, all UI-gating.
4. Remove `TeamUpdateManager` init from lib.rs.

### Phase 4 — Strip autoupdate + changelog + voice transcription
1. Delete `app/src/autoupdate/` (or gate entirely on `#[cfg(feature = "autoupdate")]` which is already the case).
2. Delete `app/src/changelog_model.rs`.
3. Remove `ServerVoiceTranscriber` and replace with a no-op `VoiceTranscriber`.
4. Remove autoupdate/changelog init from lib.rs.

### Phase 5 — Strip auth + server (the big surgery, ~46k LOC total)
This is the riskiest phase. Do it last.
1. Remove `AuthState`/`AuthStateProvider` from all KEEP code. Replace with a "always-logged-in local identity" shim that provides a stable `anonymous_id` (UUID) and returns `is_anonymous_or_logged_out() = false` always. This removes the "you must log in" gate from terminal/view.
2. Strip all telemetry macros (`send_telemetry_from_ctx!`, `send_telemetry_from_app_ctx!`, `send_telemetry_on_executor!`) — either delete call sites or replace macros with no-ops. 260 lines in terminal alone.
3. Remove `predict_am_queries` from `terminal/input.rs` (not rewired to rift_ai; simply remove the call).
4. Delete `app/src/server/` directory.
5. Delete `app/src/auth/` directory.
6. Remove `rift_server_auth`, `rift_server_client`, `firebase`, `graphql` crates from workspace.
7. Remove `ManagedSecretManager` init.
8. Remove `ChangelogModel` from lib.rs.

### Phase 6 — Strip cloud-agent AI bits
1. Delete `app/src/ai/cloud_agent_config/`, `cloud_agent_settings.rs`, `cloud_environments/`, `connected_self_hosted_workers.rs`.
2. Remove `CloudAgentSettings` from `settings/init.rs`.
3. Remove `ConnectedSelfHostedWorkersModel` from lib.rs.
4. Remove cloud-agent modals from `workspace/view.rs`.

### Phase 7 — Clean up graphql crate dependency from remaining crates
1. Move `StaticEnvVar`/`TransportType` out of `cloud_object_models` into a local types file in `crates/mcp` or `rift_core`.
2. Replace `rift_graphql::queries::rerank_fragments::FragmentLocationInput` in `crates/ai` with a local struct or remove the reranking call.
3. Delete `crates/graphql` and `crates/rift_graphql_schema`.

---

## The 3 Biggest Coupling Risks

### Risk 1: Telemetry macros pervasively scattered through KEEP code (193 files, 260 in terminal alone)
`send_telemetry_from_ctx!` / `send_telemetry_from_app_ctx!` / `send_telemetry_on_executor!` are used throughout the terminal view, editor, and other core modules. They all reference `ServerApiProvider` and `ServerApi`. Removing `server/telemetry` requires either deleting all 260+ call sites in terminal or (safer) replacing the macros with no-ops that compile away. The macro replacement approach is doable and keeps diffs legible.

### Risk 2: `AuthState` is woven into the terminal core
`terminal/view.rs` stores `Arc<AuthState>` as a struct field (line 2518) and uses it to gate AI suggestion display and user-identity for telemetry. `terminal/local_tty/terminal_manager.rs` passes it to shell-start logic. Removing auth requires a shim `AuthState` that always returns "local, onboarded" identity. The shim can live in `rift_core` and avoid the full `rift_server_auth` dependency.

### Risk 3: `GlobalResourceHandles` contains `referral_theme_status`
`GlobalResourceHandles` is a struct passed to `workspace/view.rs` through `GlobalResourceHandlesProvider` (a singleton). It currently contains a `ModelHandle<ReferralThemeStatus>`. Removing referral requires editing this core struct, regenerating tests in `global_resource_handles`, and updating `workspace/view.rs`. This is minor surgery but touches a widely-imported type.

---

## Additional Findings

- **`workspace/` (singular) is KEEP** — it is the main app window/tab container, not a cloud workspace. Confusion with `workspaces/` (plural) which IS cloud team management.
- **`predict_am_queries` was not rewired to rift_ai** — only `generate_ai_input_suggestions` was. The `terminal/input.rs` has a live call to `server_api.predict_am_queries(...)` that will fail at runtime without a server connection. This needs to be either removed or routed to rift_ai.
- **`crates/mcp` depends on `cloud_object_models`** for `StaticEnvVar` and `TransportType` — these are MCP server transport types that happen to live in a cloud crate. They must be extracted before removing `cloud_object_models`.
- **`crates/ai` depends on `graphql`** for codebase index reranking (`FragmentLocationInput`). This is a cloud-backed reranking feature that can be dropped or replaced with a local implementation.
- **Telemetry is already no-op in dev/OSS** — `ChannelState::is_release_bundle()` gates the actual Rudderstack HTTP call. Dev builds already drop telemetry events silently.
- **The `rift-oss` binary already soft-disables** `autoupdate_config: None`, `telemetry_config: None`, `crash_reporting_config: None` in `oss.rs`.
