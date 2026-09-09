//! Exact optional-role removal for initialized stock iced PaneGrid content slots.
use iced::advanced::widget::{
    Tree,
    tree::{State, Tag},
};

/// Supplied source roles, independent of visibility or empty widget payloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneRoles {
    NoTitle,
    Title,
    FullControls,
    DynamicControls,
}
impl PaneRoles {
    fn has_title(self) -> bool {
        self != Self::NoTitle
    }
    fn has_full(self) -> bool {
        matches!(self, Self::FullControls | Self::DynamicControls)
    }
    fn has_compact(self) -> bool {
        self == Self::DynamicControls
    }
}
/// Invalid source selection or exhausted bounded role lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneRoleSourceError {
    Root,
    Schema,
    Work,
}
impl std::fmt::Display for PaneRoleSourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "pane source roles: {self:?}")
    }
}
impl std::error::Error for PaneRoleSourceError {}
impl PaneRoles {
    /// Derive supplied roles from one selected pane_content source. Native pane
    /// matching and scope establishment remain caller obligations. No allocation.
    pub fn from_construction(
        construction: &crate::capture::Construction,
        pane_root: usize,
        work: std::num::NonZeroUsize,
    ) -> Result<Self, PaneRoleSourceError> {
        let mut work = work.get() - 1;
        let nodes = construction.nodes();
        if nodes
            .get(pane_root)
            .is_none_or(|node| node.canonical_type() != "pane_content")
        {
            return Err(PaneRoleSourceError::Root);
        }
        let Some(title) = role_child(construction, pane_root, "title", &mut work)? else {
            return Ok(Self::NoTitle);
        };
        if nodes[title].canonical_type() != "pane_title" {
            return Err(PaneRoleSourceError::Schema);
        }
        let Some(controls) = role_child(construction, title, "controls", &mut work)? else {
            return Ok(Self::Title);
        };
        if nodes[controls].canonical_type() != "pane_controls" {
            return Err(PaneRoleSourceError::Schema);
        }
        Ok(
            if role_child(construction, controls, "compact", &mut work)?.is_some() {
                Self::DynamicControls
            } else {
                Self::FullControls
            },
        )
    }
}
fn role_child(
    construction: &crate::capture::Construction,
    parent: usize,
    role: &str,
    work: &mut usize,
) -> Result<Option<usize>, PaneRoleSourceError> {
    for (index, node) in construction.nodes().iter().enumerate().take(parent) {
        *work = work.checked_sub(1).ok_or(PaneRoleSourceError::Work)?;
        if node.parent() == Some(parent) && node.role() == Some(role) {
            return Ok(Some(index));
        }
    }
    Ok(None)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneRoleError {
    ContentShape,
    TitleShape,
    AbsentTitle,
    AbsentFull,
    AbsentCompact,
}
impl std::fmt::Display for PaneRoleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "pane role removal: {self:?}")
    }
}
impl std::error::Error for PaneRoleError {}

/// Detached state belongs to the caller, which controls its retirement and Drop.
#[derive(Debug, Default)]
pub struct RemovedPaneRoles {
    pub title: Option<Tree>,
    pub full: Option<Tree>,
    pub compact: Option<Tree>,
}

/// Dropping preparation leaves the active native tree unchanged.
#[must_use = "commit the validated removals or abandon preparation"]
pub struct PreparedPaneRoleRemoval<'a> {
    content: &'a mut Tree,
    title: bool,
    full: bool,
    compact: bool,
}
impl PreparedPaneRoleRemoval<'_> {
    /// Move omitted native state out without allocating, diffing, invoking native
    /// callbacks or dropping the removed state. Source ownership publication is
    /// the caller's separate transaction responsibility.
    pub fn commit(self) -> RemovedPaneRoles {
        let mut removed = RemovedPaneRoles::default();
        if self.title {
            removed.title = Some(std::mem::replace(
                &mut self.content.children[1],
                Tree::empty(),
            ));
        } else if self.full || self.compact {
            let slots = &mut self.content.children[1].children;
            if self.full {
                removed.full = Some(std::mem::replace(&mut slots[1], Tree::empty()));
            }
            if self.compact {
                removed.compact = Some(std::mem::replace(&mut slots[2], Tree::empty()));
            }
        }
        removed
    }
}
fn stateless(tree: &Tree) -> bool {
    tree.tag == Tag::stateless() && matches!(tree.state, State::None)
}
fn empty(tree: &Tree) -> bool {
    stateless(tree) && tree.children.is_empty()
}

/// Prepare omission cleanup for one correctly matched pane's Content Tree.
///
/// The caller establishes pane identity and supplies authoritative previous/source
/// role presence. Native slots are body 0/title 1, then title-content 0/full 1/compact
/// 2. This validates shape; it does not infer source presence from widget state or
/// handle key changes, native compatibility changes, focus/IME events or source IDs.
/// Initialized previous Content state is required; fresh state uses native creation.
pub fn prepare_pane_role_removal(
    content: &mut Tree,
    previous: PaneRoles,
    incoming: PaneRoles,
) -> Result<PreparedPaneRoleRemoval<'_>, PaneRoleError> {
    if !stateless(content) || content.children.len() != 2 {
        return Err(PaneRoleError::ContentShape);
    }
    let title = &content.children[1];
    if previous.has_title() {
        if !stateless(title) || title.children.len() != 3 {
            return Err(PaneRoleError::TitleShape);
        }
        if !previous.has_full() && !empty(&title.children[1]) {
            return Err(PaneRoleError::AbsentFull);
        }
        if !previous.has_compact() && !empty(&title.children[2]) {
            return Err(PaneRoleError::AbsentCompact);
        }
    } else if !empty(title) {
        return Err(PaneRoleError::AbsentTitle);
    }
    Ok(PreparedPaneRoleRemoval {
        content,
        title: previous.has_title() && !incoming.has_title(),
        full: incoming.has_title() && previous.has_full() && !incoming.has_full(),
        compact: incoming.has_title() && previous.has_compact() && !incoming.has_compact(),
    })
}
