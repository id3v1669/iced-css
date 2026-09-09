//! CSS intake on the same bounded worker as its application resolver callback.
use super::{
    EmbeddedError, EmbeddedLimits, EmbeddedReader, EmbeddedResolver, EmbeddedSubmitError,
    admit_input, resolve_asset,
};
use crate::resources::stylesheet::StylesheetEncoding;
use iced_css_core::stylesheet::{
    RuleContext, StylesheetIntakeError, StylesheetIntakeLimits, UnicodeStylesheetCandidate,
    parse_separate_unicode_stylesheet,
};
use std::{future::Future, num::NonZeroUsize};
#[derive(Debug, Clone, Copy)]
pub struct EmbeddedStylesheetLimits {
    pub asset: EmbeddedLimits,
    /// Combined owned external encoding-label capacities, separate from asset metadata.
    pub metadata_bytes: NonZeroUsize,
    pub intake: StylesheetIntakeLimits,
}
pub struct EmbeddedStylesheet {
    stylesheet: UnicodeStylesheetCandidate,
    version: Vec<u8>,
    dependency_base: String,
}
impl EmbeddedStylesheet {
    pub fn stylesheet(&self) -> &UnicodeStylesheetCandidate {
        &self.stylesheet
    }
    pub fn version(&self) -> &[u8] {
        &self.version
    }
    pub fn dependency_base(&self) -> &str {
        &self.dependency_base
    }
    pub fn into_parts(self) -> (UnicodeStylesheetCandidate, Vec<u8>, String) {
        (self.stylesheet, self.version, self.dependency_base)
    }
}
#[derive(Debug)]
pub enum EmbeddedStylesheetError<E> {
    Asset(EmbeddedError<E>),
    Intake(StylesheetIntakeError),
}
impl<E: std::error::Error> std::fmt::Display for EmbeddedStylesheetError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "embedded stylesheet intake: {self:?}")
    }
}
impl<E: std::error::Error + 'static> std::error::Error for EmbeddedStylesheetError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Asset(error) => Some(error),
            Self::Intake(error) => Some(error),
        }
    }
}
impl<R: EmbeddedResolver> EmbeddedReader<R> {
    /// Resolve and parse one already-authorized stylesheet on a single resource
    /// worker. This does not collect imports, cache bytes or publish a graph.
    pub fn read_stylesheet(
        &self,
        base: String,
        reference: String,
        encoding: StylesheetEncoding,
        context: RuleContext,
        limits: EmbeddedStylesheetLimits,
    ) -> Result<
        impl Future<Output = Result<EmbeddedStylesheet, EmbeddedStylesheetError<R::Error>>>
        + Send
        + 'static
        + use<R>,
        EmbeddedSubmitError,
    > {
        admit_input(base.capacity(), reference.capacity(), limits.asset)?;
        if encoding
            .retained_bytes()
            .is_none_or(|bytes| bytes > limits.metadata_bytes.get())
        {
            return Err(EmbeddedSubmitError::Metadata);
        }
        let namespace = self.namespace.clone();
        let resolver = self.resolver.clone();
        let work = self
            .worker
            .submit(move || {
                let asset = resolve_asset(
                    &namespace,
                    resolver.as_ref(),
                    &base,
                    &reference,
                    limits.asset,
                )
                .map_err(EmbeddedStylesheetError::Asset)?;
                parse_asset(asset, encoding.metadata(), context, limits.intake)
                    .map_err(EmbeddedStylesheetError::Intake)
            })
            .map_err(EmbeddedSubmitError::Worker)?;
        Ok(async move {
            work.await
                .map_err(|error| EmbeddedStylesheetError::Asset(EmbeddedError::Worker(error)))?
        })
    }
}

pub(super) fn parse_asset(
    asset: super::EmbeddedAsset,
    encoding: iced_css_core::syntax::encoding_selection::EncodingMetadata<'_>,
    context: RuleContext,
    limits: StylesheetIntakeLimits,
) -> Result<EmbeddedStylesheet, StylesheetIntakeError> {
    let stylesheet =
        parse_separate_unicode_stylesheet(asset.body.as_slice(), encoding, context, limits)?;
    Ok(EmbeddedStylesheet {
        stylesheet,
        version: asset.version,
        dependency_base: asset.dependency_base,
    })
}
