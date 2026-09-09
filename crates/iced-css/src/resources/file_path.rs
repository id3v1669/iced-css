//! Unix file URI name decoding. Windows mapping and canonical identities are
//! separate adapters; this module performs no filesystem operation or DNS lookup.
use iced_css_core::resources::file_uri::{FileAuthority, FileUri};
use std::{
    ffi::OsString,
    num::NonZeroUsize,
    os::unix::ffi::{OsStrExt, OsStringExt},
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Copy)]
pub struct FilePathLimits {
    /// Output native path or URI buffer capacity, excluding the borrowed input.
    pub bytes: NonZeroUsize,
    /// A conservative bound for validation, percent decoding and byte copying.
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilePathError {
    UnresolvedAuthority,
    RelativePath,
    DoubleRoot,
    Nul,
    EncodedSeparator,
    Bytes,
    Work,
    Allocation,
}
impl std::fmt::Display for FilePathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Unix file URI path: {self:?}")
    }
}
impl std::error::Error for FilePathError {}

/// Decode each filename exactly once. Encoded slash and NUL cannot be Unix
/// filename bytes. All other bytes retain their spelling's decoded value; native
/// resolution owns dot components and symlinks. This is not a sandbox boundary.
pub fn unix_path(uri: FileUri<'_>, limits: FilePathLimits) -> Result<PathBuf, FilePathError> {
    if uri.authority() != FileAuthority::Local {
        return Err(FilePathError::UnresolvedAuthority);
    }
    if limits.bytes.get() > isize::MAX as usize {
        return Err(FilePathError::Bytes);
    }
    let input = uri.path().as_bytes();
    let cost = input
        .len()
        .checked_mul(3)
        .and_then(|n| n.checked_add(1))
        .ok_or(FilePathError::Work)?;
    if cost > limits.work.get() {
        return Err(FilePathError::Work);
    }
    // Validate/count before allocation. The FileUri type guarantees ASCII and
    // complete hexadecimal escapes, so decoding cannot index past its input.
    let mut size = 0usize;
    let mut offset = 0;
    while offset < input.len() {
        let (byte, width) = octet(input, offset);
        if byte == 0 {
            return Err(FilePathError::Nul);
        }
        if byte == b'/' && width == 3 {
            return Err(FilePathError::EncodedSeparator);
        }
        size += 1; // At most input.len(), already representable in an input slice.
        offset += width;
    }
    if size > limits.bytes.get() {
        return Err(FilePathError::Bytes);
    }
    let mut output = Vec::new();
    output
        .try_reserve_exact(size)
        .map_err(|_| FilePathError::Allocation)?;
    if output.capacity() > limits.bytes.get() {
        return Err(FilePathError::Bytes);
    }
    offset = 0;
    while offset < input.len() {
        let (byte, width) = octet(input, offset);
        output.push(byte);
        offset += width;
    }
    Ok(PathBuf::from(OsString::from_vec(output)))
}

fn octet(input: &[u8], offset: usize) -> (u8, usize) {
    if input[offset] == b'%' {
        (hex(input[offset + 1]) * 16 + hex(input[offset + 2]), 3)
    } else {
        (input[offset], 1)
    }
}
fn hex(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'A'..=b'F' => byte - b'A' + 10,
        // Only validated hexadecimal bytes reach this private helper.
        _ => byte - b'a' + 10,
    }
}

/// Supply an explicit file URI base for a rooted Unix path. No lossy Unicode
/// conversion, tilde expansion, dot normalization or filesystem access occurs.
pub fn unix_file_uri(path: &Path, limits: FilePathLimits) -> Result<String, FilePathError> {
    let input = path.as_os_str().as_bytes();
    if limits.bytes.get() > isize::MAX as usize {
        return Err(FilePathError::Bytes);
    }
    let cost = input
        .len()
        .checked_mul(3)
        .and_then(|n| n.checked_add(8))
        .ok_or(FilePathError::Work)?;
    if cost > limits.work.get() {
        return Err(FilePathError::Work);
    }
    if !input.starts_with(b"/") {
        return Err(FilePathError::RelativePath);
    }
    if input.starts_with(b"//") {
        return Err(FilePathError::DoubleRoot);
    }
    let mut size = 7usize; // file://
    for &byte in input {
        if byte == 0 {
            return Err(FilePathError::Nul);
        }
        size = size
            .checked_add(if literal(byte) { 1 } else { 3 })
            .ok_or(FilePathError::Bytes)?;
    }
    if size > limits.bytes.get() {
        return Err(FilePathError::Bytes);
    }
    let mut output = String::new();
    output
        .try_reserve_exact(size)
        .map_err(|_| FilePathError::Allocation)?;
    if output.capacity() > limits.bytes.get() {
        return Err(FilePathError::Bytes);
    }
    output.push_str("file://");
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    for &byte in input {
        if literal(byte) {
            output.push(char::from(byte));
        } else {
            output.push('%');
            output.push(char::from(HEX[usize::from(byte >> 4)]));
            output.push(char::from(HEX[usize::from(byte & 15)]));
        }
    }
    Ok(output)
}
fn literal(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'/')
}
