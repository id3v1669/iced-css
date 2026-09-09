use super::{CaptureError, CaptureLimits, Captured, CapturedPaneContent, Construction};
use iced::{
    Length, Pixels,
    advanced::Renderer,
    widget::pane_grid::{Catalog, DragEvent, Pane, PaneGrid, ResizeEvent, State, Style, StyleFn},
};
use std::{cell::RefCell, num::NonZeroUsize};

pub type CapturedPaneGrid<'a, Message, Theme = iced::Theme, R = iced::Renderer> =
    Captured<PaneGrid<'a, Message, Theme, R>>;

/// Capture each eager native view result with its exact Pane identity.
/// The view returns an already admitted descriptor. Aggregate evidence failure
/// after native construction discards the payload without replaying the view.
pub fn pane_grid<'a, T, Message, Theme, R>(
    state: &'a State<T>,
    view: impl Fn(Pane, &'a T, bool) -> CapturedPaneContent<'a, Message, Theme, R>,
    mut limits: CaptureLimits,
) -> Result<CapturedPaneGrid<'a, Message, Theme, R>, CaptureError>
where
    Theme: Catalog,
    R: Renderer,
{
    let count = state.len();
    if count.checked_add(1).ok_or(CaptureError::Nodes)? > limits.nodes.get() {
        return Err(CaptureError::Nodes);
    }
    let cost = count.checked_mul(2).ok_or(CaptureError::Work)?;
    limits.work = limits
        .work
        .get()
        .checked_sub(cost)
        .and_then(NonZeroUsize::new)
        .ok_or(CaptureError::Work)?;
    let mut children = Vec::<Captured<Pane>>::new();
    let bytes = |capacity: usize| {
        capacity
            .checked_mul(size_of::<Captured<Pane>>())
            .filter(|n| *n <= limits.retained_bytes.get())
            .ok_or(CaptureError::Bytes)
    };
    bytes(count)?;
    children
        .try_reserve_exact(count)
        .map_err(|_| CaptureError::Allocation)?;
    bytes(children.capacity())?;
    let collected = RefCell::new((children, None));
    let payload = PaneGrid::new(state, |pane, data, maximized| {
        let Captured {
            payload,
            construction,
        } = view(pane, data, maximized);
        // No internal borrow is held while invoking application code.
        let mut collected = collected.borrow_mut();
        if collected.0.len() < count {
            collected.0.push(Captured {
                payload: pane,
                construction,
            });
        } else {
            collected.1 = Some(CaptureError::Nodes);
        }
        payload
    });
    let (children, error) = collected.into_inner();
    if let Some(error) = error {
        return Err(error);
    }
    if children.len() != count {
        return Err(CaptureError::Role);
    }
    let construction = Construction::pane_grid(children, limits)?;
    Ok(Captured {
        payload,
        construction,
    })
}

impl<'a, Message, Theme: Catalog, R: Renderer> Captured<PaneGrid<'a, Message, Theme, R>> {
    #[must_use]
    pub fn width(mut self, value: impl Into<Length>) -> Self {
        self.payload = self.payload.width(value);
        self
    }
    #[must_use]
    pub fn height(mut self, value: impl Into<Length>) -> Self {
        self.payload = self.payload.height(value);
        self
    }
    #[must_use]
    pub fn spacing(mut self, value: impl Into<Pixels>) -> Self {
        self.payload = self.payload.spacing(value);
        self
    }
    #[must_use]
    pub fn min_size(mut self, value: impl Into<Pixels>) -> Self {
        self.payload = self.payload.min_size(value);
        self
    }
    #[must_use]
    pub fn on_click(mut self, f: impl Fn(Pane) -> Message + 'a) -> Self {
        self.payload = self.payload.on_click(f);
        self
    }
    #[must_use]
    pub fn on_drag(mut self, f: impl Fn(DragEvent) -> Message + 'a) -> Self {
        self.payload = self.payload.on_drag(f);
        self
    }
    /// Preserve native conditional conversion: maximized grids do not convert leeway.
    #[must_use]
    pub fn on_resize(
        mut self,
        leeway: impl Into<Pixels>,
        f: impl Fn(ResizeEvent) -> Message + 'a,
    ) -> Self {
        self.payload = self.payload.on_resize(leeway, f);
        self
    }
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme) -> Style + 'a) -> Self
    where
        <Theme as Catalog>::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.payload = self.payload.style(style);
        self
    }
    #[must_use]
    pub fn class(mut self, class: impl Into<<Theme as Catalog>::Class<'a>>) -> Self {
        self.payload = self.payload.class(class);
        self
    }
}
