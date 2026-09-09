//! RFC9110 10.2.2 redirect Location resolution. Status/method selection, hop
//! limits, origin policy and actual acquisition are the redirect owner's work.
use super::uri::{UriError, UriLimits, UriReference, resolve_with_work};
use std::ops::Range;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocationError {
    MultipleFields { count: usize },
    TargetFragment,
    Uri(UriError),
}
impl From<UriError> for LocationError {
    fn from(e: UriError) -> Self {
        Self::Uri(e)
    }
}
impl std::fmt::Display for LocationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HTTP redirect Location: {self:?}")
    }
}
impl std::error::Error for LocationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Uri(e) => Some(e),
            _ => None,
        }
    }
}
#[derive(Debug)]
pub struct RedirectLocation {
    uri: String,
    acquisition_end: usize,
    location_span: Range<usize>,
}
impl RedirectLocation {
    pub fn reference_uri(&self) -> &str {
        &self.uri
    }
    pub fn acquisition_uri(&self) -> &str {
        &self.uri[..self.acquisition_end]
    }
    pub fn location_span(&self) -> Range<usize> {
        self.location_span.clone()
    }
    pub fn retained_bytes(&self) -> usize {
        self.uri.capacity()
    }
    pub fn into_reference_uri(self) -> String {
        self.uri
    }
}
fn spend(work: &mut usize, amount: usize) -> Result<(), UriError> {
    *work = work.checked_sub(amount).ok_or(UriError::Work)?;
    Ok(())
}
/// `target` is the actual fragment-free request target. `request_reference` is
/// the reference that generated that request, retaining its original fragment.
/// Their relationship is supplied by the host; URL equivalence is not guessed.
/// This function is for redirection, not a 201 Location or Content-Location.
pub fn resolve_redirect_location(
    target: &str,
    request_reference: &str,
    fields: &[&[u8]],
    limits: UriLimits,
) -> Result<Option<RedirectLocation>, LocationError> {
    if fields.is_empty() {
        return Ok(None);
    }
    if fields.len() != 1 {
        return Err(LocationError::MultipleFields {
            count: fields.len(),
        });
    }
    let raw = fields[0];
    if raw.len() > limits.bytes.get() {
        return Err(UriError::Bytes.into());
    }
    let mut work = limits.work.get();
    spend(
        &mut work,
        raw.len()
            .checked_mul(3)
            .and_then(|n| n.checked_add(1))
            .ok_or(UriError::Work)?,
    )?;
    let mut start = 0;
    let mut end = raw.len();
    while start < end && matches!(raw[start], b' ' | b'\t') {
        start += 1;
    }
    while end > start && matches!(raw[end - 1], b' ' | b'\t') {
        end -= 1;
    }
    let location = std::str::from_utf8(&raw[start..end]).map_err(|_| UriError::NonAscii)?;
    let base = UriReference::parse_with_work(target, limits.bytes, &mut work)?;
    if base.fragment().is_some() {
        return Err(LocationError::TargetFragment);
    }
    let request = UriReference::parse_with_work(request_reference, limits.bytes, &mut work)?;
    let reference = UriReference::parse_with_work(location, limits.bytes, &mut work)?;
    let inherited = if reference.fragment().is_none() {
        request.fragment()
    } else {
        None
    };
    let mut uri = resolve_with_work(target, location, limits.bytes, &mut work)?;
    spend(&mut work, uri.len())?;
    let acquisition_end = uri.find('#').unwrap_or(uri.len());
    if let Some(fragment) = inherited {
        let added = fragment.len().checked_add(1).ok_or(UriError::Bytes)?;
        let total = uri.len().checked_add(added).ok_or(UriError::Bytes)?;
        if total > limits.bytes.get() {
            return Err(UriError::Bytes.into());
        }
        // Includes copied fragment and worst-case relocation of the existing URI.
        spend(&mut work, total)?;
        uri.try_reserve_exact(added)
            .map_err(|_| UriError::Allocation)?;
        if uri.capacity() > limits.bytes.get() {
            return Err(UriError::Bytes.into());
        }
        uri.push('#');
        uri.push_str(fragment);
    }
    Ok(Some(RedirectLocation {
        uri,
        acquisition_end,
        location_span: start..end,
    }))
}
