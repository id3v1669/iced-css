//! CSS paint and native-canvas contracts. Renderer lowering is implemented in
//! separate stages; configuration alone does not establish pixel qualification.
//!
//! Not yet integrated: [`crate::box_tree::BoxTree`] clips and layers with its
//! own code and does not use [`layers`] or [`backdrop`], so their tests do not
//! cover the widget's `overflow` and `clip` rendering.
pub mod backdrop;

pub mod layers;
