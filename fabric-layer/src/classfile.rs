//! A reader for the parts of a Java class file the layer needs: which classes and members it references, and
//! which Mixin annotations it carries. It reads the constant pool and the annotation attributes and nothing else
//! (no bytecode), and refuses a malformed file with an error rather than panicking.
use std::collections::BTreeSet;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Injector {
    /// The annotation's simple name: `Inject`, `Redirect`, `WrapOperation`, `Accessor`, ...
    pub kind: String,
}

#[derive(Debug, Default, Clone)]
pub struct ClassInfo {
    /// Internal name, `net/minecraft/world/entity/Entity`.
    pub name: String,
    /// Every class named in the constant pool or in a descriptor (internal names).
    pub refs: BTreeSet<String>,
    /// `(owner, member name)` for every field or method reference.
    pub members: BTreeSet<(String, String)>,
    /// The classes a `@Mixin` on this class targets (internal names; a target given as a string is kept as written).
    pub mixin_targets: Vec<String>,
    /// Every Mixin or MixinExtras annotation on the class, its fields and its methods.
    pub mixin_annotations: Vec<Injector>,
    /// The `value` of every `@At`: `HEAD`, `RETURN`, `INVOKE`, ...
    pub at_values: Vec<String>,
}

#[derive(Debug)]
pub struct ParseError(pub &'static str);
impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "bad class file: {}", self.0)
    }
}
impl std::error::Error for ParseError {}

struct Reader<'a> {
    d: &'a [u8],
    p: usize,
}
impl<'a> Reader<'a> {
    fn u1(&mut self) -> Result<u8, ParseError> {
        let v = *self.d.get(self.p).ok_or(ParseError("truncated"))?;
        self.p += 1;
        Ok(v)
    }
    fn u2(&mut self) -> Result<u16, ParseError> {
        Ok(u16::from_be_bytes([self.u1()?, self.u1()?]))
    }
    fn u4(&mut self) -> Result<u32, ParseError> {
        Ok(u32::from_be_bytes([self.u1()?, self.u1()?, self.u1()?, self.u1()?]))
    }
    fn skip(&mut self, n: usize) -> Result<(), ParseError> {
        if self.p + n > self.d.len() {
            return Err(ParseError("truncated"));
        }
        self.p += n;
        Ok(())
    }
    fn bytes(&mut self, n: usize) -> Result<&'a [u8], ParseError> {
        let s = self.d.get(self.p..self.p + n).ok_or(ParseError("truncated"))?;
        self.p += n;
        Ok(s)
    }
}

#[derive(Clone)]
enum Cp {
    None,
    Utf(String),
    Class(u16),
    Ref(u16, u16),
    Nat(u16),
    Other,
}

const MIXIN: &str = "Lorg/spongepowered/asm/mixin/Mixin;";
const AT: &str = "Lorg/spongepowered/asm/mixin/injection/At;";

