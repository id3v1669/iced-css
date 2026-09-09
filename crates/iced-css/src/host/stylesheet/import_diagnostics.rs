//! Post-publication import-cut diagnostics with bounded live-input comparison.
use iced_css_core::{
    stylesheet::{
        SheetEntryKind,
        imports::{ImportCut, ImportCutReason},
        owned_graph::OwnedStylesheetGraph,
    },
    syntax::imports::ResourceId,
};
use std::num::NonZeroUsize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ImportDiagnosticError {
    Work,
    InvalidSpan,
}
impl std::fmt::Display for ImportDiagnosticError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "import diagnostics omitted: {self:?}")
    }
}
struct Work(usize);
impl Work {
    fn charge(&mut self, amount: usize) -> Result<(), ImportDiagnosticError> {
        self.0 = self
            .0
            .checked_sub(amount)
            .ok_or(ImportDiagnosticError::Work)?;
        Ok(())
    }
    fn equal(&mut self, left: &str, right: &str) -> Result<bool, ImportDiagnosticError> {
        self.charge(1)?;
        if left.len() != right.len() {
            return Ok(false);
        }
        self.charge(left.len())?;
        Ok(left == right)
    }
}
fn first(
    graph: &OwnedStylesheetGraph,
    index: usize,
    work: &mut Work,
) -> Result<bool, ImportDiagnosticError> {
    let cut = &graph.cuts()[index];
    for earlier in &graph.cuts()[..index] {
        work.charge(1)?;
        if earlier.source == cut.source
            && earlier.entry == cut.entry
            && earlier.reason == cut.reason
        {
            return Ok(false);
        }
    }
    Ok(true)
}
fn input<'a>(
    graph: &'a OwnedStylesheetGraph,
    cut: &ImportCut,
    work: &mut Work,
) -> Result<&'a str, ImportDiagnosticError> {
    for resource in graph.resources() {
        work.charge(1)?;
        if resource.resource != cut.source {
            continue;
        }
        work.charge(1)?;
        let entry = resource
            .candidate
            .sheet()
            .entries
            .get(cut.entry)
            .ok_or(ImportDiagnosticError::InvalidSpan)?;
        let SheetEntryKind::Import(import) = &entry.kind else {
            return Err(ImportDiagnosticError::InvalidSpan);
        };
        return resource
            .candidate
            .source()
            .text()
            .get(import.location_span.clone())
            .ok_or(ImportDiagnosticError::InvalidSpan);
    }
    Err(ImportDiagnosticError::InvalidSpan)
}
fn matching(
    graph: &OwnedStylesheetGraph,
    end: usize,
    source: ResourceId,
    reason: ImportCutReason,
    text: &str,
    work: &mut Work,
) -> Result<usize, ImportDiagnosticError> {
    let mut count = 0usize;
    for (index, cut) in graph.cuts()[..end].iter().enumerate() {
        work.charge(1)?;
        if cut.source != source || cut.reason != reason || !first(graph, index, work)? {
            continue;
        }
        let other = input(graph, cut, work)?;
        if work.equal(text, other)? {
            count = count.checked_add(1).ok_or(ImportDiagnosticError::Work)?;
        }
    }
    Ok(count)
}
/// Caller has already admitted this candidate. Comparison ignores offsets but
/// preserves the multiplicity of distinct authored import entries. No cause cache
/// or URI normalization is created, and diagnostic failure cannot undo CSS.
pub(super) fn report_import_cuts(
    active: &OwnedStylesheetGraph,
    previous: Option<&OwnedStylesheetGraph>,
    limit: NonZeroUsize,
) -> Result<(), ImportDiagnosticError> {
    let mut work = Work(limit.get());
    for (index, cut) in active.cuts().iter().enumerate() {
        work.charge(1)?;
        let description = match cut.reason {
            ImportCutReason::Cycle => "cyclic",
            ImportCutReason::Unavailable => "unavailable",
            ImportCutReason::MediaExcluded => continue,
        };
        if !first(active, index, &mut work)? {
            continue;
        }
        if let Some(previous) = previous {
            let text = input(active, cut, &mut work)?;
            let old = matching(
                previous,
                previous.cuts().len(),
                cut.source,
                cut.reason,
                text,
                &mut work,
            )?;
            if old != 0 {
                let earlier = matching(active, index, cut.source, cut.reason, text, &mut work)?;
                if earlier < old {
                    continue;
                }
            }
        }
        log::error!(target: "iced_css::stylesheet", "stylesheet resource {:?} import entry {} {description}", cut.source, cut.entry);
    }
    Ok(())
}
