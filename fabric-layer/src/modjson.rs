//! `fabric.mod.json`, read as Fabric's loader reads it: ids, versions, the dependency tables, entrypoints, mixin
//! configs, the access widener, nested jars and the fields a layer reads. Unknown fields are ignored.
use serde::Deserialize;
use std::collections::BTreeMap;

/// A string or a list of strings (`"depends": {"x": ">=1"}` or `{"x": [">=1", "<0.5"]}`).
#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum OneOrMany {
    One(String),
    Many(Vec<String>),
}
impl OneOrMany {
    pub fn to_vec(&self) -> Vec<String> {
        match self {
            OneOrMany::One(s) => vec![s.clone()],
            OneOrMany::Many(v) => v.clone(),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum Entrypoint {
    Plain(String),
    Full { value: String, adapter: Option<String> },
}
impl Entrypoint {
    pub fn value(&self) -> &str {
        match self {
            Entrypoint::Plain(s) => s,
            Entrypoint::Full { value, .. } => value,
        }
    }
    pub fn adapter(&self) -> &str {
        match self {
            Entrypoint::Plain(_) => "default",
            Entrypoint::Full { adapter, .. } => adapter.as_deref().unwrap_or("default"),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum MixinRef {
    Plain(String),
    Full { config: String, environment: Option<String> },
}
impl MixinRef {
    pub fn config(&self) -> &str {
        match self {
            MixinRef::Plain(s) => s,
            MixinRef::Full { config, .. } => config,
        }
    }
    pub fn environment(&self) -> &str {
        match self {
            MixinRef::Plain(_) => "*",
            MixinRef::Full { environment, .. } => environment.as_deref().unwrap_or("*"),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct NestedJar {
    pub file: String,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct ModMeta {
    pub id: String,
    pub version: String,
    #[serde(default)]
    pub name: Option<String>,
    /// `*`, `client` or `server`.
    #[serde(default)]
    pub environment: Option<String>,
    #[serde(default)]
    pub provides: Vec<String>,
    #[serde(default)]
    pub depends: BTreeMap<String, OneOrMany>,
    #[serde(default)]
    pub recommends: BTreeMap<String, OneOrMany>,
    #[serde(default)]
    pub suggests: BTreeMap<String, OneOrMany>,
    #[serde(default)]
    pub breaks: BTreeMap<String, OneOrMany>,
    #[serde(default)]
    pub conflicts: BTreeMap<String, OneOrMany>,
    #[serde(default)]
    pub entrypoints: BTreeMap<String, Vec<Entrypoint>>,
    #[serde(default)]
    pub mixins: Vec<MixinRef>,
    #[serde(default, rename = "accessWidener")]
    pub access_widener: Option<String>,
    #[serde(default)]
    pub jars: Vec<NestedJar>,
    #[serde(default, rename = "languageAdapters")]
    pub language_adapters: BTreeMap<String, String>,
}

pub fn parse(json: &str) -> Result<ModMeta, String> {
    // Some mods ship a leading byte order mark.
    let json = json.trim_start_matches('\u{feff}');
    serde_json::from_str(json).map_err(|e| format!("fabric.mod.json: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_shapes_real_mods_use() {
        let m = parse(
            r#"{ "schemaVersion": 1, "id": "friendsandfoes", "version": "5.0.5",
                 "depends": { "fabricloader": ">=0.19.0", "fabric-api": "*", "minecraft": [">=26.3", "~26.2"] },
                 "recommends": { "modmenu": "*" },
                 "entrypoints": { "main": ["a.Main"], "client": [{"value": "a.Client", "adapter": "kotlin"}] },
                 "mixins": ["a.mixins.json", {"config": "b.mixins.json", "environment": "client"}],
                 "jars": [{"file": "META-INF/jars/x.jar"}], "icon": "ignored", "custom": {"a": 1} }"#,
        )
        .unwrap();
        assert_eq!(m.id, "friendsandfoes");
        assert_eq!(m.depends["minecraft"].to_vec().len(), 2);
        assert_eq!(m.entrypoints["client"][0].adapter(), "kotlin");
        assert_eq!(m.entrypoints["main"][0].adapter(), "default");
        assert_eq!(m.mixins[1].environment(), "client");
        assert_eq!(m.mixins[0].environment(), "*");
        assert_eq!(m.jars[0].file, "META-INF/jars/x.jar");
    }
    #[test]
    fn a_byte_order_mark_and_missing_optional_fields_are_fine() {
        let m = parse("\u{feff}{\"id\":\"a\",\"version\":\"1\"}").unwrap();
        assert!(m.depends.is_empty() && m.entrypoints.is_empty() && m.environment.is_none());
    }
    #[test]
    fn a_file_without_an_id_is_an_error_not_a_panic() {
        assert!(parse("{\"version\":\"1\"}").is_err());
        assert!(parse("not json").is_err());
    }
}
