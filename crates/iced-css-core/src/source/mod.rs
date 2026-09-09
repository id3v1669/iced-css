//! Source metadata remains independent of CSS formatting and native state slots.
mod validate;
use crate::identity::InstanceId;
mod attributes;
pub use attributes::{AttributeError, AttributeLimits};
pub use validate::validate_tree;

#[derive(Debug, Clone, Default)]
pub struct Metadata {
    attributes: Vec<(String, String)>,
}
impl Metadata {
    pub fn set_attribute(&mut self, name: &str, value: &str) {
        if let Some((_, stored)) = self.attributes.iter_mut().find(|(key, _)| key == name) {
            *stored = value.to_owned();
        } else {
            self.attributes.push((name.to_owned(), value.to_owned()));
        }
    }
    pub fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }
    /// Validate all tokens before replacing the shared class attribute.
    pub fn set_classes(&mut self, classes: &[&str]) -> Result<(), String> {
        if classes
            .iter()
            .any(|c| c.is_empty() || c.chars().any(css_whitespace))
        {
            return Err("class tokens must be nonempty and contain no CSS whitespace".into());
        }
        self.set_attribute("class", &classes.join(" "));
        Ok(())
    }
    pub fn has_class(&self, class: &str) -> bool {
        !class.is_empty()
            && self
                .attribute("class")
                .is_some_and(|s| s.split(css_whitespace).any(|c| c == class))
    }
    pub fn set_id(&mut self, id: &str) {
        self.set_attribute("id", id)
    }
    pub fn css_id(&self) -> Option<&str> {
        self.attribute("id").filter(|s| !s.is_empty())
    }
    /// The caller passes the nearest inherited/document language, never host locale.
    pub fn language<'a>(&'a self, inherited: Option<&'a str>) -> Option<&'a str> {
        match self.attribute("lang") {
            Some("") => None,
            Some(lang) => Some(lang),
            None => inherited,
        }
    }
}
fn css_whitespace(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r' | '\u{000c}')
}

/// Parent and ordered children describe source membership, not layout order.
#[derive(Debug, Clone)]
pub struct SourceNode {
    pub instance: InstanceId,
    pub parent: Option<InstanceId>,
    pub children: Vec<InstanceId>,
    pub metadata: Metadata,
}
pub mod selector_tree;
pub mod sibling_matches;
