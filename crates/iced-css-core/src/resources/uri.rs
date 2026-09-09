//! RFC3986 generic ASCII syntax and strict section 5 resolution. CSS token
//! unescaping, non-ASCII conversion and scheme-specific retrieval are separate.
use std::num::NonZeroUsize;

#[derive(Debug, Clone, Copy)]
pub struct UriLimits {
    /// Each input length and retained output-buffer capacity, not process memory.
    pub bytes: NonZeroUsize,
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UriError {
    Syntax,
    NonAscii,
    RelativeBase,
    Bytes,
    Work,
    Allocation,
}
impl std::fmt::Display for UriError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "generic URI: {self:?}")
    }
}
impl std::error::Error for UriError {}
struct Work(usize);
impl Work {
    fn linear(&mut self, bytes: usize) -> Result<(), UriError> {
        // Prepay a conservative bound for fixed scans, character validation,
        // component extraction and linear in-place moves; no input-sized recursion.
        let cost = bytes
            .checked_mul(20)
            .and_then(|n| n.checked_add(1))
            .ok_or(UriError::Work)?;
        self.0 = self.0.checked_sub(cost).ok_or(UriError::Work)?;
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UriReference<'a> {
    input: &'a str,
    scheme: Option<&'a str>,
    authority: Option<&'a str>,
    path: &'a str,
    query: Option<&'a str>,
    fragment: Option<&'a str>,
}
impl<'a> UriReference<'a> {
    pub fn parse(input: &'a str, limits: UriLimits) -> Result<Self, UriError> {
        Self::parse_with(input, limits.bytes, &mut Work(limits.work.get()))
    }
    pub(crate) fn parse_with_work(
        input: &'a str,
        bytes: NonZeroUsize,
        remaining: &mut usize,
    ) -> Result<Self, UriError> {
        let mut work = Work(*remaining);
        let result = Self::parse_with(input, bytes, &mut work);
        *remaining = work.0;
        result
    }
    pub fn scheme(self) -> Option<&'a str> {
        self.scheme
    }
    pub fn authority(self) -> Option<&'a str> {
        self.authority
    }
    pub fn path(self) -> &'a str {
        self.path
    }
    pub fn query(self) -> Option<&'a str> {
        self.query
    }
    pub fn fragment(self) -> Option<&'a str> {
        self.fragment
    }
    /// Separate fragment selection from retrieval; this does not make a relative
    /// reference absolute or apply transport/cache normalization.
    pub fn without_fragment(self) -> &'a str {
        self.input
            .split_once('#')
            .map_or(self.input, |(prefix, _)| prefix)
    }
    fn parse_with(input: &'a str, bytes: NonZeroUsize, work: &mut Work) -> Result<Self, UriError> {
        if bytes.get() > isize::MAX as usize || input.len() > bytes.get() {
            return Err(UriError::Bytes);
        }
        work.linear(input.len())?;
        if !input.is_ascii() {
            return Err(UriError::NonAscii);
        }
        let (before_fragment, fragment) = split(input, '#');
        let (hierarchy, query) = split(before_fragment, '?');
        if query.is_some_and(|v| !component(v, |c| pchar(c) || matches!(c, b'/' | b'?')))
            || fragment.is_some_and(|v| !component(v, |c| pchar(c) || matches!(c, b'/' | b'?')))
        {
            return Err(UriError::Syntax);
        }
        let (scheme, remaining) = match hierarchy.find(':') {
            Some(colon) if !hierarchy[..colon].contains('/') => {
                let candidate = &hierarchy[..colon];
                if !valid_scheme(candidate) {
                    return Err(UriError::Syntax);
                }
                (Some(candidate), &hierarchy[colon + 1..])
            }
            _ => (None, hierarchy),
        };
        let (authority, path) = if let Some(after) = remaining.strip_prefix("//") {
            let slash = after.find('/').unwrap_or(after.len());
            let authority = &after[..slash];
            if !valid_authority(authority) {
                return Err(UriError::Syntax);
            }
            (Some(authority), &after[slash..])
        } else {
            (None, remaining)
        };
        if !component(path, |c| pchar(c) || c == b'/') {
            return Err(UriError::Syntax);
        }
        Ok(Self {
            input,
            scheme,
            authority,
            path,
            query,
            fragment,
        })
    }
}
fn split(input: &str, delimiter: char) -> (&str, Option<&str>) {
    input
        .split_once(delimiter)
        .map_or((input, None), |(left, right)| (left, Some(right)))
}
fn valid_scheme(input: &str) -> bool {
    input
        .as_bytes()
        .first()
        .is_some_and(u8::is_ascii_alphabetic)
        && input
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'+' | b'-' | b'.'))
}
fn unreserved(c: u8) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, b'-' | b'.' | b'_' | b'~')
}
fn sub_delimiter(c: u8) -> bool {
    matches!(
        c,
        b'!' | b'$' | b'&' | b'\'' | b'(' | b')' | b'*' | b'+' | b',' | b';' | b'='
    )
}
fn pchar(c: u8) -> bool {
    unreserved(c) || sub_delimiter(c) || matches!(c, b':' | b'@')
}
fn component(input: &str, allowed: impl Fn(u8) -> bool) -> bool {
    let bytes = input.as_bytes();
    let mut offset = 0;
    while offset < bytes.len() {
        if bytes[offset] == b'%' {
            if !bytes.get(offset + 1).is_some_and(u8::is_ascii_hexdigit)
                || !bytes.get(offset + 2).is_some_and(u8::is_ascii_hexdigit)
            {
                return false;
            }
            offset += 3;
        } else {
            if !allowed(bytes[offset]) {
                return false;
            }
            offset += 1;
        }
    }
    true
}
fn valid_authority(authority: &str) -> bool {
    let host_port = if let Some((userinfo, rest)) = authority.split_once('@') {
        if !component(userinfo, |c| unreserved(c) || sub_delimiter(c) || c == b':') {
            return false;
        }
        rest
    } else {
        authority
    };
    if let Some(literal) = host_port.strip_prefix('[') {
        let Some((address, after)) = literal.split_once(']') else {
            return false;
        };
        if !after.is_empty()
            && !after
                .strip_prefix(':')
                .is_some_and(|v| v.bytes().all(|c| c.is_ascii_digit()))
        {
            return false;
        }
        if address.starts_with(['v', 'V']) {
            valid_future(address)
        } else {
            valid_ipv6(address)
        }
    } else {
        let (host, port) = split(host_port, ':');
        component(host, |c| unreserved(c) || sub_delimiter(c))
            && port.is_none_or(|v| v.bytes().all(|c| c.is_ascii_digit()))
    }
}
fn valid_future(address: &str) -> bool {
    let Some((version, value)) = address[1..].split_once('.') else {
        return false;
    };
    !version.is_empty()
        && version.bytes().all(|c| c.is_ascii_hexdigit())
        && !value.is_empty()
        && value
            .bytes()
            .all(|c| unreserved(c) || sub_delimiter(c) || c == b':')
}
fn valid_ipv4(address: &str) -> bool {
    let mut count = 0;
    for octet in address.split('.') {
        if octet.is_empty()
            || octet.len() > 3
            || (octet.len() > 1 && octet.starts_with('0'))
            || !octet.bytes().all(|c| c.is_ascii_digit())
        {
            return false;
        }
        let value = octet
            .bytes()
            .fold(0u16, |n, c| n * 10 + u16::from(c - b'0'));
        if value > 255 {
            return false;
        }
        count += 1;
    }
    count == 4
}
fn groups(part: &str, allow_ipv4: bool) -> Option<usize> {
    if part.is_empty() {
        return Some(0);
    }
    let mut count = 0;
    let mut segments = part.split(':').peekable();
    while let Some(segment) = segments.next() {
        if segment.contains('.') {
            if !allow_ipv4 || segments.peek().is_some() || !valid_ipv4(segment) {
                return None;
            }
            count += 2;
        } else {
            if segment.is_empty()
                || segment.len() > 4
                || !segment.bytes().all(|c| c.is_ascii_hexdigit())
            {
                return None;
            }
            count += 1;
        }
        if count > 8 {
            return None;
        }
    }
    Some(count)
}
fn valid_ipv6(address: &str) -> bool {
    if let Some((left, right)) = address.split_once("::") {
        if right.contains("::") {
            return false;
        }
        matches!((groups(left,false),groups(right,true)),(Some(left),Some(right)) if left+right<8)
    } else {
        groups(address, true) == Some(8)
    }
}

