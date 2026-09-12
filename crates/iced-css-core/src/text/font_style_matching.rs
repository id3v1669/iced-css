//! CSS2.2 style-label selection within one tentative font family.
//!
//! This stage consumes classified availability, not font bytes or native names.
use crate::animation::style::FontStyleKeyword;

/// Supplied eligible CSS style labels for one tentative family.
///
/// The caller must establish the labels' classification, namespace and
/// eligibility before variant/weight/size matching. This value does not validate
/// those facts. Default is empty; typed insertion is idempotent.
///
/// A supplied Oblique label may come from qualified synthesis. This set does
/// not synthesize Oblique from Normal availability or create any font face.
/// Storage is fixed; building it from a catalogue requires separately bounded
/// traversal by the caller.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AvailableFontStyles(u8);

fn bit(style: FontStyleKeyword) -> u8 {
    match style {
        FontStyleKeyword::Normal => 1,
        FontStyleKeyword::Italic => 2,
        FontStyleKeyword::Oblique => 4,
    }
}

impl AvailableFontStyles {
    /// Insert one already-classified label, preserving all existing labels.
    /// Repeated insertion has no further effect; no allocation occurs.
    pub fn insert(&mut self, style: FontStyleKeyword) {
        self.0 |= bit(style);
    }

    /// Test supplied label membership with one fixed mapping and bit operation.
    pub fn contains(self, style: FontStyleKeyword) -> bool {
        self.0 & bit(style) != 0
    }

    /// Select an available CSS2.2 label for an already-resolved baseline request.
    ///
    /// Normal and Oblique require exact labels. Italic prefers Italic and then
    /// accepts Oblique. At most two membership probes occur; no allocation,
    /// candidate traversal or external I/O occurs.
    ///
    /// `None` means no match at this available-label stage. It does not exhaust
    /// synthesis or family/default-family retry, require a missing glyph or
    /// dropped text, or indicate host failure. A selected label is not a face
    /// or coverage result: variant, weight, size, duplicate-face precedence and
    /// coverage retain their separate matching stages.
    ///
    /// Source inheritance, system-font roles and defaults must resolve before
    /// this call. Animated presentation angles remain separate from baseline
    /// labels; this method performs no angle-to-keyword coercion or synthesis.
    /// Existing keyword-to-animation endpoint behavior is unchanged.
    ///
    /// ```compile_fail,E0308
    /// use iced_css_core::{
    ///     animation::style::FontStyleValue,
    ///     text::font_style_matching::AvailableFontStyles,
    /// };
    /// let available = AvailableFontStyles::default();
    /// let _ = available.select(FontStyleValue::ObliqueDegrees(7.0));
    /// ```
    pub fn select(self, desired: FontStyleKeyword) -> Option<FontStyleKeyword> {
        if self.contains(desired) {
            Some(desired)
        } else if desired == FontStyleKeyword::Italic && self.contains(FontStyleKeyword::Oblique) {
            Some(FontStyleKeyword::Oblique)
        } else {
            None
        }
    }
}
