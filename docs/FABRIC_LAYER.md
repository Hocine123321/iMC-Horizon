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

## Decisions (2026-11-21, owner)

- **Where it lives:** Horizon-native, and it must also work on iOS. If it cannot be an ordinary Horizon mod, it goes into the fork's `horizon/` source (a crate
  plus small gated hooks, recorded in `HORIZON_CHANGES.md` like every other edit there).

## What that means for the design

1. **Do not rewrite Fabric's loader; give it a game.** Fabric Loader (Apache-2.0) already discovers mods, runs entrypoints and applies Mixin, and it takes the
   game through a pluggable game provider (`net.fabricmc.loader.impl.game.GameProvider`; I have not read its current source, so check before relying on it).
   A "Horizon game provider" plus Mixin (MIT) lets the real loader do that work against our classes, and the Rust `fabric-layer` crate stays what it is today:
   the preflight check (does this mod set resolve on this game, and what does it need) that runs before any JVM starts.
2. **Rust hosts and routes; Java carries the facade.** The Minecraft and Fabric API classes the mods call are Java classes (a facade jar built by CI) whose
   engine-facing methods are `native` and implemented in Rust, so the same facade works on any JVM that supports JNI. Engine events (a tick, a block break, a
   screen opening) are routed from Rust into the facade's methods, where Mixin injections at a method's start or end then run the mod's code.
3. **The JVM is the gate for iOS.** On the owner's phone a normal HotSpot VM cannot run, even with `-Xint`, because its interpreter is generated into executable
   memory (`IOS_PREPARE_WITHOUT_JIT.md` section 2). The only JIT-free JVM that is a real option is OpenJDK's **Zero** variant (a plain C++ interpreter), which
   has to be linked into the app statically; nobody has shown it running on iOS (`IOS_PREPARE_WITHOUT_JIT.md` route D). The layer therefore talks to the JVM
   through one small host interface (start, load a class, call a method, register natives), with HotSpot loaded from a JDK 25 on desktop and Zero linked in
   on iOS if that experiment succeeds. If Zero cannot be built or run, iOS needs a Rust bytecode interpreter plus the JDK's classes, which is a much larger job.
4. **The facade's shape comes from the real game's signatures.** A mod's bytecode is only valid against the right class hierarchy (a `Zombie` has to be a
   `LivingEntity`), and the mods alone do not carry it. The practical source is the player's own 26.3 client jar, read locally to produce a signature-only
   facade (class tree, fields, method descriptors; no code), the way a mapping step does. It is never committed or redistributed.

## Next steps, in order

1. **The Zero experiment (the iOS gate):** can the existing CI recipe (`java25-ios-build.yml`) build OpenJDK 25 Zero for iOS, and does it load and run a class in the
   simulator? It is a long macOS job and is the one result that decides whether iOS is a normal part of this plan or a later research project.
2. **A signature index of the player's 26.3 client jar** (a local tool, output not committed), then a facade generator that stubs every class and member the mods use.
3. **A Horizon game provider and a first run on desktop:** GlitchCore's and ResourcefulLib's entrypoints on HotSpot, with the stubs that fail printing which member was
   missing, until they initialize.
4. Then the Fabric API modules those use, then Biomes O' Plenty and TerraBlender.

## Still open

- Whether the owner will provide the 26.3 client jar for step 2 (it stays on the machine it is used on).
- Whether to start the Zero build (step 1) now: it uses a few hours of macOS CI time and changes nothing else in the repository.
- Licences: several of these mods are not freely redistributable; the layer never contains them and reads them from the player's own folder.
