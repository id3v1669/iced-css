//! CSS 2.2 styling and layout for iced.
//!
//! The entry point is [`box_tree::BoxTree`]: capture a tree of boxes (see
//! [`capture::adapters`]), cascade an author stylesheet over it with
//! [`box_tree::BoxTree::new`], and use the result as a laid-out, painted and
//! interactive iced widget. [`box_tree::BoxTree::restyled`] swaps in edited
//! sheets without capturing the tree again, which is how to build hot reload.
//!
//! # Legacy Compile annotation
//!
//! The root items [`style`], [`StyleClasses`], [`apply`], [`Constrained`],
//! [`Resolved`], [`Policy`], [`Event`], [`update`] and [`subscription`] belong
//! to an older, separate path: the `#[iced_css::style]` attribute macro, which
//! accepts only single `.class` selectors with sizing, margin and padding
//! properties, and only `policy=Compile`. It does not use the `BoxTree`
//! cascade. New code should use [`box_tree::BoxTree`].
//!
//! The legacy annotation requires literal class names in active source:
//! ```compile_fail
//! #[iced_css::style(inline=".x {width:37px;}",policy=Compile)]
//! fn view(class:&str)->iced::Element<'static,(),iced::Theme,()> {
//!     iced::widget::container(iced::widget::Space::new()).style_classes([class]).into()
//! }
//! ```
//! It also requires a literal array, rather than an array variable:
//! ```compile_fail
//! #[iced_css::style(inline=".x {width:37px;}",policy=Compile)]
//! fn view(classes:[&str;1])->iced::Element<'static,(),iced::Theme,()> {
//!     iced::widget::container(iced::widget::Space::new()).style_classes(classes).into()
//! }
//! ```
//! The annotation has no type information: it rewrites every one-argument
//! `.style_classes(..)` method call in the function body, whatever the receiver,
//! including one on a type of your own that happens to have a method of that name.
//! Such a receiver fails to compile unless it converts into an `iced::Element`;
//! call it from a function outside the annotated body instead:
//! ```compile_fail
//! struct Marker;
//! impl Marker {
//!     fn style_classes(self, _: [&str; 1]) -> usize { 1 }
//! }
//! #[iced_css::style(inline=".x {width:37px;}",policy=Compile)]
//! fn view() -> usize {
//!     Marker.style_classes(["x"])
//! }
//! ```
//! Runtime conditions do not remove invalid source as compiler cfg guards do:
//! ```compile_fail
//! #[iced_css::style(inline=".x {width:37px;}",policy=Compile)]
//! fn view(class:&str)->iced::Element<'static,(),iced::Theme,()> {
//!     if cfg!(any()) {
//!         return iced::widget::container(iced::widget::Space::new()).style_classes([class]).into();
//!     }
//!     iced::widget::Space::new().into()
//! }
//! ```

pub mod box_tree;
pub mod capture;
mod constrained;
pub mod diagnostics;
pub mod evaluation;

pub use constrained::Constrained;
pub use iced_css_core::{Length, MarginValue, Margins, Paddings, Resolved};
pub use iced_css_macro::style;

#[doc(hidden)]
pub mod dispatch {
    use super::{Constrained, MarginValue, Paddings, Resolved};

