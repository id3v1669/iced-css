//! Background HTTP exchanges and explicitly bounded GET redirect chains.
//! The host still interprets final status, content coding, MIME and cache state.
use super::{
    budget::{Lease, OperationBudget},
    http_headers::{HttpHeaderBlock, HttpHeaderError, HttpHeaderLimits},
    local::{ReadBodyError, ReadBodyLimits, read_body_blocking},
};
use iced::futures::channel::oneshot;
use iced_css_core::resources::{
    body::CompletedBody,
    entity_tag::{EntityTagError, EntityTagLimits, parse_entity_tag_fields},
    uri::{UriError, UriLimits, UriReference},
};
use std::{
    future::Future,
    num::NonZeroUsize,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, mpsc},
    time::{Duration, Instant, SystemTime},
};

#[path = "http_redirects.rs"]
mod redirects;
pub use redirects::{RedirectError, RedirectHop, RedirectLimits};

/// Consecutive wall and monotonic clock reads, not an atomic clock correlation.
/// Wall time is retained without assuming that it moves forwards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HttpClockSample {
    wall: SystemTime,
    monotonic: Instant,
}
impl HttpClockSample {
    fn capture() -> Self {
        Self {
            wall: SystemTime::now(),
            monotonic: Instant::now(),
        }
    }
    pub fn wall(self) -> SystemTime {
        self.wall
    }
    pub fn monotonic(self) -> Instant {
        self.monotonic
    }
}

/// Observations around the blocking client's send call, taken on our HTTP actor.
/// Includes upstream scheduling/connection work; excludes our actor queue and
/// subsequent body collection. These are not precise wire timestamps or cache age.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HttpExchangeTiming {
    request: HttpClockSample,
    response_headers: HttpClockSample,
}
impl HttpExchangeTiming {
    pub fn request(self) -> HttpClockSample {
        self.request
    }
    pub fn response_headers(self) -> HttpClockSample {
        self.response_headers
    }
}

