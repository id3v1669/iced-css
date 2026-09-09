use super::{CaptureError, CaptureLimits, Captured};
use iced::{
    Element,
    advanced::Renderer,
    widget::{container, pane_grid::Controls},
};

/// Structured controls descriptor, not an independently erasable widget.
pub type CapturedPaneControls<'a, Message, Theme = iced::Theme, R = iced::Renderer> =
    Captured<Controls<'a, Message, Theme, R>>;

/// Capture the supplied full controls before native element conversion.
/// Aggregate evidence admission precedes child conversions.
pub fn pane_controls<'a, Message, Theme, R, W>(
    full: Captured<W>,
    limits: CaptureLimits,
) -> Result<CapturedPaneControls<'a, Message, Theme, R>, CaptureError>
where
    Theme: container::Catalog,
    R: Renderer,
    W: Into<Element<'a, Message, Theme, R>>,
{
    let Captured {
        payload,
        construction,
    } = full;
    let construction = construction.pane_controls(limits)?;
    Ok(Captured {
        payload: Controls::new(payload),
        construction,
    })
}

/// Capture full then compact controls, including the visually hidden variant.
/// Aggregate evidence admission precedes both native child conversions.
pub fn pane_controls_dynamic<'a, Message, Theme, R, A, B>(
    full: Captured<A>,
    compact: Captured<B>,
    limits: CaptureLimits,
) -> Result<CapturedPaneControls<'a, Message, Theme, R>, CaptureError>
where
    Theme: container::Catalog,
    R: Renderer,
    A: Into<Element<'a, Message, Theme, R>>,
    B: Into<Element<'a, Message, Theme, R>>,
{
    let Captured {
        payload: full,
        construction,
    } = full;
    let Captured {
        payload: compact,
        construction: compact_construction,
    } = compact;
    let construction = construction.pane_controls_dynamic(compact_construction, limits)?;
    Ok(Captured {
        payload: Controls::dynamic(full, compact),
        construction,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, num::NonZeroUsize};
    struct Convert<'a>(&'a RefCell<Vec<&'static str>>, &'static str);
    impl<'a> From<Convert<'a>> for Element<'a, (), iced::Theme, ()> {
        fn from(value: Convert<'a>) -> Self {
            value.0.borrow_mut().push(value.1);
            iced::widget::Space::new().into()
        }
    }
    #[test]
    fn native_conversion_order_and_preconversion_limit_failure() {
        let n = NonZeroUsize::new(10000).unwrap();
        let limits = CaptureLimits {
            nodes: n,
            depth: n,
            retained_bytes: n,
            work: n,
        };
        let log = RefCell::new(Vec::new());
        let child = |name| Captured {
            payload: Convert(&log, name),
            construction: super::super::Construction::space(limits).unwrap(),
        };
        let _ = pane_controls_dynamic(child("full"), child("compact"), limits).unwrap();
        assert_eq!(*log.borrow(), ["full", "compact"]);
        log.borrow_mut().clear();
        let _ = Controls::dynamic(Convert(&log, "full"), Convert(&log, "compact"));
        assert_eq!(*log.borrow(), ["full", "compact"]);
        log.borrow_mut().clear();
        let mut restricted = limits;
        restricted.nodes = NonZeroUsize::MIN;
        assert!(matches!(
            pane_controls_dynamic(child("full"), child("compact"), restricted),
            Err(CaptureError::Nodes)
        ));
        assert!(log.borrow().is_empty());
        let _ = pane_controls(child("full"), limits).unwrap();
        assert_eq!(*log.borrow(), ["full"]);
    }
}