/// Strict section 5 resolution against the referring stylesheet's explicit base.
/// The caller must supply the effective retrieval base after redirects. No scheme
/// conversion, percent decoding, additional cache normalization or acquisition is performed.
pub fn resolve(base: &str, reference: &str, limits: UriLimits) -> Result<String, UriError> {
    resolve_with_work(base, reference, limits.bytes, &mut limits.work.get())
}
pub(crate) fn resolve_with_work(
    base: &str,
    reference: &str,
    bytes: NonZeroUsize,
    remaining: &mut usize,
) -> Result<String, UriError> {
    let mut work = Work(*remaining);
    let result = resolve_inner(base, reference, bytes, &mut work);
    *remaining = work.0;
    result
}
fn resolve_inner(
    base: &str,
    reference: &str,
    bytes: NonZeroUsize,
    work: &mut Work,
) -> Result<String, UriError> {
    let base = UriReference::parse_with(base, bytes, work)?;
    let reference = UriReference::parse_with(reference, bytes, work)?;
    let base_scheme = base.scheme.ok_or(UriError::RelativeBase)?;
    work.linear(
        base.input
            .len()
            .checked_add(reference.input.len())
            .and_then(|n| n.checked_add(1))
            .ok_or(UriError::Work)?,
    )?;
    let (scheme, authority, prefix, path, query, remove) = if let Some(scheme) = reference.scheme {
        (
            scheme,
            reference.authority,
            "",
            reference.path,
            reference.query,
            true,
        )
    } else if reference.authority.is_some() {
        (
            base_scheme,
            reference.authority,
            "",
            reference.path,
            reference.query,
            true,
        )
    } else if reference.path.is_empty() {
        (
            base_scheme,
            base.authority,
            "",
            base.path,
            reference.query.or(base.query),
            false,
        )
    } else if reference.path.starts_with('/') {
        (
            base_scheme,
            base.authority,
            "",
            reference.path,
            reference.query,
            true,
        )
    } else {
        let prefix = if base.authority.is_some() && base.path.is_empty() {
            "/"
        } else {
            base.path
                .rfind('/')
                .map_or("", |slash| &base.path[..slash + 1])
        };
        (
            base_scheme,
            base.authority,
            prefix,
            reference.path,
            reference.query,
            true,
        )
    };
    let length = prefix
        .len()
        .checked_add(path.len())
        .ok_or(UriError::Bytes)?;
    let mut output = String::new();
    reserve(&mut output, length, bytes.get())?;
    output.push_str(prefix);
    output.push_str(path);
    if remove {
        output = remove_dots(output)?;
    }
    // RFC3986 section3.3 forbids a leading // path without authority.
    // Serializing it would introduce an authority absent from the result tuple.
    if authority.is_none() && output.starts_with("//") {
        return Err(UriError::Syntax);
    }
    let mut needed = output
        .len()
        .checked_add(scheme.len())
        .and_then(|n| n.checked_add(1))
        .ok_or(UriError::Bytes)?;
    for (component, delimiter) in [(authority, 2), (query, 1), (reference.fragment, 1)] {
        if let Some(component) = component {
            needed = needed
                .checked_add(component.len())
                .and_then(|n| n.checked_add(delimiter))
                .ok_or(UriError::Bytes)?;
        }
    }
    reserve(&mut output, needed, bytes.get())?;
    if let Some(authority) = authority {
        output.insert_str(0, authority);
        output.insert_str(0, "//");
    }
    output.insert(0, ':');
    output.insert_str(0, scheme);
    if let Some(query) = query {
        output.push('?');
        output.push_str(query);
    }
    if let Some(fragment) = reference.fragment {
        output.push('#');
        output.push_str(fragment);
    }
    Ok(output)
}
fn reserve(output: &mut String, needed: usize, limit: usize) -> Result<(), UriError> {
    if needed > limit {
        return Err(UriError::Bytes);
    }
    if needed > output.capacity() {
        output
            .try_reserve_exact(needed - output.len())
            .map_err(|_| UriError::Allocation)?;
    }
    if output.capacity() > limit {
        return Err(UriError::Bytes);
    }
    Ok(())
}
fn remove_dots(path: String) -> Result<String, UriError> {
    let mut bytes = path.into_bytes();
    let mut read = 0;
    let mut written = 0;
    while read < bytes.len() {
        let remaining = &bytes[read..];
        if remaining.starts_with(b"../") {
            read += 3;
        } else if remaining.starts_with(b"./") || remaining.starts_with(b"/./") {
            read += 2;
        } else if remaining == b"/." {
            bytes[written] = b'/';
            written += 1;
            break;
        } else if remaining.starts_with(b"/../") {
            read += 3;
            written = bytes[..written]
                .iter()
                .rposition(|c| *c == b'/')
                .unwrap_or(0);
        } else if remaining == b"/.." {
            written = bytes[..written]
                .iter()
                .rposition(|c| *c == b'/')
                .unwrap_or(0);
            bytes[written] = b'/';
            written += 1;
            break;
        } else if matches!(remaining, b"." | b"..") {
            break;
        } else {
            let count = remaining[1..]
                .iter()
                .position(|c| *c == b'/')
                .map_or(remaining.len(), |n| n + 1);
            bytes.copy_within(read..read + count, written);
            read += count;
            written += count;
        }
    }
    bytes.truncate(written);
    String::from_utf8(bytes).map_err(|_| UriError::Syntax)
}

