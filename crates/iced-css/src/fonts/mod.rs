//! Repository-owned font service boundaries.
//!
//! Not yet integrated: [`crate::box_tree::BoxTree`] does not use these modules.
//! Rendering parses and matches fonts through cosmic-text, takes baseline
//! ascent and descent from cosmic-text metrics, and resolves `ex` from a
//! fallback x-height, not from [`metrics::DesignMetrics`].
pub mod metrics;
pub mod sfnt_resource;
