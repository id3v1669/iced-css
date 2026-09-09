//! Immutable effective-retrieval-base metadata paired with an owned graph.
use super::{
    Records, RuleAssemblyError,
    imports::{OccurrenceDeclaration, ProvenanceError},
    owned_graph::OwnedStylesheetGraph,
    requests::{
        ImportRequestError, ImportRequestLimits, ResolvedImportRequest, ResolvedUnicodeImports,
        resolve_import_requests, resolve_unicode_import_requests,
    },
};
use crate::{
    resources::uri::{UriError, UriReference},
    syntax::imports::ResourceId,
};
use std::num::{NonZeroU64, NonZeroUsize};

#[derive(Debug)]
pub struct StylesheetBase {
    pub resource: ResourceId,
    /// Explicit absolute effective retrieval URI, without a fragment.
    pub effective_base: String,
}
#[derive(Debug, Clone, Copy)]
pub struct BaseLimits {
    pub entries: NonZeroUsize,
    /// Actual input vector and URI String capacities; graph storage is separate.
    pub retained_bytes: NonZeroUsize,
    pub work: NonZeroUsize,
}
#[derive(Debug)]
pub enum LocatedGraphError {
    Budget(RuleAssemblyError),
    MissingBase,
    UnknownResource(ResourceId),
    DuplicateResource(ResourceId),
    Fragment(ResourceId),
    Uri {
        resource: ResourceId,
        error: UriError,
    },
    Provenance(ProvenanceError),
    Imports(ImportRequestError),
}
impl From<RuleAssemblyError> for LocatedGraphError {
    fn from(error: RuleAssemblyError) -> Self {
        Self::Budget(error)
    }
}
impl std::fmt::Display for LocatedGraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "stylesheet base metadata: {self:?}")
    }
}
impl std::error::Error for LocatedGraphError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Budget(error) => Some(error),
            Self::Uri { error, .. } => Some(error),
            Self::Provenance(error) => Some(error),
            Self::Imports(error) => Some(error),
            _ => None,
        }
    }
}
pub struct LocatedDeclaration<'a> {
    pub declaration: OccurrenceDeclaration<'a>,
    pub effective_base: &'a str,
}
/// Complete validated metadata and graph are moved/published as one candidate.
/// No URI equivalence, transport authorization or fallback base is inferred.
pub struct LocatedStylesheetGraph {
    graph: OwnedStylesheetGraph,
    bases: Vec<StylesheetBase>,
}
impl LocatedStylesheetGraph {
    pub fn new(
        graph: OwnedStylesheetGraph,
        bases: Vec<StylesheetBase>,
        limits: BaseLimits,
    ) -> Result<Self, LocatedGraphError> {
        if bases.len() > limits.entries.get() {
            return Err(RuleAssemblyError::Entries.into());
        }
        if bases.len() < graph.resources().len() {
            return Err(LocatedGraphError::MissingBase);
        }
        if limits.retained_bytes.get() > isize::MAX as usize {
            return Err(RuleAssemblyError::Bytes.into());
        }
        let bytes = bases
            .capacity()
            .checked_mul(size_of::<StylesheetBase>())
            .filter(|bytes| *bytes <= limits.retained_bytes.get())
            .ok_or(RuleAssemblyError::Bytes)?;
        let mut records = Records {
            bytes,
            limit: limits.retained_bytes.get(),
            work: limits.work.get(),
        };
        for (index, base) in bases.iter().enumerate() {
            records.spend(1)?;
            records.bytes = records
                .bytes
                .checked_add(base.effective_base.capacity())
                .filter(|bytes| *bytes <= records.limit)
                .ok_or(RuleAssemblyError::Bytes)?;
            for prior in &bases[..index] {
                records.spend(1)?;
                if prior.resource == base.resource {
                    return Err(LocatedGraphError::DuplicateResource(base.resource));
                }
            }
            let mut found = false;
            for resource in graph.resources() {
                records.spend(1)?;
                if resource.resource == base.resource {
                    found = true;
                    break;
                }
            }
            if !found {
                return Err(LocatedGraphError::UnknownResource(base.resource));
            }
            let uri = UriReference::parse_with_work(
                &base.effective_base,
                limits.retained_bytes,
                &mut records.work,
            )
            .map_err(|error| LocatedGraphError::Uri {
                resource: base.resource,
                error,
            })?;
            if uri.scheme().is_none() {
                return Err(LocatedGraphError::Uri {
                    resource: base.resource,
                    error: UriError::RelativeBase,
                });
            }
            if uri.fragment().is_some() {
                return Err(LocatedGraphError::Fragment(base.resource));
            }
        }
        Ok(Self { graph, bases })
    }
    pub fn graph(&self) -> &OwnedStylesheetGraph {
        &self.graph
    }
    fn base_with_work(
        &self,
        resource: ResourceId,
        work: &mut usize,
    ) -> Result<Option<&str>, LocatedGraphError> {
        for base in &self.bases {
            *work = work.checked_sub(1).ok_or(RuleAssemblyError::Work)?;
            if base.resource == resource {
                return Ok(Some(&base.effective_base));
            }
        }
        Ok(None)
    }
    pub fn effective_base(
        &self,
        resource: ResourceId,
        work: NonZeroUsize,
    ) -> Result<Option<&str>, LocatedGraphError> {
        self.base_with_work(resource, &mut work.get())
    }
    pub fn find_declaration(
        &self,
        identity: NonZeroU64,
        provenance_work: NonZeroUsize,
        base_work: NonZeroUsize,
    ) -> Result<Option<LocatedDeclaration<'_>>, LocatedGraphError> {
        let Some(declaration) = self
            .graph
            .find_declaration(identity, provenance_work)
            .map_err(LocatedGraphError::Provenance)?
        else {
            return Ok(None);
        };
        let effective_base = self
            .effective_base(declaration.source, base_work)?
            .ok_or(LocatedGraphError::MissingBase)?;
        Ok(Some(LocatedDeclaration {
            declaration,
            effective_base,
        }))
    }
    /// Resolve applicable imports using this resource's own effective base. Lookup
    /// work and URI/request assembly budgets remain separate, explicit bounds.
    pub fn resolve_imports(
        &self,
        resource: ResourceId,
        lookup_work: NonZeroUsize,
        limits: ImportRequestLimits,
    ) -> Result<Vec<ResolvedImportRequest>, LocatedGraphError> {
        let (candidate, base) = self.import_context(resource, lookup_work)?;
        resolve_import_requests(candidate.sheet(), base, limits).map_err(LocatedGraphError::Imports)
    }
    /// Resolve with the selected charset of this resource, independently of the
    /// root charset. Child decoding can use the returned referring metadata.
    pub fn resolve_unicode_imports(
        &self,
        resource: ResourceId,
        lookup_work: NonZeroUsize,
        limits: ImportRequestLimits,
    ) -> Result<ResolvedUnicodeImports, LocatedGraphError> {
        let (candidate, base) = self.import_context(resource, lookup_work)?;
        resolve_unicode_import_requests(candidate, base, limits).map_err(LocatedGraphError::Imports)
    }
    fn import_context(
        &self,
        resource: ResourceId,
        lookup_work: NonZeroUsize,
    ) -> Result<(&super::UnicodeStylesheetCandidate, &str), LocatedGraphError> {
        let mut work = lookup_work.get();
        let base = self
            .base_with_work(resource, &mut work)?
            .ok_or(LocatedGraphError::UnknownResource(resource))?;
        for sheet in self.graph.resources() {
            work = work.checked_sub(1).ok_or(RuleAssemblyError::Work)?;
            if sheet.resource == resource {
                return Ok((&sheet.candidate, base));
            }
        }
        Err(LocatedGraphError::UnknownResource(resource))
    }
}
