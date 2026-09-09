//! Explicit atomic URI equivalence assertions, not normalization or acquisition.
use super::{Records, RuleAssemblyError};
use crate::resources::uri::{UriError, UriReference};
use std::num::NonZeroUsize;
#[derive(Debug, Clone, Copy)]
pub struct ImportUriAliasLimits {
    /// Stored entries, and canonical-plus-alias input count per bind operation.
    pub entries: NonZeroUsize,
    /// Actual String and record-vector capacities; growth overlap is separate.
    pub bytes: NonZeroUsize,
    /// Per-operation URI validation, comparisons, copies and record moves.
    pub work: NonZeroUsize,
}
#[derive(Debug)]
pub enum ImportUriAliasError {
    Uri(UriError),
    Fragment,
    Conflict,
    Budget(RuleAssemblyError),
}
impl From<UriError> for ImportUriAliasError {
    fn from(value: UriError) -> Self {
        Self::Uri(value)
    }
}
impl From<RuleAssemblyError> for ImportUriAliasError {
    fn from(value: RuleAssemblyError) -> Self {
        Self::Budget(value)
    }
}
impl std::fmt::Display for ImportUriAliasError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "import URI alias: {self:?}")
    }
}
impl std::error::Error for ImportUriAliasError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Uri(e) => Some(e),
            Self::Budget(e) => Some(e),
            _ => None,
        }
    }
}
#[derive(Debug)]
struct Entry {
    alias: String,
    target: String,
}
/// Caller-normalized absolute fragment-free keys, with no inferred redirects,
/// filesystem equivalence or cache authority. Bind failures preserve all state.
#[derive(Debug)]
pub struct ImportUriAliases {
    entries: Vec<Entry>,
    payload_bytes: usize,
    retained_bytes: usize,
    limits: ImportUriAliasLimits,
}
impl ImportUriAliases {
    pub fn new(limits: ImportUriAliasLimits) -> Result<Self, ImportUriAliasError> {
        if limits.bytes.get() > isize::MAX as usize {
            return Err(RuleAssemblyError::Bytes.into());
        }
        Ok(Self {
            entries: Vec::new(),
            payload_bytes: 0,
            retained_bytes: 0,
            limits,
        })
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }
    fn records(&self) -> Records {
        Records {
            bytes: self.retained_bytes,
            limit: self.limits.bytes.get(),
            work: self.limits.work.get(),
        }
    }
    pub fn lookup(&self, alias: &str) -> Result<Option<&str>, ImportUriAliasError> {
        let mut records = self.records();
        validate(alias, self.limits.bytes, &mut records)?;
        find(&self.entries, alias, &mut records)
    }
    /// Atomically include the target's self mapping and every supplied alias.
    /// Conflicting existing assertions are never overwritten or chained.
    pub fn bind(&mut self, target: &str, aliases: &[&str]) -> Result<(), ImportUriAliasError> {
        if aliases
            .len()
            .checked_add(1)
            .is_none_or(|count| count > self.limits.entries.get())
        {
            return Err(RuleAssemblyError::Entries.into());
        }
        let mut records = self.records();
        validate(target, self.limits.bytes, &mut records)?;
        let mut pending = Vec::new();
        let mut payload = self.payload_bytes;
        let record_limit = self
            .limits
            .entries
            .get()
            .min(self.limits.bytes.get() / size_of::<Entry>())
            * size_of::<Entry>();
        for alias in std::iter::once(target).chain(aliases.iter().copied()) {
            records.spend(1)?;
            validate(alias, self.limits.bytes, &mut records)?;
            let known = match find(&self.entries, alias, &mut records)? {
                Some(found) => Some(found),
                None => find(&pending, alias, &mut records)?,
            };
            if let Some(found) = known {
                if !equal(found, target, &mut records)? {
                    return Err(ImportUriAliasError::Conflict);
                }
                continue;
            }
            if self
                .entries
                .len()
                .checked_add(pending.len())
                .is_none_or(|count| count >= self.limits.entries.get())
            {
                return Err(RuleAssemblyError::Entries.into());
            }
            let alias = copy(alias, &mut records)?;
            let target = copy(target, &mut records)?;
            payload = payload
                .checked_add(alias.capacity())
                .and_then(|value| value.checked_add(target.capacity()))
                .ok_or(RuleAssemblyError::Bytes)?;
            records.reserve_with_capacity_limit(&mut pending, record_limit)?;
            pending.push(Entry { alias, target });
        }
        if pending.is_empty() {
            return Ok(());
        }
        let count = self
            .entries
            .len()
            .checked_add(pending.len())
            .ok_or(RuleAssemblyError::Entries)?;
        let maximum = (self
            .limits
            .bytes
            .get()
            .checked_sub(payload)
            .ok_or(RuleAssemblyError::Bytes)?
            / size_of::<Entry>())
        .min(self.limits.entries.get());
        if count > maximum {
            return Err(RuleAssemblyError::Bytes.into());
        }
        records.spend(
            count
                .checked_mul(size_of::<Entry>())
                .ok_or(RuleAssemblyError::Work)?,
        )?;
        let mut next = Vec::new();
        next.try_reserve_exact(count)
            .map_err(|_| RuleAssemblyError::Allocation)?;
        if next.capacity() > maximum {
            return Err(RuleAssemblyError::Bytes.into());
        }
        let retained = payload
            .checked_add(
                next.capacity()
                    .checked_mul(size_of::<Entry>())
                    .ok_or(RuleAssemblyError::Bytes)?,
            )
            .ok_or(RuleAssemblyError::Bytes)?;
        // All validation/allocation is complete. These moves cannot grow next.
        next.append(&mut self.entries);
        next.append(&mut pending);
        self.entries = next;
        self.payload_bytes = payload;
        self.retained_bytes = retained;
        Ok(())
    }
}
fn validate(
    input: &str,
    maximum: NonZeroUsize,
    records: &mut Records,
) -> Result<(), ImportUriAliasError> {
    let parsed = UriReference::parse_with_work(input, maximum, &mut records.work)?;
    if parsed.scheme().is_none() {
        return Err(UriError::RelativeBase.into());
    }
    if parsed.fragment().is_some() {
        return Err(ImportUriAliasError::Fragment);
    }
    Ok(())
}
fn equal(left: &str, right: &str, records: &mut Records) -> Result<bool, ImportUriAliasError> {
    records.spend(
        left.len()
            .min(right.len())
            .checked_add(1)
            .ok_or(RuleAssemblyError::Work)?,
    )?;
    Ok(left == right)
}
fn find<'a>(
    entries: &'a [Entry],
    alias: &str,
    records: &mut Records,
) -> Result<Option<&'a str>, ImportUriAliasError> {
    for entry in entries {
        if equal(&entry.alias, alias, records)? {
            return Ok(Some(&entry.target));
        }
    }
    Ok(None)
}
fn copy(input: &str, records: &mut Records) -> Result<String, ImportUriAliasError> {
    records.spend(input.len())?;
    if input.len() > records.limit - records.bytes {
        return Err(RuleAssemblyError::Bytes.into());
    }
    let mut output = String::new();
    output
        .try_reserve_exact(input.len())
        .map_err(|_| RuleAssemblyError::Allocation)?;
    records.retain(output.capacity())?;
    output.push_str(input);
    Ok(output)
}
