//! Deferred native wrapping transitions preserve the existing CSS source node.
use super::{Captured, ColumnRecipe, RowRecipe};
use iced::{Element, alignment};

/// Deferred wrapping row; owns the complete base recipe and its children.
/// It is not an additional source node or a CSS wrapping implementation.
pub struct RowWrappingRecipe<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer> {
    base: RowRecipe<'a, Message, Theme, Renderer>,
    spacing: Option<f32>,
    alignment: Option<alignment::Horizontal>,
}
impl<'a, Message, Theme, Renderer: iced::advanced::Renderer>
    Captured<RowRecipe<'a, Message, Theme, Renderer>>
{
    /// Change the builder type without reallocating or adding source evidence.
    ///
    /// ```compile_fail
    /// # use iced_css::capture::{Captured, RowRecipe};
    /// fn base_only(parent: Captured<RowRecipe<'_, (), iced::Theme, ()>>) {
    ///     let _ = parent.wrap().width(20.0);
    /// }
    /// ```
    #[must_use]
    pub fn wrap(self) -> Captured<RowWrappingRecipe<'a, Message, Theme, Renderer>> {
        Captured {
            payload: RowWrappingRecipe {
                base: self.payload,
                spacing: None,
                alignment: None,
            },
            construction: self.construction,
        }
    }
}
impl<Message, Theme, Renderer> Captured<RowWrappingRecipe<'_, Message, Theme, Renderer>> {
    #[must_use]
    pub fn vertical_spacing(mut self, spacing: impl Into<iced::Pixels>) -> Self {
        self.payload.spacing = Some(spacing.into().0);
        self
    }
    /// Matches the retained native method name and alignment argument type.
    #[must_use]
    pub fn align_x(mut self, alignment: impl Into<alignment::Horizontal>) -> Self {
        self.payload.alignment = Some(alignment.into());
        self
    }
}
impl<'a, Message, Theme, Renderer: iced::advanced::Renderer>
    RowWrappingRecipe<'a, Message, Theme, Renderer>
{
    /// Close the base through from_vec, preserving every child, then wrap it.
    pub fn into_native(self) -> iced::widget::row::Wrapping<'a, Message, Theme, Renderer> {
        let mut wrapped = self.base.into_native().wrap();
        if let Some(value) = self.spacing {
            wrapped = wrapped.vertical_spacing(value);
        }
        if let Some(value) = self.alignment {
            wrapped = wrapped.align_x(value);
        }
        wrapped
    }
}
impl<'a, Message: 'a, Theme: 'a, Renderer: iced::advanced::Renderer + 'a>
    From<RowWrappingRecipe<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
{
    fn from(recipe: RowWrappingRecipe<'a, Message, Theme, Renderer>) -> Self {
        recipe.into_native().into()
    }
}

/// Deferred wrapping column; owns the complete base recipe and its children.
/// It is not an additional source node or a CSS wrapping implementation.
pub struct ColumnWrappingRecipe<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer> {
    base: ColumnRecipe<'a, Message, Theme, Renderer>,
    spacing: Option<f32>,
    alignment: Option<alignment::Vertical>,
}
impl<'a, Message, Theme, Renderer: iced::advanced::Renderer>
    Captured<ColumnRecipe<'a, Message, Theme, Renderer>>
{
    /// Change the builder type without reallocating or adding source evidence.
    ///
    /// ```compile_fail
    /// # use iced_css::capture::{Captured, ColumnRecipe};
    /// fn base_only(parent: Captured<ColumnRecipe<'_, (), iced::Theme, ()>>) {
    ///     let _ = parent.wrap().width(20.0);
    /// }
    /// ```
    #[must_use]
    pub fn wrap(self) -> Captured<ColumnWrappingRecipe<'a, Message, Theme, Renderer>> {
        Captured {
            payload: ColumnWrappingRecipe {
                base: self.payload,
                spacing: None,
                alignment: None,
            },
            construction: self.construction,
        }
    }
}
impl<Message, Theme, Renderer> Captured<ColumnWrappingRecipe<'_, Message, Theme, Renderer>> {
    #[must_use]
    pub fn horizontal_spacing(mut self, spacing: impl Into<iced::Pixels>) -> Self {
        self.payload.spacing = Some(spacing.into().0);
        self
    }
    /// Matches the retained native method name and alignment argument type.
    #[must_use]
    pub fn align_x(mut self, alignment: impl Into<alignment::Vertical>) -> Self {
        self.payload.alignment = Some(alignment.into());
        self
    }
}
impl<'a, Message: 'a, Theme: 'a, Renderer: iced::advanced::Renderer + 'a>
    ColumnWrappingRecipe<'a, Message, Theme, Renderer>
{
    /// Close the base through from_vec, preserving every child, then wrap it.
    /// Column wrapping is returned as an Element because iced keeps its module private.
    pub fn into_native(self) -> Element<'a, Message, Theme, Renderer> {
        let mut wrapped = self.base.into_native().wrap();
        if let Some(value) = self.spacing {
            wrapped = wrapped.horizontal_spacing(value);
        }
        if let Some(value) = self.alignment {
            wrapped = wrapped.align_x(value);
        }
        wrapped.into()
    }
}
impl<'a, Message: 'a, Theme: 'a, Renderer: iced::advanced::Renderer + 'a>
    From<ColumnWrappingRecipe<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
{
    fn from(recipe: ColumnWrappingRecipe<'a, Message, Theme, Renderer>) -> Self {
        recipe.into_native()
    }
}
