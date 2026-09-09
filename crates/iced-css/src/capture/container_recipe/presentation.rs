//! Already-resolved presentation supplied by the host after native class evaluation.
use iced::{Background, Border, Color, widget::container::Style};

/// Raw host overrides, independently of the original authored native class.
/// `None` preserves a native field; `Some(None)` clears an optional native field.
/// This is not CSS parsing, cascade selection, or generation validation.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct ContainerPresentation {
    pub background: Option<Option<Background>>,
    pub text_color: Option<Option<Color>>,
    pub border: Option<Border>,
}
impl ContainerPresentation {
    pub(super) fn apply(self, mut native: Style) -> Style {
        if let Some(background) = self.background {
            native.background = background;
        }
        if let Some(text_color) = self.text_color {
            native.text_color = text_color;
        }
        if let Some(border) = self.border {
            native.border = border;
        }
        native
    }
}
