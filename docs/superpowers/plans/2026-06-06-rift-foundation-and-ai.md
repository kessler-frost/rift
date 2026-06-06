# Rift — Plan 1: Foundation, Rename & Local AI

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring Warp's source into the `rift` repo, fully rename `warp`→`rift`, and add a standalone `rift_ai` crate (local command completion + NL→command via omlx) wired into Warp's existing suggestion seam — leaving a buildable, runnable terminal with working local AI.

**Architecture:** Surgical fork of `warpdotdev/warp`. Upstream is added as a git remote so future fixes can be cherry-picked (agent-assisted). A new app-independent `rift_ai` crate owns all AI; the app converts its rich `NextCommandContext` into a lean `RiftContext` and routes suggestion requests to `rift_ai`→omlx instead of warp-server. This plan does NOT strip cloud/auth/Drive — that is Plan 2, deliberately sequenced after a working local-AI replacement exists.

**Tech Stack:** Rust 1.92.0, Cargo workspace (60+ crates), wgpu, tokio. New deps in `rift_ai`: `reqwest` (async, json), `serde`/`serde_json`, `toml`; dev-deps `wiremock`, `tokio`. Rename tooling: `fd`, `sd`, `ripgrep` (Homebrew).

**Scope note:** This is Plan 1 of 2. Plan 2 (the strip) is written separately after an investigation spike maps auth/cloud coupling — that work cannot be honestly pre-written as exact-code steps until the dependency graph is probed.

**Parallelization:** Tasks 0→1 are strictly sequential (each gates the next on a clean build). Within `rift_ai`, Task 3 (config) and Task 4 (context types) are independent and can run as parallel subagents; Task 5 (messages) depends on neither and can also run in parallel; Tasks 6–7 (`complete`/`translate`) both depend on 4+5+6-client and can run in parallel with each other once the client exists. Task 2 (scaffold) must precede all rift_ai work; Task 9 (wiring) must come last. Recommended parallel batch: {3, 4, 5} together, then {6}, then {7, 8} together.

---

## Reference: verified upstream facts (as of 2026-06-06)

- Upstream repo: `https://github.com/warpdotdev/warp`, default branch `master`, AGPL-3.0, ~271 MB.
- Toolchain pinned in `rust-toolchain.toml`: channel `1.92.0`, components rustfmt+clippy.
- Workspace `Cargo.toml`: `members = ["crates/*", "app"]`, resolver 2. `[workspace.dependencies]` maps every internal crate name to its path.
- The cloud-optional binary is `warp-oss` (`app/src/bin/oss.rs`), which calls `warp::run()` with `Channel::Oss`, `telemetry_config: None`, `crash_reporting_config: None`, `autoupdate_config: None`. `cargo run` defaults to it (`default-run = "warp-oss"` in `app/Cargo.toml`).
- AI suggestion seam: `app/src/ai/predict/next_command_model.rs` builds a `GenerateAIInputSuggestionsRequest` (via `create_generate_ai_input_suggestions_request`) and sends it through `crate::server::server_api::ServerApi` to warp-server. Request/response types live in `app/src/ai/predict/generate_ai_input_suggestions/api/{request.rs,response.rs}`.
- Response type to satisfy:
  ```rust
  pub struct GenerateAIInputSuggestionsResponseV2 {
      pub commands: Vec<String>,
      pub ai_queries: Vec<AgentModeSuggestionV2>,
      pub most_likely_action: String,
  }
  ```
- Context types worth mirroring (in `app/src/ai/predict/generate_ai_input_suggestions.rs`):
  ```rust
  pub struct CommandContext { pub pwd: Option<String>, pub git_branch: Option<String>, pub exit_code: i64 }
  pub struct ContextMessageInput { pub input: String, pub output: String, pub context: CommandContext }
  ```
- omlx Anthropic-compatible endpoint: `POST {endpoint}/v1/messages`, default `endpoint = http://localhost:8000`. Request body (Anthropic Messages):
  ```json
  { "model": "...", "max_tokens": 256, "system": "...", "messages": [ { "role": "user", "content": "..." } ] }
  ```
  Response body:
  ```json
  { "content": [ { "type": "text", "text": "..." } ] }
  ```

---

## Task 0: Bring Warp source into the rift repo + baseline build

**Files:**
- Modify: repo root of `/Users/fimbulwinter/dev/rift` (adds entire Warp tree)

- [ ] **Step 1: Add upstream remote and fetch master**

```bash
cd /Users/fimbulwinter/dev/rift
git remote add upstream https://github.com/warpdotdev/warp.git
git fetch upstream master
```

Expected: fetch downloads upstream history (large; several minutes).

- [ ] **Step 2: Merge upstream master into main, keeping both histories**

The rift repo already has a `main` with LICENSE/README/docs; upstream's root is `master`. Merge upstream in, allowing unrelated histories, and keep our `docs/` + `LICENSE` on conflict.

```bash
git merge --allow-unrelated-histories --no-edit -X theirs upstream/master
```

