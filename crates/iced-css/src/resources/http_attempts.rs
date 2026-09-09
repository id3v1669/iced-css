//! Conservative per-attempt charges and retained actual acquisition outcomes.
use super::{HttpAttemptError, HttpAttemptLimits};
use crate::resources::http_stylesheet::{HttpStylesheetReadError, HttpStylesheetRepresentation};
use std::{num::NonZeroUsize, sync::Arc};
type Acquisition = Result<Arc<HttpStylesheetRepresentation>, Arc<HttpStylesheetReadError>>;
pub struct HttpDependency {
    reference: String,
    key: String,
    final_key: Option<String>,
    canonical: usize,
    result: Option<Acquisition>,
}
impl HttpDependency {
    pub fn reference_uri(&self) -> &str {
        &self.reference
    }
    pub fn acquisition_key(&self) -> &str {
        &self.key
    }
    /// Recorded after consistency/storage admission; failure can leave this absent
    /// even when received effective-URI metadata exists on the representation.
    pub fn final_key(&self) -> Option<&str> {
        self.final_key.as_deref()
    }
    pub fn canonical_entry(&self) -> usize {
        self.canonical
    }
    pub fn representation(&self) -> Option<&HttpStylesheetRepresentation> {
        match self.result.as_ref()? {
            Ok(value) => Some(value),
            Err(_) => None,
        }
    }
    pub fn error(&self) -> Option<&HttpStylesheetReadError> {
        match self.result.as_ref()? {
            Ok(_) => None,
            Err(error) => Some(error),
        }
    }
}
pub struct HttpDependencyReport {
    entries: Vec<HttpDependency>,
    payload: usize,
    work: usize,
    limits: HttpAttemptLimits,
}
impl HttpDependencyReport {
    pub(super) fn new(limits: HttpAttemptLimits) -> Self {
        Self {
            entries: Vec::new(),
            payload: 0,
            work: limits.work.get(),
            limits,
        }
    }
    pub fn entries(&self) -> &[HttpDependency] {
        &self.entries
    }
    /// Reserved per-acquisition allowances plus actual report/string capacities.
    /// This is not HeaderMap/backend allocator measurement or process memory.
    pub fn charged_bytes(&self) -> usize {
        self.payload + self.entries.capacity() * size_of::<HttpDependency>()
    }
    pub(super) fn record(
        &mut self,
        reference: &str,
        key: String,
        packet: usize,
    ) -> Result<usize, HttpAttemptError> {
        if self.limits.retained_bytes.get() > isize::MAX as usize {
            return Err(HttpAttemptError::Bytes);
        }
        spend(&mut self.work, 1)?;
        if self.entries.len() >= self.limits.entries.get() {
            return Err(HttpAttemptError::Entries);
        }
        let base = self
            .payload
            .checked_add(packet)
            .and_then(|n| n.checked_add(key.capacity()))
            .filter(|n| *n <= self.limits.retained_bytes.get())
            .ok_or(HttpAttemptError::Bytes)?;
        let available = NonZeroUsize::new(self.limits.retained_bytes.get() - base)
            .ok_or(HttpAttemptError::Bytes)?;
        let reference = copy(reference, available, &mut self.work)?;
        let payload = base
            .checked_add(reference.capacity())
            .filter(|n| *n <= self.limits.retained_bytes.get())
            .ok_or(HttpAttemptError::Bytes)?;
        let maximum = ((self.limits.retained_bytes.get() - payload) / size_of::<HttpDependency>())
            .min(self.limits.entries.get());
        if self.entries.capacity() > maximum || self.entries.len() >= maximum {
            return Err(HttpAttemptError::Bytes);
        }
        if self.entries.len() == self.entries.capacity() {
            let count = self
                .entries
                .len()
                .checked_add(1)
                .ok_or(HttpAttemptError::Entries)?;
            let target = count.max(self.entries.capacity().saturating_mul(2).min(maximum));
            spend(
                &mut self.work,
                self.entries
                    .len()
                    .checked_mul(size_of::<HttpDependency>())
                    .ok_or(HttpAttemptError::Work)?,
            )?;
            let mut next = Vec::new();
            next.try_reserve_exact(target)
                .map_err(|_| HttpAttemptError::Allocation)?;
            if next.capacity() > maximum {
                return Err(HttpAttemptError::Bytes);
            }
            next.append(&mut self.entries);
            self.entries = next;
        }
        let index = self.entries.len();
        self.entries.push(HttpDependency {
            reference,
            key,
            final_key: None,
            canonical: index,
            result: None,
        });
        self.payload = payload;
        Ok(index)
    }
    pub(super) fn request_copy(
        &mut self,
        index: usize,
        limit: NonZeroUsize,
    ) -> Result<String, HttpAttemptError> {
        let entry = self.entries.get(index).ok_or(HttpAttemptError::Target)?;
        copy(&entry.reference, limit, &mut self.work)
    }
    pub(super) fn text_copy(
        &mut self,
        text: &str,
        limit: NonZeroUsize,
    ) -> Result<String, HttpAttemptError> {
        copy(text, limit, &mut self.work)
    }
    pub(super) fn finish(
        &mut self,
        index: usize,
        result: Result<HttpStylesheetRepresentation, HttpStylesheetReadError>,
    ) -> Result<(), HttpAttemptError> {
        let entry = self
            .entries
            .get_mut(index)
            .ok_or(HttpAttemptError::Target)?;
        if entry.result.is_some() {
            return Err(HttpAttemptError::Target);
        }
        entry.result = Some(result.map(Arc::new).map_err(Arc::new));
        Ok(())
    }
    pub(super) fn result(&self, index: usize) -> Result<Acquisition, HttpAttemptError> {
        self.entries
            .get(index)
            .and_then(|entry| entry.result.as_ref())
            .cloned()
            .ok_or(HttpAttemptError::Target)
    }
    pub(super) fn key(&self, index: usize) -> Result<&str, HttpAttemptError> {
        self.entries
            .get(index)
            .map(|entry| entry.final_key.as_deref().unwrap_or(&entry.key))
            .ok_or(HttpAttemptError::Target)
    }
    pub(super) fn find_requested(&mut self, key: &str) -> Result<Option<usize>, HttpAttemptError> {
        for entry in &self.entries {
            if equal(entry.key.as_bytes(), key.as_bytes(), &mut self.work)? {
                return Ok(Some(entry.canonical));
            }
        }
        Ok(None)
    }
    pub(super) fn find_final(&mut self, key: &str) -> Result<Option<usize>, HttpAttemptError> {
        for entry in &self.entries {
            spend(&mut self.work, 1)?;
            if let Some(value) = &entry.final_key
                && equal(value.as_bytes(), key.as_bytes(), &mut self.work)?
            {
                return Ok(Some(entry.canonical));
            }
        }
        Ok(None)
    }
    pub(super) fn set_final(
        &mut self,
        index: usize,
        key: String,
    ) -> Result<usize, HttpAttemptError> {
        let payload = self
            .payload
            .checked_add(key.capacity())
            .filter(|n| {
                n.checked_add(self.entries.capacity() * size_of::<HttpDependency>())
                    .is_some_and(|total| total <= self.limits.retained_bytes.get())
            })
            .ok_or(HttpAttemptError::Bytes)?;
        let current = self.entries.get(index).ok_or(HttpAttemptError::Target)?;
        if current.final_key.is_some() {
            return Err(HttpAttemptError::Target);
        }
        let current_representation = current.representation().ok_or(HttpAttemptError::Target)?;
        let mut canonical = index;
        for (prior_index, prior) in self.entries[..index].iter().enumerate() {
            spend(&mut self.work, 1)?;
            let Some(prior_key) = &prior.final_key else {
                continue;
            };
            if !equal(prior_key.as_bytes(), key.as_bytes(), &mut self.work)? {
                continue;
            }
            let prior_representation = prior.representation().ok_or(HttpAttemptError::Target)?;
            if !equal(
                prior_representation.content_decoded_body().as_slice(),
                current_representation.content_decoded_body().as_slice(),
                &mut self.work,
            )? || !optional_equal(
                prior_representation.transport_charset(),
                current_representation.transport_charset(),
                &mut self.work,
            )? {
                return Err(HttpAttemptError::ConflictingRepresentation);
            }
            canonical = self.entries[prior_index].canonical;
            break;
        }
        self.entries[index].final_key = Some(key);
        self.entries[index].canonical = canonical;
        self.payload = payload;
        Ok(canonical)
    }
}
fn spend(work: &mut usize, amount: usize) -> Result<(), HttpAttemptError> {
    *work = work.checked_sub(amount).ok_or(HttpAttemptError::Work)?;
    Ok(())
}
fn equal(left: &[u8], right: &[u8], work: &mut usize) -> Result<bool, HttpAttemptError> {
    spend(
        work,
        left.len()
            .min(right.len())
            .checked_add(1)
            .ok_or(HttpAttemptError::Work)?,
    )?;
    Ok(left == right)
}
fn optional_equal(
    left: Option<&[u8]>,
    right: Option<&[u8]>,
    work: &mut usize,
) -> Result<bool, HttpAttemptError> {
    spend(work, 1)?;
    match (left, right) {
        (Some(a), Some(b)) => equal(a, b, work),
        (None, None) => Ok(true),
        _ => Ok(false),
    }
}
fn copy(input: &str, maximum: NonZeroUsize, work: &mut usize) -> Result<String, HttpAttemptError> {
    if maximum.get() > isize::MAX as usize || input.len() > maximum.get() {
        return Err(HttpAttemptError::Bytes);
    }
    spend(work, input.len())?;
    let mut output = String::new();
    output
        .try_reserve_exact(input.len())
        .map_err(|_| HttpAttemptError::Allocation)?;
    if output.capacity() > maximum.get() {
        return Err(HttpAttemptError::Bytes);
    }
    output.push_str(input);
    Ok(output)
}
