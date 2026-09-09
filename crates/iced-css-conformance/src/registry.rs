//! Closed registry: no process-global registration or silent fallback.
use crate::CaseRunner;
use std::collections::BTreeMap;
pub struct Registry {
    entries: BTreeMap<String, CaseRunner>,
}
impl Registry {
    pub fn new(entries: &[(&str, CaseRunner)]) -> Result<Self, String> {
        let mut runners = BTreeMap::new();
        for (name, runner) in entries {
            if name.trim().is_empty() {
                return Err("empty runner name".into());
            }
            if runners.insert((*name).to_owned(), *runner).is_some() {
                return Err(format!("duplicate runner name: {name}"));
            }
        }
        Ok(Self { entries: runners })
    }
    pub fn find(&self, name: &str) -> Result<CaseRunner, String> {
        self.entries
            .get(name)
            .copied()
            .ok_or_else(|| format!("unregistered runner: {name}"))
    }
}
/// Add product runners only alongside their tested implementations.
pub fn builtins() -> Result<Registry, String> {
    Registry::new(&[
        ("p1_1", crate::adapters::p1_1::run),
        ("p1_2", crate::adapters::p1_2::run),
        ("p1_3", crate::adapters::p1_3::run),
        ("p2_1", crate::adapters::p2_1::run),
        ("p2_2", crate::adapters::p2_2::run),
        ("p2_3", crate::adapters::p2_3::run),
        ("p3_3", crate::adapters::p3_3::run),
        ("p4_1", crate::adapters::p4_1::run),
        ("p5_1", crate::adapters::p5_1::run),
        ("p5_4", crate::adapters::p5_4::run),
        ("p5_3", crate::adapters::p5_3::run),
        ("p7_1", crate::adapters::p7_1::run),
        ("p7_2", crate::adapters::p7_2::run),
        ("p7_4", crate::adapters::p7_4::run),
    ])
}
pub fn lookup(name: &str) -> Result<CaseRunner, String> {
    builtins()?.find(name)
}