Expected: a merge commit. If README/LICENSE conflict, upstream's win for source files; our `docs/superpowers/` tree is preserved (no path overlap).

- [ ] **Step 3: Confirm the tree landed**

```bash
ls app crates Cargo.toml rust-toolchain.toml && test -f app/src/bin/oss.rs && echo OK
```

Expected: directories listed and `OK` printed.

- [ ] **Step 4: Install build deps and confirm toolchain**

```bash
rustup show active-toolchain   # should report 1.92.0 from rust-toolchain.toml
./script/install_cargo_build_deps 2>/dev/null || echo "no build-deps script path; continue"
```

Expected: toolchain `1.92.0`. (If `rustup` not present: `brew install rustup-init && rustup-init -y`.)

- [ ] **Step 5: Baseline build of the cloud-optional binary**

```bash
cargo build --bin warp-oss
```

Expected: a long first build (20–40 min). Ends with `Finished`. If it fails, STOP — the baseline must be green before any rename. Capture the first error and debug before continuing (do not proceed to Task 1 on a red baseline).

- [ ] **Step 6: Smoke-run the baseline**

```bash
cargo run --bin warp-oss
```

Expected: the Warp terminal window launches. Close it. (This confirms the OSS path runs without a configured account for basic terminal use.)

- [ ] **Step 7: Commit the baseline**

```bash
git add -A
git commit -m "chore: import warpdotdev/warp source as baseline

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 1: Full rename `warp` → `rift`

**Files:**
- Rename: every `crates/warp*` directory → `crates/rift*`
- Modify: all `Cargo.toml` (crate names, deps, `default-run`, bin names), all `.rs` (`use`/path identifiers), build scripts referencing crate names, env-var/path string literals.

**Why scripted, not hand-written per file:** the core operation is a deterministic, enumerable identifier replacement across ~60 crates. Blind `s/warp/rift/` is forbidden (it would corrupt `warp.dev` URLs, comments, the word "warp"). We replace **specific crate identifiers with word boundaries**, then let `cargo build` surface any residue.

- [ ] **Step 1: Install rename tooling**

```bash
brew install fd sd ripgrep
```

Expected: `fd`, `sd`, `rg` available.

- [ ] **Step 2: Capture the exact set of `warp*` crate identifiers**

```bash
cd /Users/fimbulwinter/dev/rift
rg -o '"(warp[a-z0-9_]*)"\s*=\s*\{ path' Cargo.toml -r '$1' | sort -u
fd -t d '^warp' crates -d 1
```

Expected: a list including `warp`, `warp_assets`, `warp_cli`, `warp_completer`, `warp_core`, `warp_features`, `warp_files`, `warp_graphql_schema`, `warp_js`, `warp_logging`, `warp_ripgrep`, `warp_search_core`, `warp_server_auth`, `warp_server_client`, `warp_terminal`, `warp_util`, `warp_web_event_bus`, `warpui`, `warpui_core`, `warpui_extras`. Treat this printed list as the source of truth for Step 4 (do not hardcode from memory — use what the repo reports).

- [ ] **Step 3: Rename crate directories (git-aware)**

```bash
for d in $(fd -t d '^warp' crates -d 1); do
  new="$(echo "$d" | sd 'crates/warp' 'crates/rift')"
  git mv "$d" "$new"
done
fd -t d '^rift' crates -d 1   # verify
```

Expected: directories now `crates/rift_core`, `crates/riftui`, etc. (`warpui`→`riftui`, `warp_*`→`rift_*`).

- [ ] **Step 4: Replace crate identifiers everywhere (word-boundary, longest-first)**

Order matters: replace `warpui_core`/`warpui_extras` before `warpui`, and `warp_*` before bare `warp`. The bare `warp` crate (the `app` package) is handled in Step 5, so here we only touch underscore/`ui`-suffixed identifiers.

```bash
# Longest-match-first list of identifiers to rename (NOT bare "warp").
for id in warpui_core warpui_extras warpui \
          warp_assets warp_cli warp_completer warp_core warp_features \
          warp_files warp_graphql_schema warp_js warp_logging warp_ripgrep \
          warp_search_core warp_server_auth warp_server_client warp_terminal \
          warp_util warp_web_event_bus; do
  new="$(echo "$id" | sd '^warp' 'rift')"
  # \b word boundaries so "warp_core" never matches inside "warp_core_x"
  fd -e rs -e toml -e nix -e json | xargs sd "\b$id\b" "$new"
