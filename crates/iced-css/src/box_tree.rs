//! A stylesheet-driven iced widget for captured box trees: real cascade, general block
//! flow, background and border paint, and captured text leaves wrapped at their used
//! width. Each text leaf is one block of left-aligned text with iced's default font and
//! line height. Inline layout, other captured payloads and input are not connected.
use crate::{
    animation::engine::ProgressLimits,
    capture::{CaptureLimits, CapturedElement},
    evaluation::{
        clearance_box_facts::ClearanceBoxFactsLimits,
        display_rules::{BundleLimits, DisplayRuleBundle, GraphBinding, GraphOrigin},
        source_display::{SourceDisplayLimits, evaluate_source_display},
        tree_layout::{
            BoxPaint, PreparedTree, TreeLayoutError, evaluate_tree_paint, prepare_captured_tree,
            prepare_tree_source_projection,
        },
    },
    host::source_publication::{InitialSourceHost, PublicationLimits},
};
use iced::advanced::{
    Layout, Widget, layout,
    renderer::{self, Quad},
    text::{self, Paragraph as _},
    widget::Tree,
};
use iced::{Color, Element, Length, Pixels, Point, Rectangle, Size, mouse};
use iced_css_core::{
    cascade::{
        rule::{RuleBatchLimits, RuleCascadeLimits, RuleContext},
        sheets::{AuthorSheet, AuthorSheets, Membership, SheetId, SheetLimits},
        ua_display::{UaDisplayCatalog, UaDisplayLimits},
    },
    formatting::{
        block_flow::{FlowContent, FlowGeometry, FlowLimits, FlowRect, FlowViewport},
        display::Position,
    },
    identity::SourceChangeLimits,
    selectors::{MatchLimits, SelectorLimits},
    source::selector_tree::SelectorTreeLimits,
    stylesheet::{
        RuleAssemblyLimits, StylesheetIntakeLimits, StylesheetLimits,
        imports::{LoadedGraphLimits, UnavailableImportPolicy},
        owned_graph::{OwnedGraphLimits, OwnedLoadedSheet, OwnedStylesheetGraph},
        parse_separate_unicode_stylesheet,
    },
    syntax::{
        RecoveryLimits, StringLimits,
        decode::DecodeLimits,
        encoding_selection::{EncodingMetadata, SelectionLimits},
        imports::{ImportLimits, ResourceId},
        unicode_intake::IntakeLimits,
    },
    values::{NumericLimits, color::ColorLimits},
};
use std::num::{NonZeroU64, NonZeroUsize};

/// What building a box tree can refuse. Each cause keeps its own message.
#[derive(Debug)]
pub struct BoxTreeError(String);
impl std::fmt::Display for BoxTreeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "box tree: {}", self.0)
    }
}
impl std::error::Error for BoxTreeError {}
fn failed(error: impl std::fmt::Debug) -> BoxTreeError {
    BoxTreeError(format!("{error:?}"))
}

fn nz(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).expect("limit constants are positive")
}
const COUNT: usize = 100_000;
const BYTES: usize = 64_000_000;
const WORK: usize = 200_000_000;

