//! Bounded exact-input recovery reporting, with prior-occurrence suppression.
use iced_css_core::stylesheet::{
    RejectionReason, SheetEntryKind, SheetRejectionReason, UnicodeStylesheetCandidate,
};
use std::{num::NonZeroUsize, ops::Range};
#[derive(Debug)]
pub(super) enum Error {
    Work,
    InvalidSpan,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Sheet(SheetRejectionReason),
    Declaration(RejectionReason),
}
#[derive(Clone)]
struct Notice<'a> {
    kind: Kind,
    name: Option<&'a str>,
    span: &'a Range<usize>,
}
fn charge(work: &mut usize, n: usize) -> Result<(), Error> {
    *work = work.checked_sub(n).ok_or(Error::Work)?;
    Ok(())
}
fn notices<'a>(
    candidate: &'a UnicodeStylesheetCandidate,
    work: &mut usize,
) -> Result<impl Iterator<Item = Notice<'a>> + use<'a>, Error> {
    let sheet = candidate.sheet();
    // Prepay complete iterator traversal, including skipped entries. The budget
    // calculation itself visits one entry and selected media rule per charged step.
    charge(work, 1)?;
    charge(work, sheet.rejected.len())?;
    for entry in &sheet.entries {
        charge(work, 2)?;
        match &entry.kind {
            SheetEntryKind::Style(rule) => charge(work, rule.rejected.len())?,
            SheetEntryKind::Media(media) if media.applies_to_screen => {
                for rule in &media.rules {
                    charge(work, 2)?;
                    charge(work, rule.rejected.len())?;
                }
            }
            _ => {}
        }
    }
    Ok(sheet
        .rejected
        .iter()
        .filter(|r| r.reason != SheetRejectionReason::Charset)
        .map(|r| Notice {
            kind: Kind::Sheet(r.reason),
            name: None,
            span: &r.span,
        })
        .chain(
            sheet
                .local_screen_rules()
                .flat_map(|r| r.rejected.iter())
                .map(|r| Notice {
                    kind: Kind::Declaration(r.reason),
                    name: r.name.as_deref(),
                    span: &r.span,
                }),
        ))
}
fn bytes_equal(a: &str, b: &str, work: &mut usize) -> Result<bool, Error> {
    charge(work, 1)?;
    if a.len() != b.len() {
        return Ok(false);
    }
    charge(work, a.len())?;
    Ok(a == b)
}
fn same(
    a: &Notice<'_>,
    a_source: &str,
    b: &Notice<'_>,
    b_source: &str,
    work: &mut usize,
) -> Result<bool, Error> {
    charge(work, 1)?;
    if a.kind != b.kind {
        return Ok(false);
    }
    match (a.name, b.name) {
        (Some(a), Some(b)) if !bytes_equal(a, b, work)? => return Ok(false),
        (None, Some(_)) | (Some(_), None) => return Ok(false),
        _ => {}
    }
    let a = a_source.get(a.span.clone()).ok_or(Error::InvalidSpan)?;
    let b = b_source.get(b.span.clone()).ok_or(Error::InvalidSpan)?;
    bytes_equal(a, b, work)
}
/// No cause cache or copied source is allocated. Input spelling changes remain
/// distinct diagnostics; offsets alone do not. Extra equal occurrences still log.
pub(super) fn report(
    active: &UnicodeStylesheetCandidate,
    previous: Option<&UnicodeStylesheetCandidate>,
    limit: NonZeroUsize,
) -> Result<(), Error> {
    report_with_work(active, previous, &mut limit.get(), None)
}
pub(super) fn report_with_work(
    active: &UnicodeStylesheetCandidate,
    previous: Option<&UnicodeStylesheetCandidate>,
    work: &mut usize,
    resource: Option<iced_css_core::syntax::imports::ResourceId>,
) -> Result<(), Error> {
    for (index, notice) in notices(active, work)?.enumerate() {
        charge(work, 1)?;
        let mut old_count = 0usize;
        if let Some(previous) = previous {
            for old in notices(previous, work)? {
                if same(
                    &notice,
                    active.source().text(),
                    &old,
                    previous.source().text(),
                    work,
                )? {
                    old_count += 1;
                }
            }
        }
        let mut earlier_count = 0usize;
        if old_count > 0 {
            for earlier in notices(active, work)?.take(index) {
                if same(
                    &notice,
                    active.source().text(),
                    &earlier,
                    active.source().text(),
                    work,
                )? {
                    earlier_count += 1;
                }
            }
        }
        if earlier_count >= old_count {
            if let Some(resource) = resource {
                log::error!(target: "iced_css::stylesheet", "stylesheet resource {resource:?} skipped input at decoded bytes {}..{}: {:?}", notice.span.start, notice.span.end, notice.kind);
            } else {
                log::error!(target: "iced_css::stylesheet", "stylesheet skipped input at decoded bytes {}..{}: {:?}", notice.span.start, notice.span.end, notice.kind);
            }
        }
    }
    Ok(())
}
