//! Immutable source-style evaluation candidates.
//!
//! `sibling_layout` also offers the opt-in, bounded consecutive-float cluster profile.

pub mod display_rules;
pub mod source_display;
pub mod source_eligibility;
pub mod source_participants;
pub mod source_structural_events;

pub mod source_whitespace;

pub mod source_casing;

pub mod source_structure;

pub mod source_analysis_input;

mod source_presentation_join;

pub mod source_control_input;

pub mod clearance_source_projection;
pub mod sibling_source_projection;

pub(crate) mod border_diagnostics;
pub mod clearance_box_facts;
pub mod sibling_box_facts;
pub mod sibling_layout;
pub mod tree_layout;

mod clearance_profile;

pub mod clearance_bridge;