fn big() -> NonZeroUsize {
    nz(COUNT)
}
fn numeric() -> NumericLimits {
    NumericLimits {
        work: big(),
        digits: big(),
    }
}
fn colors() -> ColorLimits {
    ColorLimits {
        syntax: StringLimits {
            work: big(),
            retained_bytes: big(),
        },
        numeric: numeric(),
    }
}
fn selectors() -> SelectorTreeLimits {
    SelectorTreeLimits {
        entries: big(),
        retained_bytes: nz(BYTES),
        work: nz(WORK),
    }
}
fn per_target() -> RuleBatchLimits {
    RuleBatchLimits {
        rules: big(),
        per_rule: RuleCascadeLimits {
            declarations: big(),
            matching: MatchLimits {
                work: nz(10_000),
                retained_bytes: big(),
            },
        },
    }
}
fn facts() -> ClearanceBoxFactsLimits {
    ClearanceBoxFactsLimits {
        work: nz(WORK),
        rule_visits: nz(WORK),
        matching_work: nz(2_000_000_000),
        declaration_offers: nz(WORK),
        membership_work: nz(WORK),
        numeric_work: nz(WORK),
        selectors: selectors(),
        per_target: per_target(),
        numeric: numeric(),
    }
}
fn parse(css: &str) -> Result<OwnedStylesheetGraph, BoxTreeError> {
    let context = RuleContext {
        first_order: 0,
        first_declaration: NonZeroU64::MIN,
    };
    let candidate = parse_separate_unicode_stylesheet(
        css.as_bytes(),
        EncodingMetadata {
            transport: None,
            linking: None,
            referring: None,
        },
        context,
        StylesheetIntakeLimits {
            intake: IntakeLimits {
                selection: SelectionLimits {
                    registry_work: big(),
                    prefix_work: big(),
                },
                decode: DecodeLimits {
                    entries: big(),
                    retained_bytes: nz(BYTES),
                    work: nz(WORK),
                },
                charset_work: big(),
            },
            sheet: StylesheetLimits {
                rules: RuleAssemblyLimits {
                    recovery: RecoveryLimits {
                        work: nz(WORK),
                        retained_bytes: nz(BYTES),
                        nesting: big(),
                    },
                    selectors: SelectorLimits {
                        work: nz(WORK),
                        retained_bytes: nz(BYTES),
                        entries: big(),
                    },
                    values: colors(),
                    entries: big(),
                    record_bytes: nz(BYTES),
                    work: nz(WORK),
                    name_bytes: big(),
                },
                statements: big(),
                record_bytes: nz(BYTES),
                work: nz(WORK),
            },
        },
    )
    .map_err(failed)?;
    OwnedStylesheetGraph::new(
        vec![OwnedLoadedSheet {
            resource: ResourceId::new(NonZeroU64::MIN),
            candidate,
            imports: vec![],
        }],
        0,
        // Author rules order after every user agent rule.
        RuleContext {
            first_order: 100_000,
            first_declaration: NonZeroU64::new(100_000).expect("positive"),
        },
        OwnedGraphLimits {
            record_bytes: nz(BYTES),
            work: nz(WORK),
            expansion: LoadedGraphLimits {
                unavailable: UnavailableImportPolicy::Record,
                entries: big(),
                record_bytes: nz(BYTES),
                work: nz(WORK),
                tracking: ImportLimits {
                    entries: big(),
                    retained_bytes: nz(BYTES),
                    work: nz(WORK),
                    occurrences: NonZeroU64::new(100).expect("positive"),
                },
            },
        },
    )
    .map_err(failed)
}

/// Default capture limits for assembling a tree with the capture adapters.
#[must_use]
pub fn capture_limits() -> CaptureLimits {
    CaptureLimits {
        nodes: nz(COUNT),
        depth: nz(2_000),
        retained_bytes: nz(BYTES),
        work: nz(WORK),
    }
}

/// An owned, width-independent styled tree. One `BoxTree` can lay out at any size.
#[derive(Debug, Clone, PartialEq)]
pub struct BoxTree {
    prepared: PreparedTree,
    paints: Vec<BoxPaint>,
    /// Captured text per arena slot; set only on leaves.
    texts: Vec<Option<String>>,
}

impl BoxTree {
    /// Cascades `css` over a captured tree. Only the capture evidence is kept.
    ///
    /// # Errors
    /// Returns a stylesheet, capture, cascade or unsupported-display failure.
    pub fn new<Message, Theme, Renderer>(
        root: CapturedElement<'_, Message, Theme, Renderer>,
        css: &str,
    ) -> Result<Self, BoxTreeError> {
        let (_, construction) = root.into_parts();
        let count = construction.nodes().len();
        let mut host = InitialSourceHost::new(
            NonZeroU64::new(COUNT as u64).expect("positive"),
            ProgressLimits {
                addresses: NonZeroU64::new(COUNT as u64).expect("positive"),
                bindings: big(),
                tracks: big(),
                retained_bytes: nz(BYTES),
                work: nz(WORK),
            },
        );
        host.prepare(
            construction,
            vec![Default::default(); count],
            None,
            PublicationLimits {
                nodes: big(),
                snapshot_bytes: nz(BYTES),
                work: nz(WORK),
                selectors: selectors(),
                identities: SourceChangeLimits {
                    entries: big(),
                    retained_bytes: nz(BYTES),
                    work: nz(WORK),
                },
            },
        )
        .map_err(failed)?
        .commit();
        let snapshot = host
            .snapshot()
            .ok_or_else(|| BoxTreeError("snapshot".to_owned()))?;
        let projection = prepare_tree_source_projection(&host, snapshot, big()).map_err(failed)?;
        let catalog = UaDisplayCatalog::new(UaDisplayLimits {
            rules: big(),
            retained_bytes: nz(BYTES),
            work: nz(WORK),
            values: colors(),
        })
        .map_err(failed)?;
        let graph = parse(css)?;
        let sheet = SheetId::new(NonZeroU64::MIN);
        let sheets = AuthorSheets::new(
            vec![AuthorSheet {
                id: sheet,
                enabled: true,
                membership: Membership::Persistent,
            }],
            vec![],
            true,
            None,
            // This bound is itself charged per membership query, so keep it small.
            SheetLimits {
                entries: big(),
                retained_bytes: nz(BYTES),
                work: nz(100),
            },
        )
        .map_err(failed)?;
        let bindings = [GraphBinding {
            graph: &graph,
            origin: GraphOrigin::Author(sheet),
        }];
        let bundle = DisplayRuleBundle::new(
            &catalog,
            &bindings,
            &sheets,
            BundleLimits {
                graphs: big(),
                rules: big(),
                retained_bytes: nz(BYTES),
                work: nz(WORK),
                membership_work: nz(WORK),
            },
        )
        .map_err(failed)?;
        let display = evaluate_source_display(
            snapshot,
            &bundle,
            SourceDisplayLimits {
                nodes: big(),
                retained_bytes: nz(BYTES),
                work: nz(WORK),
                rule_visits: nz(WORK),
                matching_work: nz(2_000_000_000),
                declaration_offers: nz(WORK),
                membership_work: nz(WORK),
                selectors: selectors(),
                per_target: per_target(),
            },
        )
        .map_err(failed)?;
        let prepared = prepare_captured_tree(&projection, &display, facts(), Some(16.0), |_| None)
            .map_err(failed)?;
        let paints = evaluate_tree_paint(&projection, &display, facts()).map_err(failed)?;
        let nodes = snapshot.construction().nodes();
        let texts = prepared
            .positions
            .iter()
            .map(|&(projected, _)| {
                projection
                    .original_index(projected)
                    .and_then(|original| nodes[original].text())
                    .map(str::to_owned)
            })
            .collect();
        Ok(Self {
            prepared,
            paints,
            texts,
        })
    }

