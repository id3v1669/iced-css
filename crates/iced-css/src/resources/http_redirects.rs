//! One actor-owned, already-authorized GET chain. This is not an HTTP cache.
use super::{HttpError, HttpExchangeTiming, HttpLimits, HttpResponse, exchange, transport_url};
use iced_css_core::resources::location::{LocationError, resolve_redirect_location};
use std::{mem::size_of, num::NonZeroUsize};

#[derive(Debug, Clone, Copy)]
pub struct RedirectLimits {
    pub hops: NonZeroUsize,
    /// Initial reference, hop vector and retained hop string/field capacities.
    /// Per-exchange bodies/headers and upstream URL allocations are separate.
    pub retained_bytes: NonZeroUsize,
    /// History scans, record moves and field copies; URI work is per exchange.
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedirectError {
    Hops,
    Loop,
    Bytes,
    Work,
    Allocation,
    Location(LocationError),
}
impl std::fmt::Display for RedirectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HTTP GET redirect chain: {self:?}")
    }
}
impl std::error::Error for RedirectError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Location(error) => Some(error),
            _ => None,
        }
    }
}
impl From<RedirectError> for HttpError {
    fn from(error: RedirectError) -> Self {
        Self::Redirect(error)
    }
}
#[derive(Debug)]
pub struct RedirectHop {
    status: u16,
    version: reqwest::Version,
    source: String,
    location: Vec<u8>,
    target: String,
    timing: HttpExchangeTiming,
}
impl RedirectHop {
    pub fn version(&self) -> reqwest::Version {
        self.version
    }
    pub fn timing(&self) -> HttpExchangeTiming {
        self.timing
    }
    pub fn status(&self) -> u16 {
        self.status
    }
    /// Actual fragment-free response target, after transport URL conversion.
    pub fn source_uri(&self) -> &str {
        &self.source
    }
    /// Field bytes retained from the parsed response HeaderMap, not raw wire framing.
    pub fn location(&self) -> &[u8] {
        &self.location
    }
    pub fn target_reference_uri(&self) -> &str {
        &self.target
    }
}
struct History {
    entries: Vec<RedirectHop>,
    retained: usize,
    work: usize,
    limits: RedirectLimits,
}
impl History {
    fn spend(&mut self, amount: usize) -> Result<(), RedirectError> {
        self.work = self.work.checked_sub(amount).ok_or(RedirectError::Work)?;
        Ok(())
    }
    fn bytes(&self, extra: usize) -> Result<usize, RedirectError> {
        self.retained
            .checked_add(extra)
            .filter(|n| *n <= self.limits.retained_bytes.get())
            .ok_or(RedirectError::Bytes)
    }
    fn repeated(&mut self, target: &str, current: &str) -> Result<bool, RedirectError> {
        self.spend(
            target
                .len()
                .checked_add(current.len())
                .and_then(|n| n.checked_add(1))
                .ok_or(RedirectError::Work)?,
        )?;
        if target == current {
            return Ok(true);
        }
        for index in 0..self.entries.len() {
            let len = self.entries[index].source.len();
            self.spend(
                target
                    .len()
                    .checked_add(len)
                    .and_then(|n| n.checked_add(1))
                    .ok_or(RedirectError::Work)?,
            )?;
            if target == self.entries[index].source {
                return Ok(true);
            }
        }
        Ok(false)
    }
    fn push(
        &mut self,
        status: u16,
        source: String,
        field: &[u8],
        target: String,
        timing: HttpExchangeTiming,
        version: reqwest::Version,
    ) -> Result<(), RedirectError> {
        let strings = source
            .capacity()
            .checked_add(target.capacity())
            .ok_or(RedirectError::Bytes)?;
        let pending = strings
            .checked_add(field.len())
            .ok_or(RedirectError::Bytes)?;
        self.bytes(pending)?;
        if self.entries.len() == self.entries.capacity() {
            let old = self.entries.capacity();
            self.bytes(
                pending
                    .checked_add(size_of::<RedirectHop>())
                    .ok_or(RedirectError::Bytes)?,
            )?;
            self.spend(
                old.checked_add(1)
                    .and_then(|n| n.checked_mul(size_of::<RedirectHop>()))
                    .ok_or(RedirectError::Work)?,
            )?;
            self.entries
                .try_reserve_exact(1)
                .map_err(|_| RedirectError::Allocation)?;
            let extra = (self.entries.capacity() - old)
                .checked_mul(size_of::<RedirectHop>())
                .ok_or(RedirectError::Bytes)?;
            self.retained = self.bytes(extra)?;
            self.bytes(pending)?;
        }
        self.spend(field.len().checked_add(1).ok_or(RedirectError::Work)?)?;
        let mut location = Vec::new();
        location
            .try_reserve_exact(field.len())
            .map_err(|_| RedirectError::Allocation)?;
        self.retained = self.bytes(
            strings
                .checked_add(location.capacity())
                .ok_or(RedirectError::Bytes)?,
        )?;
        location.extend_from_slice(field);
        self.entries.push(RedirectHop {
            status,
            version,
            source,
            location,
            target,
            timing,
        });
        Ok(())
    }
}

pub(super) fn exchange_following(
    client: &reqwest::blocking::Client,
    initial: String,
    limits: HttpLimits,
    redirect_limits: RedirectLimits,
    cancelled: impl Fn() -> bool,
) -> Result<HttpResponse, HttpError> {
    let mut history = History {
        entries: Vec::new(),
        retained: initial.capacity(),
        work: redirect_limits.work.get(),
        limits: redirect_limits,
    };
    loop {
        if cancelled() {
            return Err(HttpError::Stopped);
        }
        let reference = history
            .entries
            .last()
            .map_or(initial.as_str(), |hop| hop.target.as_str());
        let mut response = exchange(client, reference, None, limits)?;
        if cancelled() {
            return Err(HttpError::Stopped);
        }
        if !matches!(response.status, 300 | 301 | 302 | 303 | 307 | 308)
            || !response.headers.contains_key(reqwest::header::LOCATION)
        {
            response.initial_reference = Some(initial);
            response.redirect_chain = history.entries;
            return Ok(response);
        }
        if history.entries.len() == redirect_limits.hops.get() {
            return Err(RedirectError::Hops.into());
        }
        // Inspect the strict singleton without collecting another header vector.
        let count = response
            .headers
            .get_all(reqwest::header::LOCATION)
            .iter()
            .count();
        history.spend(count)?;
        if count != 1 {
            return Err(RedirectError::Location(LocationError::MultipleFields { count }).into());
        }
        let field = response.headers[reqwest::header::LOCATION].as_bytes();
        // Reborrow after history bookkeeping; the reference is stable while resolving.
        let reference = history
            .entries
            .last()
            .map_or(initial.as_str(), |hop| hop.target.as_str());
        let location =
            resolve_redirect_location(&response.effective_uri, reference, &[field], limits.uri)
                .map_err(RedirectError::Location)?;
        let Some(location) = location else {
            unreachable!("a singleton Location was supplied")
        };
        let normalized = transport_url(location.acquisition_uri(), limits.uri)?;
        if history.repeated(normalized.as_str(), &response.effective_uri)? {
            return Err(RedirectError::Loop.into());
        }
        history.push(
            response.status,
            response.effective_uri,
            field,
            location.into_reference_uri(),
            response.timing,
            response.version,
        )?;
        // The rest of this intermediate response drops on the HTTP actor. No
        // representation/cookie/validator metadata is copied into the next GET.
    }
}
