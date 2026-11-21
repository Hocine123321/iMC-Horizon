# fabric-layer

The first part of a layer that lets Fabric mods run on Horizon: a Rust front end that does what Fabric's loader does with a mod jar, and tells you what
the mod needs from the game. See `docs/FABRIC_LAYER.md`.

```sh
cargo run --release --bin fabric-inspect -- path/to/mods [--json] [--mc 26.3] [--java 25] [--loader 0.19.3]
cargo test
```

It reads `fabric.mod.json` (ids, versions, `depends`/`recommends`/`breaks`, entrypoints, mixin configs, nested jars), checks version predicates the way
Fabric does (`>=`, `~26.3-`, `^`, `1.2.x`, ranges), resolves the mods against the game and puts them in load order, and reads each class file's
constant pool and Mixin annotations (no bytecode is run). It runs no mod code. Test jars are not in the repository (they are other people's work):
point `fabric-inspect` at your own.
