//! Resource handles and authored native hints; no decoding or intrinsic sizing.
use super::{CaptureError, CaptureLimits, Construction, ConstructionNode, Kind};
use iced::{ContentFit, Length, Rotation};

#[cfg(feature = "image-without-codecs")]
/// Stock image resource plus authored hints; shared bytes are not charged.
#[derive(Debug, Clone)]
pub struct ImageSource {
    handle: iced::widget::image::Handle,
    hints: ImageHints,
}
#[cfg(feature = "image-without-codecs")]
impl ImageSource {
    pub fn handle(&self) -> &iced::widget::image::Handle {
        &self.handle
    }
    pub fn hints(&self) -> ImageHints {
        self.hints
    }
    /// A declaration check only, not a decoder result or intrinsic dimensions.
    pub fn rgba_declaration(&self) -> Option<RgbaDeclaration> {
        match &self.handle {
            iced::widget::image::Handle::Rgba {
                width,
                height,
                pixels,
                ..
            } => Some(RgbaDeclaration {
                width: *width,
                height: *height,
                pixel_bytes: pixels.len(),
                buffer_status: rgba_status(*width, *height, pixels.len()),
            }),
            _ => None,
        }
    }
    pub(super) fn owned_bytes(&self) -> usize {
        match &self.handle {
            iced::widget::image::Handle::Path(_, path) => path.capacity(),
            _ => 0,
        }
    }
}
#[cfg(feature = "image-without-codecs")]
/// Native defaults and last authored builder values, not computed CSS.
#[derive(Debug, Clone, Copy)]
pub struct ImageHints {
    pub width: Length,
    pub height: Length,
    pub expand: bool,
    pub content_fit: ContentFit,
    pub filter_method: iced::widget::image::FilterMethod,
    pub rotation: Rotation,
    pub opacity: f32,
    pub scale: f32,
    pub crop: Option<iced::Rectangle<u32>>,
    pub border_radius: iced::border::Radius,
}
#[cfg(feature = "image-without-codecs")]
impl Default for ImageHints {
    fn default() -> Self {
        Self {
            width: Length::Shrink,
            height: Length::Shrink,
            expand: false,
            content_fit: ContentFit::default(),
            filter_method: Default::default(),
            rotation: Rotation::default(),
            opacity: 1.0,
            scale: 1.0,
            crop: None,
            border_radius: Default::default(),
        }
    }
}
#[cfg(feature = "image-without-codecs")]
/// Checked declared RGBA dimensions versus supplied byte length.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RgbaBufferStatus {
    ExactLength,
    LengthMismatch,
    RequiredBytesOverflow,
}
#[cfg(feature = "image-without-codecs")]
/// Raw RGBA handle facts; dimensions have not been validated by a renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RgbaDeclaration {
    width: u32,
    height: u32,
    pixel_bytes: usize,
    buffer_status: RgbaBufferStatus,
}
#[cfg(feature = "image-without-codecs")]
impl RgbaDeclaration {
    pub fn width(self) -> u32 {
        self.width
    }
    pub fn height(self) -> u32 {
        self.height
    }
    pub fn pixel_bytes(self) -> usize {
        self.pixel_bytes
    }
    pub fn buffer_status(self) -> RgbaBufferStatus {
        self.buffer_status
    }
}
#[cfg(feature = "image-without-codecs")]
fn rgba_status(width: u32, height: u32, bytes: usize) -> RgbaBufferStatus {
    match usize::try_from(width)
        .ok()
        .and_then(|w| usize::try_from(height).ok().and_then(|h| w.checked_mul(h)))
        .and_then(|n| n.checked_mul(4))
    {
        Some(n) if n == bytes => RgbaBufferStatus::ExactLength,
        Some(_) => RgbaBufferStatus::LengthMismatch,
        None => RgbaBufferStatus::RequiredBytesOverflow,
    }
}
#[cfg(feature = "svg")]
/// Stock SVG resource plus authored hints; shared Arc backing is not charged.
#[derive(Debug, Clone)]
pub struct SvgSource {
    handle: iced::widget::svg::Handle,
    hints: SvgHints,
}
#[cfg(feature = "svg")]
impl SvgSource {
    pub fn handle(&self) -> &iced::widget::svg::Handle {
        &self.handle
    }
    pub fn hints(&self) -> SvgHints {
        self.hints
    }
}
#[cfg(feature = "svg")]
/// Native defaults and last authored builder values, not computed CSS.
#[derive(Debug, Clone, Copy)]
pub struct SvgHints {
    pub width: Length,
    pub height: Length,
    pub content_fit: ContentFit,
    pub rotation: Rotation,
    pub opacity: f32,
    pub style_selection: SvgStyleSelection,
}
#[cfg(feature = "svg")]
impl Default for SvgHints {
    fn default() -> Self {
        Self {
            width: Length::Fill,
            height: Length::Shrink,
            content_fit: ContentFit::Contain,
            rotation: Rotation::default(),
            opacity: 1.0,
            style_selection: SvgStyleSelection::Default,
        }
    }
}
#[cfg(feature = "svg")]
/// Which native class-setting route was used last; stores no callback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SvgStyleSelection {
    Default,
    Style,
    Class,
}

