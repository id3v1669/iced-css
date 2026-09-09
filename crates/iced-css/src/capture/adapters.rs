//! The first explicitly typed built-in capture adapters.
use super::{CaptureError, CaptureLimits, Captured, Construction};
use iced::{
    Element,
    advanced::text::IntoFragment,
    widget::{Container, Text},
};

/// Capture an empty Space source; configuration remains on its native payload.
pub fn space(limits: CaptureLimits) -> Result<Captured<iced::widget::Space>, CaptureError> {
    let construction = Construction::space(limits)?;
    Ok(Captured {
        payload: iced::widget::Space::new(),
        construction,
    })
}

impl Captured<iced::widget::Space> {
    #[must_use]
    pub fn width(mut self, width: impl Into<iced::Length>) -> Self {
        self.payload = self.payload.width(width);
        self
    }
    #[must_use]
    pub fn height(mut self, height: impl Into<iced::Length>) -> Self {
        self.payload = self.payload.height(height);
        self
    }
}

/// Exact Column::from_vec recipe, retaining every supplied child and Shrink hints.
pub fn column_from_vec<'a, Message, Theme, Renderer>(
    children: Vec<super::CapturedElement<'a, Message, Theme, Renderer>>,
    limits: CaptureLimits,
) -> Result<Captured<super::ColumnRecipe<'a, Message, Theme, Renderer>>, CaptureError>
where
    Renderer: iced::advanced::Renderer,
{
    let (payloads, construction) = Construction::column(children, limits)?;
    Ok(Captured {
        payload: super::ColumnRecipe::from_vec(payloads),
        construction,
    })
}

/// Convert authored text once and keep an owned copy for source processing.
/// User conversion and native allocations are outside the evidence limits.
pub fn text<'a, Theme, Renderer>(
    value: impl IntoFragment<'a>,
    limits: CaptureLimits,
) -> Result<Captured<Text<'a, Theme, Renderer>>, CaptureError>
where
    Theme: iced::widget::text::Catalog,
    Renderer: iced::advanced::text::Renderer,
{
    let fragment = value.into_fragment();
    let construction = Construction::text(&fragment, limits)?;
    Ok(Captured {
        payload: Text::new(fragment),
        construction,
    })
}

/// Exact `Row::from_vec` recipe: retain all supplied children, without push's
/// native size-hint filtering or accumulation. This is not `with_children`.
pub fn row_from_vec<'a, Message, Theme, Renderer>(
    children: Vec<super::CapturedElement<'a, Message, Theme, Renderer>>,
    limits: CaptureLimits,
) -> Result<Captured<super::RowRecipe<'a, Message, Theme, Renderer>>, CaptureError>
where
    Renderer: iced::advanced::Renderer,
{
    let (payloads, construction) = Construction::row(children, limits)?;
    Ok(Captured {
        payload: super::RowRecipe::from_vec(payloads),
        construction,
    })
}

/// Consume a captured child; arbitrary native Elements do not satisfy this boundary.
pub fn container<'a, Message, Theme, Renderer, W>(
    child: Captured<W>,
    limits: CaptureLimits,
) -> Result<Captured<Container<'a, Message, Theme, Renderer>>, CaptureError>
where
    Theme: iced::widget::container::Catalog,
    Renderer: iced::advanced::Renderer,
    W: Into<Element<'a, Message, Theme, Renderer>>,
{
    let Captured {
        payload,
        construction,
    } = child;
    let construction = construction.container(limits)?;
    Ok(Captured {
        payload: Container::new(payload),
        construction,
    })
}

/// Capture the supplied Button content before native Element erasure.
pub fn button<'a, Message, Theme, Renderer, W>(
    child: Captured<W>,
    limits: CaptureLimits,
) -> Result<Captured<iced::widget::Button<'a, Message, Theme, Renderer>>, CaptureError>
where
    Theme: iced::widget::button::Catalog,
    Renderer: iced::advanced::Renderer,
    W: Into<Element<'a, Message, Theme, Renderer>>,
{
    let Captured {
        payload,
        construction,
    } = child;
    let construction = construction.button(limits)?;
    Ok(Captured {
        payload: iced::widget::Button::new(payload),
        construction,
    })
}

impl<'a, Message, Theme, Renderer> Captured<iced::widget::Button<'a, Message, Theme, Renderer>>
where
    Theme: iced::widget::button::Catalog,
    Renderer: iced::advanced::Renderer,
{
    /// Preserve iced's direct message storage and dispatch behavior.
    #[must_use]
    pub fn on_press(mut self, message: Message) -> Self {
        self.payload = self.payload.on_press(message);
        self
    }

    /// Store the producer without invoking it during capture.
    #[must_use]
    pub fn on_press_with(mut self, producer: impl Fn() -> Message + 'a) -> Self {
        self.payload = self.payload.on_press_with(producer);
        self
    }

    /// `None` disables the native Button without removing its source membership.
    #[must_use]
    pub fn on_press_maybe(mut self, message: Option<Message>) -> Self {
        self.payload = self.payload.on_press_maybe(message);
        self
    }
}