    /// Geometry and paint by projected (preorder) index; `None` is display none.
    ///
    /// # Errors
    /// Returns a layout failure at this size.
    pub fn layout(
        &self,
        width: f32,
        height: Option<f32>,
    ) -> Result<Vec<Option<FlowGeometry>>, TreeLayoutError> {
        self.prepared.layout(
            FlowViewport {
                left: 0.0,
                top: 0.0,
                width,
                height,
            },
            FlowLimits {
                boxes: big(),
                depth: nz(2_000),
            },
        )
    }

    /// Lays out with `measure(text, font_size, width)` giving each text leaf's height.
    ///
    /// # Errors
    /// Returns a layout failure at this size.
    pub fn layout_text(
        &self,
        width: f32,
        height: Option<f32>,
        measure: &mut dyn FnMut(&str, f32, f32) -> f32,
    ) -> Result<Vec<Option<FlowGeometry>>, TreeLayoutError> {
        self.prepared.layout_measured(
            FlowViewport {
                left: 0.0,
                top: 0.0,
                width,
                height,
            },
            FlowLimits {
                boxes: big(),
                depth: nz(2_000),
            },
            &mut |arena, used| {
                let text = self.texts[arena].as_deref()?;
                Some(measure(text, self.prepared.font_sizes[arena], used))
            },
        )
    }

    /// Supplies each text leaf's (narrowest, longest) width for shrink-to-fit.
    pub fn set_text_widths(&mut self, mut widths: impl FnMut(&str, f32) -> (f32, f32)) {
        for (arena, text) in self.texts.iter().enumerate() {
            if let Some(text) = text {
                let (minimum, preferred) = widths(text, self.prepared.font_sizes[arena]);
                self.prepared.boxes[arena].content = Some(FlowContent {
                    height: 0.0,
                    preferred_width: preferred,
                    preferred_min_width: minimum,
                });
            }
        }
    }

    /// Paint by projected index.
    #[must_use]
    pub fn paints(&self) -> &[BoxPaint] {
        &self.paints
    }

    /// Projected indices in 9.9 paint order: flow blocks, then floats, then positioned
    /// boxes, each with the descendants that belong to them.
    #[must_use]
    pub fn paint_order(&self) -> Vec<usize> {
        let prepared = &self.prepared;
        let mut order = Vec::with_capacity(prepared.boxes.len());
        let mut positioned = Vec::new();
        if !prepared.boxes.is_empty() {
            paint_context(prepared, 0, &mut positioned, &mut order);
        }
        // A positioned box found while painting one is appended and handled in turn.
        let mut next = 0;
        while next < positioned.len() {
            paint_context(prepared, positioned[next], &mut positioned, &mut order);
            next += 1;
        }
        order
            .into_iter()
            .map(|arena| prepared.positions[arena].0)
            .collect()
    }