done
```

Expected: no errors. `rg -l '\bwarp_core\b'` should now return nothing.

- [ ] **Step 5: Rename the `app` package, library, and binaries**

The `app` crate is named `warp` (lib name `warp`, `warp::run()`), with bins `warp-oss`, `warp`, `stable`, etc. Rename the package/lib to `rift` and the OSS bin to `rift-oss`, and point `default-run` at it.

```bash
# Package + lib name, and the workspace dep "warp = { path = "app" }"
sd '\bname = "warp"\b' 'name = "rift"' app/Cargo.toml
sd '^warp = \{ path = "app" \}' 'rift = { path = "app" }' Cargo.toml
# default-run and the OSS bin name
sd 'default-run = "warp-oss"' 'default-run = "rift-oss"' app/Cargo.toml
sd 'name = "warp-oss"' 'name = "rift-oss"' app/Cargo.toml
# crate path references `warp::` (the app lib) → `rift::`
fd -e rs | xargs sd '\bwarp::' 'rift::'
# the bin source still says warp::run(); fixed by the line above. Verify:
rg -n 'rift::run\(\)' app/src/bin/oss.rs
```

Expected: `app/src/bin/oss.rs` now calls `rift::run()`; `rg -n 'rift::run\(\)'` matches.

- [ ] **Step 6: Rename user-facing paths and env vars**

```bash
fd -e rs -e toml | xargs sd '\bWARP_' 'RIFT_'
fd -e rs -e toml | xargs sd '"\.warp"' '".rift"'
fd -e rs -e toml | xargs sd '~/\.warp' '~/.rift'
```

Expected: no errors. (`warp.dev` URLs and the AGPL text are untouched — we only matched `WARP_`, `.warp` path tokens.)

- [ ] **Step 7: Build and chase residual identifier misses**

```bash
cargo build --bin rift-oss 2>&1 | rg 'unresolved|cannot find|no external crate|use of undeclared' | head -40
```

Expected: ideally empty. For each reported missing identifier, it will name the old `warp_*` symbol that wasn't reachable — apply the same `sd '\bOLD\b' 'NEW'` fix to the offending file, then re-run. Repeat until the build is `Finished`. (This loop is bounded by the enumerable crate list from Step 2.)

- [ ] **Step 8: Smoke-run the renamed app**

```bash
cargo run --bin rift-oss
```

Expected: terminal launches (now the `rift-oss` binary). Close it.

- [ ] **Step 9: Format and commit the rename**

```bash
./script/format 2>/dev/null || cargo fmt --all
git add -A
git commit -m "refactor: rename warp -> rift across crates, bins, and paths

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

> From here on, crate names are `rift_*`/`riftui*`, the lib is `rift`, and the run command is `cargo run --bin rift-oss`.

---

## Task 2: Scaffold the `rift_ai` crate

**Files:**
- Create: `crates/rift_ai/Cargo.toml`
- Create: `crates/rift_ai/src/lib.rs`
- Modify: `Cargo.toml` (workspace `[workspace.dependencies]` — add `rift_ai`)

- [ ] **Step 1: Create the crate manifest**

Create `crates/rift_ai/Cargo.toml`:

```toml
[package]
name = "rift_ai"
version = "0.1.0"
edition = "2021"
license = "AGPL-3.0-only"
publish = false

[dependencies]
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"
thiserror = "1"
tokio = { version = "1", features = ["rt", "macros"] }

[dev-dependencies]
wiremock = "0.6"
tokio = { version = "1", features = ["rt", "macros", "rt-multi-thread"] }
tempfile = "3"
```

- [ ] **Step 2: Register the crate in the workspace dependency table**

In the root `Cargo.toml`, under `[workspace.dependencies]`, add (alphabetical-ish, near the other `rift_*` entries):

```toml
rift_ai = { path = "crates/rift_ai" }
```

(`members = ["crates/*", ...]` already globs the new crate in.)

- [ ] **Step 3: Create an empty lib root**

Create `crates/rift_ai/src/lib.rs`:

```rust
//! Local AI for Rift: command completion and natural-language → command,
//! served by a local omlx instance via the Anthropic Messages API.

pub mod config;
pub mod context;
pub mod messages;
pub mod client;
pub mod complete;
pub mod translate;

pub use config::RiftAiConfig;
pub use context::{CommandContext, ContextMessageInput, RiftContext};
```

- [ ] **Step 4: Create empty module files so the crate compiles**

```bash
cd /Users/fimbulwinter/dev/rift/crates/rift_ai/src
: > config.rs; : > context.rs; : > messages.rs; : > client.rs; : > complete.rs; : > translate.rs
```

- [ ] **Step 5: Verify the crate is recognized and compiles empty**

Run: `cargo build -p rift_ai`
Expected: `Finished` (empty modules compile).

- [ ] **Step 6: Commit**

```bash
git add crates/rift_ai Cargo.toml
git commit -m "feat(rift_ai): scaffold local AI crate

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 3: `RiftAiConfig` + TOML loader  *(parallel-safe)*

**Files:**
- Modify: `crates/rift_ai/src/config.rs`
- Test: inline `#[cfg(test)]` in `config.rs`

- [ ] **Step 1: Write the failing tests**

Put in `crates/rift_ai/src/config.rs`:

