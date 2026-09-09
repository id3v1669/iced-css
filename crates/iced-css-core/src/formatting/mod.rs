//! CSS formatting primitives; source membership is separate from generated boxes.
pub mod absolute;
pub mod absolute_height;
mod arithmetic;
pub mod block;
pub mod display;
pub mod margins;
pub mod quotes;
pub mod relative;

pub mod counters {
    pub use crate::values::counter_scope::*;
}

pub mod generated {
    pub use crate::values::counter_scope::generated::*;
}

pub mod shrink;

pub mod replaced_constraints;

pub mod clip;

pub mod table_borders;

pub mod line_geometry;

pub mod table_placement;

pub mod table_cell_borders;

pub mod table_line_boundaries;

pub mod table_border_intersection;

pub mod table_region_borders;