pub fn parse(data: &[u8]) -> Result<ClassInfo, ParseError> {
    let mut r = Reader { d: data, p: 0 };
    if r.u4()? != 0xCAFE_BABE {
        return Err(ParseError("not a class file"));
    }
    r.skip(4)?; // minor, major
    let n = r.u2()? as usize;
    let mut cp = vec![Cp::None; n];
    let mut i = 1;
    while i < n {
        let tag = r.u1()?;
        match tag {
            1 => {
                let l = r.u2()? as usize;
                cp[i] = Cp::Utf(String::from_utf8_lossy(r.bytes(l)?).into_owned());
            }
            3 | 4 => {
                r.skip(4)?;
                cp[i] = Cp::Other;
            }
            5 | 6 => {
                r.skip(8)?;
                cp[i] = Cp::Other;
                i += 1; // a long or double takes two slots
            }
            7 => cp[i] = Cp::Class(r.u2()?),
            8 | 16 | 19 | 20 => {
                r.skip(2)?;
                cp[i] = Cp::Other;
            }
            9..=11 => {
                let owner = r.u2()?;
                cp[i] = Cp::Ref(owner, r.u2()?);
            }
            12 => {
                let name = r.u2()?;
                r.skip(2)?;
                cp[i] = Cp::Nat(name);
            }
            15 => {
                r.skip(3)?;
                cp[i] = Cp::Other;
            }
            17 | 18 => {
                r.skip(4)?;
                cp[i] = Cp::Other;
            }
            _ => return Err(ParseError("unknown constant tag")),
        }
        i += 1;
    }
    let utf = |k: u16| -> &str {
        match cp.get(k as usize) {
            Some(Cp::Utf(s)) => s,
            _ => "",
        }
    };
    let class_name = |k: u16| -> &str {
        match cp.get(k as usize) {
            Some(Cp::Class(u)) => utf(*u),
            _ => "",
        }
    };
    let mut info = ClassInfo::default();
    for c in &cp {
        match c {
            Cp::Class(u) => {
                let s = utf(*u);
                // An array class is named by its descriptor: `[Lfoo/Bar;`.
                if !s.starts_with('[') {
                    info.refs.insert(s.to_string());
                }
            }
            Cp::Ref(owner, nat) => {
                if let Some(Cp::Nat(name)) = cp.get(*nat as usize) {
                    info.members.insert((class_name(*owner).to_string(), utf(*name).to_string()));
                }
            }
            Cp::Utf(s) => collect_descriptor_classes(s, &mut info.refs),
            _ => {}
        }
    }
    r.skip(2)?; // access
    info.name = class_name(r.u2()?).to_string();
    r.skip(2)?; // super
    let ni = r.u2()? as usize;
    r.skip(2 * ni)?;

    // Annotations.
    fn element(r: &mut Reader, utf: &dyn Fn(u16) -> String, info: &mut ClassInfo, out: Option<&mut Vec<String>>) -> Result<(), ParseError> {
        let tag = r.u1()?;
        match tag {
            b'B' | b'C' | b'D' | b'F' | b'I' | b'J' | b'S' | b'Z' | b's' | b'c' => {
                let v = r.u2()?;
                if let Some(o) = out {
                    o.push(utf(v));
                }
            }
            b'e' => r.skip(4)?,
            // A nested annotation (`@Inject(at = @At(...))`): its `@At` value counts, it is not itself recorded.
            b'@' => annotation(r, utf, info, false)?,
            b'[' => {
                let count = r.u2()?;
                let mut out = out;
                for _ in 0..count {
                    element(r, utf, info, out.as_deref_mut())?;
                }
            }
            _ => return Err(ParseError("bad annotation element")),
        }
        Ok(())
    }
    fn annotation(r: &mut Reader, utf: &dyn Fn(u16) -> String, info: &mut ClassInfo, record: bool) -> Result<(), ParseError> {
        let ty = utf(r.u2()?);
        let pairs = r.u2()?;
        for _ in 0..pairs {
            let name = utf(r.u2()?);
            if ty == MIXIN && (name == "value" || name == "targets") {
                let mut v = Vec::new();
                element(r, utf, info, Some(&mut v))?;
                info.mixin_targets.extend(v.into_iter().map(|s| s.trim_start_matches('L').trim_end_matches(';').to_string()));
            } else if ty == AT && name == "value" {
                let mut v = Vec::new();
                element(r, utf, info, Some(&mut v))?;
                info.at_values.extend(v);
            } else {
                element(r, utf, info, None)?;
            }
        }
        if record && (ty.starts_with("Lorg/spongepowered/asm/mixin/") || ty.starts_with("Lcom/llamalad7/mixinextras/")) {
            let simple = ty.rsplit('/').next().unwrap_or("").trim_end_matches(';').to_string();
            info.mixin_annotations.push(Injector { kind: simple });
        }
        Ok(())
    }
    fn attributes(r: &mut Reader, utf: &dyn Fn(u16) -> String, info: &mut ClassInfo) -> Result<(), ParseError> {
        let count = r.u2()?;
        for _ in 0..count {
            let name = utf(r.u2()?);
            let len = r.u4()? as usize;
            let end = r.p.checked_add(len).filter(|e| *e <= r.d.len()).ok_or(ParseError("truncated"))?;
            if name == "RuntimeVisibleAnnotations" || name == "RuntimeInvisibleAnnotations" {
                let n = r.u2()?;
                for _ in 0..n {
                    annotation(r, utf, info, true)?;
                }
            }
            r.p = end;
        }
        Ok(())
    }
    let utf_owned = |k: u16| -> String { utf(k).to_string() };
    for _ in 0..2 {
        // fields, then methods
        let count = r.u2()?;
        for _ in 0..count {
            r.skip(6)?;
            attributes(&mut r, &utf_owned, &mut info)?;
        }
    }
    attributes(&mut r, &utf_owned, &mut info)?;
    info.mixin_targets.dedup();
    Ok(info)
}

