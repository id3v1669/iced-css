//! An owned content boundary composed with iced's public Container helpers.
use iced::{
    Element, Length, Padding, Pixels, Size,
    advanced::{Renderer, widget::Id},
    alignment,
    widget::container::{Catalog, Style, StyleFn},
};

/// Retains the original content and native class without replaying construction.
/// The native Tree belongs directly to the content; the layout has one child.
/// Raw content mutation is an integration boundary, not permission to reuse stale
/// capture evidence after replacing authored content.
pub struct ContainerRecipe<'a, Message, Theme = iced::Theme, R = iced::Renderer>
where
    Theme: Catalog,
    R: Renderer,
{
    content: Element<'a, Message, Theme, R>,
    class: Theme::Class<'a>,
    id: Option<Id>,
    padding: Padding,
    size: Size<Length>,
    maximum: Size,
    horizontal: alignment::Horizontal,
    vertical: alignment::Vertical,
    clip: bool,
    presentation: ContainerPresentation,
}
impl<'a, Message, Theme: Catalog, R: Renderer> ContainerRecipe<'a, Message, Theme, R> {
    pub(super) fn new(content: impl Into<Element<'a, Message, Theme, R>>) -> Self {
        let content = content.into();
        let hint = content.as_widget().size_hint();
        let size = Size::new(hint.width.fluid(), hint.height.fluid());
        let class = Theme::default();
        Self {
            content,
            class,
            id: None,
            padding: Padding::ZERO,
            size,
            maximum: Size::new(f32::INFINITY, f32::INFINITY),
            horizontal: alignment::Horizontal::Left,
            vertical: alignment::Vertical::Top,
            clip: false,
            presentation: ContainerPresentation::default(),
        }
    }
    /// Replace accepted host presentation without invoking the original class.
    /// The default value restores unmodified native presentation. The host owns
    /// validation and publication; this raw integration method does neither.
    ///
    /// ```compile_fail,E0499
    /// use iced_css::capture::{ContainerRecipe,ContainerPresentation};
    /// use iced::{advanced::{Layout,widget::Tree},Rectangle,Vector};
    /// fn change(mut recipe:ContainerRecipe<'_,(),iced::Theme,()>,tree:&mut Tree,layout:Layout<'_>) {
    ///     let overlay=recipe.native_content_mut().as_widget_mut().overlay(tree,layout,&(),&Rectangle::default(),Vector::ZERO);
    ///     recipe.set_presentation(ContainerPresentation::default());
    ///     drop(overlay);
    /// }
    /// ```
    pub fn set_presentation(&mut self, presentation: ContainerPresentation) {
        self.presentation = presentation;
    }
    /// Borrow content for native state preparation without constructing another Tree.
    pub fn native_content(&self) -> &Element<'a, Message, Theme, R> {
        &self.content
    }
    /// Borrow content for native lifecycle forwarding while retaining its owner.
    /// Replacing authored content requires fresh capture evidence.
    ///
    /// ```compile_fail,E0505
    /// use iced_css::capture::ContainerRecipe;
    /// use iced::{advanced::{Layout,widget::Tree},Rectangle,Vector};
    /// fn erase(mut recipe:ContainerRecipe<'_,(),iced::Theme,()>,tree:&mut Tree,layout:Layout<'_>) {
    ///     let overlay=recipe.native_content_mut().as_widget_mut().overlay(tree,layout,&(),&Rectangle::default(),Vector::ZERO);
    ///     let _:iced::Element<'_,(),iced::Theme,()>=recipe.into();
    ///     drop(overlay);
    /// }
    /// ```
    pub fn native_content_mut(&mut self) -> &mut Element<'a, Message, Theme, R> {
        &mut self.content
    }
    #[must_use]
    pub fn id(mut self, value: impl Into<Id>) -> Self {
        self.id = Some(value.into());
        self
    }
    #[must_use]
    pub fn padding(mut self, value: impl Into<Padding>) -> Self {
        self.padding = value.into();
        self
    }
    #[must_use]
    pub fn width(mut self, value: impl Into<Length>) -> Self {
        self.size.width = value.into();
        self
    }
    #[must_use]
    pub fn height(mut self, value: impl Into<Length>) -> Self {
        self.size.height = value.into();
        self
    }
    #[must_use]
    pub fn max_width(mut self, value: impl Into<Pixels>) -> Self {
        self.maximum.width = value.into().0;
        self
    }
    #[must_use]
    pub fn max_height(mut self, value: impl Into<Pixels>) -> Self {
        self.maximum.height = value.into().0;
        self
    }
    #[must_use]
    pub fn align_x(mut self, value: impl Into<alignment::Horizontal>) -> Self {
        self.horizontal = value.into();
        self
    }
    #[must_use]
    pub fn align_y(mut self, value: impl Into<alignment::Vertical>) -> Self {
        self.vertical = value.into();
        self
    }
    #[must_use]
    pub fn center(self, value: impl Into<Length>) -> Self {
        let value = value.into();
        self.center_x(value).center_y(value)
    }
    #[must_use]
    pub fn center_x(self, value: impl Into<Length>) -> Self {
        self.width(value).align_x(alignment::Horizontal::Center)
    }
    #[must_use]
    pub fn center_y(self, value: impl Into<Length>) -> Self {
        self.height(value).align_y(alignment::Vertical::Center)
    }
    #[must_use]
    pub fn align_left(self, value: impl Into<Length>) -> Self {
        self.width(value).align_x(alignment::Horizontal::Left)
    }
    #[must_use]
    pub fn align_right(self, value: impl Into<Length>) -> Self {
        self.width(value).align_x(alignment::Horizontal::Right)
    }
    #[must_use]
    pub fn align_top(self, value: impl Into<Length>) -> Self {
        self.height(value).align_y(alignment::Vertical::Top)
    }
    #[must_use]
    pub fn align_bottom(self, value: impl Into<Length>) -> Self {
        self.height(value).align_y(alignment::Vertical::Bottom)
    }
    #[must_use]
    pub fn clip(mut self, value: bool) -> Self {
        self.clip = value;
        self
    }
    #[must_use]
    pub fn style(self, value: impl Fn(&Theme) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class(Box::new(value) as StyleFn<'a, Theme>)
    }
    #[must_use]
    pub fn class(mut self, value: impl Into<Theme::Class<'a>>) -> Self {
        self.class = value.into();
        self
    }
}
mod presentation;
mod widget;
pub use presentation::ContainerPresentation;

#[cfg(test)]
mod tests;
