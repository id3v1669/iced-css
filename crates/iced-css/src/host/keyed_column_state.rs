//! Native parent key-cache preparation, separate from source/native child transport.
use iced::{
    advanced::{
        Renderer, Widget,
        widget::tree::{State, Tag},
    },
    widget::keyed::Column,
};
use std::marker::PhantomData;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyedColumnStateError {
    Tag,
}
impl std::fmt::Display for KeyedColumnStateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "keyed column parent state: {self:?}")
    }
}
impl std::error::Error for KeyedColumnStateError {}

/// Exclusive old-state ownership and an immutable incoming-view borrow.
/// Dropping this preparation leaves the active State untouched.
///
/// ```compile_fail,E0505
/// use iced::{advanced::widget::Tree,widget::keyed::Column};
/// use iced_css::host::keyed_column_state::prepare_keyed_column_state;
/// fn change_incoming(column:Column<'_,u8,(),iced::Theme,()>,tree:&mut Tree) {
///     let prepared=prepare_keyed_column_state(&column,&tree.tag,&mut tree.state).unwrap();
///     drop(column);
///     prepared.commit();
/// }
/// ```
#[must_use = "commit the prepared key state with the matching child order, or abandon it"]
pub struct PreparedKeyedColumnState<'state, 'incoming> {
    previous: &'state mut State,
    prepared: State,
    _tag: &'state Tag,
    _incoming: PhantomData<&'incoming ()>,
}
impl PreparedKeyedColumnState<'_, '_> {
    /// Swap parent state and return the old state for controlled retirement.
    /// No native diff, child-state initialization or retired-state Drop occurs.
    pub fn commit(self) -> State {
        std::mem::replace(self.previous, self.prepared)
    }
}

/// Prepare stock keyed Column's incoming parent key state after exact Tag admission.
///
/// The caller establishes source-parent identity/compatibility and validates incoming
/// key/child alignment. Install the matching child-state vector before native diff.
/// This does not collect source metadata, move children, or initialize shadow Trees.
/// Preparation calls native Widget::state (key-vector cloning/allocation); its native
/// effects/failures are outside source budgets. Keep the incoming view unchanged
/// through commit and the subsequent native diff.
pub fn prepare_keyed_column_state<'state, 'incoming, Key, Message, Theme, R>(
    incoming: &'incoming Column<'_, Key, Message, Theme, R>,
    previous_tag: &'state Tag,
    previous: &'state mut State,
) -> Result<PreparedKeyedColumnState<'state, 'incoming>, KeyedColumnStateError>
where
    Key: Copy + PartialEq + 'static,
    R: Renderer,
{
    if *previous_tag != incoming.tag() {
        return Err(KeyedColumnStateError::Tag);
    }
    let prepared = incoming.state();
    Ok(PreparedKeyedColumnState {
        previous,
        prepared,
        _tag: previous_tag,
        _incoming: PhantomData,
    })
}