```rust
use std::path::Path;

use serde::Deserialize;

/// Local AI configuration, loaded from `~/.config/rift/config.toml` (`[ai]` table).
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct RiftAiConfig {
    pub endpoint: String,
    pub model: String,
    #[serde(default = "default_api_key")]
    pub api_key: String,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
}

fn default_api_key() -> String {
    "omlx-local".to_string()
}

fn default_timeout_ms() -> u64 {
    3000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_config() {
        let toml = r#"
[ai]
endpoint = "http://localhost:8000"
model = "qwen"
api_key = "k"
timeout_ms = 1500
"#;
        let cfg = RiftAiConfig::from_toml_str(toml).unwrap();
        assert_eq!(cfg.endpoint, "http://localhost:8000");
        assert_eq!(cfg.model, "qwen");
        assert_eq!(cfg.api_key, "k");
        assert_eq!(cfg.timeout_ms, 1500);
    }

    #[test]
    fn applies_defaults_for_optional_fields() {
        let toml = r#"
[ai]
endpoint = "http://localhost:8000"
model = "qwen"
"#;
        let cfg = RiftAiConfig::from_toml_str(toml).unwrap();
        assert_eq!(cfg.api_key, "omlx-local");
        assert_eq!(cfg.timeout_ms, 3000);
    }

    #[test]
    fn missing_required_field_is_error() {
        let toml = r#"
[ai]
model = "qwen"
"#;
        assert!(RiftAiConfig::from_toml_str(toml).is_err());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p rift_ai config::`
Expected: FAIL — `from_toml_str` not found.

- [ ] **Step 3: Implement the loader**

Append to `crates/rift_ai/src/config.rs`:

```rust
#[derive(Debug, Deserialize)]
struct ConfigFile {
    ai: RiftAiConfig,
}

impl RiftAiConfig {
    /// Parse a `[ai]`-tabled TOML string.
    pub fn from_toml_str(s: &str) -> Result<Self, toml::de::Error> {
        Ok(toml::from_str::<ConfigFile>(s)?.ai)
    }

    /// Load from a TOML file at `path`.
    pub fn load_from(path: &Path) -> std::io::Result<Self> {
        let text = std::fs::read_to_string(path)?;
        Self::from_toml_str(&text)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    /// Default location: `~/.config/rift/config.toml`.
    pub fn default_path() -> std::path::PathBuf {
        let home = std::env::var("HOME").unwrap_or_default();
        Path::new(&home).join(".config/rift/config.toml")
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p rift_ai config::`
Expected: 3 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/rift_ai/src/config.rs
git commit -m "feat(rift_ai): RiftAiConfig with TOML loader and defaults

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 4: `RiftContext` and prompt-context types  *(parallel-safe)*

**Files:**
- Modify: `crates/rift_ai/src/context.rs`
- Test: inline `#[cfg(test)]` in `context.rs`

- [ ] **Step 1: Write the failing test**

Put in `crates/rift_ai/src/context.rs`:

```rust
use serde::{Deserialize, Serialize};

/// Per-command environment context (mirrors Warp's `CommandContext`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandContext {
    pub pwd: Option<String>,
    pub git_branch: Option<String>,
    pub exit_code: i64,
}

/// A prior command + its output + the environment it ran in.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContextMessageInput {
    pub input: String,
    pub output: String,
    pub context: CommandContext,
}

/// App-independent context passed to both `complete` and `translate`.
/// The app builds this from its rich `NextCommandContext`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RiftContext {
    /// Recent command/output history, oldest first.
    pub history: Vec<ContextMessageInput>,
    /// What the user has typed so far on the current prompt (may be empty).
    pub current_input: String,
    /// Shell name, e.g. "zsh" (best-effort).
    pub shell: Option<String>,
}

impl RiftContext {
    /// Render history as compact JSON lines for inclusion in a prompt.
    pub fn history_as_jsonl(&self) -> String {
        self.history
            .iter()
            .filter_map(|m| serde_json::to_string(m).ok())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> RiftContext {
        RiftContext {
            history: vec![ContextMessageInput {
                input: "ls".into(),
                output: "a b c".into(),
                context: CommandContext {
                    pwd: Some("/tmp".into()),
                    git_branch: None,
                    exit_code: 0,
                },
            }],
            current_input: "gi".into(),
            shell: Some("zsh".into()),
        }
    }

    #[test]
    fn history_renders_one_json_line_per_entry() {
        let ctx = sample();
        let jsonl = ctx.history_as_jsonl();
        assert_eq!(jsonl.lines().count(), 1);
        assert!(jsonl.contains("\"input\":\"ls\""));
        assert!(jsonl.contains("\"exit_code\":0"));
    }

    #[test]
    fn empty_history_renders_empty_string() {
        let ctx = RiftContext::default();
        assert_eq!(ctx.history_as_jsonl(), "");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p rift_ai context::`
Expected: FAIL — initially the file is empty so the test module won't reference; after pasting it fails only if logic is wrong. (If it compiles and passes immediately, that's acceptable for pure data types — proceed.)

- [ ] **Step 3: (Implementation is included above)**

The types and `history_as_jsonl` are the implementation. No extra code needed.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p rift_ai context::`
Expected: 2 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/rift_ai/src/context.rs
git commit -m "feat(rift_ai): app-independent RiftContext + prompt-context types

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 5: Anthropic Messages request/response types  *(parallel-safe)*

**Files:**
- Modify: `crates/rift_ai/src/messages.rs`
- Test: inline `#[cfg(test)]` in `messages.rs`