/// Exact Stack::from_vec recipe: retain every supplied payload, with native
/// Shrink defaults and base layer0. This does not call filtering push/extend.
///
/// ```compile_fail,E0599
/// # use iced_css::capture::{Captured, CapturedElement};
/// fn native_filter_bypass<'a>(
///     parent: Captured<iced::widget::Stack<'a, (), iced::Theme, ()>>,
///     child: CapturedElement<'a, (), iced::Theme, ()>,
/// ) {
///     let _ = parent.push(child);
/// }
/// ```
pub fn stack_from_vec<'a, Message, Theme, Renderer>(
    children: Vec<super::CapturedElement<'a, Message, Theme, Renderer>>,
    limits: CaptureLimits,
) -> Result<Captured<iced::widget::Stack<'a, Message, Theme, Renderer>>, CaptureError>
where
    Renderer: iced::advanced::Renderer,
{
    let (payloads, construction) = Construction::stack(children, limits)?;
    Ok(Captured {
        payload: iced::widget::Stack::from_vec(payloads),
        construction,
    })
}
impl<'a, Message, Theme, Renderer> Captured<iced::widget::Stack<'a, Message, Theme, Renderer>>
where
    Renderer: iced::advanced::Renderer,
{
    /// Insert a managed child below the current native base without changing it.
    /// Source order moves to the front; void children are retained.
    ///
    /// ```compile_fail,E0308
    /// # use iced_css::capture::{Captured, CaptureLimits};
    /// fn bypass<'a>(parent: Captured<iced::widget::Stack<'a, (), iced::Theme, ()>>,
    ///     child: iced::Element<'a, (), iced::Theme, ()>, limits: CaptureLimits) {
    ///     let _ = parent.push_under(child, limits);
    /// }
    /// ```
    ///
    /// # Errors
    /// Rejects aggregate evidence bounds/allocation before child conversion.
    pub fn push_under<W>(
        self,
        child: Captured<W>,
        limits: CaptureLimits,
    ) -> Result<Self, CaptureError>
    where
        W: Into<Element<'a, Message, Theme, Renderer>>,
    {
        let Captured {
            payload,
            construction,
        } = self;
        let Captured {
            payload: child,
            construction: child_construction,
        } = child;
        let construction = construction.prepend_ordered(child_construction, limits)?;
        Ok(Self {
            payload: payload.push_under(child),
            construction,
        })
    }
    #[must_use]
    pub fn width(mut self, width: impl Into<iced::Length>) -> Self {
        self.payload = self.payload.width(width);
        self
    }
    #[must_use]
    pub fn height(mut self, height: impl Into<iced::Length>) -> Self {
        self.payload = self.payload.height(height);
        self
    }
    #[must_use]
    pub fn clip(mut self, clip: bool) -> Self {
        self.payload = self.payload.clip(clip);
        self
    }
}

/// Retain every supplied Grid child before native construction.
pub fn grid_from_vec<'a, Message, Theme, Renderer>(
    children: Vec<super::CapturedElement<'a, Message, Theme, Renderer>>,
    limits: CaptureLimits,
) -> Result<Captured<iced::widget::Grid<'a, Message, Theme, Renderer>>, CaptureError>
where
    Renderer: iced::advanced::Renderer,
{
    let (payloads, construction) = Construction::grid(children, limits)?;
    Ok(Captured {
        payload: iced::widget::Grid::from_vec(payloads),
        construction,
    })
}
impl<'a, Message, Theme, Renderer> Captured<iced::widget::Grid<'a, Message, Theme, Renderer>>
where
    Renderer: iced::advanced::Renderer,
{
    #[must_use]
    pub fn width(mut self, width: impl Into<iced::Pixels>) -> Self {
        self.payload = self.payload.width(width);
        self
    }
    #[must_use]
    pub fn height(mut self, height: impl Into<iced::widget::grid::Sizing>) -> Self {
        self.payload = self.payload.height(height);
        self
    }
    #[must_use]
    pub fn spacing(mut self, spacing: impl Into<iced::Pixels>) -> Self {
        self.payload = self.payload.spacing(spacing);
        self
    }
    #[must_use]
    pub fn columns(mut self, columns: usize) -> Self {
        self.payload = self.payload.columns(columns);
        self
    }
    #[must_use]
    pub fn fluid(mut self, maximum: impl Into<iced::Pixels>) -> Self {
        self.payload = self.payload.fluid(maximum);
        self
    }
    /// Append a managed source child and its complete native payload. Grid's
    /// native insertion does not filter void hints; other families may differ.
    ///
    /// ```compile_fail,E0308
    /// # use iced_css::capture::{Captured, CaptureLimits};
    /// fn bypass<'a>(parent: Captured<iced::widget::Grid<'a, (), iced::Theme, ()>>,
    ///     child: iced::Element<'a, (), iced::Theme, ()>, limits: CaptureLimits) {
    ///     let _ = parent.push(child, limits);
    /// }
    /// ```
    ///
    /// # Errors
    /// Rejects aggregate evidence limits/allocation before native insertion.
    pub fn push<W>(self, child: Captured<W>, limits: CaptureLimits) -> Result<Self, CaptureError>
    where
        W: Into<Element<'a, Message, Theme, Renderer>>,
    {
        let Captured {
            payload,
            construction,
        } = self;
        let Captured {
            payload: child,
            construction: child_construction,
        } = child;
        let construction = construction.append_ordered(child_construction, limits)?;
        Ok(Self {
            payload: payload.push(child),
            construction,
        })
    }
}

