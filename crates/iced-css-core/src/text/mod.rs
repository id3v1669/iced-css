//! Source-preserving text preprocessing. Shaping and line construction are separate.
pub mod bidi_data;
pub mod casing_contexts;
pub mod casing_data;
pub mod casing_output;
pub mod casing_selection;
pub mod grapheme_boundaries;
pub mod grapheme_data;
pub mod lf_conversion;
pub mod lf_whitespace;
pub mod line_boundaries;
pub mod line_break_aux_data;
pub mod line_break_data;
pub mod line_edges;
pub mod newlines;
pub mod preserved_spaces;
pub mod space_collapse;
pub mod unicode;
pub mod whitespace_preprocessing;
pub mod word_boundaries;
pub mod word_data;

pub mod bidi_initial;

pub mod bidi_isolates;

pub mod bidi_explicit;

pub mod bidi_runs;

pub mod bidi_weak;

pub mod bidi_bracket_pairs;