- [ ] **Step 1: Write the failing tests**

Put in `crates/rift_ai/src/messages.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Message {
    pub role: String,
    pub content: String,
}

/// Anthropic Messages API request body.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct MessagesRequest {
    pub model: String,
    pub max_tokens: u32,
    pub system: String,
    pub messages: Vec<Message>,
}

impl MessagesRequest {
    pub fn single_user(model: &str, system: &str, user: &str, max_tokens: u32) -> Self {
        Self {
            model: model.to_string(),
            max_tokens,
            system: system.to_string(),
            messages: vec![Message { role: "user".into(), content: user.into() }],
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ContentBlock {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub text: String,
}

/// Anthropic Messages API response body (text blocks only — what omlx returns).
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct MessagesResponse {
    #[serde(default)]
    pub content: Vec<ContentBlock>,
}

impl MessagesResponse {
    /// Concatenate all text blocks, trimmed.
    pub fn text(&self) -> String {
        self.content
            .iter()
            .filter(|b| b.kind == "text")
            .map(|b| b.text.as_str())
            .collect::<String>()
            .trim()
            .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_serializes_to_anthropic_shape() {
        let req = MessagesRequest::single_user("m", "sys", "hi", 256);
        let v: serde_json::Value = serde_json::to_value(&req).unwrap();
        assert_eq!(v["model"], "m");
        assert_eq!(v["max_tokens"], 256);
        assert_eq!(v["system"], "sys");
        assert_eq!(v["messages"][0]["role"], "user");
        assert_eq!(v["messages"][0]["content"], "hi");
    }

    #[test]
    fn response_extracts_concatenated_text() {
        let json = r#"{"content":[{"type":"text","text":"  git status  "}]}"#;
        let resp: MessagesResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.text(), "git status");
    }

    #[test]
    fn response_ignores_non_text_blocks() {
        let json = r#"{"content":[{"type":"thinking","text":"x"},{"type":"text","text":"ls"}]}"#;
        let resp: MessagesResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.text(), "ls");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail/compile**

Run: `cargo test -p rift_ai messages::`
Expected: compiles; the three tests pass (types + helpers are the implementation). If any assertion fails, fix the helper until green.

- [ ] **Step 3: Commit**

```bash
git add crates/rift_ai/src/messages.rs
git commit -m "feat(rift_ai): Anthropic Messages request/response types

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 6: omlx HTTP client

**Files:**
- Modify: `crates/rift_ai/src/client.rs`
- Test: inline `#[cfg(test)]` in `client.rs` (uses `wiremock`)

**Depends on:** Task 3 (config), Task 5 (messages).

- [ ] **Step 1: Write the failing test (against a wiremock server)**

Put in `crates/rift_ai/src/client.rs`:

```rust
use std::time::Duration;

use thiserror::Error;

use crate::config::RiftAiConfig;
use crate::messages::{MessagesRequest, MessagesResponse};

#[derive(Debug, Error)]
pub enum AiError {
    #[error("request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("ai backend returned status {0}")]
    Status(u16),
}

/// POST a Messages request to `{endpoint}/v1/messages` and parse the response.
pub async fn send_messages(
    cfg: &RiftAiConfig,
    req: &MessagesRequest,
) -> Result<MessagesResponse, AiError> {
    let url = format!("{}/v1/messages", cfg.endpoint.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(cfg.timeout_ms))
        .build()?;
    let resp = client
        .post(url)
        .header("x-api-key", &cfg.api_key)
        .header("anthropic-version", "2023-06-01")
        .json(req)
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(AiError::Status(resp.status().as_u16()));
    }
    Ok(resp.json::<MessagesResponse>().await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn cfg_for(endpoint: String) -> RiftAiConfig {
        RiftAiConfig::from_toml_str(&format!(
            "[ai]\nendpoint = \"{endpoint}\"\nmodel = \"m\"\n"
        ))
        .unwrap()
    }

    #[tokio::test]
    async fn posts_to_v1_messages_and_parses_text() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .and(header("anthropic-version", "2023-06-01"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "content": [ { "type": "text", "text": "git status" } ]
            })))
            .mount(&server)
            .await;

        let cfg = cfg_for(server.uri());
        let req = MessagesRequest::single_user("m", "sys", "u", 64);
        let resp = send_messages(&cfg, &req).await.unwrap();
        assert_eq!(resp.text(), "git status");
    }

    #[tokio::test]
    async fn non_200_is_status_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;

        let cfg = cfg_for(server.uri());
        let req = MessagesRequest::single_user("m", "sys", "u", 64);
        let err = send_messages(&cfg, &req).await.unwrap_err();
        assert!(matches!(err, AiError::Status(503)));
    }
}
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test -p rift_ai client::`
Expected: 2 passed (wiremock spins up a local server; no real omlx needed).

- [ ] **Step 3: Commit**

