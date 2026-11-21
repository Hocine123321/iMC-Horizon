//! Reading a mod jar: its `fabric.mod.json`, the jars nested in it (jar-in-jar), its mixin configs and its classes.
use crate::classfile::{self, ClassInfo};
use crate::modjson::{self, ModMeta};
use serde::Deserialize;
use std::io::{Cursor, Read};
use zip::ZipArchive;

/// One mod's files, in memory.
pub struct ModJar {
    pub meta: ModMeta,
    /// Where it came from: the file name, or `outer.jar!inner.jar` for a nested one.
    pub origin: String,
    archive: ZipArchive<Cursor<Vec<u8>>>,
}

/// A mixin config (`*.mixins.json`), reduced to what a layer needs.
#[derive(Debug, Deserialize, Default)]
pub struct MixinConfig {
    #[serde(default)]
    pub package: String,
    #[serde(default)]
    pub mixins: Vec<String>,
    #[serde(default)]
    pub client: Vec<String>,
    #[serde(default)]
    pub server: Vec<String>,
    #[serde(default)]
    pub plugin: Option<String>,
    #[serde(default)]
    pub refmap: Option<String>,
}

const MAX_NESTING: usize = 3;
const MAX_ENTRY_BYTES: u64 = 256 * 1024 * 1024;

impl ModJar {
    pub fn open(bytes: Vec<u8>, origin: &str) -> Result<ModJar, String> {
        let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(|e| format!("{origin}: not a jar ({e})"))?;
        let json = read_entry(&mut archive, "fabric.mod.json").ok_or_else(|| format!("{origin}: no fabric.mod.json"))?;
        let meta = modjson::parse(&String::from_utf8_lossy(&json)).map_err(|e| format!("{origin}: {e}"))?;
        Ok(ModJar { meta, origin: origin.to_string(), archive })
    }

    /// This jar's nested mods, as the loader would add them (`jars` in `fabric.mod.json`), with their own nested jars.
    pub fn nested(&mut self) -> Vec<Result<ModJar, String>> {
        self.nested_to(MAX_NESTING)
    }
    fn nested_to(&mut self, depth: usize) -> Vec<Result<ModJar, String>> {
        let mut out = Vec::new();
        if depth == 0 {
            return out;
        }
        for j in self.meta.jars.clone() {
            // A nested path stays inside the jar: no `..`, no absolute path.
            if j.file.contains("..") || j.file.starts_with('/') {
                out.push(Err(format!("{}: nested jar path {:?} is refused", self.origin, j.file)));
                continue;
            }
            let Some(bytes) = read_entry(&mut self.archive, &j.file) else {
                out.push(Err(format!("{}: nested jar {} is missing", self.origin, j.file)));
                continue;
            };
            let origin = format!("{}!{}", self.origin, j.file.rsplit('/').next().unwrap_or(&j.file));
            match ModJar::open(bytes, &origin) {
                Ok(mut inner) => {
                    let deeper = inner.nested_to(depth - 1);
                    out.push(Ok(inner));
                    out.extend(deeper);
                }
                // A nested library with no fabric.mod.json is a plain library: not a mod, not an error.
                Err(e) if e.contains("no fabric.mod.json") => {}
                Err(e) => out.push(Err(e)),
            }
        }
        out
    }

    pub fn mixin_configs(&mut self) -> Vec<(String, Result<MixinConfig, String>)> {
        let mut out = Vec::new();
        for m in self.meta.mixins.clone() {
            let name = m.config().to_string();
            let cfg = match read_entry(&mut self.archive, &name) {
                None => Err(format!("{name}: not in the jar")),
                Some(b) => serde_json::from_slice::<MixinConfig>(&b).map_err(|e| format!("{name}: {e}")),
            };
            out.push((name, cfg));
        }
        out
    }

    /// Every class in this jar (not in its nested jars), parsed; a class that cannot be read is skipped and counted.
    pub fn classes(&mut self) -> (Vec<ClassInfo>, usize) {
        let names: Vec<String> = self
            .archive
            .file_names()
            .filter(|n| n.ends_with(".class") && !n.starts_with("META-INF/"))
            .map(str::to_string)
            .collect();
        let mut out = Vec::with_capacity(names.len());
        let mut bad = 0;
        for n in names {
            match read_entry(&mut self.archive, &n).map(|b| classfile::parse(&b)) {
                Some(Ok(c)) => out.push(c),
                _ => bad += 1,
            }
        }
        (out, bad)
    }

    pub fn has_entry(&self, name: &str) -> bool {
        self.archive.file_names().any(|n| n == name)
    }
}

fn read_entry(archive: &mut ZipArchive<Cursor<Vec<u8>>>, name: &str) -> Option<Vec<u8>> {
    let f = archive.by_name(name).ok()?;
    if f.size() > MAX_ENTRY_BYTES {
        return None;
    }
    let mut buf = Vec::with_capacity(f.size() as usize);
    f.take(MAX_ENTRY_BYTES).read_to_end(&mut buf).ok()?;
    Some(buf)
}
