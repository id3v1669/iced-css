use iced_css::{
    box_tree::{BoxTree, capture_limits},
    capture::{CapturedElement, adapters},
};
use iced_css_core::source::AttributeLimits;
use std::num::NonZeroUsize;

type Element = CapturedElement<'static, (), iced::Theme, ()>;

fn node(id: &str, children: Vec<Element>) -> Element {
    let bound = NonZeroUsize::new(1000).unwrap();
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
fn text(id: &str, content: &'static str) -> Element {
    let bound = NonZeroUsize::new(1000).unwrap();
    adapters::text(content, capture_limits())
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
fn text2(id: &str) -> Element {
    text(id, "words")
}
fn page() -> Element {
    node(
        "page",
        vec![
            node("side", vec![]),
            node("main", vec![node("card", vec![])]),
            node("note", vec![]),
        ],
    )
}
const CSS: &str = "
    #page { background-color:#102030; padding:10px; }
    #side { float:left; width:25%; height:100px; background-color:rgb(255,0,0); }
    #main { overflow:hidden; color:#00ff00; border:2px solid; }
    #card { height:40px; margin:5px; background-color:rgb(0%,50%,100%); }
    #note { position:absolute; right:0; top:0; width:10px; height:10px;
            border:1px solid #0000ff; border-top-color:transparent; }
";

#[test]
fn stylesheet_drives_geometry_at_any_width() {
    let tree = BoxTree::new(page(), CSS).unwrap();
    let wide = tree.layout(400.0, Some(300.0)).unwrap();
    let side = wide[1].unwrap();
    assert_eq!((side.border.left, side.border.right), (10.0, 105.0));
    let main = wide[2].unwrap();
    assert_eq!((main.border.left, main.border.right), (105.0, 390.0));
    assert_eq!(main.border.bottom, 10.0 + 2.0 + 5.0 + 40.0 + 5.0 + 2.0);
    let card = wide[3].unwrap();
    assert_eq!((card.border.left, card.border.top), (112.0, 17.0));
    let note = wide[4].unwrap();
    assert_eq!((note.border.left, note.border.right), (388.0, 400.0));
    // Percentages follow the width with no rebuild.
    let narrow = tree.layout(200.0, Some(300.0)).unwrap();
    assert_eq!(narrow[1].unwrap().border.right, 55.0);
}

#[test]
fn paint_cascades_explicit_inherited_current_and_transparent_colors() {
    let tree = BoxTree::new(page(), CSS).unwrap();
    let paints = tree.paints();
    let page_color = paints[0].background.unwrap();
    assert!((page_color[0] - 16.0 / 255.0).abs() < 1e-6);
    assert_eq!(paints[1].background, Some([1.0, 0.0, 0.0]));
    // Unspecified background is transparent.
    assert_eq!(paints[2].background, None);
    // Border initial is currentColor, here the box's own `color`.
    assert_eq!(paints[2].border[0], Some([0.0, 1.0, 0.0]));
    assert_eq!(paints[3].background, Some([0.0, 0.5, 1.0]));
    assert_eq!(paints[4].border[1], Some([0.0, 0.0, 1.0]));
    assert_eq!(paints[4].border[0], None);
}

#[test]
fn inherited_color_reaches_a_descendants_current_color_border() {
    let tree = BoxTree::new(
        page(),
        "#main { color:#00ff00; } #card { border:1px solid; height:1px; }",
    )
    .unwrap();
    assert_eq!(tree.paints()[3].border[2], Some([0.0, 1.0, 0.0]));
}

#[test]
fn paint_order_is_flow_then_float_then_positioned() {
    let tree = BoxTree::new(page(), CSS).unwrap();
    assert_eq!(tree.paint_order(), vec![0, 2, 3, 1, 4]);
}

#[test]
fn unsupported_display_is_reported() {
    let error = BoxTree::new(page(), "#note { display:table; }").unwrap_err();
    assert!(error.to_string().contains("display"));
}

#[test]
fn inside_a_positioned_box_blocks_paint_before_floats() {
    let tree = BoxTree::new(
        node(
            "page",
            vec![node(
                "main",
                vec![node("side", vec![]), node("card", vec![])],
            )],
        ),
        "#main { position:relative; }
         #side { float:left; width:10px; height:10px; }
         #card { height:10px; }",
    )
    .unwrap();
    assert_eq!(tree.paint_order(), vec![0, 1, 3, 2]);
}

#[test]
fn inherit_reads_the_parent_same_property_color() {
    let tree = BoxTree::new(
        page(),
        "#main { background-color:#ff0000; border:2px solid #00ff00; }
         #card { background-color:inherit; color:#0000ff;
                 border:2px solid; border-top-color:inherit; }",
    )
    .unwrap();
    let card = tree.paints()[3];
    assert_eq!(card.background, Some([1.0, 0.0, 0.0]));
    assert_eq!(card.border[0], Some([0.0, 1.0, 0.0]));
    assert_eq!(card.border[1], Some([0.0, 0.0, 1.0]));
}

#[test]
fn out_of_range_percentages_clip() {
    let tree = BoxTree::new(
        page(),
        "#card { background-color:rgb(150%,-20%,99999999999999999999999999999999999999999999%); }",
    )
    .unwrap();
    assert_eq!(tree.paints()[3].background, Some([1.0, 0.0, 1.0]));
}

#[test]
fn text_leaves_wrap_at_their_used_width_and_drive_box_heights() {
    let root = node(
        "page",
        vec![text("copy", "one two three four"), node("after", vec![])],
    );
    let tree = BoxTree::new(
        root,
        "#page { padding:0 10px; }
         #copy { font-size:20px; color:#ff0000; margin:7px; padding:9px; }
         #after { height:1px; }",
    )
    .unwrap();
    assert_eq!(tree.paints()[1].color, Some([1.0, 0.0, 0.0]));
    let mut seen = Vec::new();
    // A fake shaper: one 20px line per 40px of width used, at the requested size.
    let g = tree
        .layout_text(100.0, None, &mut |leaf, width| {
            let size = leaf.runs[0].size;
            seen.push((leaf.text(), size, width));
            size * (360.0 / width).ceil()
        })
        .unwrap();
    assert_eq!(seen, vec![("one two three four".to_owned(), 20.0, 80.0)]);
    assert_eq!(g[1].unwrap().content.bottom, 100.0);
    assert_eq!(g[2].unwrap().border.top, 100.0);
}

#[test]
fn block_text_keeps_its_box_properties() {
    let root = node("page", vec![text("copy", "words")]);
    let tree = BoxTree::new(
        root,
        "#copy { display:block; margin-top:7px; padding:9px; width:50px; }",
    )
    .unwrap();
    let g = tree
        .layout_text(100.0, None, &mut |_, width| {
            assert_eq!(width, 50.0);
            20.0
        })
        .unwrap();
    assert_eq!(g[1].unwrap().content.top, 16.0);
    assert_eq!(g[1].unwrap().border.bottom, 45.0);
}

#[test]
fn positioned_boxes_paint_in_tree_order_with_their_descendants() {
    let tree = BoxTree::new(
        node(
            "page",
            vec![
                node("first", vec![node("inner", vec![])]),
                node("second", vec![]),
            ],
        ),
        "#first, #inner, #second { position:absolute; }",
    )
    .unwrap();
    assert_eq!(tree.paint_order(), vec![0, 1, 2, 3]);
    let tree = BoxTree::new(
        node(
            "page",
            vec![
                node("wrap", vec![node("inner", vec![])]),
                node("second", vec![]),
            ],
        ),
        "#inner, #second { position:absolute; }",
    )
    .unwrap();
    assert_eq!(tree.paint_order(), vec![0, 1, 2, 3]);
}

#[test]
fn text_measures_at_a_whole_pixel_and_empty_text_takes_no_line() {
    let root = node("page", vec![text("empty", "  "), text2("copy")]);
    let tree = BoxTree::new(root, "#copy { width:33.3px; display:block; }").unwrap();
    let mut widths = Vec::new();
    let g = tree
        .layout_text(100.0, None, &mut |leaf, width| {
            widths.push((leaf.text(), width));
            10.0
        })
        .unwrap();
    assert_eq!(widths, vec![("words".to_owned(), 34.0)]);
    assert_eq!(g[1].unwrap().border.bottom, 0.0);
    assert_eq!(g[2].unwrap().border.bottom, 10.0);
}

#[test]
fn inline_text_box_paint_is_dropped_but_its_color_stays() {
    let root = node("page", vec![text("copy", "words")]);
    let tree = BoxTree::new(
        root,
        "#copy { background-color:#ff0000; color:#00ff00; border:1px solid; }",
    )
    .unwrap();
    assert_eq!(tree.paints()[1].background, None);
    assert_eq!(tree.paints()[1].border[0], None);
    assert_eq!(tree.paints()[1].color, Some([0.0, 1.0, 0.0]));
    // CSS 2.2 `color` does not accept transparent: the declaration is dropped.
    let root = node("page", vec![text("copy", "words")]);
    let tree = BoxTree::new(root, "#copy { color:transparent; }").unwrap();
    assert_eq!(tree.paints()[1].color, Some([0.0, 0.0, 0.0]));
}

#[test]
fn adjacent_inline_siblings_share_one_paragraph_of_styled_runs() {
    let root = node(
        "page",
        vec![
            text("first", "  Hello   "),
            node("span", vec![text("second", " big  world ")]),
            text("third", " tail "),
            node("block", vec![]),
            text("fourth", "after"),
        ],
    );
    let tree = BoxTree::new(
        root,
        "#span { display:inline; background-color:#ffff00; font-weight:bold; font-style:italic;
                 font-family:Georgia, serif; text-decoration:underline; }
         #second { font-size:32px; color:#ff0000; }
         #third { text-transform:uppercase; }
         #page { text-align:center; line-height:1.5; }
         #block { height:3px; }",
    )
    .unwrap();
    let leaves = tree.text_leaves();
    assert_eq!(leaves.len(), 2);
    let leaf = leaves[0].1;
    assert_eq!(leaf.text(), "Hello big world TAIL");
    assert_eq!(leaf.align, "center");
    assert_eq!(
        leaf.line_height,
        iced_css::evaluation::tree_layout::TextLineHeight::Multiplier(1.5)
    );
    let big = &leaf.runs[1];
    assert_eq!(big.size, 32.0);
    assert_eq!(big.color, Some([1.0, 0.0, 0.0]));
    assert_eq!(big.background, Some([1.0, 1.0, 0.0]));
    assert_eq!(big.weight, 700);
    assert!(big.italic && big.underline);
    assert_eq!(big.family.as_deref(), Some("Georgia"));
    assert_eq!(leaf.runs[0].background, None);
    assert_eq!(leaf.runs[0].weight, 400);
    assert_eq!(leaves[1].1.text(), "after");
    let g = tree.layout_text(100.0, None, &mut |_, _| 20.0).unwrap();
    // Group, inline element and its text all report the anonymous block.
    assert_eq!(g[1].unwrap().border.bottom, 20.0);
    assert_eq!(g[2], g[1]);
    assert_eq!(g[3], g[1]);
    assert_eq!(g[4], g[1]);
    // The block after the group, at projected index 5.
    assert_eq!(g[5].unwrap().border.top, 20.0);
}

