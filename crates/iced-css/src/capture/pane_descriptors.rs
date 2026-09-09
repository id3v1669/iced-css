use super::{CaptureError, CaptureLimits, Captured, CapturedPaneControls};
use iced::{
    Element, Padding,
    advanced::Renderer,
    widget::{
        container::{Catalog, Style, StyleFn},
        pane_grid::{Content, TitleBar},
    },
};

pub type CapturedPaneTitle<'a, Message, Theme = iced::Theme, R = iced::Renderer> =
    Captured<TitleBar<'a, Message, Theme, R>>;
pub type CapturedPaneContent<'a, Message, Theme = iced::Theme, R = iced::Renderer> =
    Captured<Content<'a, Message, Theme, R>>;

/// Capture title content before native conversion. Controls are initially absent.
pub fn pane_title<'a, Message, Theme, R, W>(
    content: Captured<W>,
    limits: CaptureLimits,
) -> Result<CapturedPaneTitle<'a, Message, Theme, R>, CaptureError>
where
    Theme: Catalog,
    R: Renderer,
    W: Into<Element<'a, Message, Theme, R>>,
{
    let Captured {
        payload,
        construction,
    } = content;
    let construction = construction.pane_title(limits)?;
    Ok(Captured {
        payload: TitleBar::new(payload),
        construction,
    })
}

/// Capture body content before native conversion. The title is initially absent.
pub fn pane_content<'a, Message, Theme, R, W>(
    body: Captured<W>,
    limits: CaptureLimits,
) -> Result<CapturedPaneContent<'a, Message, Theme, R>, CaptureError>
where
    Theme: Catalog,
    R: Renderer,
    W: Into<Element<'a, Message, Theme, R>>,
{
    let Captured {
        payload,
        construction,
    } = body;
    let construction = construction.pane_content(limits)?;
    Ok(Captured {
        payload: Content::new(payload),
        construction,
    })
}

impl<'a, Message, Theme: Catalog, R: Renderer> Captured<TitleBar<'a, Message, Theme, R>> {
    /// Replace the optional controls subtree; wrap widget content with pane_controls first.
    pub fn controls(
        self,
        controls: CapturedPaneControls<'a, Message, Theme, R>,
        limits: CaptureLimits,
    ) -> Result<Self, CaptureError> {
        let Captured {
            payload,
            construction,
        } = self;
        let Captured {
            payload: controls,
            construction: child,
        } = controls;
        let construction = construction.replace_pane_role(child, limits)?;
        Ok(Captured {
            payload: payload.controls(controls),
            construction,
        })
    }
    #[must_use]
    pub fn padding(mut self, value: impl Into<Padding>) -> Self {
        self.payload = self.payload.padding(value);
        self
    }
    #[must_use]
    pub fn always_show_controls(mut self) -> Self {
        self.payload = self.payload.always_show_controls();
        self
    }
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.payload = self.payload.style(style);
        self
    }
    #[must_use]
    pub fn class(mut self, class: impl Into<Theme::Class<'a>>) -> Self {
        self.payload = self.payload.class(class);
        self
    }
}
impl<'a, Message, Theme: Catalog, R: Renderer> Captured<Content<'a, Message, Theme, R>> {
    /// Replace the optional title subtree, ordered before the retained body in source.
    pub fn title_bar(
        self,
        title: CapturedPaneTitle<'a, Message, Theme, R>,
        limits: CaptureLimits,
    ) -> Result<Self, CaptureError> {
        let Captured {
            payload,
            construction,
        } = self;
        let Captured {
            payload: title,
            construction: child,
        } = title;
        let construction = construction.replace_pane_role(child, limits)?;
        Ok(Captured {
            payload: payload.title_bar(title),
            construction,
        })
    }
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.payload = self.payload.style(style);
        self
    }
    #[must_use]
    pub fn class(mut self, class: impl Into<Theme::Class<'a>>) -> Self {
        self.payload = self.payload.class(class);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, num::NonZeroUsize};
    struct Convert<'a>(&'a Cell<usize>);
    impl<'a> From<Convert<'a>> for Element<'a, (), iced::Theme, ()> {
        fn from(v: Convert<'a>) -> Self {
            v.0.set(v.0.get() + 1);
            iced::widget::Space::new().into()
        }
    }
    #[test]
    fn descriptor_constructors_convert_once_after_evidence_admission() {
        let n = NonZeroUsize::new(10000).unwrap();
        let limits = CaptureLimits {
            nodes: n,
            depth: n,
            retained_bytes: n,
            work: n,
        };
        let count = Cell::new(0);
        let child = || Captured {
            payload: Convert(&count),
            construction: super::super::Construction::space(limits).unwrap(),
        };
        let _ = pane_title(child(), limits).unwrap();
        let _ = pane_content(child(), limits).unwrap();
        assert_eq!(count.get(), 2);
        let mut cap = limits;
        cap.nodes = NonZeroUsize::MIN;
        assert!(matches!(pane_title(child(), cap), Err(CaptureError::Nodes)));
        assert!(matches!(
            pane_content(child(), cap),
            Err(CaptureError::Nodes)
        ));
        assert_eq!(count.get(), 2);
    }
}
