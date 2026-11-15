# Next steps (rewritten 2026-11-14)

This is the working plan. Read the objective first; if a task does not serve it, it waits.

## Objective

Make iMC the best way to play Minecraft: Java Edition on an iPhone: the game runs well and is smooth, and the app around it (the launcher) feels like a finished native iOS app, not a developer tool.

The game runs on the phone (confirmed 2026-11-10) and the launcher has been remade and is in daily use by the owner. What is left is measuring and tuning, finishing Play without closing the app (Quit Game back to the launcher), the pieces of the launcher that do not exist yet, and preparing the game files on the phone.

## Rules that stay in force

- Never commit Mojang files. No dependency or `Cargo.lock` change without approval.
- Do not push while a build of 10 minutes or more is running.
- `horizon/` may now be edited, but only in small, iOS-gated, documented changes (`HORIZON_CHANGES.md` and the patch files), approved by the owner on 2026-11-10.
- Glass is the system's or nothing: no hand-drawn imitation. If a piece cannot be done natively, it stays plain and we say so.
- Telemetry stays as it is. The file manager stays inside `Documents/Horizon` and asks before deleting. The Marketplace never installs native mods.
- Removed by the owner and not coming back: world backup and export, signature and update checks, multiplayer polish (old M16, M21, M23), custom extra buttons.

## What has been done (2026-11-10 to 2026-11-14)

Evidence labels: **phone** = the owner has used it on the phone and reported back; **CI** = compiled by CI (and, for Rust logic, covered by tests), not yet confirmed on a phone. Releases: v59 is the newest (2026-11-14).

**Toolchain and pipeline**
- CI builds on `macos-26` with Xcode 26 and the iOS 26 SDK (v35 onward); the build fails if the SDK is older. It repeats the compiler's first errors as annotations, checks the stack budget (the check was made to match frames by stable name pieces, 2026-11-13), checks that the built app carries every alternate icon, and the shell check keeps the theme list, the icon sets and `project.yml` in agreement. Every IPA carries its version, build number and commit, shown in About. (CI)
- The repository was renamed to `iMC-Horizon` (2026-11-13) and has a root MIT licence and a README. `CODE_MAP.md` says where everything is.

**Launcher (M12, M13, M14, M24 stages 2 and 4)**
- Remade in SwiftUI, portrait, on system Liquid Glass: Play, Packs, Controls and Settings tabs, a blue-and-red palette with nine presets and any two colours, Lightweight visuals, haptics, a dirt-block app icon with one alternate icon per colour theme (or picked by hand). The old UIKit screens are gone. **phone** (the owner has used it and reported problems, which were fixed in later builds; there is no formal walkthrough yet)
- Account page with the player's head (from Mojang's servers, optional), sign-in code card, ownership check. Download page with speed and time left, retry and states. Game files page (import, check, delete). File manager (flat tinted list, swipe actions, rename, add files, text viewer, share). **phone** for the head, file manager and Marketplace; the rest CI.
- Marketplace: registry and Modrinth resource packs, install, update, remove, the mod switch, page arrows, and a project page with an overview, a full Markdown description with Expand, and a changelog. (CI)
- Diagnostics is a real tool now: a check-up the person runs (about thirty checks with advice and a shareable report), a log viewer (crash reading, filters, memory chart), and tools. (CI)
- "Checking the game files" no longer waits for the whole manifest to be parsed and the folder walked; the verdict is remembered per manifest and the Play tab paints the last answer at once. (CI)
- Developer options (unlocked from About) hold the experimental switches. A lag pass moved file and settings work off the main thread; the session log gained main-thread tick and stall columns.

**Touch controls (M5, M19)**
- Touch editor overhaul (2026-11-09): size and fade sliders, snap and guide lines, overlap and safe-area checks, a panel that folds away, named layouts. The in-game editor is joined by an editor in the launcher (full-screen landscape).
- Cursor mode, a trackpad-style pointer for menus (2026-11-10); touch calibration; a Back button; Attack and Use fixed; Return from the iOS keyboard sends.
- 2026-11-13 and 14: Bedrock-style world controls (a tap hits or uses, a rest holds), a steadier inventory cursor (slower right click, wider slop, tap-drag grab off by default), a Shift button, Cursor and Shift in the editor, two more built-in layouts (Bedrock, Big thumbs), a "Phone preset" row that shows its state, and hold-on-air off by default with a longer wait when on. `cargo test -p horizon-client-input` covers the logic (339 tests at the time); the `game/` glue is CI-compiled. The launcher's Controls tab edits all 27 game settings through the same file. (CI; the Bedrock controls are the newest thing to try on the phone)