impl<'a, Message, Theme, Renderer> Captured<iced::widget::Grid<'a, Message, Theme, Renderer>>
where
    Renderer: iced::advanced::Renderer,
{
    /// None supplies no source child and preserves storage. Its existing result
    /// still has to fit the requested evidence limits (one validation step).
    ///
    /// # Errors
    /// Rejects result limits or an admitted Some child's append failure.
    pub fn push_maybe<W>(
        self,
        child: Option<Captured<W>>,
        limits: CaptureLimits,
    ) -> Result<Self, CaptureError>
    where
        W: Into<Element<'a, Message, Theme, Renderer>>,
    {
        match child {
            Some(child) => self.push(child, limits),
            None => {
                self.construction.validate_limits(limits)?;
                Ok(self)
            }
        }
    }
    /// Consume one iterator, preserving yielded order and stopping at its first
    /// None. One work budget covers validation, next calls and all append visits.
    /// Work does not bound execution time inside application iterator callbacks.
    ///
    /// # Errors
    /// Rejects admission/work/allocation failures without a partial captured Grid.
    /// The iterator may have yielded a prefix before an input-dependent failure.
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

pub use super::scrollable::{scrollable, scrollable_with_direction};

pub use super::tooltip::tooltip;

/// Preserve base/top source roles before stock hover erases its private widget.
///
/// ```compile_fail,E0308
/// # use iced_css::capture::{CapturedElement, adapters, CaptureLimits};
/// fn bypass<'a>(base: CapturedElement<'a, (), iced::Theme, ()>,
///     top: iced::Element<'a, (), iced::Theme, ()>, limits: CaptureLimits) {
///     let _ = adapters::hover(base, top, limits);
/// }
/// ```
///
/// # Errors
/// Rejects aggregate evidence bounds/allocation before native child conversions.
pub fn hover<'a, Message: 'a, Theme: 'a, Renderer, A, B>(
    base: Captured<A>,
    top: Captured<B>,
    limits: CaptureLimits,
) -> Result<super::CapturedElement<'a, Message, Theme, Renderer>, CaptureError>
where
    Renderer: iced::advanced::Renderer + 'a,
    A: Into<Element<'a, Message, Theme, Renderer>>,
    B: Into<Element<'a, Message, Theme, Renderer>>,
{
    let Captured {
        payload: base,
        construction,
    } = base;
    let Captured {
        payload: top,
        construction: top_construction,
    } = top;
    let construction = construction.hover(top_construction, limits)?;
    Ok(Captured {
        payload: iced::widget::hover(base, top),
        construction,
    })
}

pub use super::pin::pin;

pub use super::mouse_area::mouse_area;

pub use super::float::float;

pub use super::themer::themer;

pub use super::sensor::sensor;

pub use super::rule::{horizontal_rule, vertical_rule};

pub use super::progress_bar::progress_bar;

/// Capture iced's event-opaque helper while retaining its managed source child.
///
/// This does not register custom replaced content.
///
/// ```compile_fail,E0308
/// # use iced_css::capture::{adapters, CaptureLimits};
/// fn bypass(child: iced::Element<'_, (), iced::Theme, ()>, limits: CaptureLimits) {
///     let _ = adapters::opaque(child, limits);
/// }
/// ```
///
/// # Errors
/// Rejects aggregate evidence bounds/allocation before native child conversion.
pub fn opaque<'a, Message: 'a, Theme: 'a, Renderer, W>(
    child: Captured<W>,
    limits: CaptureLimits,
) -> Result<super::CapturedElement<'a, Message, Theme, Renderer>, CaptureError>
where
    Renderer: iced::advanced::Renderer + 'a,
    W: Into<Element<'a, Message, Theme, Renderer>>,
{
    let Captured {
        payload,
        construction,
    } = child;
    let construction = construction.opaque(limits)?;
    Ok(Captured {
        payload: iced::widget::opaque(payload),
        construction,
    })
}

pub use super::checkbox::checkbox;
pub use super::pick_list::pick_list;
pub use super::radio::radio;
pub use super::sliders::{slider, vertical_slider};
pub use super::toggler::toggler;

pub use super::pane_controls::{pane_controls, pane_controls_dynamic};