#[test]
fn white_space_modes_and_float_inside_a_run() {
    let root = node(
        "page",
        vec![
            text("one", "a  b\n c"),
            node("side", vec![]),
            text("two", " tail"),
        ],
    );
    let tree = BoxTree::new(
        root,
        "#one { white-space:pre; } #side { float:right; width:10px; height:50px; }",
    )
    .unwrap();
    let leaves = tree.text_leaves();
    assert_eq!(leaves.len(), 1);
    assert_eq!(leaves[0].1.text(), "a  b\n c tail");
    assert!(!leaves[0].1.wraps);
    let g = tree.layout_text(100.0, None, &mut |_, _| 20.0).unwrap();
    // The float is hoisted before the run: same top, and the run yields room.
    assert_eq!(g[2].unwrap().border.top, 0.0);
    assert_eq!(g[1].unwrap().leaf_band.unwrap().right, 90.0);
}

#[test]
fn inherited_decoration_stops_at_floats() {
    let root = node(
        "page",
        vec![text("copy", "x"), node("side", vec![text("in", "y")])],
    );
    let tree = BoxTree::new(
        root,
        "#page { text-decoration:line-through; }
         #side { float:left; }",
    )
    .unwrap();
    let leaves = tree.text_leaves();
    assert_eq!(leaves.len(), 2);
    let run = |wanted: &str| {
        leaves
            .iter()
            .find(|(_, leaf)| leaf.text() == wanted)
            .unwrap()
            .1
            .runs[0]
            .line_through
    };
    assert!(run("x"));
    assert!(!run("y"));
}