**Play without closing the app (M24 stage 3)**
- Built behind a switch in Developer options: the engine starts with the app and waits, the launcher sits in a window of its own above it, Play goes on without a relaunch. Tried on the phone 2026-11-11 (it worked; the boot-screen flash that showed up was fixed). **phone**
- 2026-11-13: while the engine waits it now draws nothing and wakes every 50 ms (it used to render the title panorama every frame behind the launcher, which heated the phone and made the launcher lag), and the launcher gives back its caches when it is dismissed. (CI; not measured)
- **Not done:** Quit Game still ends the process (the request groundwork exists and is inert), so there is no return to the launcher, and the five-cycle memory test has not been run.

## The order

### 1. Go through the current build on the phone, and keep a list
Install the newest release and walk every tab (Play, Packs, Controls, Settings and every page under it), the touch editor, the new Bedrock controls and the alternate icons (the one thing nobody has seen: whether a re-signed install keeps working alternate icons). Write down what looks or behaves wrong. This turns "CI-compiled" into "seen" for most of the list above. Done when the list is empty or turned into tasks.

### 2. Finish Play without closing the app (M24 stage 3, `horizon/` edit)
Quit Game returns to the launcher instead of ending the process: an idle state in the engine's `App` enum, the launcher shown again, and a fresh engine on the next Play. Then a memory test of five Play and Quit cycles on the phone (the fork measured two, on a simulator). Done when five cycles leave memory flat and the switch can move out of Developer options.

### 3. Measure, then tune (M6, M7, M15, M17)
The fixed route from `IOS_IMPLEMENTATION_ROADMAP.md` (M15 step 2): a new world, three render distances, walking and flying, five minutes each, one background and return. The session log has footprint, free memory, heat, battery and main-thread stalls; **engine frame times are the gap** and need a counter. Then the defaults (render distance, resolution scale, Auto Performance thresholds), then frame pacing and 60/120 Hz (`CADisableMinimumFrameDurationOnPhone`). Nothing is tuned without a before and after. Also measure whether the idle-engine change (2026-11-13) made the launcher smoother with Play without closing the app on.

### 4. Preparing the game files on the phone (M8a, M8b)
Open. On the owner's phone `MAP_JIT` was refused twice with a debugger attached; the `allow-jit` entitlement test build exists (2026-11-10) and no result from it is recorded. `IOS_PREPARE_WITHOUT_JIT.md` lists the routes that need no JIT and what each costs; the baseline is still a folder prepared on a computer (`PC_PREPARATION_GUIDE.md`). Download from Mojang is built and CI-compiled and waits for the sign-in below.

### 5. Sign-in (M9)
Built; needs the owner's Azure app and Mojang's approval before it works for everyone. A private test mode (another launcher's app ID, pasted in Developer options) lets the owner try it meanwhile.

### 6. Launcher pieces that do not exist yet
Worlds list (size and date) and Screenshots page (with Share and Save to Photos) (M12); folder import and quick links in the file manager (M13); checking signed Marketplace projects and revoked installed ones (M14). World backup and export, signature and update checks stay removed.

### 7. Later, each on its own
Haptics in the game and an accessibility pass (M19: bigger text, a one-handed layout beyond the left-handed one), controller support (M18; first check whether the iOS build includes the controller feature), clean exit and a one-button bug-report bundle (M20: the check-up report and "share all files" exist, a single zip and the clean-exit marker do not), tests that run the Objective-C rules in CI and a signing guide (M22, M11).

## What the owner needs to do
- Install the newest build and send the list from item 1 (including what the Bedrock controls feel like).
- Say whether the entitlement test build (item 4) could run the probes: Settings > Diagnostics > Tools > "Run the JIT probes on the next start".
- Say where Mojang's approval for the sign-in app ID stands (item 5); nothing in the repository records it.

## Reference
Roadmap: `IOS_IMPLEMENTATION_ROADMAP.md` (milestone table and dated progress notes). Decisions: `IOS_DECISIONS_AND_APP_VISION.md`. What the launcher does: `LAUNCHER.md`. Where code lives: `CODE_MAP.md`. What changed under `horizon/`: `HORIZON_CHANGES.md`. What happened and why: `SESSION_LOG.md`. Fork findings: the `heyskylo/horizon-ios` repo (MIT; keep its copyright notice if code is copied).
