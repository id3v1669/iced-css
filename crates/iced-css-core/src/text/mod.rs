//! Source-preserving text preprocessing. Shaping and line construction are separate.
pub mod casing_contexts;
pub mod casing_data;
pub mod casing_output;
pub mod casing_selection;
pub mod lf_conversion;
pub mod lf_whitespace;
pub mod line_edges;
pub mod newlines;
pub mod preserved_spaces;
pub mod space_collapse;
pub mod unicode;
pub mod whitespace_preprocessing;
pub mod word_boundaries;
pub mod word_data;