#[test]
fn widget_lays_out_and_draws_inline_runs_on_a_real_renderer_trait() {
    let root = node("page", vec![text("copy", "words"), node("box", vec![])]);
    let mut element = BoxTree::new(root, "#box { height:5px; background-color:#112233; }")
        .unwrap()
        .view::<(), iced::Theme, ()>();
    let mut tree = iced::advanced::widget::Tree::new(&element);
    let limits =
        iced::advanced::layout::Limits::new(iced::Size::ZERO, iced::Size::new(100.0, 80.0));
    let laid = element.as_widget_mut().layout(&mut tree, &(), &limits);
    assert_eq!(laid.size(), iced::Size::new(100.0, 80.0));
    element.as_widget().draw(
        &tree,
        &mut (),
        &iced::Theme::Light,
        &iced::advanced::renderer::Style::default(),
        iced::advanced::Layout::new(&laid),
        iced::mouse::Cursor::Unavailable,
        &iced::Rectangle::new(iced::Point::ORIGIN, iced::Size::new(100.0, 80.0)),
    );
}

#[test]
fn block_text_paints_its_background_as_a_box_not_a_highlight() {
    let root = node("page", vec![text("copy", "a\u{a0}b")]);
    let tree = BoxTree::new(root, "#copy { display:block; background-color:#ff0000; }").unwrap();
    assert_eq!(tree.paints()[1].background, Some([1.0, 0.0, 0.0]));
    let leaves = tree.text_leaves();
    assert_eq!(leaves[0].1.runs[0].background, None);
    // U+00A0 is not collapsible white space.
    assert_eq!(leaves[0].1.text(), "a\u{a0}b");
}

