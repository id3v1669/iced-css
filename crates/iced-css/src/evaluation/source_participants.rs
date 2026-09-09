//! Fixed borrowed source facts, before any formatting-tree or paragraph production.
use super::source_eligibility::{Eligibility, SourceEligibility};
use crate::capture::{
    ConstructionNode, ContentCapability, ControlContentFamily, ReplacementCapability,
};
use iced_css_core::{
    formatting::display::{Display, DisplayResolution, Flow},
    identity::SourceRef,
    values::{bidi::ComputedBidiStyle, text_keywords::ComputedTextKeywords},
};

/// Eligibility takes precedence over a source's own computed flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceParticipation {
    DisplayNone,
    SuppressedByAncestor { index: usize },
    Eligible { flow: Flow },
}

/// Required outer category, not an emitted box or a certificate of its internals.
/// InlineBlock/InlineTable are distinct from the CSS inline-replaced category.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceOuterCategory {
    Suppressed,
    InlineNonReplaced,
    InlineReplaced,
    InlineRecipeRequired(ControlContentFamily),
    InlineBlock,
    InlineTable,
    Block,
    ListItem,
    Table,
    TableRowGroup,
    TableHeaderGroup,
    TableFooterGroup,
    TableRow,
    TableColumnGroup,
    TableColumn,
    TableCell,
    TableCaption,
}

/// Allocation-free original-index views over one exact eligibility owner.
/// No arbitrary token or second snapshot can be supplied to join unrelated owners.
/// Each query does bounded constant work; repeated calls are caller-owned work.
/// Borrowed predecessor storage remains live and is not counted as new payload.
/// This view neither constructs boxes nor admits a complete formatting domain:
/// eligible control recipes must still be handled explicitly by a later producer.
///
/// ```
/// use iced_css::evaluation::{source_eligibility::SourceEligibility,
///     source_participants::SourceParticipants};
/// fn wrapper_can_drop(owner: &SourceEligibility<'_, '_, '_, '_>) {
///     let view = SourceParticipants::new(owner);
///     if let Some(item) = view.get(0) {
///         drop(view);
///         let _ = item.token();
///     }
/// }
/// ```
/// ```compile_fail,E0505
/// use iced_css::evaluation::{source_eligibility::SourceEligibility,
///     source_participants::SourceParticipants};
/// fn owner_cannot_drop(owner: SourceEligibility<'_, '_, '_, '_>) {
///     let item = SourceParticipants::new(&owner).get(0).unwrap();
///     drop(owner);
///     let _ = item.token();
/// }
/// ```
#[must_use]
pub struct SourceParticipants<'e, 'd, 's, 'b, 'r> {
    eligibility: &'e SourceEligibility<'d, 's, 'b, 'r>,
}
impl<'e, 'd, 's, 'b, 'r> SourceParticipants<'e, 'd, 's, 'b, 'r> {
    pub fn new(eligibility: &'e SourceEligibility<'d, 's, 'b, 'r>) -> Self {
        Self { eligibility }
    }
    pub fn eligibility(&self) -> &'e SourceEligibility<'d, 's, 'b, 'r> {
        self.eligibility
    }
    pub fn node_count(&self) -> usize {
        self.eligibility.node_count()
    }
    /// Checked original construction index, not an eligible preorder ordinal.
    /// None denotes an invalid index; every valid source includes suppressed facts.
    pub fn get(&self, source: usize) -> Option<SourceParticipant<'e, 'd, 's, 'b, 'r>> {
        let display = self.eligibility.display();
        display.snapshot().construction().nodes().get(source)?;
        self.eligibility.token(source)?;
        display.resolution(source)?;
        display.text_keywords(source)?;
        display.bidi_style(source)?;
        self.eligibility.disposition(source)?;
        Some(SourceParticipant {
            eligibility: self.eligibility,
            source,
        })
    }
}