/// RFC3986 section6.2.2 generic syntax normalization of an absolute ASCII URI.
/// Preserve authored/effective provenance separately when using this as a key.
/// Scheme-specific defaults, filesystem identity and transport/cache policy are
/// separate. UriLimits bounds each buffer; an intermediate URI and the resolver's
/// buffers can coexist. All work shares one allowance.
pub fn normalize_syntax(input: &str, limits: UriLimits) -> Result<String, UriError> {
    let mut work = Work(limits.work.get());
    let uri = UriReference::parse_with(input, limits.bytes, &mut work)?;
    let scheme = uri.scheme.ok_or(UriError::RelativeBase)?;
    work.linear(input.len())?;
    let mut output = String::new();
    reserve(&mut output, input.len(), limits.bytes.get())?;
    normalized_component(&mut output, scheme, true);
    output.push(':');
    if let Some(authority) = uri.authority {
        output.push_str("//");
        let host_port = if let Some((userinfo, host_port)) = authority.split_once('@') {
            normalized_component(&mut output, userinfo, false);
            output.push('@');
            host_port
        } else {
            authority
        };
        let host_end = if host_port.starts_with('[') {
            host_port
                .find(']')
                .map(|end| end + 1)
                .ok_or(UriError::Syntax)?
        } else {
            host_port.find(':').unwrap_or(host_port.len())
        };
        normalized_component(&mut output, &host_port[..host_end], true);
        normalized_component(&mut output, &host_port[host_end..], false);
    }
    normalized_component(&mut output, uri.path, false);
    if let Some(query) = uri.query {
        output.push('?');
        normalized_component(&mut output, query, false);
    }
    if let Some(fragment) = uri.fragment {
        output.push('#');
        normalized_component(&mut output, fragment, false);
    }
    // The absolute-reference branch reuses the validated section5.2 dot removal
    // and serialization contract. This is not retrieval against a guessed base.
    resolve_inner(&output, &output, limits.bytes, &mut work)
}
fn normalized_component(output: &mut String, input: &str, lowercase: bool) {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let bytes = input.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte == b'%' {
            // Only complete escapes from a validated UriReference reach here.
            let decoded = (hex_value(bytes[index + 1]) << 4) | hex_value(bytes[index + 2]);
            if unreserved(decoded) {
                output.push(char::from(if lowercase {
                    decoded.to_ascii_lowercase()
                } else {
                    decoded
                }));
            } else {
                output.push('%');
                output.push(char::from(HEX[usize::from(decoded >> 4)]));
                output.push(char::from(HEX[usize::from(decoded & 15)]));
            }
            index += 3;
        } else {
            output.push(char::from(if lowercase {
                byte.to_ascii_lowercase()
            } else {
                byte
            }));
            index += 1;
        }
    }
}
fn hex_value(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'A'..=b'F' => byte - b'A' + 10,
        _ => byte - b'a' + 10,
    }
}
