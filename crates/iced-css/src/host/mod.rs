//! Host lifecycle building blocks; full source publication remains separate.
pub mod reconcile;

pub mod compatibility;
pub mod leaf_slots;
pub mod source_slots;
pub mod stylesheet;

pub mod pane_roles;

pub mod keyed_column_state;

pub mod initialize;

mod table_slots;
pub use table_slots::{TableCellSlot, TableRow, TableSlotError, TableSlots};

pub mod source_publication;