/// A privately checked original index borrowing eligibility directly. This can
/// outlive the lightweight SourceParticipants wrapper, but not its actual owner.
/// Node, token, style and content references borrow this entry; eligibility()
/// preserves the original eligibility borrow lifetime. Text/runs/handles are never
/// copied. Control recipes remain explicit even for known outer categories.
#[must_use]
pub struct SourceParticipant<'e, 'd, 's, 'b, 'r> {
    eligibility: &'e SourceEligibility<'d, 's, 'b, 'r>,
    source: usize,
}
impl<'e, 'd, 's, 'b, 'r> SourceParticipant<'e, 'd, 's, 'b, 'r> {
    pub fn eligibility(&self) -> &'e SourceEligibility<'d, 's, 'b, 'r> {
        self.eligibility
    }
    pub fn source_index(&self) -> usize {
        self.source
    }
    pub fn node(&self) -> &ConstructionNode {
        &self.eligibility.display().snapshot().construction().nodes()[self.source]
    }
    /// Exact borrowed token of this owner, not a cloned or reconstructed token.
    pub fn token(&self) -> &SourceRef {
        self.eligibility
            .token(self.source)
            .expect("checked immutable source token")
    }
    pub fn resolution(&self) -> &DisplayResolution {
        self.eligibility
            .display()
            .resolution(self.source)
            .expect("checked immutable display")
    }
    pub fn text_keywords(&self) -> &ComputedTextKeywords {
        self.eligibility
            .display()
            .text_keywords(self.source)
            .expect("checked immutable text keywords")
    }
    pub fn bidi_style(&self) -> &ComputedBidiStyle {
        self.eligibility
            .display()
            .bidi_style(self.source)
            .expect("checked immutable bidi style")
    }
    pub fn disposition(&self) -> Eligibility {
        self.eligibility
            .disposition(self.source)
            .expect("checked immutable eligibility")
    }
    pub fn content(&self) -> ContentCapability<'_> {
        self.node().content_capability()
    }
    pub fn participation(&self) -> SourceParticipation {
        match self.disposition() {
            Eligibility::DisplayNone => SourceParticipation::DisplayNone,
            Eligibility::Ancestor { index } => SourceParticipation::SuppressedByAncestor { index },
            Eligibility::Eligible => SourceParticipation::Eligible {
                flow: self.resolution().flow,
            },
        }
    }
    /// Suppression precedes display projection; computed flow remains independent.
    /// No computed display transformation or internal control recipe is executed.
    pub fn outer_category(&self) -> SourceOuterCategory {
        if self.disposition() != Eligibility::Eligible {
            return SourceOuterCategory::Suppressed;
        }
        match self.resolution().display {
            Display::None => SourceOuterCategory::Suppressed,
            Display::Inline => match self.content().replacement() {
                ReplacementCapability::NonReplaced => SourceOuterCategory::InlineNonReplaced,
                ReplacementCapability::Replaced => SourceOuterCategory::InlineReplaced,
                ReplacementCapability::RecipeRequired(family) => {
                    SourceOuterCategory::InlineRecipeRequired(family)
                }
            },
            Display::InlineBlock => SourceOuterCategory::InlineBlock,
            Display::InlineTable => SourceOuterCategory::InlineTable,
            Display::Block => SourceOuterCategory::Block,
            Display::ListItem => SourceOuterCategory::ListItem,
            Display::Table => SourceOuterCategory::Table,
            Display::TableRowGroup => SourceOuterCategory::TableRowGroup,
            Display::TableHeaderGroup => SourceOuterCategory::TableHeaderGroup,
            Display::TableFooterGroup => SourceOuterCategory::TableFooterGroup,
            Display::TableRow => SourceOuterCategory::TableRow,
            Display::TableColumnGroup => SourceOuterCategory::TableColumnGroup,
            Display::TableColumn => SourceOuterCategory::TableColumn,
            Display::TableCell => SourceOuterCategory::TableCell,
            Display::TableCaption => SourceOuterCategory::TableCaption,
        }
    }
}
