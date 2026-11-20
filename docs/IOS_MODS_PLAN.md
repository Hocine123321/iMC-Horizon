# Horizon mods on iOS: what is possible and the plan (2026-11-20)

Goal (owner, 2026-11-20): the Rust mods of Horizon's registry should work in the iOS app; today the Marketplace labels them "Desktop only".
Everything below was read from upstream (`horizon-rust/horizon` at `1f0410f`, 2026-11-20; `horizon-rust/horizon-mods` at 2026-11-20), not run.

## What upstream has now

| Kind | What it is | How it runs |
|---|---|---|
| Content mod (`.hmod`, `kind = "mod"`) | Data and assets | Read at start. Works on the phone today. |
| **WebAssembly mod** (`.hmod` with `[wasm]` and `mod.wasm`, `kind = "mod"`) | Rust against the `mod_api` SDK (hooks, Parts, `[[inject]]` listeners), built to wasm | Run by the game through **wasmtime 50** (`crates/horizon-mod-wasm`), sandboxed. "Nothing to build": Horizon's own Zoom is one since 3.0.0. |
| **Native mod** (`.hmod` with `[native]`, `kind = "native-mod"`) | Rust that changes the game's own code (`[[native.mixin]]`, loops, bridges) | Compiled **by the launcher on the player's computer** with a pinned Rust toolchain against the **exact Horizon commit** named in `[native] horizon`; registry-built binaries are not used. |

The live registry today holds 143 native mods, 44 `mod` entries (content or wasm), 7 shaders, 5 resource packs and 1 data pack. The 137 older source-patching
(`.hnmod`) projects are frozen in `archive/native-v1/` and are not in the live index. The two other places this repo says "133 native mods" are from the
index of 2026-11-10.

## Why native mods cannot simply run on iOS

1. iOS has no compiler for the app to use, and it will not load native code that was not in the signed app when it was installed.
2. Each native mod pins one exact Horizon commit. The iOS app is one fixed commit plus this project's patches, so a mod built for another commit does not match.
3. The only way a native mod gets in is compiled into the IPA, by CI, for that one mod and that one commit. That is a custom build per mod, not a Marketplace install.

## The route that works: WebAssembly mods

- A mod written against `mod_api` builds to native **or** to `mod.wasm` from the same source; the game loads either (`how-to/wasm-mods.md` in the modding book).
- A wasm mod is data to the app, not native code, so installing one on the phone is allowed like any other file.
- The runtime is wasmtime with the `cranelift`, `runtime` and **`pulley`** features. Cranelift's normal output is machine code in executable memory, which iOS
  refuses without JIT (our JIT tests still fail). **Pulley** is wasmtime's portable bytecode interpreter: it needs no executable memory. The runtime already has
  a Pulley case (`engine.is_pulley()` in its tests). To use it on the phone, `engine_config()` in `horizon-mod-wasm/src/runtime.rs` would set
  `config.target("pulley64")` on iOS only. **Not yet confirmed:** that wasmtime 50 with Pulley builds for `aarch64-apple-ios` and runs a real mod on the phone.
- Costs to measure: mods run slower in an interpreter (fine for a zoom or a HUD, a concern for a mod that works every tick), and compiling a `mod.wasm` to Pulley
  bytecode at load uses memory and time (`compile_budget.rs` exists upstream for exactly this).
- What cannot be wasm: anything that needs `[native]` tables (the game's private types, a change in the middle of a function, a hot loop).

## Prerequisite: our `horizon/` is too old for it

`horizon/` is on upstream `2832168` (2026-11-16). It has `horizon-hooks`, `horizon-mod-api-gen` and `horizon-mods` but **no wasm runtime** (no wasmtime in
`Cargo.lock`). Upstream `1f0410f` (2026-11-20) has `horizon-mod-wasm`, `horizon-mod-host`, `horizon-mod-runtime` and the rest of the SDK.

Dry run (a scratch copy, nothing changed here): our `docs/horizon-changes-upstream.patch` applied to `1f0410f` with `git apply --reject`: 15 of the 69 files
had rejected hunks, 37 hunks in all (`game.rs` 12, `graphics_ui.rs` 6, `options_ui.rs` 4, `gfx.rs` 3, `settings.rs` 2, the rest 1 each, and `Cargo.lock`
and `Cargo.toml`). The other 54 files applied cleanly. That is a re-port of a few hours, using `HORIZON_CHANGES.md` as the guide, not a rewrite.

## Plan

1. **Move `horizon/` to current upstream** (the re-port above), as a new baseline commit and a re-applied patch, the way section R of `HORIZON_CHANGES.md` did it. CI
   must build, `tools/horizon_diff.sh --check` must pass, and Quit Game, sign-in and the settings pages must still be as before.
2. **Make the wasm runtime iOS-safe**: Pulley on iOS in `engine_config()`, one small gated edit (written up in `HORIZON_CHANGES.md`). Prove it compiles for
   `aarch64-apple-ios` in CI, and runs a wasm mod in the simulator.
3. **A real mod on the phone**: install Horizon's Zoom `.hmod` (a wasm mod) and turn it on from the game's Options > Mods. Record load time, memory and the speed
   of its hooks in the session log.
4. **The Marketplace**: `mod` entries with `[wasm]` become installable ("Works on iPhone"); native mods say why they are not and whether the project has a
   wasm version; the registry-index rules in `MarketIndex.m` are updated for the `[wasm]` table.
5. **Native mods the owner wants**: one at a time. If it only uses hooks, Parts and `[[inject]]`, build it to wasm from its source (its licence permitting).
   If it needs `[native]`, the only route is a custom IPA built by CI at that mod's pinned Horizon commit, which also needs our patches re-applied there.

## Not decided

Whether to keep following upstream's newest commit every few days (cheap each time now that the patch and baseline exist, but the wasm and mod runtime are
moving quickly) or to pin a commit after step 2 works and update only on a schedule.
