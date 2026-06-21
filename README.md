<p align="center">
  <img src="branding/rift-hero.png" alt="Rift" width="820">
</p>

<p align="center">
  <em>A personal take on a nice terminal.</em>
</p>

<p align="center">
  <img src="branding/rift-demo.gif" alt="Rift demo: commands, history menu, command search, tabs" width="820">
</p>

**Rift** is a fork of [Warp](https://www.warp.dev) that keeps the good terminal and leaves the rest behind, small enough that I can read and change it myself.

## Why?

Even though I use Claude Code as my daily driver, I don't want my terminal to be "AI native", or have any telemetry, autoupdates, or cloud features at all. I just want a nice looking terminal that lets me pin the prompt at the bottom, and no other terminal gave me that cleanly.

Warp did, but it came wrapped in all the stuff I didn't want. So when it open-sourced, I forked it, kept the parts I liked, and cut the rest.

## What you get

- **Command blocks** you can scroll, select, copy, and rerun, instead of one endless wall of text
- **A prompt you can pin to the bottom**, so your input stays put no matter how much scrolls by
- **GPU rendering** (wgpu/Metal) that stays smooth with huge scrollback
- **Editor-style input** with multiline editing, syntax awareness, and a vim mode
- **Command search and a history menu** for fast recall
- **Vertical tabs** down the side
- **Themes**, including a built-in dark theme
- **SSH integration** that carries blocks and input modes to remote sessions
- **Fish-style autosuggestion** from your history plus rule-based fixes, with no AI or network
- **Inline images** via the iTerm and Kitty graphics protocols

## Download

Grab the latest `.dmg` from the [Releases](../../releases/latest) page, open it, and drag Rift into Applications.

Apple Silicon only (M1 or newer). Intel Macs aren't supported.

It isn't notarized yet, so the first time you run it, clear the quarantine flag:

```sh
xattr -dr com.apple.quarantine /Applications/Rift.app
```

You can also right-click the app and choose Open, or [build it yourself](#building).

## How Rift differs from Warp

| | Warp | Rift |
|---|---|---|
| Account / login | Required for most features | None. Nothing can be login-walled or rate-limited. |
| Telemetry | Live Rudderstack key, UGC events | Gone. No phone-home, because the code isn't there. |
| AI agents | The core of the product | Gone. No agent, no MCP, no inline AI. |
| Cloud / Drive / teams | Woven throughout | Gone. Fully offline, one local user. |
| Billing / credits | Compiled in, "buy credits" banner | Gone. |
| Auto-update | On | Removed. You stay on the version you build. |

## What got stripped

Compared against `warpdotdev/warp` (the `upstream` remote):

- About 703,000 lines of Rust gone, roughly half the codebase (1.39M down to 691K).
- About 1,500 source files deleted.
- 20 crates removed (71 to 51): the whole `ai`, cloud-object, server, auth, GraphQL, and firebase layers.

This was never about shipping a smaller binary. It's that the code is actually gone, not flagged off, so there's nothing left to quietly switch back on.

## Building

The default binary is `rift-oss`. The toolchain is pinned in `rust-toolchain.toml`, and you'll need `protoc` (`brew install protobuf`).

```bash
./script/bootstrap   # platform-specific setup
./script/run         # build and run Rift
./script/presubmit   # fmt, clippy, and tests
```

[CLAUDE.md](CLAUDE.md) has the full engineering guide: coding style, testing, and platform notes.

## Relationship to upstream

Rift tracks `warpdotdev/warp` as the `upstream` remote, and I pull changes by hand with cherry-picks, so nothing depends on someone else porting them first. The whole codebase is renamed from `warp` to `rift`, so it drifts from upstream on purpose. That's the trade for owning the fork outright.

Last synced with upstream: **2026-06-14**.

## A personal fork

I built this for myself, and I'm sharing it in case others wanted it too. It's shaped for how I work, not really meant to be a product, so I'll keep it pointed at my own use. Issues and PRs are welcome.

Not affiliated with or endorsed by Warp.

## Licensing

Inherited from Warp. The UI framework crates (`riftui_core` and `riftui`) are MIT ([LICENSE-MIT](LICENSE-MIT)); everything else is AGPL v3 ([LICENSE-AGPL](LICENSE-AGPL)).

## Open source dependencies

A few of the projects Rift (and Warp before it) is built on:

- [Tokio](https://github.com/tokio-rs/tokio)
- [NuShell](https://github.com/nushell/nushell)
- [Fig Completion Specs](https://github.com/withfig/autocomplete)
- [Alacritty](https://github.com/alacritty/alacritty)
- [FontKit](https://github.com/servo/font-kit)
- [Core-foundation](https://github.com/servo/core-foundation-rs)
- [Smol](https://github.com/smol-rs/smol)
