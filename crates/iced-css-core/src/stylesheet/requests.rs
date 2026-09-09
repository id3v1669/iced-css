//! Resolve applicable import references before resource identity binding. This
//! module neither fetches resources nor deduplicates stylesheet occurrences.
use super::{ParsedStylesheet, Records, RuleAssemblyError, SheetEntryKind};
use crate::resources::uri::{UriError, resolve_with_work};
use std::{num::NonZeroUsize, ops::Range};

#[derive(Debug, Clone, Copy)]
pub struct ImportRequestLimits {
    pub entries: NonZeroUsize,
    /// Request record and URI String capacities. Borrowed parsed sheet data and
    /// temporary old/new allocation overlap are excluded.
    pub retained_bytes: NonZeroUsize,
    pub work: NonZeroUsize,
}
#[derive(Debug)]
pub struct ResolvedImportRequest {
    pub entry: usize,
    pub location_span: Range<usize>,
    uri: String,
    acquisition_end: usize,
}
impl ResolvedImportRequest {
    pub fn reference_uri(&self) -> &str {
        &self.uri
    }
    /// Fragment-free acquisition view; query presence and spelling are preserved.
    pub fn acquisition_uri(&self) -> &str {
        &self.uri[..self.acquisition_end]
    }
}
#[derive(Debug)]
pub enum ImportRequestError {
    Budget(RuleAssemblyError),
    Uri {
        entry: usize,
        location: Range<usize>,
        error: UriError,
    },
}
impl From<RuleAssemblyError> for ImportRequestError {
    fn from(error: RuleAssemblyError) -> Self {
        Self::Budget(error)
    }
}
impl std::fmt::Display for ImportRequestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "import request resolution: {self:?}")
    }
}
impl std::error::Error for ImportRequestError {}

/// Resolve each applicable import against the explicit effective retrieval base.
/// Inactive media is excluded before URI processing. URI conversion/implementation
/// failures stay located errors; they never silently discard valid required CSS.
///
/// The same work allowance covers scanning, URI resolution and fragment removal.
/// Each resolution additionally bounds its inputs and output by remaining storage,
/// matching the existing generic resolver contract. No partial candidate escapes.
pub fn resolve_import_requests(
    sheet: &ParsedStylesheet,
    effective_base: &str,
    limits: ImportRequestLimits,
) -> Result<Vec<ResolvedImportRequest>, ImportRequestError> {
    if limits.retained_bytes.get() > isize::MAX as usize {
        return Err(RuleAssemblyError::Bytes.into());
    }
    let mut records = Records {
        bytes: 0,
        limit: limits.retained_bytes.get(),
        work: limits.work.get(),
    };
    let mut requests = Vec::new();
    for (entry, statement) in sheet.entries.iter().enumerate() {
        records.spend(1)?;
        let SheetEntryKind::Import(import) = &statement.kind else {
            continue;
        };
        if !import.applies_to_screen {
            continue;
        }
        if requests.len() == limits.entries.get() {
            return Err(RuleAssemblyError::Entries.into());
        }
        records.reserve(&mut requests)?;
        let available =
            NonZeroUsize::new(records.limit - records.bytes).ok_or(RuleAssemblyError::Bytes)?;
        let uri = resolve_with_work(
            effective_base,
            &import.location.text,
            available,
            &mut records.work,
        )
        .map_err(|error| ImportRequestError::Uri {
            entry,
            location: import.location_span.clone(),
            error,
        })?;
        records.bytes = records
            .bytes
            .checked_add(uri.capacity())
            .filter(|bytes| *bytes <= records.limit)
            .ok_or(RuleAssemblyError::Bytes)?;
        // Generic URI validation already guarantees ASCII; the first literal '#'
        // is the fragment delimiter, never an escaped path/query character.
        records.spend(uri.len())?;
        let acquisition_end = uri.find('#').unwrap_or(uri.len());
        requests.push(ResolvedImportRequest {
            entry,
            location_span: import.location_span.clone(),
            uri,
            acquisition_end,
        });
    }
    Ok(requests)
}

/// Owned URI requests and the selected charset of their referring stylesheet.
/// The static charset label remains valid after the parsed parent is dropped.
/// Each request retains its own import entry even when acquisition URIs match.
#[derive(Debug)]
pub struct ResolvedUnicodeImports {
    requests: Vec<ResolvedImportRequest>,
    referring_label: &'static [u8],
}
impl ResolvedUnicodeImports {
    pub(super) fn charge_retained(&self, records: &mut Records) -> Result<(), RuleAssemblyError> {
        records.retain(
            self.requests
                .capacity()
                .checked_mul(size_of::<ResolvedImportRequest>())
                .ok_or(RuleAssemblyError::Bytes)?,
        )?;
        for request in &self.requests {
            records.spend(1)?;
            records.retain(request.uri.capacity())?;
        }
        Ok(())
    }
    pub fn requests(&self) -> &[ResolvedImportRequest] {
        &self.requests
    }
    /// Supply only actual child transport/linking metadata. The parent charset
    /// occupies the referring priority; CSS detection still runs before it.
    pub fn encoding_metadata<'a>(
        &self,
        transport: Option<&'a [u8]>,
        linking: Option<&'a [u8]>,
    ) -> crate::syntax::encoding_selection::EncodingMetadata<'a> {
        crate::syntax::encoding_selection::EncodingMetadata {
            transport,
            linking,
            referring: Some(self.referring_label),
        }
    }
}

/// Resolve imports from a decoded separate-file candidate without losing the
/// selected referring charset. All URI/record work uses the existing explicit
/// resolver limits; the static label introduces no additional heap storage.
/// This performs no acquisition, caching or publication.
pub fn resolve_unicode_import_requests(
    candidate: &super::UnicodeStylesheetCandidate,
    effective_base: &str,
    limits: ImportRequestLimits,
) -> Result<ResolvedUnicodeImports, ImportRequestError> {
    Ok(ResolvedUnicodeImports {
        requests: resolve_import_requests(candidate.sheet(), effective_base, limits)?,
        referring_label: candidate.source().referring_label(),
    })
}
