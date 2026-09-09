//! CSS22 9.7 computed display/float and box participation, before tree generation.
use crate::values::property::{ParsedProperty, SpecifiedValue};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisplayError {
    pub property: &'static str,
}
impl std::fmt::Display for DisplayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} requires its resolved CSS keyword", self.property)
    }
}
impl std::error::Error for DisplayError {}
fn keyword(parsed: &ParsedProperty, property: &'static str) -> Result<&'static str, DisplayError> {
    if parsed.property.descriptor().name == property
        && let SpecifiedValue::Keyword(keyword) = parsed.value
    {
        return Ok(keyword.as_str());
    }
    Err(DisplayError { property })
}
macro_rules! keywords {
    ($name:ident, $property:literal, {$($variant:ident => $text:literal),+ $(,)?}) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum $name { $($variant),+ }
        impl $name {
            pub(crate) fn from_keyword(text: &str) -> Result<Self, DisplayError> {
                match text {
                    $($text => Ok(Self::$variant),)+
                    _ => Err(DisplayError { property: $property }),
                }
            }
            pub(crate) fn from_resolved(value: &SpecifiedValue) -> Result<Self, DisplayError> {
                if let SpecifiedValue::Keyword(keyword) = value {
                    Self::from_keyword(keyword.as_str())
                } else { Err(DisplayError { property: $property }) }
            }
        }
        impl TryFrom<&ParsedProperty> for $name {
            type Error = DisplayError;
            fn try_from(parsed: &ParsedProperty) -> Result<Self, Self::Error> {
                Self::from_keyword(keyword(parsed, $property)?)
            }
        }
    };
}

keywords!(Display, "display", {
    Inline => "inline", Block => "block", ListItem => "list-item",
    InlineBlock => "inline-block", Table => "table", InlineTable => "inline-table",
    TableRowGroup => "table-row-group", TableHeaderGroup => "table-header-group",
    TableFooterGroup => "table-footer-group", TableRow => "table-row",
    TableColumnGroup => "table-column-group", TableColumn => "table-column",
    TableCell => "table-cell", TableCaption => "table-caption", None => "none",
});
keywords!(Position, "position", { Static => "static", Relative => "relative", Absolute => "absolute", Fixed => "fixed" });
keywords!(Float, "float", { None => "none", Left => "left", Right => "right" });
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flow {
    Suppressed,
    AbsolutelyPositioned,
    Floating,
    Normal,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisplayResolution {
    pub display: Display,
    pub position: Position,
    pub float: Float,
    pub flow: Flow,
}
impl Display {
    fn transformed(self) -> Self {
        match self {
            Self::InlineTable => Self::Table,
            Self::Inline
            | Self::InlineBlock
            | Self::TableRowGroup
            | Self::TableHeaderGroup
            | Self::TableFooterGroup
            | Self::TableRow
            | Self::TableColumnGroup
            | Self::TableColumn
            | Self::TableCell
            | Self::TableCaption => Self::Block,
            // DP09 preserves root list-item under CSS22's explicit choice.
            Self::Block | Self::ListItem | Self::Table | Self::None => self,
        }
    }
}
pub fn resolve_display(
    display: Display,
    position: Position,
    float: Float,
    is_root: bool,
) -> DisplayResolution {
    if display == Display::None {
        return DisplayResolution {
            display,
            position,
            float,
            flow: Flow::Suppressed,
        };
    }
    let (display, float, flow) = if matches!(position, Position::Absolute | Position::Fixed) {
        (
            display.transformed(),
            Float::None,
            Flow::AbsolutelyPositioned,
        )
    } else if float != Float::None {
        (display.transformed(), float, Flow::Floating)
    } else {
        (
            if is_root {
                display.transformed()
            } else {
                display
            },
            float,
            Flow::Normal,
        )
    };
    DisplayResolution {
        display,
        position,
        float,
        flow,
    }
}