#[derive(Debug)]
pub struct HttpSettings {
    pub operations: NonZeroUsize,
    /// Total time for one exchange, from connecting until its body has been read.
    /// Each followed redirect hop is a separate exchange.
    pub timeout: Duration,
    pub connect_timeout: Duration,
    pub additional_roots: Vec<Vec<u8>>,
    pub root_entries: NonZeroUsize,
    /// Sum of retained DER vector capacities; upstream TLS allocations are separate.
    pub root_bytes: NonZeroUsize,
}
#[derive(Debug, Clone, Copy)]
pub struct HttpLimits {
    pub uri: UriLimits,
    pub header_fields: NonZeroUsize,
    /// Sum of received field name/value lengths, checked after upstream parsing.
    pub header_bytes: NonZeroUsize,
    /// Work for validating an explicitly supplied request entity tag.
    pub header_work: NonZeroUsize,
    pub read: ReadBodyLimits,
}
#[derive(Debug)]
pub enum HttpError {
    Settings,
    Roots,
    Limit,
    Stopped,
    Panicked,
    Headers,
    Allocation,
    Uri(UriError),
    EntityTag(EntityTagError),
    Redirect(RedirectError),
    InvalidHeader(reqwest::header::InvalidHeaderValue),
    UnsupportedUri,
    Spawn(std::io::Error),
    Tls(rustls::Error),
    Request(reqwest::Error),
    Read(ReadBodyError),
}
impl std::fmt::Display for HttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HTTP exchange: {self:?}")
    }
}
impl std::error::Error for HttpError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Uri(e) => Some(e),
            Self::EntityTag(e) => Some(e),
            Self::Redirect(e) => Some(e),
            Self::InvalidHeader(e) => Some(e),
            Self::Spawn(e) => Some(e),
            Self::Tls(e) => Some(e),
            Self::Request(e) => Some(e),
            Self::Read(e) => Some(e),
            _ => None,
        }
    }
}
#[derive(Debug)]
pub struct HttpResponse {
    status: u16,
    version: reqwest::Version,
    effective_uri: String,
    headers: reqwest::header::HeaderMap,
    body: CompletedBody,
    initial_reference: Option<String>,
    redirect_chain: Vec<RedirectHop>,
    timing: HttpExchangeTiming,
}
/// Owned received metadata, still describing the coded HTTP representation.
/// Content decoding does not rewrite its length, coding, validator or URI fields.
#[derive(Debug)]
pub struct HttpResponseMetadata {
    pub status: u16,
    /// Reported response protocol, not a claim about every intermediary.
    pub version: reqwest::Version,
    pub effective_uri: String,
    pub headers: reqwest::header::HeaderMap,
    pub initial_reference: Option<String>,
    pub redirect_chain: Vec<RedirectHop>,
    pub timing: HttpExchangeTiming,
}
impl HttpResponseMetadata {
    /// Borrow every parsed field for core processing without storage filtering.
    pub fn header_block(
        &self,
        limits: HttpHeaderLimits,
    ) -> Result<HttpHeaderBlock<'_>, HttpHeaderError> {
        HttpHeaderBlock::prepare(&self.headers, limits)
    }
}
impl HttpResponse {
    /// Borrow every parsed field for core processing without storage filtering.
    pub fn header_block(
        &self,
        limits: HttpHeaderLimits,
    ) -> Result<HttpHeaderBlock<'_>, HttpHeaderError> {
        HttpHeaderBlock::prepare(&self.headers, limits)
    }
    /// Move the body independently while retaining actual transport metadata.
    /// This performs no allocation, decoding, cache admission or MIME inference.
    pub fn into_parts(self) -> (HttpResponseMetadata, CompletedBody) {
        let Self {
            status,
            version,
            effective_uri,
            headers,
            body,
            initial_reference,
            redirect_chain,
            timing,
        } = self;
        (
            HttpResponseMetadata {
                status,
                version,
                effective_uri,
                headers,
                initial_reference,
                redirect_chain,
                timing,
            },
            body,
        )
    }
    /// Present for the explicitly redirecting API, preserving the supplied reference.
    pub fn initial_reference_uri(&self) -> Option<&str> {
        self.initial_reference.as_deref()
    }
    pub fn redirects(&self) -> &[RedirectHop] {
        &self.redirect_chain
    }
    /// The reference used for the last request, retaining its fragment in a chain.
    pub fn final_reference_uri(&self) -> &str {
        self.redirect_chain.last().map_or_else(
            || {
                self.initial_reference
                    .as_deref()
                    .unwrap_or(&self.effective_uri)
            },
            |hop| hop.target_reference_uri(),
        )
    }

    pub fn status(&self) -> u16 {
        self.status
    }
    pub fn version(&self) -> reqwest::Version {
        self.version
    }
    /// Timing of this response's request, not the start of its redirect chain.
    pub fn timing(&self) -> HttpExchangeTiming {
        self.timing
    }
    pub fn effective_uri(&self) -> &str {
        &self.effective_uri
    }
    pub fn headers(&self) -> &reqwest::header::HeaderMap {
        &self.headers
    }
    pub fn body(&self) -> &CompletedBody {
        &self.body
    }
    pub fn into_body(self) -> CompletedBody {
        self.body
    }
}
struct Job {
    uri: String,
    validator: Option<Vec<u8>>,
    redirects: Option<RedirectLimits>,
    limits: HttpLimits,
    lease: Arc<Lease>,
    sender: oneshot::Sender<Result<HttpResponse, HttpError>>,
}
/// Clones share one actor and operation limit. Client construction and destruction
/// run on that actor, never on the caller's executor. Dropping a future cannot
/// forcibly cancel blocking I/O; host lifetime tickets still govern publication.
#[derive(Clone)]
pub struct DirectHttpTransport {
    sender: mpsc::Sender<Job>,
    budget: OperationBudget,
}
impl DirectHttpTransport {
    pub async fn new(settings: HttpSettings) -> Result<Self, HttpError> {
        if settings.timeout.is_zero() || settings.connect_timeout.is_zero() {
            return Err(HttpError::Settings);
        }
        let root_bytes = settings
            .additional_roots
            .iter()
            .try_fold(0usize, |n, r| n.checked_add(r.capacity()))
            .ok_or(HttpError::Roots)?;
        if settings.additional_roots.len() > settings.root_entries.get()
            || root_bytes > settings.root_bytes.get()
        {
            return Err(HttpError::Roots);
        }
        let budget = OperationBudget::new(settings.operations);
        let (sender, receiver) = mpsc::channel::<Job>();
        let (ready_tx, ready_rx) = oneshot::channel();
        let thread = std::thread::Builder::new()
            .name("iced-css-http".into())
            .spawn(move || {
                let timeout = settings.timeout;
                let client = match catch_unwind(AssertUnwindSafe(|| build_client(settings))) {
                    Ok(Ok(client)) => client,
                    Ok(Err(error)) => {
                        let _ = ready_tx.send(Err(error));
                        return;
                    }
                    Err(_) => {
                        let _ = ready_tx.send(Err(HttpError::Panicked));
                        return;
                    }
                };
                if ready_tx.send(Ok(())).is_err() {
                    return;
                }
                while let Ok(job) = receiver.recv() {
                    if job.sender.is_canceled() {
                        continue;
                    }
                    let result = catch_unwind(AssertUnwindSafe(|| match job.redirects {
                        Some(limits) => redirects::exchange_following(
                            &client,
                            timeout,
                            job.uri,
                            job.limits,
                            limits,
                            || job.sender.is_canceled(),
                        ),
                        None => exchange(
                            &client,
                            timeout,
                            &job.uri,
                            job.validator.as_deref(),
                            job.limits,
                        ),
                    }))
                    .unwrap_or(Err(HttpError::Panicked));
                    drop(job.lease);
                    let _ = job.sender.send(result);
                }
            })
            .map_err(HttpError::Spawn)?;
        drop(thread);
        ready_rx.await.map_err(|_| HttpError::Stopped)??;
        Ok(Self { sender, budget })
    }
    pub fn in_flight(&self) -> usize {
        self.budget.active()
    }
    /// Submit an already-authorized GET. Queue entries and unread results consume
    /// the same admission budget. No automatic polling/refresh is introduced.
    pub fn get(
        &self,
        uri: String,
        limits: HttpLimits,
    ) -> Result<
        impl Future<Output = Result<HttpResponse, HttpError>> + Send + 'static + use<>,
        HttpError,
    > {
        self.submit_get(uri, None, limits, None)
    }
    /// One explicit If-None-Match validator. Receiving 304 does not itself select
    /// a stored response or authorize publishing its body.
    pub fn get_if_none_match(
        &self,
        uri: String,
        validator: Vec<u8>,
        limits: HttpLimits,
    ) -> Result<
        impl Future<Output = Result<HttpResponse, HttpError>> + Send + 'static + use<>,
        HttpError,
    > {
        if validator.capacity() > limits.header_bytes.get() {
            return Err(HttpError::Headers);
        }
        parse_entity_tag_fields(
            &[&validator],
            EntityTagLimits {
                bytes: limits.header_bytes,
                work: limits.header_work,
            },
        )
        .map_err(HttpError::EntityTag)?;
        self.submit_get(uri, Some(validator), limits, None)
    }
    /// One already-authorized GET acquisition with bounded safe-GET redirects.
    /// No cookies, caller credentials or conditional validators are forwarded.
    pub fn get_following_redirects(
        &self,
        uri: String,
        limits: HttpLimits,
        redirects: RedirectLimits,
    ) -> Result<
        impl Future<Output = Result<HttpResponse, HttpError>> + Send + 'static + use<>,
        HttpError,
    > {
        if redirects.retained_bytes.get() > isize::MAX as usize
            || uri.capacity() > redirects.retained_bytes.get()
        {
            return Err(HttpError::Redirect(RedirectError::Bytes));
        }
        self.submit_get(uri, None, limits, Some(redirects))
    }
    fn submit_get(
        &self,
        mut uri: String,
        validator: Option<Vec<u8>>,
        limits: HttpLimits,
        redirects: Option<RedirectLimits>,
    ) -> Result<
        impl Future<Output = Result<HttpResponse, HttpError>> + Send + 'static + use<>,
        HttpError,
    > {
        if uri.capacity() > limits.uri.bytes.get() {
            return Err(HttpError::Uri(UriError::Bytes));
        }
        let parsed = validate_http_reference(&uri, limits.uri)?;
        let end = parsed.without_fragment().len();
        if redirects.is_none() {
            uri.truncate(end);
        }
        let lease = self.budget.acquire().ok_or(HttpError::Limit)?;
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(Job {
                uri,
                validator,
                redirects,
                limits,
                lease: lease.clone(),
                sender,
            })
            .map_err(|_| HttpError::Stopped)?;
        Ok(async move {
            let result = receiver.await.unwrap_or(Err(HttpError::Stopped));
            drop(lease);
            result
        })
    }
}
fn validate_http_reference(uri: &str, limits: UriLimits) -> Result<UriReference<'_>, HttpError> {
    let parsed = UriReference::parse(uri, limits).map_err(HttpError::Uri)?;
    if !parsed
        .scheme()
        .is_some_and(|s| s.eq_ignore_ascii_case("http") || s.eq_ignore_ascii_case("https"))
        || parsed.authority().is_none_or(|authority| {
            authority.is_empty() || authority.starts_with(':') || authority.contains('@')
        })
    {
        return Err(HttpError::UnsupportedUri);
    }
    Ok(parsed)
}
fn transport_url(uri: &str, limits: UriLimits) -> Result<reqwest::Url, HttpError> {
    let parsed = validate_http_reference(uri, limits)?;
    let url =
        reqwest::Url::parse(parsed.without_fragment()).map_err(|_| HttpError::UnsupportedUri)?;
    if url.host_str().is_none_or(str::is_empty) {
        return Err(HttpError::UnsupportedUri);
    }
    if url.as_str().len() > limits.bytes.get() {
        return Err(HttpError::Uri(UriError::Bytes));
    }
    Ok(url)
}
fn build_client(settings: HttpSettings) -> Result<reqwest::blocking::Client, HttpError> {
    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    for root in settings.additional_roots {
        roots
            .add(rustls::pki_types::CertificateDer::from(root))
            .map_err(HttpError::Tls)?;
    }
    let tls = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .map_err(HttpError::Tls)?
    .with_root_certificates(roots)
    .with_no_client_auth();
    reqwest::blocking::Client::builder()
        .no_proxy()
        .use_preconfigured_tls(tls)
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .no_gzip()
        .no_brotli()
        .no_zstd()
        .no_deflate()
        .timeout(settings.timeout)
        .connect_timeout(settings.connect_timeout)
        .build()
        .map_err(HttpError::Request)
}
fn exchange(
    client: &reqwest::blocking::Client,
    timeout: Duration,
    uri: &str,
    validator: Option<&[u8]>,
    limits: HttpLimits,
) -> Result<HttpResponse, HttpError> {
    let url = transport_url(uri, limits.uri)?;
    // The blocking client timeout restarts on every body read; the per-request
    // timeout is total, from connecting until the body has been read.
    let mut request = client.get(url).timeout(timeout);
    if let Some(validator) = validator {
        let header = reqwest::header::HeaderValue::from_bytes(validator)
            .map_err(HttpError::InvalidHeader)?;
        request = request.header(reqwest::header::IF_NONE_MATCH, header);
    }
    let request_time = HttpClockSample::capture();
    // Request URLs can carry query secrets; keep them out of error diagnostics.
    let mut response = request
        .send()
        .map_err(|error| HttpError::Request(error.without_url()))?;
    let timing = HttpExchangeTiming {
        request: request_time,
        response_headers: HttpClockSample::capture(),
    };
    if response.headers().len() > limits.header_fields.get() {
        return Err(HttpError::Headers);
    }
    let bytes = response
        .headers()
        .iter()
        .try_fold(0usize, |n, (k, v)| {
            n.checked_add(k.as_str().len())
                .and_then(|n| n.checked_add(v.as_bytes().len()))
        })
        .ok_or(HttpError::Headers)?;
    if bytes > limits.header_bytes.get() {
        return Err(HttpError::Headers);
    }
    let effective = response.url().as_str();
    if effective.len() > limits.uri.bytes.get() {
        return Err(HttpError::Uri(UriError::Bytes));
    }
    let mut effective_uri = String::new();
    effective_uri
        .try_reserve_exact(effective.len())
        .map_err(|_| HttpError::Allocation)?;
    if effective_uri.capacity() > limits.uri.bytes.get() {
        return Err(HttpError::Uri(UriError::Bytes));
    }
    effective_uri.push_str(effective);
    let status = response.status().as_u16();
    let version = response.version();
    let headers = std::mem::take(response.headers_mut());
    let expected = response.content_length();
    let body = read_body_blocking(&mut response, expected, limits.read).map_err(HttpError::Read)?;
    Ok(HttpResponse {
        status,
        version,
        effective_uri,
        headers,
        body,
        initial_reference: None,
        redirect_chain: Vec::new(),
        timing,
    })
}
