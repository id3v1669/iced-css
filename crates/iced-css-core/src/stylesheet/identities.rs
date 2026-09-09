//! Owner-scoped exact URI/decoding-context identities, without acquisition policy.
use super::{
    Records, RuleAssemblyError, RuleContext, collector::ImportTarget, imports::ImportCycleId,
};
use crate::{
    resources::uri::{UriError, UriReference},
    syntax::{encoding_selection::EncodingMetadata, imports::ResourceId},
};
use std::num::{NonZeroU64, NonZeroUsize};

#[derive(Debug, Clone, Copy)]
pub struct IdentityLimits {
    pub entries: NonZeroUsize,
    /// Total retained record, URI and label capacities, excluding growth overlap.
    pub bytes: NonZeroUsize,
    /// Per identify operation: validation, comparisons, copies and record growth.
    pub work: NonZeroUsize,
}
#[derive(Debug)]
pub enum IdentityError {
    Uri(UriError),
    Fragment,
    Budget(RuleAssemblyError),
}
impl From<RuleAssemblyError> for IdentityError {
    fn from(error: RuleAssemblyError) -> Self {
        Self::Budget(error)
    }
}
impl From<UriError> for IdentityError {
    fn from(error: UriError) -> Self {
        Self::Uri(error)
    }
}
impl std::fmt::Display for IdentityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "import identity: {self:?}")
    }
}
impl std::error::Error for IdentityError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Uri(error) => Some(error),
            Self::Budget(error) => Some(error),
            Self::Fragment => None,
        }
    }
}
struct Entry {
    uri: String,
    transport: Option<Vec<u8>>,
    linking: Option<Vec<u8>>,
    referring: Option<Vec<u8>>,
    context: RuleContext,
    target: ImportTarget,
}
/// Keep one registry for an identity owner's lifetime. Numeric tokens from
/// different registries must never be mixed in one graph. No cache, revision,
/// redirect alias, filesystem or request authorization identity is inferred.
/// URI keys are exact, absolute, fragment-free effective acquisition/base URIs;
/// callers supply their explicit normalization policy before admission.
pub struct ImportIdentityRegistry {
    entries: Vec<Entry>,
    retained: usize,
    limits: IdentityLimits,
}
impl ImportIdentityRegistry {
    pub fn new(limits: IdentityLimits) -> Result<Self, IdentityError> {
        if limits.bytes.get() > isize::MAX as usize {
            return Err(RuleAssemblyError::Bytes.into());
        }
        Ok(Self {
            entries: Vec::new(),
            retained: 0,
            limits,
        })
    }
    /// Equal URI keys share a cycle. Source identity additionally includes all
    /// supplied encoding labels and the rule context. Missing and present-empty
    /// labels differ. Labels are copied on new admission; no caller borrow remains.
    /// Failure neither admits a partial record nor consumes an identity.
    pub fn identify(
        &mut self,
        uri: &str,
        metadata: EncodingMetadata<'_>,
        context: RuleContext,
    ) -> Result<ImportTarget, IdentityError> {
        let mut records = Records {
            bytes: self.retained,
            limit: self.limits.bytes.get(),
            work: self.limits.work.get(),
        };
        let parsed = UriReference::parse_with_work(uri, self.limits.bytes, &mut records.work)?;
        if parsed.scheme().is_none() {
            return Err(UriError::RelativeBase.into());
        }
        if parsed.fragment().is_some() {
            return Err(IdentityError::Fragment);
        }
        let mut cycle = None;
        for entry in &self.entries {
            records.spend(1)?;
            if !equal(entry.uri.as_bytes(), uri.as_bytes(), &mut records)? {
                continue;
            }
            cycle = Some(entry.target.cycle);
            if entry.context == context
                && optional_equal(entry.transport.as_deref(), metadata.transport, &mut records)?
                && optional_equal(entry.linking.as_deref(), metadata.linking, &mut records)?
                && optional_equal(entry.referring.as_deref(), metadata.referring, &mut records)?
            {
                return Ok(entry.target);
            }
        }
        if self.entries.len() >= self.limits.entries.get() {
            return Err(RuleAssemblyError::Entries.into());
        }
        let number = self
            .entries
            .len()
            .checked_add(1)
            .and_then(|n| u64::try_from(n).ok())
            .and_then(NonZeroU64::new)
            .ok_or(RuleAssemblyError::Identity)?;
        let target = ImportTarget {
            source: ResourceId::new(number),
            cycle: cycle.unwrap_or(ImportCycleId::new(number)),
        };
        records.spend(uri.len())?;
        if uri.len() > records.limit - records.bytes {
            return Err(RuleAssemblyError::Bytes.into());
        }
        let mut owned = String::new();
        owned
            .try_reserve_exact(uri.len())
            .map_err(|_| RuleAssemblyError::Allocation)?;
        records.retain(owned.capacity())?;
        owned.push_str(uri);
        let entry = Entry {
            uri: owned,
            transport: copy_label(metadata.transport, &mut records)?,
            linking: copy_label(metadata.linking, &mut records)?,
            referring: copy_label(metadata.referring, &mut records)?,
            context,
            target,
        };
        // After reservation succeeds, only infallible moves remain. Failed growth
        // leaves the old vector and all of its identities untouched.
        records.reserve(&mut self.entries)?;
        self.entries.push(entry);
        self.retained = records.bytes;
        Ok(target)
    }
}
fn equal(a: &[u8], b: &[u8], records: &mut Records) -> Result<bool, RuleAssemblyError> {
    records.spend(1)?;
    if a.len() != b.len() {
        return Ok(false);
    }
    records.spend(a.len())?;
    Ok(a == b)
}
fn optional_equal(
    a: Option<&[u8]>,
    b: Option<&[u8]>,
    records: &mut Records,
) -> Result<bool, RuleAssemblyError> {
    records.spend(1)?;
    match (a, b) {
        (Some(a), Some(b)) => equal(a, b, records),
        (None, None) => Ok(true),
        _ => Ok(false),
    }
}
fn copy_label(
    label: Option<&[u8]>,
    records: &mut Records,
) -> Result<Option<Vec<u8>>, RuleAssemblyError> {
    let Some(label) = label else {
        return Ok(None);
    };
    records.spend(label.len())?;
    if label.len() > records.limit - records.bytes {
        return Err(RuleAssemblyError::Bytes);
    }
    let mut owned = Vec::new();
    owned
        .try_reserve_exact(label.len())
        .map_err(|_| RuleAssemblyError::Allocation)?;
    records.retain(owned.capacity())?;
    owned.extend_from_slice(label);
    Ok(Some(owned))
}
