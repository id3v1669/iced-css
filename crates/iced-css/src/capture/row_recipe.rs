//! Deferred Row ownership: native push's void filter cannot delete managed children.
use super::{CaptureError, CaptureLimits, Captured};
use iced::{Element, Length, Padding, alignment::Vertical};

/// Native Row construction state, retaining all children until explicit lowering.
/// This is a payload recipe, not an additional CSS source node or a CSS layout.
pub struct RowRecipe<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer> {
    children: Vec<Element<'a, Message, Theme, Renderer>>,
    width: Length,
    height: Length,
    spacing: Option<f32>,
    padding: Option<Padding>,
    align_y: Option<Vertical>,
    clip: Option<bool>,
}
impl<'a, Message, Theme, Renderer> RowRecipe<'a, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    pub(super) fn from_vec(children: Vec<Element<'a, Message, Theme, Renderer>>) -> Self {
        Self {
            children,
            width: Length::Shrink,
            height: Length::Shrink,
            spacing: None,
            padding: None,
            align_y: None,
            clip: None,
        }
    }
    fn push_child(&mut self, child: Element<'a, Message, Theme, Renderer>) {
        let hint = child.as_widget().size_hint();
        if !hint.is_void() {
            self.width = self.width.enclose(hint.width);
            self.height = self.height.enclose(hint.height);
        }
        // Q211: source membership and complete payload ownership survive void hints.
        self.children.push(child);
    }
    /// Close through stock from_vec; no child filtering or user conversion replay.
    pub fn into_native(self) -> iced::widget::Row<'a, Message, Theme, Renderer> {
        let mut row = iced::widget::Row::from_vec(self.children)
            .width(self.width)
            .height(self.height);
        if let Some(value) = self.spacing {
            row = row.spacing(value);
        }
        if let Some(value) = self.padding {
            row = row.padding(value);
        }
        if let Some(value) = self.align_y {
            row = row.align_y(value);
        }
        if let Some(value) = self.clip {
            row = row.clip(value);
        }
        row
    }
}
impl<'a, Message: 'a, Theme: 'a, Renderer: iced::advanced::Renderer + 'a>
    From<RowRecipe<'a, Message, Theme, Renderer>> for Element<'a, Message, Theme, Renderer>
{
    fn from(recipe: RowRecipe<'a, Message, Theme, Renderer>) -> Self {
        recipe.into_native().into()
    }
}
impl<'a, Message, Theme, Renderer> Captured<RowRecipe<'a, Message, Theme, Renderer>>
where
    Renderer: iced::advanced::Renderer,
{
    #[must_use]
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.payload.width = width.into();
        self
    }
    #[must_use]
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.payload.height = height.into();
        self
    }
    #[must_use]
    pub fn spacing(mut self, spacing: impl Into<iced::Pixels>) -> Self {
        self.payload.spacing = Some(spacing.into().0);
        self
    }
    #[must_use]
    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.payload.padding = Some(padding.into());
        self
    }
    #[must_use]
    pub fn align_y(mut self, alignment: impl Into<Vertical>) -> Self {
        self.payload.align_y = Some(alignment.into());
        self
    }
    #[must_use]
    pub fn clip(mut self, clip: bool) -> Self {
        self.payload.clip = Some(clip);
        self
    }
    /// Retain the complete managed child, including a void native hint.
    ///
    /// ```compile_fail,E0308
    /// # use iced_css::capture::{Captured, RowRecipe, CaptureLimits};
    /// fn bypass<'a>(parent: Captured<RowRecipe<'a, (), iced::Theme, ()>>,
    ///     child: iced::Element<'a, (), iced::Theme, ()>, limits: CaptureLimits) {
    ///     let _ = parent.push(child, limits);
    /// }
    /// ```
    ///
    /// # Errors
    /// Rejects aggregate evidence limits/allocation before native child conversion.
    pub fn push<W>(self, child: Captured<W>, limits: CaptureLimits) -> Result<Self, CaptureError>
    where
        W: Into<Element<'a, Message, Theme, Renderer>>,
    {
        let Captured {
            mut payload,
            construction,
        } = self;
        let Captured {
            payload: child,
            construction: child_construction,
        } = child;
        let construction = construction.append_ordered(child_construction, limits)?;
        payload.push_child(child.into());
        Ok(Self {
            payload,
            construction,
        })
    }
}

impl<'a, Message, Theme, Renderer> Captured<RowRecipe<'a, Message, Theme, Renderer>>
where
    Renderer: iced::advanced::Renderer,
{
    /// Preserve yielded order and complete payloads using one shared work budget.
    /// Existing from_vec hints are not replayed. Iterator callbacks/native payload
    /// work remain outside the evidence-visit bound; no polling occurs after None.
    ///
    /// ```compile_fail
    /// # use iced_css::capture::{Captured, RowRecipe, CaptureLimits};
    /// fn bypass<'a>(parent: Captured<RowRecipe<'a, (), iced::Theme, ()>>,
    ///     child: iced::Element<'a, (), iced::Theme, ()>, limits: CaptureLimits) {
    ///     let _ = parent.extend(vec![child], limits);
    /// }
    /// ```
    ///
    /// # Errors
    /// Rejects admission/work/allocation failures without a partial captured Row.
    pub fn extend<W>(
        self,
        children: impl IntoIterator<Item = Captured<W>>,
        limits: CaptureLimits,
    ) -> Result<Self, CaptureError>
    where
        W: Into<Element<'a, Message, Theme, Renderer>>,
    {
        super::ordered_insertion::extend(self, children, limits, Self::push::<W>)
    }
}
