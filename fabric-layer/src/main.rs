//! `fabric-inspect JAR|DIR... [--json] [--mc 26.3] [--java 25] [--loader 0.19.3]`
use fabric_layer::jar::ModJar;
use fabric_layer::report::{self, ModReport};
use fabric_layer::resolve::{self, Host};
use std::collections::BTreeSet;
use std::path::Path;

fn collect_jars(path: &Path, out: &mut Vec<std::path::PathBuf>) {
    if path.is_dir() {
        let mut entries: Vec<_> = std::fs::read_dir(path).map(|d| d.flatten().map(|e| e.path()).collect()).unwrap_or_default();
        entries.sort();
        for e in entries {
            collect_jars(&e, out);
        }
    } else if path.extension().is_some_and(|e| e == "jar") {
        out.push(path.to_path_buf());
    }
}

fn main() {
    let mut json = false;
    let mut host = Host::default();
    let mut paths = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--json" => json = true,
            "--mc" => host.minecraft = args.next().unwrap_or_default(),
            "--java" => host.java = args.next().unwrap_or_default(),
            "--loader" => host.loader = args.next().unwrap_or_default(),
            "-h" | "--help" => {
                eprintln!("usage: fabric-inspect JAR|DIR... [--json] [--mc 26.3] [--java 25] [--loader 0.19.3]");
                return;
            }
            _ => paths.push(a),
        }
    }
    let mut files = Vec::new();
    for p in &paths {
        collect_jars(Path::new(p), &mut files);
    }
    if files.is_empty() {
        eprintln!("no jars given");
        std::process::exit(2);
    }
    let mut mods: Vec<ModJar> = Vec::new();
    let mut errors = Vec::new();
    for f in &files {
        let name = f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        match std::fs::read(f).map_err(|e| e.to_string()).and_then(|b| ModJar::open(b, &name)) {
            Ok(mut m) => {
                let nested = m.nested();
                mods.push(m);
                for n in nested {
                    match n {
                        Ok(j) => mods.push(j),
                        Err(e) => errors.push(e),
                    }
                }
            }
            Err(e) => errors.push(e),
        }
    }
    let metas: Vec<_> = mods.iter().map(|m| &m.meta).collect();
    let resolution = resolve::resolve(&metas, &host);
    let reports: Vec<ModReport> = mods.iter_mut().map(report::analyse).collect();

    if json {
        let v = serde_json::json!({ "host": { "minecraft": host.minecraft, "java": host.java, "loader": host.loader }, "errors": errors, "resolution": resolution, "mods": reports });
        println!("{}", serde_json::to_string_pretty(&v).unwrap());
        return;
    }
    println!("Host: Minecraft {}, Java {}, Fabric Loader {}", host.minecraft, host.java, host.loader);
    println!("{} mods (nested jars included), load order:", reports.len());
    println!("  {}", resolution.order.join(" -> "));
    for e in &errors {
        println!("ERROR {e}");
    }
    if resolution.problems.is_empty() {
        println!("Dependencies: all satisfied");
    }
    for p in &resolution.problems {
        println!("PROBLEM [{}] {}: {}", p.kind, p.mod_id, p.detail);
    }
    for (id, dep) in &resolution.missing_optional {
        println!("  optional, not present: {id} wants {dep}");
    }
    let mut all_classes = BTreeSet::new();
    let mut all_members = BTreeSet::new();
    for r in &reports {
        println!("\n== {} {} ({}) [{}]", r.id, r.version, r.origin, r.environment);
        if r.classes == 0 && r.entrypoints.is_empty() && r.mixin_configs.is_empty() {
            println!("   no code (data only, or a container of nested mods)");
            continue;
        }
        println!("   {} classes{}; entrypoints: {}", r.classes, if r.unreadable_classes > 0 { format!(" ({} unreadable)", r.unreadable_classes) } else { String::new() }, if r.entrypoints.is_empty() { "none".to_string() } else { r.entrypoints.iter().map(|(k, v)| format!("{k}={}", v.len())).collect::<Vec<_>>().join(" ") });
        println!("   uses {} Minecraft classes, {} of their members; Fabric loader API {} refs; Fabric API: {}", r.minecraft_classes, r.minecraft_members, r.fabric_loader_classes, if r.fabric_api_packages.is_empty() { "none".into() } else { r.fabric_api_packages.iter().take(6).map(|(k, v)| format!("{k}({v})")).collect::<Vec<_>>().join(", ") });
        if r.mixin_classes > 0 {
            let (simple, body) = r.simple_and_body_dependent();
            println!("   {} mixin classes on {} targets; injections at a method's start or end: {simple}, inside its body: {body}", r.mixin_classes, r.mixin_targets.len());
            println!("   mixin annotations: {}", r.mixin_annotations.iter().map(|(k, v)| format!("{k} {v}")).collect::<Vec<_>>().join(", "));
            println!("   targets by area: {}", r.mixin_targets_by_area.iter().map(|(k, v)| format!("{k} {v}")).collect::<Vec<_>>().join("; "));
        }
        for p in &r.mixin_config_problems {
            println!("   MIXIN CONFIG PROBLEM {p}");
        }
        if !r.other_loader_references.is_empty() {
            println!("   references other loaders: {:?}", r.other_loader_references);
        }
        if !r.id.starts_with("fabric-") && r.id != "fabric-api" {
            all_classes.extend(r.minecraft_class_names.iter().cloned());
            all_members.extend(r.minecraft_member_names.iter().cloned());
        }
    }
    println!("\nMinecraft API surface of the mods other than Fabric API's own modules: {} classes, {} members", all_classes.len(), all_members.len());
}
