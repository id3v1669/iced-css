//! Typed construction evidence for registered adapters.
//!
//! This is generated-integration machinery. Annotation generation and the full
//! adapter registry are not implemented by these initial built-in adapters.
pub mod adapters;
mod evidence;
pub mod source;
pub use evidence::{CaptureError, CaptureLimits, Construction, ConstructionNode};

/// A native payload whose managed construction has not been erased.
///
/// There is no blanket import from native widgets, `Deref`, or `Into<Element>`.
/// A registered adapter consumes captured children explicitly.
///
/// ```compile_fail,E0277
/// # use iced_css::capture::{CapturedElement};
/// fn bypass(child: CapturedElement<'_, (), iced::Theme, ()>) {
///     let _ = iced::widget::container(child);
/// }
/// ```
///
/// ```compile_fail,E0308
/// # use iced_css::capture::{adapters, CaptureLimits};
/// fn bypass(child: iced::Element<'_, (), iced::Theme, ()>, limits: CaptureLimits) {
///     let _ = adapters::container(child, limits);
/// }
/// ```
pub struct Captured<W> {
    pub(super) payload: W,
    pub(super) construction: Construction,
}

/// Element erasure that still retains typed construction evidence.
pub type CapturedElement<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer> =
    Captured<iced::Element<'a, Message, Theme, Renderer>>;

impl<W> Captured<W> {
    /// Own an explicit application key for this source root. Caller-provided bytes
    /// must encode application identity injectively. Duplicate sibling keys are
    /// rejected by host matching, not by capture without a parent scope.
    ///
    /// An empty key is explicit; it differs from an unkeyed construction site.
    /// Limits cover final evidence storage, excluding temporary replacement overlap.
    pub fn source_key(mut self, key: &[u8], limits: CaptureLimits) -> Result<Self, CaptureError> {
        self.construction.set_source_key(key, limits)?;
        Ok(self)
    }

    /// Write a canonical root attribute. Attribute bounds apply to this root;
    /// parent assembly separately checks aggregate evidence capacity.
    pub fn style_attribute(
        mut self,
        name: &str,
        value: &str,
        limits: iced_css_core::source::AttributeLimits,
    ) -> Result<Self, iced_css_core::source::AttributeError> {
        self.construction
            .edit_metadata(limits, |metadata, limits| {
                metadata.try_set_attribute(name, value, limits)
            })?;
        Ok(self)
    }

    /// Replace the shared id attribute; an empty ID has no ID-selector token.
    pub fn style_id(
        self,
        id: &str,
        limits: iced_css_core::source::AttributeLimits,
    ) -> Result<Self, iced_css_core::source::AttributeError> {
        self.style_attribute("id", id, limits)
    }

    /// Serialize nonempty CSS-whitespace-free class tokens into the class slot.
    pub fn style_classes(
        mut self,
        classes: &[&str],
        limits: iced_css_core::source::AttributeLimits,
    ) -> Result<Self, iced_css_core::source::AttributeError> {
        self.construction
            .edit_metadata(limits, |metadata, limits| {
                metadata.try_set_classes(classes, limits)
            })?;
        Ok(self)
    }
    /// Inspect the independently owned, pre-identity source recipe.
    pub fn construction(&self) -> &Construction {
        &self.construction
    }

    /// Explicit native integration boundary. A raw payload cannot be imported
    /// back into managed construction through a blanket conversion.
    pub fn into_parts(self) -> (W, Construction) {
        (self.payload, self.construction)
    }

    /// Erase the native widget type while preserving evidence and borrow lifetime.
    pub fn erase<'a, Message, Theme, Renderer>(
        self,
    ) -> CapturedElement<'a, Message, Theme, Renderer>
    where
        W: Into<iced::Element<'a, Message, Theme, Renderer>>,
    {
        Captured {
            payload: self.payload.into(),
            construction: self.construction,
        }
    }
}

impl<'a, Message, Theme, Renderer> CapturedElement<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: iced::advanced::Renderer + 'a,
{
    /// Preserve source evidence while iced maps emitted messages at event time.
    pub fn map<B: 'a>(
        self,
        mapper: impl Fn(Message) -> B + 'a,
    ) -> CapturedElement<'a, B, Theme, Renderer> {
        Captured {
            payload: self.payload.map(mapper),
            construction: self.construction,
        }
    }

    /// Explain native layout without adding a CSS source node.
    #[must_use]
    pub fn explain(self, color: impl Into<iced::Color>) -> Self {
        Captured {
            payload: self.payload.explain(color),
            construction: self.construction,
        }
    }
}

mod row_recipe;
pub use row_recipe::RowRecipe;

mod ordered_insertion;

mod column_recipe;
pub use column_recipe::ColumnRecipe;

mod wrapping_recipe;
pub use wrapping_recipe::{ColumnWrappingRecipe, RowWrappingRecipe};

mod container_builders;

mod button_builders;

mod text_builders;

mod scrollable;

mod tooltip;

mod pin;

mod mouse_area;

mod float;

mod themer;

mod sensor;

mod rule;

mod progress_bar;

mod checkbox;
mod pane_controls;
pub use pane_controls::CapturedPaneControls;
mod pick_list;
pub use pick_list::CapturedPickList;
mod radio;
mod sliders;
mod toggler;

pub mod keyed_children;

pub mod context;

mod pane_descriptors;
pub use pane_descriptors::{CapturedPaneContent, CapturedPaneTitle};
