//! Already-authorized Unix file stylesheet acquisition for a background collector.
use super::{
    file_dependencies::{FileDependencyError, FileDependencyLimits, FileDependencyReport},
    file_path::{FilePathError, unix_path},
    file_uri_read::FileUriReadLimits,
    local::{ReadBodyError, read_file_blocking},
};
use iced_css_core::{
    resources::file_uri::{FileUri, FileUriError},
    stylesheet::{
        RuleContext, StylesheetIntakeError, StylesheetIntakeLimits,
        collector::{AcquiredStylesheet, CollectorRoot, ImportProbe, ImportProvider, ImportTarget},
        identities::{IdentityError, ImportIdentityRegistry},
        parse_separate_unicode_stylesheet,
    },
    syntax::encoding_selection::EncodingMetadata,
};
#[derive(Debug, Clone, Copy)]
pub struct FileImportLimits {
    /// Independent URI validation, path conversion and body/read allowances.
    /// Effective base storage uses the URI byte allowance, separately from body.
    pub file: FileUriReadLimits,
    pub intake: StylesheetIntakeLimits,
    pub dependencies: FileDependencyLimits,
}
#[derive(Debug)]
pub enum FileImportError {
    Dependencies(FileDependencyError),
    Identity(IdentityError),
    Uri(FileUriError),
    Path(FilePathError),
    Read(ReadBodyError),
    Intake(StylesheetIntakeError),
    BaseBytes,
    Allocation,
    Target,
}
impl std::fmt::Display for FileImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "file import: {self:?}")
    }
}
impl std::error::Error for FileImportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Dependencies(e) => Some(e),
            Self::Identity(e) => Some(e),
            Self::Uri(e) => Some(e),
            Self::Path(e) => Some(e),
            Self::Read(e) => Some(e),
            Self::Intake(e) => Some(e),
            _ => None,
        }
    }
}
/// Validate and record the requested root path without reading content or
/// allocating a stylesheet identity. Errors retain the empty/rejected report.
/// This is attempted-path evidence, not existence or native coverage evidence.
pub fn root_file_dependencies(
    uri: &str,
    limits: FileImportLimits,
) -> (Result<(), FileImportError>, FileDependencyReport) {
    let mut report = FileDependencyReport::new(limits.dependencies);
    let result = record_file_dependency(&mut report, uri, limits.file).map(|_| ());
    (result, report)
}
fn record_file_dependency(
    report: &mut FileDependencyReport,
    uri: &str,
    limits: FileUriReadLimits,
) -> Result<usize, FileImportError> {
    let parsed = FileUri::parse(uri, limits.uri).map_err(FileImportError::Uri)?;
    let path = unix_path(parsed, limits.path).map_err(FileImportError::Path)?;
    report
        .record(uri, path)
        .map_err(FileImportError::Dependencies)
}
/// Borrows one persistent identity owner; creates no request or worker itself.
/// Run on the existing graph worker after request authorization. This provider
/// re-reads needed local inputs and never consults a previous candidate/body.
/// Exact file URI keys preserve the supplied effective-base spelling. Other
/// schemes, filesystem alias equivalence and remote cache policy are separate.
pub struct UnixFileImportProvider<'a> {
    identities: &'a mut ImportIdentityRegistry,
    context: RuleContext,
    limits: FileImportLimits,
    dependencies: FileDependencyReport,
}
impl<'a> UnixFileImportProvider<'a> {
    pub fn new(
        identities: &'a mut ImportIdentityRegistry,
        context: RuleContext,
        limits: FileImportLimits,
    ) -> Self {
        Self {
            identities,
            context,
            limits,
            dependencies: FileDependencyReport::new(limits.dependencies),
        }
    }
    /// Load a required root with explicit external encoding metadata. All root
    /// failures are returned; no prior body is used and no partial root escapes.
    pub fn read_root(
        &mut self,
        uri: &str,
        encoding: EncodingMetadata<'_>,
    ) -> Result<CollectorRoot, FileImportError> {
        let (target, index) = self.identify_uri(uri, encoding)?;
        let input = self.read_input(index, encoding)?;
        Ok(CollectorRoot { target, input })
    }
    /// Retain attempted paths even when reading or collecting returns an error.
    /// A report rejection is explicit; this is not complete watch coverage.
    pub fn into_dependencies(self) -> FileDependencyReport {
        self.dependencies
    }
    fn identify_uri(
        &mut self,
        uri: &str,
        encoding: EncodingMetadata<'_>,
    ) -> Result<(ImportTarget, usize), FileImportError> {
        let index = record_file_dependency(&mut self.dependencies, uri, self.limits.file)?;
        let target = self
            .identities
            .identify(uri, encoding, self.context)
            .map_err(FileImportError::Identity)?;
        Ok((target, index))
    }
    fn read_input(
        &self,
        index: usize,
        encoding: EncodingMetadata<'_>,
    ) -> Result<AcquiredStylesheet, FileImportError> {
        let input = self
            .dependencies
            .entries()
            .get(index)
            .ok_or(FileImportError::Target)?;
        let uri = input.acquisition_uri();
        let mut effective_base = String::new();
        effective_base
            .try_reserve_exact(uri.len())
            .map_err(|_| FileImportError::Allocation)?;
        if effective_base.capacity() > self.limits.file.uri.bytes.get() {
            return Err(FileImportError::BaseBytes);
        }
        effective_base.push_str(uri);
        let body = read_file_blocking(input.native_path(), self.limits.file.read)
            .map_err(FileImportError::Read)?;
        let candidate = parse_separate_unicode_stylesheet(
            body.as_slice(),
            encoding,
            self.context,
            self.limits.intake,
        )
        .map_err(FileImportError::Intake)?;
        Ok(AcquiredStylesheet {
            candidate,
            effective_base,
        })
    }
}
impl ImportProvider for UnixFileImportProvider<'_> {
    type Error = FileImportError;
    fn identify(&mut self, probe: ImportProbe<'_>) -> Result<ImportTarget, Self::Error> {
        self.identify_uri(probe.request.acquisition_uri(), probe.encoding)
            .map(|(target, _)| target)
    }
    fn acquire(
        &mut self,
        target: ImportTarget,
        probe: ImportProbe<'_>,
    ) -> Result<Option<AcquiredStylesheet>, Self::Error> {
        let uri = probe.request.acquisition_uri();
        let (expected, index) = self.identify_uri(uri, probe.encoding)?;
        if expected != target {
            return Err(FileImportError::Target);
        }
        match self.read_input(index, probe.encoding) {
            Ok(input) => Ok(Some(input)),
            // The expansion policy handles initial omission versus required
            // reload success. No richer OS error survives this generic cut.
            Err(FileImportError::Read(ReadBodyError::Io(_))) => Ok(None),
            Err(error) => Err(error),
        }
    }
}
