#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Length {
    Px(f32),
    Percent(f32),
    Auto,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MarginValue {
    Px(f32),
    Auto,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Margins {
    pub top: MarginValue,
    pub right: MarginValue,
    pub bottom: MarginValue,
    pub left: MarginValue,
}

impl Default for Margins {
    fn default() -> Self {
        Margins {
            top: MarginValue::Px(0.0),
            right: MarginValue::Px(0.0),
            bottom: MarginValue::Px(0.0),
            left: MarginValue::Px(0.0),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Resolved {
    pub width: Option<Length>,
    pub height: Option<Length>,
    pub min_width: Option<Length>,
    pub max_width: Option<Length>,
    pub margin: Option<Margins>,
}

impl crate::Stylesheet {
    pub fn resolve(&self, classes: &[&str]) -> Resolved {
        use crate::parse::Property;

        let mut resolved = Resolved::default();

        for rule in &self.rules {
            if !classes.contains(&rule.class.as_str()) {
                continue;
            }
            for declaration in &rule.declarations {
                match *declaration {
                    Property::Width(v) => resolved.width = Some(v),
                    Property::Height(v) => resolved.height = Some(v),
                    Property::MinWidth(v) => resolved.min_width = Some(v),
                    Property::MaxWidth(v) => resolved.max_width = Some(v),
                    Property::Margin(m) => resolved.margin = Some(m),
                    Property::MarginTop(v) => {
                        resolved.margin.get_or_insert_with(Margins::default).top = v;
                    }
                    Property::MarginRight(v) => {
                        resolved.margin.get_or_insert_with(Margins::default).right = v;
                    }
                    Property::MarginBottom(v) => {
                        resolved.margin.get_or_insert_with(Margins::default).bottom = v;
                    }
                    Property::MarginLeft(v) => {
                        resolved.margin.get_or_insert_with(Margins::default).left = v;
                    }
                    Property::Unsupported(_) => {}
                }
            }
        }

        resolved
    }

    pub fn has_class(&self, class: &str) -> bool {
        self.rules.iter().any(|rule| rule.class == class)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    #[test]
    fn later_rule_wins() {
        let sheet = parse(".wide { width: 10px; } .base { width: 20px; height: 30px; }").unwrap();
        let resolved = sheet.resolve(&["wide", "base"]);
        assert_eq!(resolved.width, Some(Length::Px(20.0)));
        assert_eq!(resolved.height, Some(Length::Px(30.0)));
    }

    #[test]
    fn class_list_order_irrelevant() {
        let sheet = parse(".wide { width: 10px; } .base { width: 20px; }").unwrap();
        assert_eq!(
            sheet.resolve(&["base", "wide"]),
            sheet.resolve(&["wide", "base"])
        );
    }

    #[test]
    fn merge_distinct_properties() {
        let sheet = parse(".w { width: 10px; } .h { height: 20px; }").unwrap();
        let resolved = sheet.resolve(&["w", "h"]);
        assert_eq!(resolved.width, Some(Length::Px(10.0)));
        assert_eq!(resolved.height, Some(Length::Px(20.0)));
    }

    #[test]
    fn unlisted_classes_ignored() {
        let sheet = parse(".a { width: 10px; } .b { width: 99px; }").unwrap();
        let resolved = sheet.resolve(&["a"]);
        assert_eq!(resolved.width, Some(Length::Px(10.0)));
    }

    #[test]
    fn margin_longhand_over_shorthand() {
        let sheet = parse(".m { margin: 10px; margin-left: 40px; }").unwrap();
        let resolved = sheet.resolve(&["m"]);
        let m = resolved.margin.unwrap();
        assert_eq!(m.top, MarginValue::Px(10.0));
        assert_eq!(m.left, MarginValue::Px(40.0));
    }

    #[test]
    fn margin_longhand_alone_zeroes_rest() {
        let sheet = parse(".m { margin-top: 5px; }").unwrap();
        let m = sheet.resolve(&["m"]).margin.unwrap();
        assert_eq!(m.top, MarginValue::Px(5.0));
        assert_eq!(m.right, MarginValue::Px(0.0));
        assert_eq!(m.bottom, MarginValue::Px(0.0));
        assert_eq!(m.left, MarginValue::Px(0.0));
    }

    #[test]
    fn has_class() {
        let sheet = parse(".a { width: 10px; }").unwrap();
        assert!(sheet.has_class("a"));
        assert!(!sheet.has_class("b"));
    }
}
