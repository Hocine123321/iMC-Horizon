# Optimizing game performance (M25): the plan

Status: **started 2026-11-17.** Phase 0 (the instrument) is built; phase 1 (the baseline) is waiting for a run on the phone.
Priority: **the most important work for an iPhone Minecraft port** (owner, 2026-11-17). It replaces, as one plan, what M6, M7, M15 step 2 to 3 and M17
described separately; those entries now point here.

## What "performance" means here

Four things, in this order of how a player feels them:

1. **Smooth.** Frame rate that holds, and few frames that take much longer than the rest (hitches, which feel worse than a lower steady rate).
2. **Stays alive.** No jetsam kill: the app never reaches the memory iOS allows it (`os_proc_available_memory`).
3. **Stays cool.** Play does not push `thermalState` to `serious` or `critical`, where iOS slows the phone for you.
4. **Light on the battery**, and a short wait from Play to the first frame.

The device that decides is the owner's **iPhone 12** (A14, 4 GB). A faster phone is a bonus, not a target.

## Rules

- **No change without a before and after** on the same route and the same phone state. A change that does not move a number is not kept.
- **One change at a time.** Two at once cannot be told apart.
- **Chosen by the platform, never global.** A tuning goes in the phone settings (the Mobile preset, Auto Performance, iOS-only code paths), so desktop
  Horizon behaves as before.
- **Pixels and parity stay.** Anything that makes the picture worse (lower resolution, shorter far terrain) is a visible trade the owner approves, and is
  a setting before it is a default. Nothing is done that changes what the game computes.
- **Honest numbers.** A value the phone could not give is empty, never 0 (`SESSION_LOG.md`); thermal start state is recorded with every run.
- Edits to `horizon/` are small, iOS-gated and written up in `HORIZON_CHANGES.md` the same day, so a future update of `horizon/` can re-apply them.

## What already exists (and what each can change)

| Lever | Where | Notes |
|---|---|---|
| Render distance, simulation distance | Options; Mobile preset | The biggest single cost driver in most voxel games; the first thing to measure at 4, 8 and 12. |
| Resolution scale and dynamic resolution | Graphics > General (`ResolutionScale`, `DynamicResolution`) | Cuts GPU work per pixel. A visible trade. |
| Present mode and pacing | Graphics > Advanced (`PresentMode`, `Pacing`) | What the frame rate is tied to; 60 versus 120 Hz also needs `CADisableMinimumFrameDurationOnPhone`. **To check on the phone.** |
| Auto Performance | Settings > Graphics > iMC | Gives memory back (render distance, far terrain, resolution) when iOS says the phone is short. Thresholds are guesses until measured. |
| Phone preset | iMC > Presets | Mobile, Battery saver; sets many of the above in one tap. |
| Worker threads, upload budget, chunk updates | Graphics > Performance | The A14 has 2 fast and 4 efficient cores; the defaults were set for desktops. |
| Far terrain / LOD, shader packs, blue map | Off on the phone by default | Large memory and GPU cost; keep off. |
| Idle engine | Done 2026-11-13 | The covered engine draws nothing while it waits for Play (and now while idle after Quit Game). |
| Not available on Metal | `GpuCulling`, `TranslucentBatching`, HDR | Hidden on Apple builds; they are not a lever here. |

## The instrument (phase 0, built 2026-11-17)

The session log (`SESSION_LOG.md`, `Documents/Horizon/session-log.csv`, one line every 5 seconds) now holds the **engine's own frame times**: frames per
second, average and worst frame, and how many frames ran over 16.7 ms (missed 60 Hz) and over 33.4 ms (missed 30 Hz). They come from five counters in the
engine (`frame_probe.rs`), read through the bridge. Together with the columns that were already there (memory footprint, memory still available, thermal
state, battery, main-thread stalls) this is enough to run phase 1.

`python3 tools/perf_summary.py session-log.csv [--from S] [--to S]` turns a log (or part of one, by seconds) into the numbers the plan compares:
frame rate, share of slow frames, peak memory and the lowest headroom, thermal start and peak, battery drop per 10 minutes.

**Still missing, to add only if phase 2 needs it:** time per frame phase (update, tick, prepare, render; the engine already measures them for
`--benchmark`) and GPU time, so a slow frame can be told apart as CPU or GPU; GPU bytes by category (the engine's census lacks depth, targets and the
atlas, `IOS_MEMORY_STRATEGY.md` 1.2).

