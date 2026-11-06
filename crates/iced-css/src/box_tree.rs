//! A stylesheet-driven iced widget for captured box trees: real cascade, general block
//! flow, background and border paint. It draws boxes only: the captured payload (text,
//! controls) and inline layout are not connected yet, and input is not routed.
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
    widget::Tree,
};
use iced::{Color, Element, Length, Rectangle, Size, mouse};
use iced_css_core::{
    cascade::{
        rule::{RuleBatchLimits, RuleCascadeLimits, RuleContext},
        sheets::{AuthorSheet, AuthorSheets, Membership, SheetId, SheetLimits},
        ua_display::{UaDisplayCatalog, UaDisplayLimits},
    },
    formatting::{
        block_flow::{FlowGeometry, FlowLimits, FlowRect, FlowViewport},
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
        Ok(Self { prepared, paints })
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
        let mut layer = vec![0_u8; prepared.boxes.len()];
        for arena in 0..prepared.boxes.len() {
            if prepared.boxes[arena].float.is_some() {
                layer[arena] = layer[arena].max(1);
            }
            if prepared.positions[arena].1 != Position::Static {
                layer[arena] = 2;
            }
            for &child in &prepared.boxes[arena].children {
                layer[child] = layer[arena];
            }
        }
        let mut order: Vec<usize> = (0..prepared.boxes.len()).collect();
        order.sort_by_key(|&arena| layer[arena]);
        order
            .into_iter()
            .map(|arena| prepared.positions[arena].0)
            .collect()
    }

    /// Wraps the tree as an iced element that fills the width it is given.
    pub fn view<'a, Message: 'a, Theme: 'a, Renderer>(self) -> Element<'a, Message, Theme, Renderer>
    where
        Renderer: renderer::Renderer + 'a,
    {
        Element::new(BoxTreeWidget {
            tree: self,
            geometry: Vec::new(),
        })
    }
}

struct BoxTreeWidget {
    tree: BoxTree,
    geometry: Vec<Option<FlowGeometry>>,
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

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for BoxTreeWidget
where
    Renderer: renderer::Renderer,
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
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let available = limits.max();
        // A refused layout draws nothing; `BoxTree::layout` reports the cause.
        self.geometry = self
            .tree
            .layout(available.width, Some(available.height))
            .unwrap_or_default();
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
        let origin = layout.bounds().position();
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
        for index in self.tree.paint_order() {
            let Some(Some(geometry)) = self.geometry.get(index) else {
                continue;
            };
            let paint = self.tree.paints[index];
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::adapters;
    use iced::advanced::Renderer as _;
    use iced::{Background, Point, Theme, Transformation};
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
        let mut widget = BoxTreeWidget {
            tree,
            geometry: Vec::new(),
        };
        let mut renderer = Recording::default();
        let mut state = Tree::empty();
        let node = Widget::<(), Theme, Recording>::layout(
            &mut widget,
            &mut state,
            &renderer,
            &layout::Limits::new(Size::ZERO, Size::new(100.0, 80.0)),
        );
        let moved = node.move_to(Point::new(5.0, 7.0));
        Widget::<(), Theme, Recording>::draw(
            &widget,
            &state,
            &mut renderer,
            &Theme::Light,
            &renderer::Style::default(),
            Layout::new(&moved),
            mouse::Cursor::Unavailable,
            &Rectangle::new(Point::ORIGIN, Size::new(100.0, 80.0)),
        );
        renderer.reset(Rectangle::default());
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
