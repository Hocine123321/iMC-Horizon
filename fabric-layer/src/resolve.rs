//! Dependency resolution as Fabric's loader does it, for a set of mods and the game they run on: which are missing or
//! too old, what breaks what, and the order they load in (a mod after the mods it depends on).
use crate::modjson::ModMeta;
use crate::version::{self, Version};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

/// What the game and the runtime provide as mods of their own.
#[derive(Debug, Clone)]
pub struct Host {
    pub minecraft: String,
    pub java: String,
    pub loader: String,
}
impl Default for Host {
    fn default() -> Self {
        Host { minecraft: "26.3".into(), java: "25".into(), loader: "0.19.3".into() }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Problem {
    pub mod_id: String,
    pub kind: &'static str, // missing | too-old-or-new | breaks | conflicts | unreadable | cycle
    pub detail: String,
}

#[derive(Debug, Serialize)]
pub struct Resolution {
    /// Mod ids in load order.
    pub order: Vec<String>,
    pub problems: Vec<Problem>,
    /// Optional dependencies (recommends, suggests) that are not present: only informative.
    pub missing_optional: Vec<(String, String)>,
}

pub fn resolve(mods: &[&ModMeta], host: &Host) -> Resolution {
    // id (and every `provides` alias) -> version
    let mut avail: BTreeMap<String, String> = BTreeMap::new();
    avail.insert("minecraft".into(), host.minecraft.clone());
    avail.insert("java".into(), host.java.clone());
    avail.insert("fabricloader".into(), host.loader.clone());
    let mut present: BTreeMap<String, &ModMeta> = BTreeMap::new();
    for m in mods {
        match present.get(&m.id) {
            // Two jars with one id: the higher version is used, as Fabric does for nested duplicates.
            Some(old) if Version::parse(&old.version) >= Version::parse(&m.version) => continue,
            _ => {}
        }
        present.insert(m.id.clone(), m);
    }
    for m in present.values() {
        avail.insert(m.id.clone(), m.version.clone());
        for p in &m.provides {
            avail.entry(p.clone()).or_insert_with(|| m.version.clone());
        }
    }
    let mut problems = Vec::new();
    let mut missing_optional = Vec::new();
    let mut edges: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (id, m) in &present {
        for (dep, preds) in &m.depends {
            edges.entry(id.clone()).or_default();
            match avail.get(dep) {
                None => problems.push(Problem { mod_id: id.clone(), kind: "missing", detail: format!("needs {dep} {}", preds.to_vec().join(" or ")) }),
                Some(have) => {
                    let v = Version::parse(have);
                    match v.as_ref().and_then(|v| version::satisfies_any(v, &preds.to_vec())) {
                        Some(true) => {}
                        Some(false) => problems.push(Problem { mod_id: id.clone(), kind: "too-old-or-new", detail: format!("needs {dep} {} but there is {have}", preds.to_vec().join(" or ")) }),
                        None => problems.push(Problem { mod_id: id.clone(), kind: "unreadable", detail: format!("cannot compare {dep} {have} with {}", preds.to_vec().join(" or ")) }),
                    }
                    if present.contains_key(dep) {
                        edges.get_mut(id).unwrap().insert(dep.clone());
                    }
                }
            }
        }
        for (dep, preds) in m.recommends.iter().chain(m.suggests.iter()) {
            if !avail.contains_key(dep) {
                missing_optional.push((id.clone(), format!("{dep} {}", preds.to_vec().join(" or "))));
            } else if present.contains_key(dep) {
                edges.entry(id.clone()).or_default().insert(dep.clone());
            }
        }
        for (table, kind) in [(&m.breaks, "breaks"), (&m.conflicts, "conflicts")] {
            for (other, preds) in table {
                if let Some(have) = avail.get(other) {
                    if Version::parse(have).and_then(|v| version::satisfies_any(&v, &preds.to_vec())) == Some(true) {
                        problems.push(Problem { mod_id: id.clone(), kind, detail: format!("{kind} {other} {have}") });
                    }
                }
            }
        }
    }
    // Load order: depth first, dependencies first; a cycle is reported once and cut.
    let mut order = Vec::new();
    let mut state: BTreeMap<&str, u8> = BTreeMap::new();
    fn visit<'a>(id: &'a str, edges: &'a BTreeMap<String, BTreeSet<String>>, state: &mut BTreeMap<&'a str, u8>, order: &mut Vec<String>, problems: &mut Vec<Problem>) {
        match state.get(id) {
            Some(2) => return,
            Some(1) => {
                problems.push(Problem { mod_id: id.to_string(), kind: "cycle", detail: "depends on itself through other mods".into() });
                return;
            }
            _ => {}
        }
        state.insert(id, 1);
        if let Some(ds) = edges.get(id) {
            for d in ds {
                visit(d, edges, state, order, problems);
            }
        }
        state.insert(id, 2);
        order.push(id.to_string());
    }
    for id in present.keys() {
        visit(id, &edges, &mut state, &mut order, &mut problems);
    }
    Resolution { order, problems, missing_optional }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modjson::parse;

    fn m(json: &str) -> ModMeta {
        parse(json).unwrap()
    }

    #[test]
    fn dependencies_load_first_and_a_clean_set_has_no_problems() {
        let a = m(r#"{"id":"a","version":"1","depends":{"minecraft":"~26.3","b":">=2"}}"#);
        let b = m(r#"{"id":"b","version":"2.1","depends":{"fabricloader":">=0.19.0","java":">=21"}}"#);
        let r = resolve(&[&a, &b], &Host::default());
        assert!(r.problems.is_empty(), "{:?}", r.problems);
        assert_eq!(r.order, vec!["b", "a"]);
    }
    #[test]
    fn missing_old_and_game_version_problems() {
        let a = m(r#"{"id":"a","version":"1","depends":{"fabric-api":"*","b":">=3","minecraft":">=26.4"}}"#);
        let b = m(r#"{"id":"b","version":"2"}"#);
        let r = resolve(&[&a, &b], &Host::default());
        let kinds: Vec<_> = r.problems.iter().map(|p| (p.mod_id.as_str(), p.kind)).collect();
        assert!(kinds.contains(&("a", "missing")));
        assert_eq!(kinds.iter().filter(|k| k.1 == "too-old-or-new").count(), 2, "{kinds:?}");
    }
    #[test]
    fn optional_dependencies_are_only_noted_and_breaks_is_a_problem() {
        let a = m(r#"{"id":"a","version":"1","recommends":{"modmenu":"*"},"breaks":{"b":"<2"}}"#);
        let b = m(r#"{"id":"b","version":"1"}"#);
        let r = resolve(&[&a, &b], &Host::default());
        assert_eq!(r.missing_optional.len(), 1);
        assert!(r.problems.iter().any(|p| p.kind == "breaks"));
    }
    #[test]
    fn provides_aliases_satisfy_a_dependency() {
        let a = m(r#"{"id":"a","version":"1","depends":{"old-name":"*"}}"#);
        let b = m(r#"{"id":"b","version":"1","provides":["old-name"]}"#);
        assert!(resolve(&[&a, &b], &Host::default()).problems.is_empty());
    }
    #[test]
    fn a_cycle_is_reported_and_does_not_loop() {
        let a = m(r#"{"id":"a","version":"1","depends":{"b":"*"}}"#);
        let b = m(r#"{"id":"b","version":"1","depends":{"a":"*"}}"#);
        let r = resolve(&[&a, &b], &Host::default());
        assert!(r.problems.iter().any(|p| p.kind == "cycle"));
        assert_eq!(r.order.len(), 2);
    }
    #[test]
    fn the_higher_of_two_duplicate_ids_wins() {
        let a1 = m(r#"{"id":"lib","version":"1.0"}"#);
        let a2 = m(r#"{"id":"lib","version":"2.0"}"#);
        let user = m(r#"{"id":"u","version":"1","depends":{"lib":">=2"}}"#);
        assert!(resolve(&[&a1, &a2, &user], &Host::default()).problems.is_empty());
    }
}