    /// Never implemented: it makes a `style_classes` call that the
    /// `#[iced_css::style]` macro did not rewrite a compile error.
    #[diagnostic::on_unimplemented(
        message = "`style_classes` was not rewritten by `#[iced_css::style]`",
        label = "this call would apply no style",
        note = "call `style_classes` directly in the body of a function annotated with \
                `#[iced_css::style(..)]`, not in a nested item or an unannotated function",
        note = "for `BoxTree` stylesheets use `iced_css::capture::Captured::style_classes`"
    )]
    pub trait Rewritten {}
    use iced::widget::{Button, Container, button, container};

    pub struct NativeContainer;
    pub struct NativeButton;
    pub struct Wrapped;

    pub trait TagContainer {
        fn __iced_css_tag(&self) -> NativeContainer {
            NativeContainer
        }
    }
    impl<'a, M, T, R> TagContainer for Container<'a, M, T, R>
    where
        T: container::Catalog,
        R: iced::advanced::Renderer,
    {
    }

    pub trait TagButton {
        fn __iced_css_tag(&self) -> NativeButton {
            NativeButton
        }
    }
    impl<'a, M, T, R> TagButton for Button<'a, M, T, R>
    where
        T: button::Catalog,
        R: iced::advanced::Renderer,
    {
    }

    pub trait TagWrapped {
        fn __iced_css_tag(&self) -> Wrapped {
            Wrapped
        }
    }
    impl<T> TagWrapped for &T {}

    fn height_length(resolved: &Resolved) -> Option<iced::Length> {
        match resolved.height {
            Some(super::Length::Px(_) | super::Length::Percent(_)) => Some(iced::Length::Fill),
            Some(super::Length::Auto) | None => None,
        }
    }

    fn padding(p: Paddings) -> iced::Padding {
        iced::Padding {
            top: p.top,
            right: p.right,
            bottom: p.bottom,
            left: p.left,
        }
    }

    impl NativeContainer {
        pub fn apply<'a, M, T, R>(
            self,
            mut widget: Container<'a, M, T, R>,
            resolved: Resolved,
        ) -> iced::Element<'a, M, T, R>
        where
            M: 'a,
            T: container::Catalog + 'a,
            R: iced::advanced::Renderer + 'a,
        {
            if let Some(p) = resolved.padding {
                widget = widget.padding(padding(p));
            }
            widget = widget.width(iced::Length::Fill);
            if let Some(height) = height_length(&resolved) {
                widget = widget.height(height);
            }
            finish(widget, resolved)
        }
    }

    impl NativeButton {
        pub fn apply<'a, M, T, R>(
            self,
            mut widget: Button<'a, M, T, R>,
            resolved: Resolved,
        ) -> iced::Element<'a, M, T, R>
        where
            M: Clone + 'a,
            T: button::Catalog + container::Catalog + 'a,
            R: iced::advanced::Renderer + 'a,
        {
            if let Some(p) = resolved.padding {
                widget = widget.padding(padding(p));
            }
            widget = widget.width(iced::Length::Fill);
            if let Some(height) = height_length(&resolved) {
                widget = widget.height(height);
            }
            finish(widget, resolved)
        }
    }

    impl Wrapped {
        pub fn apply<'a, M, T, R>(
            self,
            widget: impl Into<iced::Element<'a, M, T, R>>,
            resolved: Resolved,
        ) -> iced::Element<'a, M, T, R>
        where
            M: 'a,
            T: container::Catalog + 'a,
            R: iced::advanced::Renderer + 'a,
        {
            let mut element_box = iced::widget::container(widget).width(iced::Length::Fill);
            if let Some(p) = resolved.padding {
                element_box = element_box.padding(padding(p));
            }
            if let Some(height) = height_length(&resolved) {
                element_box = element_box.height(height);
            }
            finish(element_box, resolved)
        }
    }

    fn finish<'a, M, T, R>(
        element_box: impl Into<iced::Element<'a, M, T, R>>,
        resolved: Resolved,
    ) -> iced::Element<'a, M, T, R>
    where
        M: 'a,
        T: container::Catalog + 'a,
        R: iced::advanced::Renderer + 'a,
    {
        let mut element: iced::Element<'a, M, T, R> =
            Constrained::new(element_box, &resolved).into();

        if let Some(margin) = resolved.margin {
            let px = |value: MarginValue| match value {
                MarginValue::Px(v) => v,
                MarginValue::Auto => 0.0,
            };
            let mut wrapper = iced::widget::container(element).padding(iced::Padding {
                top: px(margin.top),
                right: px(margin.right),
                bottom: px(margin.bottom),
                left: px(margin.left),
            });

            wrapper = match (margin.left, margin.right) {
                (MarginValue::Auto, MarginValue::Auto) => wrapper.center_x(iced::Length::Fill),
                (MarginValue::Auto, _) => wrapper
                    .width(iced::Length::Fill)
                    .align_x(iced::alignment::Horizontal::Right),
                (_, MarginValue::Auto) => wrapper
                    .width(iced::Length::Fill)
                    .align_x(iced::alignment::Horizontal::Left),
                _ => wrapper,
            };

            element = wrapper.into();
        }

        element
    }
}

pub fn apply<'a, Message, Theme, Renderer>(
    element: impl Into<iced::Element<'a, Message, Theme, Renderer>>,
    resolved: Resolved,
) -> iced::Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: iced::widget::container::Catalog + 'a,
    Renderer: iced::advanced::Renderer + 'a,
{
    dispatch::Wrapped.apply(element, resolved)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Policy {
    Compile,
    OnDemand,
    Auto,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Reload,
    Reloaded(Result<(), Error>),
    //to be used for internal events like hover
    #[doc(hidden)]
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error(pub String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "iced-css error: {}", self.0)
    }
}

impl std::error::Error for Error {}

/// Runtime stylesheet update for the `OnDemand` and `Auto` legacy policies.
///
/// # Panics
///
/// Always panics: these policies are not implemented, and the
/// `#[iced_css::style]` macro rejects them. For runtime stylesheet changes use
/// [`box_tree::BoxTree::restyled`].
pub fn update(event: Event) {
    let _ = event;
    unimplemented!("iced-css: runtime update not implemented yet")
}

/// File-watching subscription for the `Auto` legacy policy.
///
/// # Panics
///
/// Always panics: the `Auto` policy is not implemented, and the
/// `#[iced_css::style]` macro rejects it. For runtime stylesheet changes use
/// [`box_tree::BoxTree::restyled`].
pub fn subscription<Message: From<Event> + Send + 'static>() -> iced::Subscription<Message> {
    unimplemented!("iced-css: file subscription not implemented yet")
}

/// Placeholder method that the legacy `#[iced_css::style]` macro rewrites.
///
/// The macro replaces every one-argument `.style_classes(..)` call in the body of
/// the function it annotates. A call it does not rewrite, outside such a function
/// or in an item nested inside one, fails to compile instead of returning the
/// widget unstyled:
/// ```compile_fail
/// use iced_css::StyleClasses;
/// fn view() -> iced::Element<'static, ()> {
///     iced::widget::container(iced::widget::Space::new()).style_classes(["x"])
/// }
/// ```
/// To give a box an id or classes for a [`box_tree::BoxTree`] stylesheet, use
/// [`capture::Captured::style_id`] and [`capture::Captured::style_classes`].
pub trait StyleClasses<'a, Message> {
    /// Rewritten by `#[iced_css::style]`; a compile error anywhere else.
    fn style_classes<const N: usize>(
        self,
        classes: [&'static str; N],
    ) -> iced::Element<'a, Message>
    where
        Self: dispatch::Rewritten;
}

impl<'a, Message, T> StyleClasses<'a, Message> for T
where
    T: Into<iced::Element<'a, Message>>,
{
    fn style_classes<const N: usize>(self, classes: [&'static str; N]) -> iced::Element<'a, Message>
    where
        Self: dispatch::Rewritten,
    {
        let _ = classes;
        self.into()
    }
}

pub mod animation;
pub mod host;
pub mod paint;

pub mod resources;

pub mod fonts;