```bash
git add crates/rift_ai/src/client.rs
git commit -m "feat(rift_ai): omlx Messages HTTP client with timeout + error mapping

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 7: `complete()` — command completion  *(parallel with Task 8)*

**Files:**
- Modify: `crates/rift_ai/src/complete.rs`
- Test: inline `#[cfg(test)]` in `complete.rs` (uses `wiremock`)

**Depends on:** Tasks 4, 5, 6.

- [ ] **Step 1: Write the failing test**

Put in `crates/rift_ai/src/complete.rs`:

```rust
use crate::client::{send_messages, AiError};
use crate::config::RiftAiConfig;
use crate::context::RiftContext;
use crate::messages::MessagesRequest;

const COMPLETE_SYSTEM: &str = "You are a shell command completion engine. \
Given recent command history (JSON lines) and the user's partial input, return up to 5 \
likely full shell commands, one per line, most likely first. Output ONLY commands, no prose, \
no numbering, no backticks.";

const COMPLETE_MAX_TOKENS: u32 = 256;

/// Return ranked candidate commands for the current partial input.
pub async fn complete(ctx: &RiftContext, cfg: &RiftAiConfig) -> Result<Vec<String>, AiError> {
    let user = format!(
        "history:\n{}\nshell: {}\npartial_input: {}",
        ctx.history_as_jsonl(),
        ctx.shell.as_deref().unwrap_or("unknown"),
        ctx.current_input,
    );
    let req = MessagesRequest::single_user(&cfg.model, COMPLETE_SYSTEM, &user, COMPLETE_MAX_TOKENS);
    let resp = send_messages(cfg, &req).await?;
    Ok(parse_commands(&resp.text()))
}

/// Split model text into clean command lines (drop blanks, numbering, backticks).
fn parse_commands(text: &str) -> Vec<String> {
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
    use crate::config::RiftAiConfig;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn parse_commands_strips_noise_and_caps_at_five() {
        let text = "1. git status\n`git log`\n\n2) git add .\ngit commit\ngit push\ngit pull\n";
        let cmds = parse_commands(text);
        assert_eq!(cmds, vec!["git status", "git log", "git add .", "git commit", "git push"]);
    }

    #[tokio::test]
    async fn complete_returns_parsed_commands() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "content": [ { "type": "text", "text": "git status\ngit stash" } ]
            })))
            .mount(&server)
            .await;
        let cfg = RiftAiConfig::from_toml_str(&format!(
            "[ai]\nendpoint = \"{}\"\nmodel = \"m\"\n", server.uri()
        )).unwrap();
        let ctx = RiftContext { current_input: "git st".into(), ..Default::default() };
        let cmds = complete(&ctx, &cfg).await.unwrap();
        assert_eq!(cmds, vec!["git status", "git stash"]);
    }
}
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test -p rift_ai complete::`
Expected: 2 passed.

- [ ] **Step 3: Commit**

```bash
git add crates/rift_ai/src/complete.rs
git commit -m "feat(rift_ai): complete() command completion via omlx

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 8: `translate()` — natural language → command  *(parallel with Task 7)*

**Files:**
- Modify: `crates/rift_ai/src/translate.rs`
- Test: inline `#[cfg(test)]` in `translate.rs` (uses `wiremock`)

**Depends on:** Tasks 4, 5, 6.

- [ ] **Step 1: Write the failing test**

Put in `crates/rift_ai/src/translate.rs`:

```rust
use crate::client::{send_messages, AiError};
use crate::config::RiftAiConfig;
use crate::context::RiftContext;
use crate::messages::MessagesRequest;

const TRANSLATE_SYSTEM: &str = "You translate a natural-language request into a single shell \
command for the user's shell. Use the provided history for context. Output ONLY the command on \
one line — no prose, no explanation, no backticks, no markdown.";

const TRANSLATE_MAX_TOKENS: u32 = 200;

/// Translate a natural-language request into a single shell command.
pub async fn translate(nl: &str, ctx: &RiftContext, cfg: &RiftAiConfig) -> Result<String, AiError> {
    let user = format!(
        "history:\n{}\nshell: {}\nrequest: {}",
        ctx.history_as_jsonl(),
        ctx.shell.as_deref().unwrap_or("unknown"),
        nl,
    );
    let req = MessagesRequest::single_user(&cfg.model, TRANSLATE_SYSTEM, &user, TRANSLATE_MAX_TOKENS);
    let resp = send_messages(cfg, &req).await?;
    Ok(clean_command(&resp.text()))
}

/// Take the first non-empty line and strip backticks/markdown fences.
fn clean_command(text: &str) -> String {
    text.lines()
        .map(|l| l.trim().trim_matches('`').trim())
        .find(|l| !l.is_empty() && !l.starts_with("```"))
        .unwrap_or("")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn clean_command_takes_first_real_line_without_fences() {
        assert_eq!(clean_command("```sh\nfind . -mtime -1\n```"), "find . -mtime -1");
        assert_eq!(clean_command("`ls -la`"), "ls -la");
        assert_eq!(clean_command(""), "");
    }

    #[tokio::test]
    async fn translate_returns_single_command() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "content": [ { "type": "text", "text": "find . -type f -mtime -1" } ]
            })))
            .mount(&server)
            .await;
        let cfg = RiftAiConfig::from_toml_str(&format!(
            "[ai]\nendpoint = \"{}\"\nmodel = \"m\"\n", server.uri()
        )).unwrap();
        let cmd = translate("files changed today", &RiftContext::default(), &cfg).await.unwrap();
        assert_eq!(cmd, "find . -type f -mtime -1");
    }
}
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test -p rift_ai translate::`
Expected: 2 passed.

- [ ] **Step 3: Full crate test + commit**

```bash
cargo test -p rift_ai
git add crates/rift_ai/src/translate.rs
git commit -m "feat(rift_ai): translate() natural-language to command via omlx

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

