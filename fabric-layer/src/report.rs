//! What one mod needs from the game: the Minecraft classes and members it uses, the Mixins it applies and where, the
//! Fabric API and loader classes it calls. This is the list of what a layer has to provide for that mod.
use crate::jar::ModJar;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Serialize, Default)]
pub struct ModReport {
    pub id: String,
    pub version: String,
    pub origin: String,
    pub environment: String,
    pub entrypoints: BTreeMap<String, Vec<String>>,
    pub classes: usize,
    pub unreadable_classes: usize,
    pub minecraft_classes: usize,
    pub minecraft_members: usize,
    /// How the Minecraft classes it references split by area.
    pub minecraft_classes_by_area: BTreeMap<&'static str, usize>,
    pub mixin_classes: usize,
    pub mixin_targets: BTreeMap<String, usize>,
    pub mixin_targets_by_area: BTreeMap<&'static str, usize>,
    /// `Inject`, `Redirect`, `WrapOperation`, `Accessor`, ... and how many of each.
    pub mixin_annotations: BTreeMap<String, usize>,
    /// Where the injections go: `HEAD`, `RETURN` are simple; `INVOKE`, `FIELD`, `CONSTANT` depend on the target's code.
    pub injection_points: BTreeMap<String, usize>,
    pub fabric_api_packages: BTreeMap<String, usize>,
    pub fabric_loader_classes: usize,
    pub other_loader_references: BTreeSet<String>,
    pub access_widener: Option<String>,
    pub mixin_configs: Vec<String>,
    pub mixin_config_problems: Vec<String>,
    #[serde(skip)]
    pub minecraft_class_names: BTreeSet<String>,
    #[serde(skip)]
    pub minecraft_member_names: BTreeSet<(String, String)>,
}

/// The part of the game a Minecraft class belongs to, by its package.
pub fn area(internal: &str) -> &'static str {
    let n = internal.replace('.', "/");
    let rest = n.strip_prefix("net/minecraft/").unwrap_or(&n);
    if rest.starts_with("client/") {
        "client (rendering, screens, input)"
    } else if rest.starts_with("network/") {
        "network"
    } else if rest.starts_with("server/") {
        "server"
    } else if rest.starts_with("world/") {
        "world (levels, entities, items, blocks)"
    } else if rest.starts_with("core/") || rest.starts_with("resources/") || rest.starts_with("nbt/") || rest.starts_with("util/") || rest.starts_with("tags/") {
        "core (registries, resources, nbt, util)"
    } else {
        "other"
    }
}

/// The injection points that need no knowledge of the target's code beyond its signature.
pub const SIMPLE_POINTS: [&str; 3] = ["HEAD", "RETURN", "TAIL"];

pub fn analyse(jar: &mut ModJar) -> ModReport {
    let mut r = ModReport {
        id: jar.meta.id.clone(),
        version: jar.meta.version.clone(),
        origin: jar.origin.clone(),
        environment: jar.meta.environment.clone().unwrap_or_else(|| "*".into()),
        access_widener: jar.meta.access_widener.clone(),
        ..Default::default()
    };
    for (k, v) in &jar.meta.entrypoints {
        r.entrypoints.insert(k.clone(), v.iter().map(|e| format!("{} ({})", e.value(), e.adapter())).collect());
    }
    for (name, cfg) in jar.mixin_configs() {
        match cfg {
            Ok(c) => r.mixin_configs.push(format!("{name}: {} common, {} client, {} server{}", c.mixins.len(), c.client.len(), c.server.len(), if c.plugin.is_some() { ", has a plugin" } else { "" })),
            Err(e) => r.mixin_config_problems.push(e),
        }
    }
    let (classes, bad) = jar.classes();
    r.classes = classes.len();
    r.unreadable_classes = bad;
    for c in &classes {
        for n in &c.refs {
            if n.starts_with("net/minecraft/") {
                r.minecraft_class_names.insert(n.clone());
            } else if let Some(rest) = n.strip_prefix("net/fabricmc/fabric/api/") {
                let pkg: Vec<&str> = rest.split('/').take(3).collect();
                *r.fabric_api_packages.entry(pkg.join("/")).or_default() += 1;
            } else if n.starts_with("net/fabricmc/loader/") {
                r.fabric_loader_classes += 1;
            } else if n.starts_with("net/neoforged/") || n.starts_with("net/minecraftforge/") {
                r.other_loader_references.insert(n.split('/').take(2).collect::<Vec<_>>().join("/"));
            }
        }
        for (owner, name) in &c.members {
            if owner.starts_with("net/minecraft/") {
                r.minecraft_member_names.insert((owner.clone(), name.clone()));
            }
        }
        if !c.mixin_targets.is_empty() {
            r.mixin_classes += 1;
            for t in &c.mixin_targets {
                let t = t.replace('.', "/");
                *r.mixin_targets_by_area.entry(area(&t)).or_default() += 1;
                *r.mixin_targets.entry(t).or_default() += 1;
            }
        }
        for a in &c.mixin_annotations {
            *r.mixin_annotations.entry(a.kind.clone()).or_default() += 1;
        }
        for a in &c.at_values {
            *r.injection_points.entry(a.to_uppercase()).or_default() += 1;
        }
    }
    r.minecraft_classes = r.minecraft_class_names.len();
    r.minecraft_members = r.minecraft_member_names.len();
    for n in &r.minecraft_class_names {
        *r.minecraft_classes_by_area.entry(area(n)).or_default() += 1;
    }
    r
}

impl ModReport {
    /// Injections that depend on what is inside the target method (a call, a field, a constant, a local), against
    /// those at its start or end.
    pub fn simple_and_body_dependent(&self) -> (usize, usize) {
        let simple = self.injection_points.iter().filter(|(k, _)| SIMPLE_POINTS.contains(&k.as_str())).map(|(_, v)| v).sum();
        let all: usize = self.injection_points.values().sum();
        (simple, all - simple)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn areas_follow_the_package() {
        assert!(area("net/minecraft/client/renderer/FogRenderer").starts_with("client"));
        assert!(area("net.minecraft.world.entity.Entity").starts_with("world"));
        assert_eq!(area("net/minecraft/network/FriendlyByteBuf"), "network");
        assert_eq!(area("net/minecraft/server/level/ServerPlayer"), "server");
        assert!(area("net/minecraft/core/BlockPos").starts_with("core"));
        assert_eq!(area("com/example/Foo"), "other");
    }
}