impl ConstructionNode {
    #[cfg(feature = "image-without-codecs")]
    pub fn image_source(&self) -> Option<&ImageSource> {
        match &self.kind {
            Kind::Image(source) => Some(source),
            _ => None,
        }
    }
    #[cfg(feature = "svg")]
    pub fn svg_source(&self) -> Option<&SvgSource> {
        match &self.kind {
            Kind::Svg(source) => Some(source),
            _ => None,
        }
    }
}
impl Construction {
    #[cfg(feature = "image-without-codecs")]
    pub(in crate::capture) fn image(
        handle: iced::widget::image::Handle,
        limits: CaptureLimits,
    ) -> Result<Self, CaptureError> {
        let source = ImageSource {
            handle,
            hints: ImageHints::default(),
        };
        let owned = source.owned_bytes();
        Self::leaf_with_owned(Kind::Image(source), owned, limits)
    }
    #[cfg(feature = "svg")]
    pub(in crate::capture) fn svg(
        handle: iced::widget::svg::Handle,
        limits: CaptureLimits,
    ) -> Result<Self, CaptureError> {
        Self::leaf(
            Kind::Svg(SvgSource {
                handle,
                hints: SvgHints::default(),
            }),
            limits,
        )
    }
    #[cfg(feature = "image-without-codecs")]
    pub(in crate::capture) fn image_hints_mut(&mut self) -> &mut ImageHints {
        let root = self.root();
        match &mut self.nodes[root].kind {
            Kind::Image(source) => &mut source.hints,
            _ => unreachable!("typed image capture"),
        }
    }
    #[cfg(feature = "svg")]
    pub(in crate::capture) fn svg_hints_mut(&mut self) -> &mut SvgHints {
        let root = self.root();
        match &mut self.nodes[root].kind {
            Kind::Svg(source) => &mut source.hints,
            _ => unreachable!("typed SVG capture"),
        }
    }
}
#[cfg(all(test, feature = "image-without-codecs"))]
mod tests {
    use super::*;
    use std::{mem::size_of, num::NonZeroUsize, path::PathBuf};
    fn limits() -> CaptureLimits {
        let n = NonZeroUsize::new(100000).unwrap();
        CaptureLimits {
            nodes: n,
            depth: n,
            retained_bytes: n,
            work: n,
        }
    }
    fn path() -> (iced::widget::image::Handle, usize) {
        let mut p = PathBuf::from("absent");
        p.reserve(4096);
        let cap = p.capacity();
        (iced::widget::image::Handle::from_path(p), cap)
    }
    #[test]
    fn actual_path_capacity_is_admitted_and_overflow_is_rejected() {
        let (h, cap) = path();
        let c = Construction::image(h, limits()).unwrap();
        assert_eq!(c.owned_bytes, cap);
        assert_eq!(
            c.retained_bytes(),
            c.nodes.capacity() * size_of::<ConstructionNode>() + cap
        );
        let (h, cap) = path();
        let mut l = limits();
        l.retained_bytes = NonZeroUsize::new(size_of::<ConstructionNode>() + cap - 1).unwrap();
        assert!(matches!(
            Construction::image(h, l),
            Err(CaptureError::Bytes)
        ));
        assert!(matches!(
            Construction::leaf_with_owned(Kind::Space, usize::MAX, limits()),
            Err(CaptureError::Bytes)
        ));
    }
    #[test]
    fn pane_title_replacement_keeps_mandatory_image_path_capacity() {
        let (h, cap) = path();
        let c = Construction::image(h, limits())
            .unwrap()
            .pane_content(limits())
            .unwrap();
        let title = Construction::space(limits())
            .unwrap()
            .pane_title(limits())
            .unwrap();
        let c = c.replace_pane_role(title, limits()).unwrap();
        assert_eq!(c.owned_bytes, cap);
        let title = Construction::space(limits())
            .unwrap()
            .pane_title(limits())
            .unwrap();
        let c = c.replace_pane_role(title, limits()).unwrap();
        assert_eq!(c.owned_bytes, cap);
        assert_eq!(
            c.retained_bytes(),
            c.nodes.capacity() * size_of::<ConstructionNode>() + cap
        );
        assert_eq!(
            c.nodes
                .iter()
                .filter(|n| n.image_source().is_some())
                .count(),
            1
        );
        let title = Construction::space(limits())
            .unwrap()
            .pane_title(limits())
            .unwrap();
        let mut l = limits();
        l.retained_bytes =
            NonZeroUsize::new(c.nodes.len() * size_of::<ConstructionNode>() + cap - 1).unwrap();
        assert!(matches!(
            c.replace_pane_role(title, l),
            Err(CaptureError::Bytes)
        ));
    }
    #[test]
    fn single_parent_keeps_owned_path_and_bytes_backing_is_excluded() {
        let (h, cap) = path();
        let c = Construction::image(h, limits())
            .unwrap()
            .button(limits())
            .unwrap();
        assert_eq!(c.owned_bytes, cap);
        let h = iced::widget::image::Handle::from_rgba(1, 1, vec![0; 200000]);
        let c = Construction::image(h, limits()).unwrap();
        assert_eq!(c.owned_bytes, 0);
        assert_eq!(
            c.retained_bytes(),
            c.nodes.capacity() * size_of::<ConstructionNode>()
        );
    }

    #[test]
    fn pair_and_ordered_parents_keep_path_capacity() {
        let (h, cap) = path();
        let c = Construction::image(h, limits()).unwrap();
        let c = c
            .hover(Construction::space(limits()).unwrap(), limits())
            .unwrap();
        assert_eq!(c.owned_bytes, cap);
        let child = crate::capture::Captured {
            payload: (),
            construction: c,
        };
        let (_, c) = Construction::row(vec![child], limits()).unwrap();
        assert_eq!(c.owned_bytes, cap);
        assert_eq!(
            c.retained_bytes(),
            c.nodes.capacity() * size_of::<ConstructionNode>() + cap
        );
        let mut l = limits();
        l.retained_bytes = NonZeroUsize::new(c.retained_bytes()).unwrap();
        assert_eq!(
            super::super::check_bytes(c.nodes.capacity(), cap, l),
            Ok(())
        );
        l.retained_bytes = NonZeroUsize::new(c.retained_bytes() - 1).unwrap();
        assert_eq!(
            super::super::check_bytes(c.nodes.capacity(), cap, l),
            Err(CaptureError::Bytes)
        );
    }
}