Expected: entire `rift_ai` suite green.

---

## Task 9: Wire `rift_ai` into the suggestion seam  *(must be last)*

**Goal:** When AI input suggestions are requested, build a `RiftContext` from the app's `NextCommandContext` and call `rift_ai::complete()` instead of sending a `GenerateAIInputSuggestionsRequest` to warp-server. Populate `GenerateAIInputSuggestionsResponseV2.commands` from the result. On any AI error, return empty suggestions so the terminal never blocks.

**Files:**
- Create: `app/src/ai/predict/rift_bridge.rs` (conversion + routing; keeps the diff isolated and easy to find later)
- Modify: `app/Cargo.toml` (add `rift_ai` dependency)
- Modify: `app/src/ai/predict/mod.rs` (declare the new module)
- Modify: `app/src/ai/predict/next_command_model.rs` (call the bridge instead of `ServerApi` for suggestions)

**Depends on:** Tasks 7, 8. (Tasks 0–1 already renamed everything; this task is written against `rift_*` names.)

- [ ] **Step 1: Add the crate dependency to the app**

In `app/Cargo.toml` under `[dependencies]`, add:

```toml
rift_ai = { workspace = true }
```

Run: `cargo build -p rift 2>&1 | tail -5`
Expected: builds (dependency resolves).

- [ ] **Step 2: Write the failing bridge test (pure conversion)**

Create `app/src/ai/predict/rift_bridge.rs`:

```rust
//! Bridges Warp's app-side suggestion context to the app-independent `rift_ai` crate.
//! This is the single seam where Rift's local AI replaces warp-server suggestion calls.

use rift_ai::context::{CommandContext, ContextMessageInput, RiftContext};

use super::generate_ai_input_suggestions::ContextMessageInput as AppContextMessageInput;

/// Convert the app's per-message context inputs + current input into a `RiftContext`.
pub fn to_rift_context(
    messages: &[AppContextMessageInput],
    current_input: &str,
    shell: Option<&str>,
) -> RiftContext {
    let history = messages
        .iter()
        .map(|m| ContextMessageInput {
            input: m.input.clone(),
            output: m.output.clone(),
            context: CommandContext {
                pwd: m.context.pwd.clone(),
                git_branch: m.context.git_branch.clone(),
                exit_code: m.context.exit_code,
            },
        })
        .collect();
    RiftContext {
        history,
        current_input: current_input.to_string(),
        shell: shell.map(str::to_string),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::predict::generate_ai_input_suggestions::{
        CommandContext as AppCmdCtx, ContextMessageInput as AppMsg,
    };

    #[test]
    fn converts_messages_and_current_input() {
        let msgs = vec![AppMsg {
            input: "ls".into(),
            output: "a".into(),
            context: AppCmdCtx { pwd: Some("/tmp".into()), git_branch: None, exit_code: 0 },
        }];
        let ctx = to_rift_context(&msgs, "gi", Some("zsh"));
        assert_eq!(ctx.history.len(), 1);
        assert_eq!(ctx.history[0].input, "ls");
        assert_eq!(ctx.current_input, "gi");
        assert_eq!(ctx.shell.as_deref(), Some("zsh"));
    }
}
```

> NOTE for the implementer: the exact field names/visibility of the app's `ContextMessageInput`/`CommandContext` must be confirmed against `app/src/ai/predict/generate_ai_input_suggestions.rs` at execution time and the `use`/field accesses adjusted to match (they may need `pub` exposure). This is the one place the plan touches code whose private fields we haven't fully enumerated — verify before assuming.

- [ ] **Step 3: Declare the module**

In `app/src/ai/predict/mod.rs`, add:

```rust
pub(crate) mod rift_bridge;
```

- [ ] **Step 4: Run the bridge test**

Run: `cargo test -p rift rift_bridge::`
Expected: 1 passed. If the app's context fields are private, make them `pub`/`pub(crate)` (smallest change) and re-run.

- [ ] **Step 5: Add the local-AI routing function to the bridge**

Append to `app/src/ai/predict/rift_bridge.rs`:

