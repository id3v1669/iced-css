//! RFC8089 acquisition URI syntax, updated by RFC9844's removal of RFC6874.
//! This borrowed descriptor does not decode names or establish filesystem identity.
use super::uri::{UriError, UriLimits, UriReference};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileAuthority<'a> {
    /// Absent/empty authority or the literal localhost (ASCII case-insensitive).
    Local,
    /// A syntactically valid host whose locality has not been established.
    Unresolved(&'a str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileUriError {
    Uri(UriError),
    Scheme,
    Authority,
    Path,
    Query,
    Fragment,
}
impl From<UriError> for FileUriError {
    fn from(error: UriError) -> Self {
        Self::Uri(error)
    }
}
impl std::fmt::Display for FileUriError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "file acquisition URI: {self:?}")
    }
}
impl std::error::Error for FileUriError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Uri(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileUri<'a> {
    input: &'a str,
    path: &'a str,
    authority: FileAuthority<'a>,
}
impl<'a> FileUri<'a> {
    /// Parse an absolute acquisition URI. Strip resource-reference fragments at
    /// the referring-resource boundary first. No allocation, I/O or DNS occurs.
    pub fn parse(input: &'a str, limits: UriLimits) -> Result<Self, FileUriError> {
        let mut remaining = limits.work.get();
        let uri = UriReference::parse_with_work(input, limits.bytes, &mut remaining)?;
        // Prepay the additional bounded scheme/authority/path scans, sharing the
        // generic parser's work allowance rather than resetting it.
        let cost = input
            .len()
            .checked_mul(4)
            .and_then(|n| n.checked_add(1))
            .ok_or(UriError::Work)?;
        remaining.checked_sub(cost).ok_or(UriError::Work)?;
        if !uri
            .scheme()
            .is_some_and(|scheme| scheme.eq_ignore_ascii_case("file"))
        {
            return Err(FileUriError::Scheme);
        }
        if uri.query().is_some() {
            return Err(FileUriError::Query);
        }
        if uri.fragment().is_some() {
            return Err(FileUriError::Fragment);
        }
        let host = uri.authority().unwrap_or("");
        // Generic validation already checks host grammar. Here remove the extra
        // authority productions: userinfo and port (including an empty port).
        if host.contains('@')
            || if host.starts_with('[') {
                !host.ends_with(']')
            } else {
                host.contains(':')
            }
        {
            return Err(FileUriError::Authority);
        }
        let path = uri.path();
        // RFC3986 path-absolute = "/" [ segment-nz *( "/" segment ) ].
        if !path.starts_with('/') || path.starts_with("//") {
            return Err(FileUriError::Path);
        }
        let authority = if host.is_empty() || host.eq_ignore_ascii_case("localhost") {
            FileAuthority::Local
        } else {
            FileAuthority::Unresolved(host)
        };
        Ok(Self {
            input,
            path,
            authority,
        })
    }
    pub fn as_str(self) -> &'a str {
        self.input
    }
    pub fn path(self) -> &'a str {
        self.path
    }
    pub fn authority(self) -> FileAuthority<'a> {
        self.authority
    }
}
