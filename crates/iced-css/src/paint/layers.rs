//! Effective native layer clips in a common, untransformed viewport space.
use iced::{Point, Rectangle, Size, advanced::Renderer};
pub use iced_css_core::formatting::clip::{ClipError, ClipRectangle, ClipRegion};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipLayerError {
    Viewport,
}
impl std::fmt::Display for ClipLayerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("clip viewport dimensions must be finite and nonnegative")
    }
}
impl std::error::Error for ClipLayerError {}

/// Frame-local effective clipping. Managed descendant layers use the scope passed
/// to the draw callback. Raw native layers and transformations bypass this contract.
#[derive(Debug, Clone, Copy)]
pub struct ClipScope {
    effective: ClipRegion,
}
impl ClipScope {
    pub fn for_viewport(size: Size) -> Result<Self, ClipLayerError> {
        if !size.width.is_finite()
            || !size.height.is_finite()
            || size.width < 0.0
            || size.height < 0.0
        {
            return Err(ClipLayerError::Viewport);
        }
        let effective = ClipRegion::from_edges([0.0, 0.0, size.width, size.height])
            .map_err(|_| ClipLayerError::Viewport)?;
        Ok(Self { effective })
    }
    pub fn effective_clip(self) -> ClipRegion {
        self.effective
    }
    /// Intersect before native layer creation. None means an empty intersection:
    /// no native layer is emitted and `draw` is never invoked.
    pub fn with_clip<R: Renderer, T>(
        &self,
        renderer: &mut R,
        requested: ClipRegion,
        draw: impl FnOnce(&mut R, &Self) -> T,
    ) -> Option<T> {
        let effective = self.effective.intersect(requested);
        let [left, top, right, bottom] = effective.edges()?;
        // The root viewport bounds these coordinates to [0, finite dimension].
        // Their differences are therefore finite even for enormous input clips.
        let bounds = Rectangle::new(Point::new(left, top), Size::new(right - left, bottom - top));
        let child = Self { effective };
        let mut result = None;
        renderer.with_layer(bounds, |renderer| {
            result = Some(draw(renderer, &child));
        });
        result
    }
}