    /// Wraps the tree as an iced element that fills the size it is given.
    pub fn view<'a, Message: 'a, Theme: 'a, Renderer>(self) -> Element<'a, Message, Theme, Renderer>
    where
        Renderer: text::Renderer + 'a,
        Renderer::Font: 'a,
    {
        Element::new(BoxTreeWidget::<Renderer> {
            tree: self,
            measured: false,
            geometry: Vec::new(),
            paragraphs: Vec::new(),
        })
    }
}

/// One 9.9 layer: this box and its in-flow blocks, then its floats, each as a unit.
/// Positioned descendants go to the caller's list.
fn paint_context(
    prepared: &PreparedTree,
    root: usize,
    positioned: &mut Vec<usize>,
    order: &mut Vec<usize>,
) {
    let mut floats = Vec::new();
    let mut stack = vec![root];
    while let Some(arena) = stack.pop() {
        order.push(arena);
        for &child in prepared.boxes[arena].children.iter().rev() {
            if prepared.positions[child].1 != Position::Static {
                continue;
            }
            if prepared.boxes[child].float.is_some() {
                continue;
            }
            stack.push(child);
        }
        // Children in source order, whatever the stack order above.
        for &child in &prepared.boxes[arena].children {
            if prepared.positions[child].1 != Position::Static {
                positioned.push(child);
            } else if prepared.boxes[child].float.is_some() {
                floats.push(child);
            }
        }
    }
    for float in floats {
        paint_context(prepared, float, positioned, order);
    }
}

struct BoxTreeWidget<Renderer: text::Renderer> {
    tree: BoxTree,
    measured: bool,
    geometry: Vec<Option<FlowGeometry>>,
    /// The projected index, the shaped text and its color, one per text leaf.
    paragraphs: Vec<(usize, Renderer::Paragraph, Color)>,
}

fn paragraph<Renderer: text::Renderer>(
    renderer: &Renderer,
    content: &str,
    size: f32,
    width: f32,
) -> Renderer::Paragraph {
    Renderer::Paragraph::with_text(text::Text {
        content,
        bounds: Size::new(width, f32::INFINITY),
        size: Pixels(size),
        line_height: text::LineHeight::default(),
        font: renderer.default_font(),
        align_x: text::Alignment::Default,
        align_y: iced::alignment::Vertical::Top,
        shaping: text::Shaping::Advanced,
        wrapping: text::Wrapping::Word,
    })
}

/// Backgrounds and border bands in paint order, offset to the layout origin.
fn draw_boxes<Renderer: renderer::Renderer>(
    tree: &BoxTree,
    geometry: &[Option<FlowGeometry>],
    renderer: &mut Renderer,
    origin: Point,
) {
    let at = |rect: Rectangle| Rectangle {
        x: rect.x + origin.x,
        y: rect.y + origin.y,
        ..rect
    };
    let quad = |renderer: &mut Renderer, bounds: Rectangle, color: [f32; 3]| {
        if bounds.width > 0.0 && bounds.height > 0.0 {
            renderer.fill_quad(
                Quad {
                    bounds: at(bounds),
                    ..Quad::default()
                },
                Color::from_rgb(color[0], color[1], color[2]),
            );
        }
    };
    for index in tree.paint_order() {
        let Some(Some(geometry)) = geometry.get(index) else {
            continue;
        };
        let paint = tree.paints[index];
        let border = geometry.border;
        if let Some(color) = paint.background {
            // 14.2: the background extends under the border.
            quad(
                renderer,
                rectangle(border.left, border.right, border.top, border.bottom),
                color,
            );
        }
        let bands = border_bands(border, geometry.padding);
        for (band, color) in bands.into_iter().zip(paint.border) {
            if let Some(color) = color {
                quad(renderer, band, color);
            }
        }
    }
}

fn rectangle(left: f32, right: f32, top: f32, bottom: f32) -> Rectangle {
    Rectangle {
        x: left,
        y: top,
        width: (right - left).max(0.0),
        height: (bottom - top).max(0.0),
    }
}