/// Adds every `Lsome/Class;` in a descriptor or signature.
fn collect_descriptor_classes(s: &str, out: &mut BTreeSet<String>) {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'L' {
            if let Some(end) = s[i + 1..].find(';') {
                let name = &s[i + 1..i + 1 + end];
                if !name.is_empty() && name.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'/' | b'$' | b'_')) && name.contains('/') {
                    out.insert(name.to_string());
                    i += end + 2;
                    continue;
                }
            }
        }
        i += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A class file built by hand: `a/B` with one `@Mixin(targets = "x.Y")` and one method annotated `@At("HEAD")`.
    pub fn build() -> Vec<u8> {
        let mut cp: Vec<Vec<u8>> = Vec::new();
        let utf = |cp: &mut Vec<Vec<u8>>, s: &str| -> u16 {
            let mut v = vec![1u8];
            v.extend((s.len() as u16).to_be_bytes());
            v.extend(s.as_bytes());
            cp.push(v);
            cp.len() as u16
        };
        let n_this = utf(&mut cp, "a/B");
        cp.push([vec![7u8], n_this.to_be_bytes().to_vec()].concat());
        let this = cp.len() as u16;
        let rt = utf(&mut cp, "RuntimeInvisibleAnnotations");
        let mixin = utf(&mut cp, MIXIN);
        let targets = utf(&mut cp, "targets");
        let value = utf(&mut cp, "value");
        let tgt = utf(&mut cp, "net.minecraft.world.entity.Entity");
        let at = utf(&mut cp, AT);
        let head = utf(&mut cp, "HEAD");
        let inject = utf(&mut cp, "Lorg/spongepowered/asm/mixin/injection/Inject;");
        let mut out = vec![0xCA, 0xFE, 0xBA, 0xBE, 0, 0, 0, 69];
        out.extend(((cp.len() + 1) as u16).to_be_bytes());
        for e in &cp {
            out.extend(e);
        }
        out.extend([0, 0]); // access
        out.extend(this.to_be_bytes());
        out.extend([0, 0, 0, 0]); // super, interfaces
        out.extend([0, 0]); // fields
        // one method with an @Inject(at = @At("HEAD")) (the nested annotation is the value of `at`)
        out.extend([0, 1]);
        out.extend([0, 0, 0, 0, 0, 0]); // access, name, desc
        out.extend([0, 1]); // attributes
        out.extend(rt.to_be_bytes());
        let mut attr = vec![0u8, 1];
        attr.extend(inject.to_be_bytes());
        attr.extend([0, 1]);
        attr.extend(value.to_be_bytes()); // element name (any utf)
        attr.push(b'@');
        attr.extend(at.to_be_bytes());
        attr.extend([0, 1]);
        attr.extend(value.to_be_bytes());
        attr.push(b's');
        attr.extend(head.to_be_bytes());
        out.extend((attr.len() as u32).to_be_bytes());
        out.extend(attr);
        // class attributes: @Mixin(targets = "net.minecraft.world.entity.Entity")
        out.extend([0, 1]);
        out.extend(rt.to_be_bytes());
        let mut attr = vec![0u8, 1];
        attr.extend(mixin.to_be_bytes());
        attr.extend([0, 1]);
        attr.extend(targets.to_be_bytes());
        attr.push(b's');
        attr.extend(tgt.to_be_bytes());
        out.extend((attr.len() as u32).to_be_bytes());
        out.extend(attr);
        out
    }

    #[test]
    fn reads_mixin_targets_injectors_and_at_values() {
        let c = parse(&build()).unwrap();
        assert_eq!(c.name, "a/B");
        assert_eq!(c.mixin_targets, vec!["net.minecraft.world.entity.Entity"]);
        let kinds: Vec<_> = c.mixin_annotations.iter().map(|i| i.kind.as_str()).collect();
        assert!(kinds.contains(&"Mixin") && kinds.contains(&"Inject"), "{kinds:?}");
        assert_eq!(c.at_values, vec!["HEAD"]);
    }

    #[test]
    fn refuses_garbage_and_truncation_without_panicking() {
        assert!(parse(b"not a class").is_err());
        let good = build();
        for cut in [0, 5, 12, good.len() / 2, good.len() - 1] {
            assert!(parse(&good[..cut]).is_err() || cut == good.len(), "cut {cut}");
        }
    }

    #[test]
    fn descriptor_classes_are_found() {
        let mut s = BTreeSet::new();
        collect_descriptor_classes("(Lnet/minecraft/world/entity/Entity;I)Lnet/minecraft/world/level/Level;", &mut s);
        assert_eq!(s.len(), 2);
        let mut t = BTreeSet::new();
        collect_descriptor_classes("Lfoo;", &mut t); // no package separator: not a class reference we count
        assert!(t.is_empty());
    }
}
