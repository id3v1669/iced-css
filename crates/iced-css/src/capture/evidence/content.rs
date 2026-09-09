//! Borrowed content responsibilities of the explicitly captured source families.
#[cfg(feature = "image-without-codecs")]
use super::ImageSource;
#[cfg(feature = "svg")]
use super::SvgSource;
use super::{ConstructionNode, Kind};

/// A declared managed-content family, not a native layout or box recipe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManagedContentFamily {
    Table,
    TableHeaderGroup,
    TableRowGroup,
    TableRow,
    TableHeaderCell,
    TableCell,
    PaneGrid,
    PaneControls,
    PaneTitle,
    PaneContent,
    Container,
    Row,
    Column,
    KeyedColumn,
    Stack,
    Grid,
    Scrollable,
    Tooltip,
    Hover,
    Pin,
    MouseArea,
    Float,
    Themer,
    Sensor,
    Opaque,
}

/// Known control content whose internal formatting recipe remains required.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlContentFamily {
    Checkbox,
    Toggler,
    Radio,
    Button,
    PickList,
    Rule,
    ProgressBar,
    Slider,
    VerticalSlider,
    Space,
}

/// Original content borrowed from one construction node. Rich runs are not
/// source children; control labels are not ordinary inline text participants.
/// Resource variants borrow their existing evidence without cloning handles.
#[derive(Debug)]
pub enum ContentCapability<'a> {
    Text(&'a str),
    RichText {
        text: &'a str,
        runs: &'a [std::ops::Range<usize>],
    },
    CheckboxLabel(Option<&'a str>),
    TogglerLabel(Option<&'a str>),
    RadioLabel(&'a str),
    Managed(ManagedContentFamily),
    RecipeRequired(ControlContentFamily),
    #[cfg(feature = "image-without-codecs")]
    Image(&'a ImageSource),
    #[cfg(feature = "svg")]
    Svg(&'a SvgSource),
}

/// Replacement evidence is independent of computed display and suppression.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplacementCapability {
    NonReplaced,
    Replaced,
    RecipeRequired(ControlContentFamily),
}
impl ContentCapability<'_> {
    pub fn replacement(&self) -> ReplacementCapability {
        match self {
            Self::Text(_) | Self::RichText { .. } | Self::Managed(_) => {
                ReplacementCapability::NonReplaced
            }
            Self::CheckboxLabel(_) => {
                ReplacementCapability::RecipeRequired(ControlContentFamily::Checkbox)
            }
            Self::TogglerLabel(_) => {
                ReplacementCapability::RecipeRequired(ControlContentFamily::Toggler)
            }
            Self::RadioLabel(_) => {
                ReplacementCapability::RecipeRequired(ControlContentFamily::Radio)
            }
            Self::RecipeRequired(family) => ReplacementCapability::RecipeRequired(*family),
            #[cfg(feature = "image-without-codecs")]
            Self::Image(_) => ReplacementCapability::Replaced,
            #[cfg(feature = "svg")]
            Self::Svg(_) => ReplacementCapability::Replaced,
        }
    }
    /// Unresolved internal control recipe, including optional or required labels.
    pub fn recipe_requirement(&self) -> Option<ControlContentFamily> {
        match self.replacement() {
            ReplacementCapability::RecipeRequired(family) => Some(family),
            ReplacementCapability::NonReplaced | ReplacementCapability::Replaced => None,
        }
    }
}
impl ConstructionNode {
    /// Inspect known content without allocating, cloning resources, scanning text,
    /// or inferring replacement from absent text or a canonical type name.
    pub fn content_capability(&self) -> ContentCapability<'_> {
        match &self.kind {
            Kind::Text(text) => ContentCapability::Text(text),
            Kind::RichText { text, runs } => ContentCapability::RichText { text, runs },
            Kind::Checkbox(label) => ContentCapability::CheckboxLabel(label.as_deref()),
            Kind::Toggler(label) => ContentCapability::TogglerLabel(label.as_deref()),
            Kind::Radio(label) => ContentCapability::RadioLabel(label),
            Kind::Table => ContentCapability::Managed(ManagedContentFamily::Table),
            Kind::TableHeaderGroup => {
                ContentCapability::Managed(ManagedContentFamily::TableHeaderGroup)
            }
            Kind::TableRowGroup => ContentCapability::Managed(ManagedContentFamily::TableRowGroup),
            Kind::TableRow => ContentCapability::Managed(ManagedContentFamily::TableRow),
            Kind::TableHeaderCell => {
                ContentCapability::Managed(ManagedContentFamily::TableHeaderCell)
            }
            Kind::TableCell => ContentCapability::Managed(ManagedContentFamily::TableCell),
            Kind::PaneGrid => ContentCapability::Managed(ManagedContentFamily::PaneGrid),
            Kind::PaneControls => ContentCapability::Managed(ManagedContentFamily::PaneControls),
            Kind::PaneTitle => ContentCapability::Managed(ManagedContentFamily::PaneTitle),
            Kind::PaneContent => ContentCapability::Managed(ManagedContentFamily::PaneContent),
            Kind::Container => ContentCapability::Managed(ManagedContentFamily::Container),
            Kind::Row => ContentCapability::Managed(ManagedContentFamily::Row),
            Kind::Column => ContentCapability::Managed(ManagedContentFamily::Column),
            Kind::KeyedColumn => ContentCapability::Managed(ManagedContentFamily::KeyedColumn),
            Kind::Stack => ContentCapability::Managed(ManagedContentFamily::Stack),
            Kind::Grid => ContentCapability::Managed(ManagedContentFamily::Grid),
            Kind::Scrollable => ContentCapability::Managed(ManagedContentFamily::Scrollable),
            Kind::Tooltip => ContentCapability::Managed(ManagedContentFamily::Tooltip),
            Kind::Hover => ContentCapability::Managed(ManagedContentFamily::Hover),
            Kind::Pin => ContentCapability::Managed(ManagedContentFamily::Pin),
            Kind::MouseArea => ContentCapability::Managed(ManagedContentFamily::MouseArea),
            Kind::Float => ContentCapability::Managed(ManagedContentFamily::Float),
            Kind::Themer => ContentCapability::Managed(ManagedContentFamily::Themer),
            Kind::Sensor => ContentCapability::Managed(ManagedContentFamily::Sensor),
            Kind::Opaque => ContentCapability::Managed(ManagedContentFamily::Opaque),
            Kind::Button => ContentCapability::RecipeRequired(ControlContentFamily::Button),
            Kind::PickList => ContentCapability::RecipeRequired(ControlContentFamily::PickList),
            Kind::Rule => ContentCapability::RecipeRequired(ControlContentFamily::Rule),
            Kind::ProgressBar => {
                ContentCapability::RecipeRequired(ControlContentFamily::ProgressBar)
            }
            Kind::Slider => ContentCapability::RecipeRequired(ControlContentFamily::Slider),
            Kind::VerticalSlider => {
                ContentCapability::RecipeRequired(ControlContentFamily::VerticalSlider)
            }
            Kind::Space => ContentCapability::RecipeRequired(ControlContentFamily::Space),
            #[cfg(feature = "image-without-codecs")]
            Kind::Image(source) => ContentCapability::Image(source),
            #[cfg(feature = "svg")]
            Kind::Svg(source) => ContentCapability::Svg(source),
        }
    }
}

#[cfg(test)]
#[path = "content_tests.rs"]
mod tests;