mod interactive {
    use super::*;
    use iced::advanced::{Layout, Shell, clipboard, layout::Limits, widget::Tree};
    use iced::{Event, Point, Rectangle, Size, Theme, mouse};

    type View<'a> = iced::Element<'a, u8, Theme, ()>;

    fn space(height: f32) -> iced::widget::Space {
        iced::widget::space()
            .width(iced::Length::Fill)
            .height(height)
    }
    struct Rig<'a> {
        element: View<'a>,
        tree: Tree,
        node: iced::advanced::layout::Node,
        captured: bool,
    }
    fn limits() -> Limits {
        Limits::new(Size::ZERO, Size::new(100.0, 200.0))
    }
    impl<'a> Rig<'a> {
        fn new(mut element: View<'a>) -> Self {
            let mut tree = Tree::new(&element);
            let node = element.as_widget_mut().layout(&mut tree, &(), &limits());
            Self {
                element,
                tree,
                node,
                captured: false,
            }
        }
        fn send(&mut self, event: mouse::Event, at: Point) -> Vec<u8> {
            let mut messages = Vec::new();
            let mut shell = Shell::new(&mut messages);
            self.element.as_widget_mut().update(
                &mut self.tree,
                &Event::Mouse(event),
                Layout::new(&self.node),
                mouse::Cursor::Available(at),
                &(),
                &mut clipboard::Null,
                &mut shell,
                &Rectangle::new(Point::ORIGIN, Size::new(100.0, 200.0)),
            );
            self.captured = shell.is_event_captured();
            let relayout = shell.is_layout_invalid();
            drop(shell);
            if relayout {
                self.node = self
                    .element
                    .as_widget_mut()
                    .layout(&mut self.tree, &(), &limits());
            }
            messages
        }
        fn slot(&self) -> Rectangle {
            self.node.children()[0].bounds()
        }
    }

    #[test]
    fn a_native_widget_sizes_and_sits_in_its_box() {
        let root = node("page", vec![node("top", vec![]), node("slot", vec![])]);
        let tree = BoxTree::new(root, "#top { height:10px; } #slot { padding:3px; }").unwrap();
        let view: View<'_> = tree.interactive().widget("slot", space(25.0)).into();
        let rig = Rig::new(view);
        assert_eq!(
            rig.slot(),
            Rectangle::new(Point::new(3.0, 13.0), Size::new(94.0, 25.0))
        );
        // Unknown ids and non-leaf ids are ignored, not errors.
        let root = node("page", vec![node("top", vec![])]);
        let tree = BoxTree::new(root, "").unwrap();
        let view: View<'_> = tree
            .interactive()
            .widget("missing", space(1.0))
            .widget("page", space(1.0))
            .into();
        assert!(Rig::new(view).node.children().is_empty());
    }

    #[test]
    fn hover_and_active_rules_restyle_and_relayout() {
        let root = node("page", vec![node("top", vec![]), node("slot", vec![])]);
        let tree = BoxTree::new(
            root,
            "#top { height:10px; }
             #top:hover { height:30px; }
             #top:active { height:50px; }",
        )
        .unwrap();
        let view: View<'_> = tree.interactive().widget("slot", space(1.0)).into();
        let mut rig = Rig::new(view);
        assert_eq!(rig.slot().y, 10.0);
        let on = Point::new(5.0, 5.0);
        rig.send(mouse::Event::CursorMoved { position: on }, on);
        assert_eq!(rig.slot().y, 30.0);
        rig.send(mouse::Event::ButtonPressed(mouse::Button::Left), on);
        assert_eq!(rig.slot().y, 50.0);
        rig.send(mouse::Event::ButtonReleased(mouse::Button::Left), on);
        assert_eq!(rig.slot().y, 30.0);
        let off = Point::new(5.0, 150.0);
        rig.send(mouse::Event::CursorMoved { position: off }, off);
        assert_eq!(rig.slot().y, 10.0);
    }

    #[test]
    fn hover_matches_ancestors_of_the_hovered_box() {
        let root = node(
            "page",
            vec![
                node("outer", vec![node("inner", vec![])]),
                node("slot", vec![]),
            ],
        );
        let tree = BoxTree::new(
            root,
            "#inner { height:10px; } #outer:hover { padding-bottom:7px; }",
        )
        .unwrap();
        let view: View<'_> = tree.interactive().widget("slot", space(1.0)).into();
        let mut rig = Rig::new(view);
        let on = Point::new(5.0, 5.0);
        rig.send(mouse::Event::CursorMoved { position: on }, on);
        assert_eq!(rig.slot().y, 17.0);
    }

    #[test]
    fn press_publishes_for_the_nearest_box_and_hit_tests_clip_and_paint_order() {
        let root = node(
            "page",
            vec![
                node("clip", vec![node("over", vec![])]),
                node("cover", vec![]),
            ],
        );
        let tree = BoxTree::new(
            root,
            "#clip { overflow:hidden; height:10px; }
             #over { height:50px; }
             #cover { position:absolute; left:50px; top:0; width:50px; height:100px; }",
        )
        .unwrap();
        let view: View<'_> = tree
            .interactive()
            .on_press("page", 1)
            .on_press("over", 2)
            .on_press("cover", 3)
            .into();
        let mut rig = Rig::new(view);
        let press = mouse::Event::ButtonPressed(mouse::Button::Left);
        assert_eq!(rig.send(press, Point::new(5.0, 5.0)), vec![2]);
        // `over` overflows #clip below 10, so the press falls through to no box.
        assert_eq!(rig.send(press, Point::new(5.0, 30.0)), Vec::<u8>::new());
        // The positioned box paints on top, so it takes the hit.
        assert_eq!(rig.send(press, Point::new(60.0, 5.0)), vec![3]);
    }

    #[test]
    fn press_bubbles_to_the_nearest_handler_and_captures_the_event() {
        let root = node("page", vec![node("outer", vec![node("inner", vec![])])]);
        let tree = BoxTree::new(root, "#inner { height:10px; }").unwrap();
        let view: View<'_> = tree.interactive().on_press("page", 9).into();
        let mut rig = Rig::new(view);
        let press = mouse::Event::ButtonPressed(mouse::Button::Left);
        assert_eq!(rig.send(press, Point::new(5.0, 5.0)), vec![9]);
        assert!(rig.captured);
        let nowhere = Point::new(5.0, 150.0);
        assert!(rig.send(press, nowhere).is_empty());
        assert!(!rig.captured);
    }

    #[test]
    fn nested_clips_intersect_and_positioned_boxes_escape_as_css_says() {
        let root = node(
            "page",
            vec![node(
                "outer",
                vec![node(
                    "mid",
                    vec![
                        node("deep", vec![]),
                        node("fixed", vec![]),
                        node("abs", vec![]),
                    ],
                )],
            )],
        );
        let tree = BoxTree::new(
            root,
            "#outer { overflow:hidden; height:20px; }
             #mid { overflow:hidden; height:40px; }
             #deep { height:80px; }
             #fixed { position:fixed; left:0; top:100px; width:10px; height:10px; }
             #abs { position:absolute; left:20px; top:120px; width:10px; height:10px; }",
        )
        .unwrap();
        let view: View<'_> = tree
            .interactive()
            .on_press("deep", 1)
            .on_press("fixed", 2)
            .on_press("abs", 3)
            .into();
        let mut rig = Rig::new(view);
        let press = mouse::Event::ButtonPressed(mouse::Button::Left);
        assert_eq!(rig.send(press, Point::new(5.0, 10.0)), vec![1]);
        // #mid would show down to 40, but #outer's clip applies through it.
        assert!(rig.send(press, Point::new(5.0, 30.0)).is_empty());
        // No ancestor is positioned, so both positioned boxes escape every clip.
        assert_eq!(rig.send(press, Point::new(5.0, 105.0)), vec![2]);
        assert_eq!(rig.send(press, Point::new(25.0, 125.0)), vec![3]);
    }

    #[test]
    fn draw_with_clips_widgets_and_text_runs_without_panicking() {
        let root = node(
            "page",
            vec![
                text("copy", "words"),
                node("clip", vec![node("slot", vec![])]),
                node("float", vec![]),
            ],
        );
        let tree = BoxTree::new(
            root,
            "#clip { overflow:hidden; height:5px; background-color:#ff0000; }
             #float { float:left; width:5px; height:5px; }",
        )
        .unwrap();
        let view: View<'_> = tree.interactive().widget("slot", space(20.0)).into();
        let rig = Rig::new(view);
        rig.element.as_widget().draw(
            &rig.tree,
            &mut (),
            &Theme::Light,
            &iced::advanced::renderer::Style::default(),
            Layout::new(&rig.node),
            mouse::Cursor::Unavailable,
            &Rectangle::new(Point::ORIGIN, Size::new(100.0, 200.0)),
        );
    }
}