## Phase 1: the baseline (needs the phone)

One fixed route, run **three times** on a cool phone, unplugged, Low Power Mode off, brightness fixed, Wi-Fi as usual:

1. Start the game; a **new singleplayer world with the same seed** each time (write the seed in the notes), creative.
2. Render distance **4**: walk 5 minutes in a straight line over unvisited ground, then fly 5 minutes.
3. Render distance **8**: the same. Then **12**: the same (stop early if the app is killed, and write down when).
4. Home, wait 30 seconds, return to the game (the background and back test).
5. Quit Game to the launcher, then Play again (the cycle that Quit Game now makes possible), and repeat step 2 once.

After each run: copy `session-log.csv` out (Diagnostics > Share), run `perf_summary.py` over each 5-minute stretch (`--from` and `--to`), and write the
table in `docs/ios-evidence/perf-baseline-<date>.md` with the phone, iOS version, build number, settings, seed and thermal state at the start. **Three runs
should agree**; how closely decides what size of change can be believed (the spread is the noise floor).

Then the targets get numbers. **Provisional goals, for the owner to confirm or change after the baseline:** an average at or near 60 fps at the default Mobile
preset and render distance; at most about 1 frame in 100 over 33 ms; no jetsam kill in a 30-minute session; thermal not past `fair`. Nothing is tuned
against a goal until it has a measured starting point.

## Phase 2: find the bottleneck

Read the baseline before touching anything. The shapes, and what each points at:

| What the log shows | Likely cause | First look |
|---|---|---|
| Average frame time is high and steady | Too much work every frame, on the GPU or the render thread | Resolution scale, render distance, the render phase time (add the phase columns) |
| Average is fine, but many frames over 34 ms and a large worst frame | Hitches: chunk meshing or uploads landing on the frame, or memory pressure | Upload budget, chunk updates, worker count; does it line up with new ground being walked |
| Frame rate drops **later** in the run, thermal rising | Heat throttling | Frame cap, resolution, the battery modes (M17); is it the GPU or the CPU that heats |
| `available_mb` falls toward 0 | Heading for a jetsam kill | Render distance cap, mesh pool, the arena (`IOS_MEMORY_STRATEGY.md` tiers 1 to 2) |
| `main_stalls` high while frames are fine | The window thread is held up, not the engine | The launcher, the log writer, touch handling |
| Footprint does not return after Quit Game | The game is not fully freed | The five-cycle test (NEXT_STEPS item 2) |

## Phase 3: policy and settings (low risk), then structure (only if the data demands)

In this order, each as one measured change against the baseline:

1. **Mobile preset defaults** from the baseline: render distance, resolution scale, particle and entity distance, worker threads for 2 + 4 cores.
2. **Frame pacing:** a frame cap (30, 45, 60, device maximum) and whether the phone runs at its full refresh rate; menus and the launcher at a low rate (M17).
3. **Auto Performance thresholds** and a thermal step-down (`serious` lowers a level, `critical` another, slow recovery, no flapping).
4. **Memory:** the arena constants, mesh pool and audio cache sizes for the phone (`IOS_MEMORY_STRATEGY.md` tier 1), and the memory-warning handler.
5. **Structural, only if the numbers point to it:** the integrated server's duplicate chunk data, the render target sizes, an allocator experiment, a
   narrower terrain vertex, texture compression. Each is large and risky; none is started without data that names it.

## Phase 4: keep it from getting worse

- Every kept change gets one line in `docs/ios-evidence/perf-log.md`: what, why, before, after, build number.
- The baseline route is rerun after each larger change, and before a release the owner cares about.
- Once there are numbers, CI can compare the engine's own benchmark (`--benchmark`) against a stored result; that needs a runner that can run it, so it
  is a later step (M22).

## Next actions

1. **Owner:** install the build that has the frame columns, play five minutes at the default settings, and send or share the session log (or just run
   `perf_summary.py` on it). That is a first look, not the baseline.
2. **Claude:** with that first look, decide whether the phase columns (CPU against GPU) are needed before phase 1, and write the baseline sheet template.
3. **Then** phase 1 as above.