```rust
use rift_ai::config::RiftAiConfig;
use super::generate_ai_input_suggestions::GenerateAIInputSuggestionsResponseV2;

/// Run local completion and shape it into the response the suggestion pipeline expects.
/// Any error (no omlx, timeout, parse) degrades to empty suggestions — never blocks the terminal.
pub async fn local_suggestions(ctx: &RiftContext, cfg: &RiftAiConfig)
    -> GenerateAIInputSuggestionsResponseV2
{
    let commands = rift_ai::complete::complete(ctx, cfg).await.unwrap_or_default();
    GenerateAIInputSuggestionsResponseV2 {
        commands,
        ai_queries: Vec::new(),
        most_likely_action: String::new(),
    }
}
```

- [ ] **Step 6: Build to confirm the bridge compiles against real types**

Run: `cargo build -p rift 2>&1 | tail -20`
Expected: builds. Fix any field/visibility/type mismatches against the real `GenerateAIInputSuggestionsResponseV2` (verified shape: `commands: Vec<String>`, `ai_queries: Vec<AgentModeSuggestionV2>`, `most_likely_action: String`).

- [ ] **Step 7: Route the suggestion call through the bridge**

In `app/src/ai/predict/next_command_model.rs`, find the call site that sends the request via `ServerApi` (search for `ServerApi`/`create_generate_ai_input_suggestions_request` and the `.await` that yields a `GenerateAIInputSuggestionsResponseV2`). Replace the server round-trip with the local path. Concretely, where the response is currently obtained from the server, substitute:

```rust
// Rift: serve suggestions locally via omlx instead of warp-server.
let rift_cfg = rift_ai::config::RiftAiConfig::load_from(
    &rift_ai::config::RiftAiConfig::default_path(),
)
.ok();
let response = match rift_cfg {
    Some(cfg) => {
        let rctx = crate::ai::predict::rift_bridge::to_rift_context(
            &context_messages,            // existing Vec<ContextMessageInput> in scope
            &current_input,               // existing partial input in scope
            shell_name.as_deref(),        // best-effort shell; None if unavailable
        );
        crate::ai::predict::rift_bridge::local_suggestions(&rctx, &cfg).await
    }
    // No config file → no local AI configured → empty suggestions (terminal unaffected).
    None => GenerateAIInputSuggestionsResponseV2::default(),
};
```

> The variable names `context_messages`, `current_input`, `shell_name` are placeholders for whatever the surrounding function already has in scope — bind them from the existing locals (the function already assembles `context_messages` and the partial input to build the server request). Do NOT introduce new data sources; reuse what the server path already gathered.

- [ ] **Step 8: Build the app**

Run: `cargo build --bin rift-oss 2>&1 | tail -20`
Expected: `Finished`. Resolve any borrow/move issues by cloning the existing locals as the server path did.

- [ ] **Step 9: Create a config and smoke-test end-to-end against omlx**

```bash
mkdir -p ~/.config/rift
cat > ~/.config/rift/config.toml <<'EOF'
[ai]
endpoint = "http://localhost:8000"
model = "REPLACE_WITH_A_LOADED_OMLX_MODEL"
timeout_ms = 3000
EOF
omlx start                 # ensure omlx is serving on :8000 with a model loaded
cargo run --bin rift-oss
```

Expected: in the running terminal, typing a partial command surfaces AI-suggested completions sourced from omlx (verify omlx logs show `POST /v1/messages`). With omlx stopped, the terminal still runs and simply shows no AI suggestions (graceful degradation).

- [ ] **Step 10: Commit**

```bash
git add app/Cargo.toml app/src/ai/predict/mod.rs app/src/ai/predict/rift_bridge.rs app/src/ai/predict/next_command_model.rs
git commit -m "feat: route AI input suggestions through rift_ai/omlx

Replaces the warp-server suggestion round-trip with local omlx completion,
degrading to empty suggestions when omlx is unavailable.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 10: Plan-1 acceptance checkpoint

- [ ] **Step 1: Full workspace test of the new crate + bridge**

Run: `cargo test -p rift_ai && cargo test -p rift rift_bridge::`
Expected: all green.

- [ ] **Step 2: Clippy on the new code**

Run: `cargo clippy -p rift_ai --all-targets -- -D warnings`
Expected: no warnings.

- [ ] **Step 3: Confirm the deliverable**

Manually verify all of the following are true:
- `cargo run --bin rift-oss` launches a terminal named/branded Rift.
- With omlx running + a model loaded, partial input yields local AI completions.
- With omlx stopped, the terminal runs normally with no AI suggestions and no hangs.
- No `warp_*`/`warpui*` crate identifiers remain: `rg -l '\bwarp_core\b|\bwarpui\b' --type rust` returns nothing.

- [ ] **Step 4: Tag the milestone**

```bash
git tag rift-plan1-complete
```

---

## Deferred to Plan 2 (the strip — needs an investigation spike first)

- Map what depends on `auth`/cloud before deleting (the spike).
- Remove cloud AI server calls now that `rift_ai` serves suggestions.
- Strip `auth/`, `drive/`, `workspace(s)/`, `billing/`, `pricing/`, telemetry, autoupdate, cloud-agent modules, and their call sites.
- `translate()` is implemented in `rift_ai` (Task 8) but not yet surfaced in the UI; Plan 2 (or a small follow-up) wires the NL→command entry point. Tracked so it isn't forgotten.
