//! Fabric version numbers and the dependency predicates in `fabric.mod.json` (`>=0.19.3`, `~26.3-`,
//! `>=26.3 <26.4`, `1.2.x`, `*`, `^1.2`). Versions are dot-separated numbers with an optional `-prerelease` and a
//! `+build` that is ignored; a predicate is comparators joined by spaces (all must hold); a dependency may give a list
//! of predicates (any may hold).
use std::cmp::Ordering;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    pub parts: Vec<u64>,
    pub pre: Option<String>,
}

impl Version {
    pub fn parse(s: &str) -> Option<Version> {
        let s = s.trim();
        let s = s.split('+').next()?;
        let (core, pre) = match s.split_once('-') {
            Some((c, p)) => (c, Some(p.to_string())),
            None => (s, None),
        };
        if core.is_empty() {
            return None;
        }
        let parts = core.split('.').map(|p| p.parse::<u64>().ok()).collect::<Option<Vec<_>>>()?;
        Some(Version { parts, pre })
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        let n = self.parts.len().max(other.parts.len());
        for i in 0..n {
            let a = self.parts.get(i).copied().unwrap_or(0);
            let b = other.parts.get(i).copied().unwrap_or(0);
            match a.cmp(&b) {
                Ordering::Equal => {}
                o => return o,
            }
        }
        // A prerelease is lower than the release it leads to.
        match (&self.pre, &other.pre) {
            (None, None) => Ordering::Equal,
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (Some(a), Some(b)) => a.cmp(b),
        }
    }
}
impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// One comparator: all of `low` (inclusive or not) and `high`, either may be open.
#[derive(Debug, Clone)]
struct Range {
    low: Option<(Version, bool)>,
    high: Option<(Version, bool)>,
}

fn parse_comparator(tok: &str) -> Option<Range> {
    let tok = tok.trim();
    if tok.is_empty() || tok == "*" || tok == "x" || tok == "X" {
        return Some(Range { low: None, high: None });
    }
    let (op, rest) = if let Some(r) = tok.strip_prefix(">=") {
        (">=", r)
    } else if let Some(r) = tok.strip_prefix("<=") {
        ("<=", r)
    } else if let Some(r) = tok.strip_prefix('>') {
        (">", r)
    } else if let Some(r) = tok.strip_prefix('<') {
        ("<", r)
    } else if let Some(r) = tok.strip_prefix('=') {
        ("=", r)
    } else if let Some(r) = tok.strip_prefix('~') {
        ("~", r)
    } else if let Some(r) = tok.strip_prefix('^') {
        ("^", r)
    } else {
        ("=", tok)
    };
    // A trailing `-` (as in `~26.3-`) allows the version's own prereleases: no lower bound below the release.
    let rest = rest.trim().trim_end_matches('-');
    // A wildcard component (`1.2.x`) is the same as `~1.2`.
    let mut wild = false;
    let cleaned: Vec<&str> = rest
        .split('.')
        .take_while(|p| {
            let w = matches!(*p, "x" | "X" | "*");
            wild |= w;
            !w
        })
        .collect();
    let v = Version::parse(&cleaned.join("."))?;
    let bump = |v: &Version, at: usize| {
        let mut p = v.parts.clone();
        p.truncate(at + 1);
        p[at] += 1;
        Version { parts: p, pre: None }
    };
    let op = if wild && op == "=" { "~" } else { op };
    Some(match op {
        ">=" => Range { low: Some((v, true)), high: None },
        ">" => Range { low: Some((v, false)), high: None },
        "<=" => Range { low: None, high: Some((v, true)) },
        "<" => Range { low: None, high: Some((v, false)) },
        "=" => Range { low: Some((v.clone(), true)), high: Some((v, true)) },
        // `~1.2.3` and `~1.2` are below the next minor; `~1` is below the next major.
        "~" => {
            let at = if v.parts.len() >= 2 { 1 } else { 0 };
            let hi = bump(&v, at);
            Range { low: Some((v, true)), high: Some((hi, false)) }
        }
        // `^1.2.3` is below the next major; `^0.2.3` below the next minor; `^0.0.3` below the next patch.
        "^" => {
            let at = v.parts.iter().position(|p| *p != 0).unwrap_or(v.parts.len().saturating_sub(1));
            let hi = bump(&v, at);
            Range { low: Some((v, true)), high: Some((hi, false)) }
        }
        _ => return None,
    })
}

/// Whether `v` satisfies one predicate (comparators separated by spaces, all must hold).
pub fn satisfies(v: &Version, predicate: &str) -> Option<bool> {
    let mut ok = true;
    for tok in predicate.split_whitespace() {
        let r = parse_comparator(tok)?;
        if let Some((lo, inclusive)) = &r.low {
            // A release satisfies a lower bound that is that release's own prerelease.
            ok &= if *inclusive { v >= lo } else { v > lo };
        }
        if let Some((hi, inclusive)) = &r.high {
            ok &= if *inclusive { v <= hi } else { v < hi };
        }
    }
    Some(ok)
}

/// Whether `v` satisfies any of the predicates; `None` when one cannot be read.
pub fn satisfies_any(v: &Version, predicates: &[String]) -> Option<bool> {
    if predicates.is_empty() {
        return Some(true);
    }
    let mut any = false;
    for p in predicates {
        any |= satisfies(v, p)?;
    }
    Some(any)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }
    fn ok(ver: &str, pred: &str) -> bool {
        satisfies(&v(ver), pred).unwrap()
    }

    #[test]
    fn plain_comparisons() {
        assert!(ok("0.19.3", ">=0.19.3"));
        assert!(ok("0.20.0", ">=0.19.3"));
        assert!(!ok("0.19.2", ">=0.19.3"));
        assert!(ok("21", ">=17"));
        assert!(ok("25", ">=21"));
        assert!(ok("26.3", "*"));
    }
    #[test]
    fn ranges_and_tilde() {
        assert!(ok("26.3", ">=26.3 <26.4"));
        assert!(!ok("26.4", ">=26.3 <26.4"));
        assert!(ok("26.3", "~26.3-"));
        assert!(ok("26.3.1", "~26.3"));
        assert!(!ok("26.4", "~26.3"));
        assert!(!ok("26.2", ">=26.3"));
        assert!(ok("26.3", ">=26.2"));
    }
    #[test]
    fn wildcards_and_caret() {
        assert!(ok("1.2.9", "1.2.x"));
        assert!(!ok("1.3.0", "1.2.x"));
        assert!(ok("1.9.0", "^1.2"));
        assert!(!ok("2.0.0", "^1.2"));
        assert!(ok("0.2.9", "^0.2.3"));
        assert!(!ok("0.3.0", "^0.2.3"));
    }
    #[test]
    fn long_mod_versions_and_prereleases() {
        assert!(v("26.3.0.0.7") > v("26.3.0.0.5"));
        assert!(v("1.0.0-beta") < v("1.0.0"));
        assert_eq!(v("0.166.2+26.3").parts, vec![0, 166, 2]);
        assert!(satisfies_any(&v("1.0"), &["2.0".into(), ">=1.0".into()]).unwrap());
        assert!(!satisfies_any(&v("1.0"), &["2.0".into(), "3.0".into()]).unwrap());
    }
    #[test]
    fn an_unreadable_predicate_is_none_not_a_guess() {
        assert!(satisfies(&v("1.0"), ">=banana").is_none());
    }
}