/// The four border edge bands, top, right, bottom, left. Sides may differ in color.
fn border_bands(border: FlowRect, padding: FlowRect) -> [Rectangle; 4] {
    [
        rectangle(border.left, border.right, border.top, padding.top),
        rectangle(padding.right, border.right, padding.top, padding.bottom),
        rectangle(border.left, border.right, padding.bottom, border.bottom),
        rectangle(border.left, padding.left, padding.top, padding.bottom),
    ]
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for BoxTreeWidget<Renderer>
where
    Renderer: text::Renderer,
{
    fn size(&self) -> Size<Length> {
        Size {
            width: Length::Fill,
            height: Length::Fill,
        }
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let available = limits.max();
        if !self.measured {
            self.measured = true;
            self.tree.set_text_widths(|content, size| {
                let longest = paragraph(renderer, content, size, f32::INFINITY)
                    .min_bounds()
                    .width;
                let narrowest = content
                    .split_whitespace()
                    .map(|word| {
                        paragraph(renderer, word, size, f32::INFINITY)
                            .min_bounds()
                            .width
                    })
                    .fold(0.0_f32, f32::max);
                (narrowest, longest)
            });
        }
        // A refused layout draws nothing; `BoxTree::layout_text` reports the cause.
        self.geometry = self
            .tree
            .layout_text(
                available.width,
                Some(available.height),
                &mut |content, size, width| {
                    paragraph(renderer, content, size, width)
                        .min_bounds()
                        .height
                },
            )
            .unwrap_or_default();
        self.paragraphs.clear();
        for (arena, content) in self.tree.texts.iter().enumerate() {
            let Some(content) = content else { continue };
            let projected = self.tree.prepared.positions[arena].0;
            let Some(Some(geometry)) = self.geometry.get(projected) else {
                continue;
            };
            let box_width = geometry.content.right - geometry.content.left;
            let color = self.tree.paints[projected]
                .color
                .map_or(Color::BLACK, |c| Color::from_rgb(c[0], c[1], c[2]));
            self.paragraphs.push((
                projected,
                paragraph(
                    renderer,
                    content,
                    self.tree.prepared.font_sizes[arena],
                    box_width,
                ),
                color,
            ));
        }
        layout::Node::new(available)
    }

    fn draw(
        &self,
        _tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let origin = bounds.position();
        renderer.with_layer(bounds, |renderer| {
            draw_boxes(&self.tree, &self.geometry, renderer, origin);
            for (projected, shaped, color) in &self.paragraphs {
                if let Some(Some(geometry)) = self.geometry.get(*projected) {
                    let position = Point::new(
                        origin.x + geometry.content.left,
                        origin.y + geometry.content.top,
                    );
                    renderer.fill_paragraph(shaped, position, *color, bounds);
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::adapters;
    use iced::{Background, Theme, Transformation};
    use iced_css_core::source::AttributeLimits;

    #[derive(Default)]
    struct Recording(Vec<(Rectangle, Color)>);
    impl renderer::Renderer for Recording {
        fn start_layer(&mut self, _: Rectangle) {}
        fn end_layer(&mut self) {}
        fn start_transformation(&mut self, _: Transformation) {}
        fn end_transformation(&mut self) {}
        fn fill_quad(&mut self, quad: Quad, background: impl Into<Background>) {
            let Background::Color(color) = background.into() else {
                panic!("box paint is a solid color");
            };
            self.0.push((quad.bounds, color));
        }
        fn allocate_image(
            &mut self,
            _: &iced::advanced::image::Handle,
            _: impl FnOnce(Result<iced::advanced::image::Allocation, iced::advanced::image::Error>)
            + Send
            + 'static,
        ) {
        }
        fn reset(&mut self, _: Rectangle) {}
    }

    fn node(
        id: &str,
        children: Vec<CapturedElement<'static, (), Theme, Recording>>,
    ) -> CapturedElement<'static, (), Theme, Recording> {
        let bound = nz(1000);
        adapters::column_from_vec(children, capture_limits())
            .unwrap()
            .style_id(
                id,
                AttributeLimits {
                    entries: bound,
                    retained_bytes: bound,
                    work: bound,
                },
            )
            .unwrap()
            .erase()
    }

    #[test]
    fn draw_emits_backgrounds_then_border_bands_at_the_layout_origin() {
        let root = node("a", vec![node("b", vec![])]);
        let tree = BoxTree::new(
            root,
            "#a { background-color:#ff0000; height:50px; }
             #b { width:20px; height:10px; border-top:2px solid #0000ff; }",
        )
        .unwrap();
        let geometry = tree.layout(100.0, Some(80.0)).unwrap();
        let mut renderer = Recording::default();
        draw_boxes(&tree, &geometry, &mut renderer, Point::new(5.0, 7.0));
        assert_eq!(
            renderer.0,
            vec![
                (
                    Rectangle::new(Point::new(5.0, 7.0), Size::new(100.0, 50.0)),
                    Color::from_rgb(1.0, 0.0, 0.0)
                ),
                (
                    Rectangle::new(Point::new(5.0, 7.0), Size::new(20.0, 2.0)),
                    Color::from_rgb(0.0, 0.0, 1.0)
                ),
            ]
        );
    }
}
