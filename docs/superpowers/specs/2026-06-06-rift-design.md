# Rift — Design

**Date:** 2026-06-06
**Status:** Approved design, pending implementation plan
**Repo:** `kessler-frost/rift` (private)

## Summary

Rift is a personal fork of [warpdotdev/warp](https://github.com/warpdotdev/warp),
stripped of all cloud, account, and team functionality, with Warp's cloud AI
replaced by a local AI layer backed by [omlx](https://github.com/jundot/omlx)
running on Apple Silicon. It keeps Warp's terminal core: blocks UI, GPU
rendering (wgpu), and editor-style input.

The fork is private. AGPL-3.0's distribution and network-service obligations are
not triggered by local personal use, so there is no obligation to publish.

## Goals

- Strip cloud/auth/Drive/team/billing/telemetry from Warp
- Keep blocks UI, GPU rendering, editor-style multi-line input, local settings
- Add a local AI layer with two capabilities:
  - **Command completion** — inline, fast, LLM-powered suggestions
  - **Natural language → command** — translate plain English into a shell command
- All AI runs locally via omlx's Anthropic-compatible Messages API

## Non-Goals

- No cloud sync, accounts, or team collaboration
- No support for multiple AI backends (omlx only — no trait abstraction)
- No upstream rebase tracking; upstream fixes are cherry-picked on demand
  (agent-assisted, see Rename section)

## Approach

**Approach 3 (chosen): surgical fork + clean AI layer.** Strip cloud/auth/Drive
with targeted module deletions. Add a self-contained `rift_ai` crate that owns
all AI behavior. The terminal core is otherwise inherited from Warp.

Approaches considered and rejected:
- *Upstream-tracking surgical fork* — minimal diff, but constrained by upstream
- *Full ownership fork* — clean but owns all terminal-core maintenance

## Architecture

### Studying upstream first

Per project convention, Warp's existing implementation was analyzed before
designing anything new:

- `crates/ai/src/api_keys.rs` — Warp already models pluggable endpoints via
  `CustomEndpoint { name, url, api_key, models[] }` and `ApiKeyManager`. This is
  buried under BYO-key and cloud-credential complexity (AWS Bedrock, OIDC, etc.).
- `crates/ai/src/llm_id.rs` — clean `LLMId(String)` newtype.
- `app/src/ai/predict/generate_ai_input_suggestions.rs` — the suggestion
  pipeline, with well-structured context types worth keeping:
  - `CommandContext { pwd, git_branch, exit_code }`
  - `ContextMessageInput { input, output, context }`
  - `HistoryContext { previous_commands[], next_command }`
  - `NextCommandContext { history_contexts[], ai_execution_context, context_messages[] }`
  - These call Warp's cloud server — that endpoint call is what Rift replaces.

### Strip plan

Cloud/auth/Drive live mostly as **modules inside `app/src/`**, not standalone
crates, so cuts are surgical file/module deletions plus removing call sites.

Targets in `app/src/`:
`auth/`, `drive/`, `workspace/`, `workspaces/`, `billing/`, `pricing/`,
`referral_theme_status.rs`, `reward_view.rs`, `autoupdate/`,
`cloud_agent_config/`, `cloud_agent_settings.rs`, `cloud_environments/`,
`connected_self_hosted_workers*`, `external_secrets/`,
`warp_managed_paths_watcher*`, `voltron.rs`, and Warp's cloud AI endpoint calls.

Kept (logic unchanged, renamed per below): `crates/warpui/`, `crates/warp_core/`,
terminal emulation, shell management, blocks, editor input, local settings.

### `rift_ai` crate

A flat crate — no trait abstraction, since omlx is the only backend.

```
crates/rift_ai/
  config.rs    — RiftAiConfig { endpoint, model, api_key, timeout_ms }; TOML loader
  context.rs   — RiftContext, CommandContext, ContextMessageInput
                 (lean, serializable subset adapted from Warp; app-independent)
  messages.rs  — Anthropic Messages request/response serde types + pure builders/parsers
  client.rs    — async POST to {endpoint}/v1/messages
  complete.rs  — complete(ctx: &RiftContext, cfg: &RiftAiConfig) -> Vec<String>
  translate.rs — translate(nl: &str, ctx: &RiftContext, cfg: &RiftAiConfig) -> String
```

Both `complete` and `translate` take the same context type so translate
benefits from the same pwd/git/history signals as completion (e.g. "deploy it"
resolves better with recent commands in view), and a single context type keeps
the crate simple.

Because Warp's `NextCommandContext` is app-coupled (it references
`WarpAiExecutionContext` and persistence models), `rift_ai` stays
app-independent by defining its own lean, serializable `RiftContext`
(the prompt-relevant subset of `CommandContext` / `ContextMessageInput`). The
app converts its rich `NextCommandContext` into a `RiftContext` at the call
site and passes it to both functions. Signatures:
`complete(ctx: &RiftContext, cfg: &RiftAiConfig)` and
`translate(nl: &str, ctx: &RiftContext, cfg: &RiftAiConfig)`.

Both POST to `{endpoint}/v1/messages` (omlx's Anthropic-compatible endpoint).
`complete` streams for responsiveness; `translate` is a single completion
constrained by system prompt to emit only a shell command.

**Sequencing (decided at planning):** Build and wire `rift_ai` *before*
stripping the cloud AI path, so a working local-AI replacement always exists
before anything is removed. Order: foundation → rename → `rift_ai` → wiring →
(Plan 2) strip cloud AI calls → strip auth/Drive/billing.

Warp's `ApiKeyManager`, `CustomEndpoint`, AWS/OIDC credential machinery, and
cloud server client are deleted and replaced by `RiftAiConfig`.

### Configuration

Single flat TOML at `~/.config/rift/config.toml`:

```toml
[ai]
endpoint = "http://localhost:8000"
model = "your-model-here"
api_key = "omlx-local"
timeout_ms = 3000
```

Warp's `settings/` module is largely kept (themes, keybindings, local prefs) but
anything referencing cloud sync or workspace IDs is stripped. No keychain writes
required (the omlx api_key is a local dummy value); keychain use stays optional.

### Rename: `warp` → `rift` (full)

Renamed everywhere — crates, binary, strings, paths:

1. **Crate names** — `warp_core` → `rift_core`, `warpui` → `riftui`,
   `warpui_core` → `riftui_core`, etc. Mechanical find/replace across all
   `Cargo.toml` files and `use` statements.
2. **Binary** — `app/Cargo.toml` `[package] name = "rift"` → produces the `rift` CLI.
3. **Strings & paths** — `~/.warp/` → `~/.rift/`, `WARP_*` env vars → `RIFT_*`,
   user-visible "Warp" UI strings → "Rift".

Done as a single early commit, before feature work, for a clean searchable diff.

**Upstream tracking:** Full rename intentionally diverges from upstream. When
pulling upstream terminal/rendering fixes, an agent-assisted workflow
hand-picks and adapts the relevant changes through the rename — the user does
not hand-resolve rename conflicts manually.

## Error Handling

- Missing/unreachable omlx endpoint: AI features degrade gracefully — completion
  yields nothing, NL→command surfaces a clear "AI backend unavailable" message.
  The terminal itself never blocks on AI.
- Malformed config: fall back to documented defaults; log once, don't crash.
- AI timeout (`timeout_ms`): abandon the suggestion silently; terminal stays
  responsive.

## Testing

- `rift_ai` unit tests: request construction, response parsing, config loading,
  context serialization (port Warp's existing context tests where applicable).
- Integration: a mock Anthropic-Messages server validating `complete` and
  `translate` request/response shapes.
- Manual: smoke-test against a live omlx instance on the M4 Pro.

## Open Questions (deferred to implementation)

- AI backend latency strategy (dedicated small model vs. current model vs.
  shell-side fast path) — decided during implementation.
