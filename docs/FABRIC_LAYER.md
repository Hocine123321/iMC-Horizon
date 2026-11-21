# Fabric mods on Horizon: the translation layer (started 2026-11-21)

Owner decision (2026-11-21): build a Rust loader and translation layer so Fabric mods run on Horizon without rewriting them. The iOS side of mods
(`IOS_MODS_PLAN.md`) is held for later; this is desktop first. Part 1 is `fabric-layer/` (this repository, outside `horizon/`).

## What was measured (seven real jars for Minecraft 26.3, scanned 2026-11-21)

`fabric-inspect` on Xaero's Minimap 26.6.2, ResourcefulLib 6.0.1, Biomes O' Plenty 26.3.0.0.7, Fabric API 0.166.2, Not Enough Animations 1.12.7,
Friends and Foes 5.0.5 and GlitchCore 26.3.0.0.5:

- **No name mapping is needed.** All seven use Minecraft's own class names (`net/minecraft/world/entity/Entity`), none the old `class_1234`
  intermediary names. Horizon plays the same 26.3.
- **63 mods once nested jars are counted** (Fabric API alone is 44). The load order resolves; the only hard problem is that Biomes O' Plenty needs
  **TerraBlender >= 26.3.0.0.6**, which was not among the jars. Mod Menu and YACL are optional and absent.
- **The Minecraft API the six mods use** (Fabric API's own modules left out): **1,303 classes and about 3,850 of their members**; by package, 339
  `world/level`, 210 `world/entity`, 114 `client/renderer`, 75 `world/item`, 73 `client/model`, 67 `client/gui`.
- **Fabric API itself** is 44 modules, 2,024 classes, **484 Mixin classes** and 1,125 distinct Minecraft classes. Its mixins target the real game's code, so
  a layer provides the Fabric API's behaviour itself instead of running its mixins.
- **Mixins per mod** (targets; injections at a method's start or end / inside its body):

| Mod | Mixin targets | Start or end | Inside a body | What they touch |
|---|---|---|---|---|
| Biomes O' Plenty | 2 | 3 | 0 | fog and screen effects (client rendering); the rest is content registered through GlitchCore and TerraBlender |
| GlitchCore | 14 | 14 | 6 | mixed: client, server, a few world |
| ResourcefulLib | 8 | 4 | 5 | mostly client rendering and server |
| Xaero's Minimap (with Xaero's Lib) | 41 | 42 | 12 | mostly client UI and rendering (37 of 41), some server and network |
| Not Enough Animations (with TRender, TRansition) | 22 | 19 | 1 | player and entity model and render code (all but a few) |
| Friends and Foes | 49 | 19 | 20 | mob AI and behaviour in vanilla entity classes (52 of 56 target uses in world and entity) |

## What a layer has to be

1. **The loader** (done as a front end in `fabric-layer`): read jars, resolve dependencies, order them, find entrypoints and mixin configs.
2. **Something that runs the bytecode.** Fabric API 0.166.2 requires Java 25. A real JVM beside the game is the realistic start (the same direction as
   upstream's Java bridge, `IOS_MODS_PLAN.md`); a Rust bytecode interpreter would also need Java's whole standard library.
3. **The Minecraft and Fabric API classes the mods call, as a facade** over Horizon's Rust game. The measured size above is the floor for these seven mods.
4. **Mixins.** An injection at a method's start or end can be given a Horizon hook. An injection inside a method's body (a call, a field, a constant, a
   local) needs the original method's code, which a facade does not have, so each of those needs its own native equivalent or is dropped. The mods
   that patch rendering and model internals (Not Enough Animations, most of Xaero's) change code Horizon's Rust renderer does not share.

## Which mod first

- **Libraries and registration first:** GlitchCore, ResourcefulLib and the Fabric API modules those use (lifecycle events, networking, registry, resource
  loader) prove the loader, the JVM and the facade without any gameplay.
- **Then Biomes O' Plenty** (with TerraBlender): 2 render mixins and mostly content, so it is the closest to a real playable result, provided Horizon can
  register new blocks and biomes from a mod.
- **Hardest:** Friends and Foes (new mobs with AI inside vanilla entity code, 20 body-dependent injections) and Xaero's (a map that reads the world and draws
  over the client). Not Enough Animations changes how vanilla models animate; Horizon's own renderer decides that.

## Built so far (`fabric-layer/`, `cargo test`: 18 tests)

`classfile.rs` (class file constant pool and Mixin annotations, no bytecode), `modjson.rs` (`fabric.mod.json`), `version.rs` (Fabric version predicates),
`jar.rs` (jars and jar-in-jar), `resolve.rs` (dependencies, conflicts, load order), `report.rs` (what each mod needs) and the `fabric-inspect` command.
It runs no mod code.

## Open decisions

- How the layer runs Java: a JDK 25 the player installs, or one Horizon downloads (upstream's docs say its Java bridge would be an optional download).
- Where the layer lives: as a Horizon native mod (desktop only, built against a pinned Horizon commit) or as a separate process Horizon talks to.
- Licences: several of these mods are not freely redistributable; the layer must never contain them, and it reads them from the player's own folder.
