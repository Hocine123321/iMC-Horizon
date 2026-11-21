//! A Fabric mod loader front end for Horizon: reads mod jars, resolves them against the game, and reports what each
//! needs. See `docs/FABRIC_LAYER.md` in the repository for what it is the first part of.
pub mod classfile;
pub mod jar;
pub mod modjson;
pub mod report;
pub mod resolve;
pub mod version;
