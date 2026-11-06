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
fn text_beside_an_inline_block_is_reported() {
    let root = node("page", vec![text("t", "words"), node("b", vec![])]);
    let error = BoxTree::new(root, "#b { display:inline-block; }").unwrap_err();
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
        // They stay registered, at no size: a restyle may give them a box.
        let rig = Rig::new(view);
        assert!(
            rig.node
                .children()
                .iter()
                .all(|child| child.size() == Size::ZERO)
        );
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
    fn hidden_boxes_and_their_widgets_take_no_press() {
        let root = node("page", vec![node("gone", vec![])]);
        let tree = BoxTree::new(root, "#gone { visibility:hidden; height:10px; }").unwrap();
        let view: View<'_> = tree
            .interactive()
            .on_press("page", 1)
            .on_press("gone", 2)
            .into();
        let mut rig = Rig::new(view);
        let press = mouse::Event::ButtonPressed(mouse::Button::Left);
        assert_eq!(rig.send(press, Point::new(5.0, 5.0)), vec![1]);
    }

    #[test]
    fn a_stale_hover_from_a_bigger_tree_does_not_panic() {
        let big = BoxTree::new(
            node(
                "page",
                vec![node("a", vec![]), node("b", vec![]), node("c", vec![])],
            ),
            "#a, #b, #c { height:10px; }",
        )
        .unwrap();
        let view: View<'_> = big.interactive().into();
        let mut rig = Rig::new(view);
        let at = Point::new(5.0, 25.0);
        rig.send(mouse::Event::CursorMoved { position: at }, at);
        // A rebuilt view keeps widget state, so the old node index comes back.
        let small = BoxTree::new(node("page", vec![]), "").unwrap();
        let mut next: View<'_> = small.interactive().into();
        rig.tree.diff(&next);
        let node = next.as_widget_mut().layout(&mut rig.tree, &(), &limits());
        assert_eq!(node.size(), Size::new(100.0, 200.0));
    }

    #[test]
    fn hover_rule_that_removes_or_reveals_a_box_moves_widgets_safely() {
        let root = node(
            "page",
            vec![
                node("trigger", vec![]),
                node("gone", vec![]),
                node("menu", vec![]),
            ],
        );
        let tree = BoxTree::new(
            root,
            "#trigger { height:10px; }
             #menu { display:none; }
             #trigger:hover ~ #menu { display:block; }
             #page:hover #gone { display:none; }
             #page:hover #menu { display:block; }",
        )
        .unwrap();
        let view: View<'_> = tree
            .interactive()
            .widget("gone", space(5.0))
            .widget("menu", space(7.0))
            .into();
        let mut rig = Rig::new(view);
        assert_eq!(rig.node.children()[0].size().height, 5.0);
        assert_eq!(rig.node.children()[1].size().height, 0.0);
        let on = Point::new(5.0, 5.0);
        rig.send(mouse::Event::CursorMoved { position: on }, on);
        assert_eq!(rig.node.children()[0].size().height, 0.0);
        assert_eq!(rig.node.children()[1].size().height, 7.0);
        assert_eq!(rig.node.children()[1].bounds().y, 10.0);
    }

    #[test]
    fn a_covered_widget_gets_no_cursor() {
        let root = node("page", vec![node("slot", vec![]), node("cover", vec![])]);
        let tree = BoxTree::new(
            root,
            "#slot { width:50px; height:50px; }
             #cover { position:absolute; left:0; top:0; width:25px; height:50px; }",
        )
        .unwrap();
        let button = iced::widget::button(space(50.0))
            .width(50.0)
            .height(50.0)
            .on_press(1_u8);
        let view: View<'_> = tree.interactive().widget("slot", button).into();
        let mut rig = Rig::new(view);
        let press = mouse::Event::ButtonPressed(mouse::Button::Left);
        let release = mouse::Event::ButtonReleased(mouse::Button::Left);
        let covered = Point::new(10.0, 10.0);
        rig.send(press, covered);
        assert!(rig.send(release, covered).is_empty());
        let open = Point::new(40.0, 10.0);
        rig.send(press, open);
        assert_eq!(rig.send(release, open), vec![1]);
    }

    #[test]
    fn fill_height_widgets_fill_definite_boxes_and_add_nothing_to_auto_ones() {
        let root = node(
            "page",
            vec![
                node("fixed", vec![]),
                node("auto", vec![]),
                node("after", vec![]),
            ],
        );
        let tree = BoxTree::new(root, "#fixed { height:30px; } #after { height:1px; }").unwrap();
        let fill = || {
            iced::widget::space()
                .width(iced::Length::Fill)
                .height(iced::Length::Fill)
        };
        let view: View<'_> = tree
            .interactive()
            .widget("fixed", fill())
            .widget("auto", fill())
            .widget("after", space(1.0))
            .into();
        let rig = Rig::new(view);
        assert_eq!(rig.node.children()[0].size().height, 30.0);
        assert_eq!(rig.node.children()[1].size().height, 0.0);
        assert_eq!(rig.node.children()[2].bounds().y, 30.0);
    }

    fn key(rig: &mut Rig<'_>, named: iced::keyboard::key::Named, shift: bool) -> Vec<u8> {
        let modifiers = if shift {
            iced::keyboard::Modifiers::SHIFT
        } else {
            iced::keyboard::Modifiers::empty()
        };
        let event = Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Named(named),
            modified_key: iced::keyboard::Key::Named(named),
            physical_key: iced::keyboard::key::Physical::Unidentified(
                iced::keyboard::key::NativeCode::Unidentified,
            ),
            location: iced::keyboard::Location::Standard,
            modifiers,
            text: None,
            repeat: false,
        });
        let mut messages = Vec::new();
        let mut shell = Shell::new(&mut messages);
        rig.element.as_widget_mut().update(
            &mut rig.tree,
            &event,
            Layout::new(&rig.node),
            mouse::Cursor::Unavailable,
            &(),
            &mut clipboard::Null,
            &mut shell,
            &Rectangle::new(Point::ORIGIN, Size::new(100.0, 200.0)),
        );
        rig.captured = shell.is_event_captured();
        let relayout = shell.is_layout_invalid();
        drop(shell);
        if relayout {
            rig.node = rig
                .element
                .as_widget_mut()
                .layout(&mut rig.tree, &(), &limits());
        }
        messages
    }

    fn focus_rig<'a>() -> Rig<'a> {
        let attrs = AttributeLimits {
            entries: NonZeroUsize::new(100).unwrap(),
            retained_bytes: NonZeroUsize::new(100_000).unwrap(),
            work: NonZeroUsize::new(100_000).unwrap(),
        };
        let tabbed = node("second", vec![])
            .style_attribute("tabindex", "2", attrs)
            .unwrap();
        let first = node("first", vec![])
            .style_attribute("tabindex", "1", attrs)
            .unwrap();
        let skipped = node("skip", vec![])
            .style_attribute("tabindex", "-1", attrs)
            .unwrap();
        let root = node(
            "page",
            vec![
                tabbed,
                first,
                node("act", vec![]),
                skipped,
                node("slot", vec![]),
            ],
        );
        let tree = BoxTree::new(
            root,
            "#first, #second, #act, #skip { height:5px; }
             #first:focus { height:10px; }
             #second:focus { height:20px; }
             #act:focus { height:40px; }
             #skip:focus { height:80px; }",
        )
        .unwrap();
        let view: View<'_> = tree
            .interactive()
            .widget("slot", space(1.0))
            .on_press("act", 5)
            .into();
        Rig::new(view)
    }

    #[test]
    fn tab_cycles_by_tabindex_then_document_order_and_shift_goes_back() {
        use iced::keyboard::key::Named;
        let mut rig = focus_rig();
        assert_eq!(rig.slot().y, 20.0);
        key(&mut rig, Named::Tab, false);
        assert!(rig.captured);
        assert_eq!(rig.slot().y, 25.0); // #first
        key(&mut rig, Named::Tab, false);
        assert_eq!(rig.slot().y, 35.0); // #second
        key(&mut rig, Named::Tab, false);
        assert_eq!(rig.slot().y, 55.0); // #act; #skip is not in the order
        key(&mut rig, Named::Tab, false);
        assert_eq!(rig.slot().y, 25.0); // wraps
        key(&mut rig, Named::Tab, true);
        assert_eq!(rig.slot().y, 55.0); // back to #act
        key(&mut rig, Named::Escape, false);
        assert_eq!(rig.slot().y, 20.0);
    }

    #[test]
    fn enter_and_space_press_the_focused_box_and_a_click_moves_focus() {
        use iced::keyboard::key::Named;
        let mut rig = focus_rig();
        assert!(key(&mut rig, Named::Enter, false).is_empty());
        assert!(!rig.captured);
        for _ in 0..3 {
            key(&mut rig, Named::Tab, false);
        }
        assert_eq!(key(&mut rig, Named::Enter, false), vec![5]);
        assert_eq!(key(&mut rig, Named::Space, false), vec![5]);
        assert!(rig.captured);
        // #skip sits at 50 here (5+5+40): a click focuses it, outside the Tab order.
        let press = mouse::Event::ButtonPressed(mouse::Button::Left);
        let release = mouse::Event::ButtonReleased(mouse::Button::Left);
        rig.send(press, Point::new(5.0, 52.0));
        rig.send(release, Point::new(5.0, 52.0));
        assert_eq!(rig.slot().y, 95.0);
        // A click on nothing focusable clears it.
        rig.send(press, Point::new(5.0, 190.0));
        rig.send(release, Point::new(5.0, 190.0));
        assert_eq!(rig.slot().y, 20.0);
    }

    #[test]
    fn hidden_and_absent_boxes_take_no_focus() {
        use iced::keyboard::key::Named;
        let root = node(
            "page",
            vec![
                node("gone", vec![]),
                node("none", vec![]),
                node("slot", vec![]),
            ],
        );
        let tree = BoxTree::new(
            root,
            "#gone { visibility:hidden; height:3px; } #none { display:none; }
             #gone:focus, #none:focus { height:9px; }",
        )
        .unwrap();
        let view: View<'_> = tree
            .interactive()
            .widget("slot", space(1.0))
            .on_press("gone", 1)
            .on_press("none", 2)
            .into();
        let mut rig = Rig::new(view);
        key(&mut rig, Named::Tab, false);
        assert!(!rig.captured);
        assert_eq!(rig.slot().y, 3.0);
    }

    #[test]
    fn stale_or_hidden_focus_does_nothing_and_does_not_panic() {
        use iced::keyboard::key::Named;
        let mut rig = focus_rig();
        for _ in 0..3 {
            key(&mut rig, Named::Tab, false);
        }
        // A rebuilt view with fewer nodes keeps the old focus index in its state.
        let small = BoxTree::new(node("page", vec![]), "").unwrap();
        let mut next: View<'_> = small.interactive().on_press("page", 1).into();
        rig.tree.diff(&next);
        rig.node = next.as_widget_mut().layout(&mut rig.tree, &(), &limits());
        rig.element = next;
        assert!(key(&mut rig, Named::Enter, false).is_empty());
        // A focused box that a rule hides fires nothing.
        let root = node("page", vec![node("a", vec![])]);
        let tree =
            BoxTree::new(root, "#a { height:5px; } #a:focus { visibility:hidden; }").unwrap();
        let view: View<'_> = tree.interactive().on_press("a", 9).into();
        let mut rig = Rig::new(view);
        key(&mut rig, Named::Tab, false);
        assert!(key(&mut rig, Named::Enter, false).is_empty());
    }

    #[test]
    fn tab_scrolls_the_focused_box_into_view_through_every_scroller() {
        use iced::keyboard::key::Named;
        let root = node(
            "page",
            vec![node(
                "list",
                vec![
                    node("first", vec![]),
                    node("slot", vec![]),
                    node("spacer", vec![]),
                    node("far", vec![]),
                ],
            )],
        );
        let tree = BoxTree::new(
            root,
            "#list { overflow:auto; height:50px; }
             #first, #far { height:10px; }
             #slot { height:5px; }
             #spacer { height:100px; }",
        )
        .unwrap();
        let view: View<'_> = tree
            .interactive()
            .widget("slot", space(5.0))
            .on_press("first", 1)
            .on_press("far", 2)
            .into();
        let mut rig = Rig::new(view);
        assert_eq!(rig.slot().y, 10.0);
        key(&mut rig, Named::Tab, false);
        assert_eq!(rig.slot().y, 10.0);
        // #far ends at 125; to show it in a 50px box the list scrolls by 75.
        key(&mut rig, Named::Tab, false);
        assert_eq!(rig.slot().y, -65.0);
        key(&mut rig, Named::Tab, false);
        assert_eq!(rig.slot().y, 10.0);
    }

    fn wheel(lines: f32) -> mouse::Event {
        mouse::Event::WheelScrolled {
            delta: mouse::ScrollDelta::Pixels { x: 0.0, y: lines },
        }
    }

    #[test]
    fn wheel_scrolls_the_nearest_scroller_within_its_limit() {
        let root = node(
            "page",
            vec![
                node(
                    "list",
                    vec![
                        node("tall", vec![]),
                        node("slot", vec![]),
                        node("fixed", vec![]),
                    ],
                ),
                node("next", vec![]),
            ],
        );
        let tree = BoxTree::new(
            root,
            "#list { overflow:auto; height:50px; }
             #tall { height:100px; }
             #fixed { position:absolute; top:5px; left:0; height:5px; width:5px; }
             #next { height:10px; }",
        )
        .unwrap();
        let view: View<'_> = tree
            .interactive()
            .widget("slot", space(10.0))
            .on_press("next", 8)
            .into();
        let mut rig = Rig::new(view);
        let inside = Point::new(5.0, 10.0);
        assert_eq!(rig.slot().y, 100.0);
        rig.send(wheel(-30.0), inside);
        assert_eq!(rig.slot().y, 70.0);
        // The content ends 60 below the box's bottom edge; scrolling stops there.
        rig.send(wheel(-1000.0), inside);
        assert_eq!(rig.slot().y, 40.0);
        rig.send(wheel(25.0), inside);
        assert_eq!(rig.slot().y, 65.0);
        // Outside the scroller the wheel does nothing.
        rig.send(wheel(-5.0), Point::new(5.0, 55.0));
        assert_eq!(rig.slot().y, 65.0);
        // Scrolled content under the box's edge is clipped: a press falls through.
        assert_eq!(
            rig.send(
                mouse::Event::ButtonPressed(mouse::Button::Left),
                Point::new(5.0, 55.0)
            ),
            vec![8]
        );
    }

    #[test]
    fn hidden_overflow_does_not_scroll_and_inner_scroller_takes_the_wheel_first() {
        let root = node(
            "page",
            vec![node(
                "outer",
                vec![
                    node("inner", vec![node("slot", vec![])]),
                    node("rest", vec![]),
                ],
            )],
        );
        let css = "#outer { overflow:scroll; height:50px; }
             #inner { overflow:auto; height:40px; }
             #slot { height:60px; }
             #rest { height:100px; }";
        let tree = BoxTree::new(root, css).unwrap();
        let view: View<'_> = tree.interactive().widget("slot", space(60.0)).into();
        let mut rig = Rig::new(view);
        let inside = Point::new(5.0, 5.0);
        rig.send(wheel(-15.0), inside);
        assert_eq!(rig.slot().y, -15.0);
        // The inner box is spent after 20; the next wheel moves the outer one.
        rig.send(wheel(-1000.0), inside);
        assert_eq!(rig.slot().y, -20.0);
        rig.send(wheel(-1000.0), inside);
        assert_eq!(rig.slot().y, -110.0);
        let root = node("page", vec![node("clip", vec![node("slot", vec![])])]);
        let tree = BoxTree::new(
            root,
            "#clip { overflow:hidden; height:5px; } #slot { height:60px; }",
        )
        .unwrap();
        let view: View<'_> = tree.interactive().widget("slot", space(60.0)).into();
        let mut rig = Rig::new(view);
        rig.send(wheel(-10.0), Point::new(5.0, 2.0));
        assert_eq!(rig.slot().y, 0.0);
    }

    #[test]
    fn a_widget_survives_generated_text_on_its_box() {
        let root = node("page", vec![node("slot", vec![])]);
        let tree = BoxTree::new(root, "#slot:before { content:\"x\"; }").unwrap();
        let view: View<'_> = tree.interactive().widget("slot", space(9.0)).into();
        assert_eq!(Rig::new(view).slot().height, 9.0);
    }

    #[test]
    fn a_drag_keeps_the_cursor_after_leaving_the_widget() {
        let root = node("page", vec![node("slot", vec![]), node("rest", vec![])]);
        let tree = BoxTree::new(
            root,
            "#slot { width:50px; height:50px; } #rest { height:50px; }",
        )
        .unwrap();
        let button = iced::widget::button(space(50.0))
            .width(50.0)
            .height(50.0)
            .on_press(1_u8);
        let view: View<'_> = tree.interactive().widget("slot", button).into();
        let mut rig = Rig::new(view);
        rig.send(
            mouse::Event::ButtonPressed(mouse::Button::Left),
            Point::new(10.0, 10.0),
        );
        // The press state reached the widget; its release is still routed on the way out,
        // and a release outside the button does not click it.
        assert!(
            rig.send(
                mouse::Event::ButtonReleased(mouse::Button::Left),
                Point::new(10.0, 80.0)
            )
            .is_empty()
        );
        // The same release over the button clicks: it had the cursor throughout.
        rig.send(
            mouse::Event::ButtonPressed(mouse::Button::Left),
            Point::new(10.0, 10.0),
        );
        assert_eq!(
            rig.send(
                mouse::Event::ButtonReleased(mouse::Button::Left),
                Point::new(10.0, 10.0)
            ),
            vec![1]
        );
    }

    #[test]
    fn scroll_reach_stops_at_inner_clips_and_counts_end_padding() {
        let root = node(
            "page",
            vec![node(
                "outer",
                vec![
                    node("slot", vec![]),
                    node("inner", vec![node("huge", vec![])]),
                ],
            )],
        );
        let tree = BoxTree::new(
            root,
            "#outer { overflow:auto; height:50px; padding-bottom:5px; }
             #slot { height:10px; }
             #inner { overflow:hidden; height:60px; }
             #huge { height:1000px; }",
        )
        .unwrap();
        let view: View<'_> = tree.interactive().widget("slot", space(10.0)).into();
        let mut rig = Rig::new(view);
        rig.send(wheel(-5000.0), Point::new(5.0, 45.0));
        // Content ends at 70, past a 55 padding bottom, plus 5 of end padding: limit 20.
        assert_eq!(rig.slot().y, -20.0);
    }

    #[test]
    fn a_static_root_scrolls_absolute_children_with_the_page() {
        let root = node("page", vec![node("slot", vec![]), node("far", vec![])]);
        let tree = BoxTree::new(
            root,
            "#page { overflow:auto; }
             #slot { height:10px; }
             #far { position:absolute; top:500px; left:0; width:5px; height:5px; }",
        )
        .unwrap();
        let view: View<'_> = tree
            .interactive()
            .widget("slot", space(10.0))
            .on_press("far", 4)
            .into();
        let mut rig = Rig::new(view);
        // The viewport is 200 tall: the page can reach the box at 500 by scrolling 305.
        rig.send(wheel(-10000.0), Point::new(5.0, 5.0));
        assert_eq!(rig.slot().y, -305.0);
        assert_eq!(
            rig.send(
                mouse::Event::ButtonPressed(mouse::Button::Left),
                Point::new(2.0, 197.0)
            ),
            vec![4]
        );
    }

    fn wheel_across(amount: f32) -> mouse::Event {
        mouse::Event::WheelScrolled {
            delta: mouse::ScrollDelta::Pixels { x: amount, y: 0.0 },
        }
    }

    #[test]
    fn wheel_scrolls_across_and_rtl_boxes_stay_put() {
        let root = node("page", vec![node("list", vec![node("slot", vec![])])]);
        let css = "#list { overflow:auto; height:50px; width:60px; padding-right:5px; }
             #slot { width:100px; height:10px; }";
        let tree = BoxTree::new(root, css).unwrap();
        let view: View<'_> = tree.interactive().widget("slot", space(10.0)).into();
        let mut rig = Rig::new(view);
        let inside = Point::new(5.0, 5.0);
        rig.send(wheel_across(-15.0), inside);
        assert_eq!(rig.slot().x, -15.0);
        // Content reaches 100 past a 65 padding edge, plus 5 of end padding: limit 40.
        rig.send(wheel_across(-1000.0), inside);
        assert_eq!(rig.slot().x, -40.0);
        // Nothing to scroll vertically here, so the vertical wheel leaves it alone.
        rig.send(wheel(-20.0), inside);
        assert_eq!(rig.slot().y, 0.0);
        let root = node("page", vec![node("list", vec![node("slot", vec![])])]);
        let tree = BoxTree::new(
            root,
            "#list { overflow:auto; height:50px; width:60px; direction:rtl; }
             #slot { width:100px; height:10px; }",
        )
        .unwrap();
        let view: View<'_> = tree.interactive().widget("slot", space(10.0)).into();
        let mut rig = Rig::new(view);
        let before = rig.slot().x;
        rig.send(wheel_across(-15.0), inside);
        assert_eq!(rig.slot().x, before);
    }

    #[test]
    fn draw_with_clips_widgets_and_text_runs_without_panicking() {
        let root = node(
            "page",
            vec![
                text("copy", "words"),
                node("clip", vec![node("slot", vec![])]),
                node("float", vec![]),
                node("item", vec![]),
            ],
        );
        let tree = BoxTree::new(
            root,
            "#clip { overflow:auto; height:5px; background-color:#ff0000; }
             #float { float:left; width:5px; height:5px; }
             #item { display:list-item; }",
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

#[test]
fn z_index_orders_stacking_contexts_and_negative_goes_under_flow() {
    let root = node(
        "page",
        vec![
            node("flow", vec![]),
            node("low", vec![]),
            node("high", vec![node("nested", vec![])]),
            node("auto", vec![node("escape", vec![])]),
            node("zero", vec![]),
        ],
    );
    let tree = BoxTree::new(
        root,
        "#low { position:absolute; z-index:-1; }
         #high { position:absolute; z-index:5; }
         #nested { position:absolute; z-index:-100; }
         #auto { position:relative; }
         #escape { position:absolute; z-index:3; }
         #zero { position:absolute; z-index:0; }",
    )
    .unwrap();
    // Preorder: page0 flow1 low2 high3 nested4 auto5 escape6 zero7.
    // nested stays inside high's context despite its z-index; escape joins the root's.
    assert_eq!(tree.paint_order(), vec![0, 2, 1, 5, 7, 6, 3, 4]);
    assert_eq!(tree.paints()[2].z_index, Some(-1));
    assert_eq!(tree.paints()[5].z_index, None);
}

#[test]
fn visibility_inherits_and_can_be_overridden() {
    let root = node(
        "page",
        vec![node(
            "gone",
            vec![node("back", vec![]), node("still", vec![])],
        )],
    );
    let tree = BoxTree::new(
        root,
        "#gone { visibility:hidden; height:10px; } #back { visibility:visible; }",
    )
    .unwrap();
    let paints = tree.paints();
    assert!(paints[0].visible);
    assert!(!paints[1].visible);
    assert!(paints[2].visible);
    assert!(!paints[3].visible);
    // It still takes its room.
    assert_eq!(
        tree.layout(100.0, None).unwrap()[1].unwrap().border.bottom,
        10.0
    );
}

#[test]
fn outline_width_style_and_color() {
    let root = node(
        "page",
        vec![node("a", vec![]), node("b", vec![]), node("c", vec![])],
    );
    let tree = BoxTree::new(
        root,
        "#a { outline:2px solid #ff0000; }
         #b { outline-style:dotted; color:#00ff00; }
         #c { outline-width:4px; }",
    )
    .unwrap();
    let paints = tree.paints();
    assert_eq!(paints[1].outline, Some((2.0, [1.0, 0.0, 0.0])));
    // medium is 3px; the initial invert falls back to `color`.
    assert_eq!(paints[2].outline, Some((3.0, [0.0, 1.0, 0.0])));
    assert_eq!(paints[3].outline, None);
    // Outlines take no room.
    assert_eq!(
        tree.layout(100.0, None).unwrap()[1].unwrap().border.left,
        0.0
    );
}

#[test]
fn before_and_after_join_the_inline_run_in_order() {
    let root = node(
        "page",
        vec![
            text("one", "middle"),
            node("span", vec![text("two", "core")]),
            node("empty", vec![]),
        ],
    );
    let tree = BoxTree::new(
        root,
        "#one:before { content:\"[\"; color:#ff0000; }
         #one:after { content:\"]\"; font-weight:bold; font-size:24px; }
         #span { display:inline; }
         #span:before { content:open-quote; }
         #span:after { content:close-quote attr(id); }
         #empty:before { content:\"made\"; }",
    )
    .unwrap();
    let leaves = tree.text_leaves();
    assert_eq!(leaves.len(), 2);
    let leaf = leaves[0].1;
    assert_eq!(leaf.text(), "[middle]\u{201c}core\u{201d}span");
    assert_eq!(leaf.runs[0].color, Some([1.0, 0.0, 0.0]));
    assert_eq!(leaf.runs[2].weight, 700);
    assert_eq!(leaf.runs[2].size, 24.0);
    assert_eq!(leaves[1].1.text(), "made");
}

#[test]
fn block_pseudo_gets_its_own_block_and_clearfix_contains_floats() {
    let root = node(
        "page",
        vec![
            node("wrap", vec![node("side", vec![])]),
            node("after", vec![]),
        ],
    );
    let tree = BoxTree::new(
        root,
        "#wrap:after { content:\"\"; display:block; clear:both; }
         #side { float:left; width:10px; height:30px; }
         #after { height:1px; }",
    )
    .unwrap();
    let g = tree.layout_text(100.0, None, &mut |_, _| 0.0).unwrap();
    assert_eq!(g[1].unwrap().border.bottom, 30.0);
    assert_eq!(g[3].unwrap().border.top, 30.0);
}

#[test]
fn block_text_leaf_keeps_generated_runs_inside_and_its_own_paint() {
    let root = node("page", vec![text("copy", "body"), node("box", vec![])]);
    let tree = BoxTree::new(
        root,
        "#copy { display:block; } #copy:before { content:\">\"; } #copy:after { content:\"<\"; }
         #box { background-color:#00ff00; border:1px solid #ff0000; }
         #box:before { content:\"inside\"; }",
    )
    .unwrap();
    let leaves = tree.text_leaves();
    assert_eq!(leaves[0].1.text(), ">body<");
    // Both generated runs stay inside the text block, in the leaf itself.
    assert_eq!(leaves[0].0, 1);
    assert_eq!(leaves.len(), 2);
    assert_eq!(leaves[1].1.text(), "inside");
    // The anonymous block made for generated content leaves its owner's paint.
    assert_eq!(tree.paints()[2].background, Some([0.0, 1.0, 0.0]));
    assert_eq!(tree.paints()[2].border[0], Some([1.0, 0.0, 0.0]));
    // The text block has a box; the anonymous block inside #box has none.
    assert_eq!(tree.paint_order(), vec![0, 1, 2]);
}

#[test]
fn list_items_get_outside_markers_by_type_and_ordinal() {
    let root = node(
        "page",
        vec![
            node("a", vec![text("ta", "one")]),
            node("b", vec![text("tb", "two")]),
            node("c", vec![text("tc", "three")]),
            node("d", vec![text("td", "four")]),
        ],
    );
    let tree = BoxTree::new(
        root,
        "#a, #b, #c, #d { display:list-item; }
         #a { list-style-type:decimal; }
         #b { list-style-type:lower-roman; }
         #c { list-style-type:none; }
         #d { list-style-position:inside; }",
    )
    .unwrap();
    let texts: Vec<_> = tree.text_leaves().iter().map(|(_, l)| l.text()).collect();
    // Outside markers take no room; an inside one joins the first line (ordinal 4).
    assert_eq!(texts, vec!["one", "two", "three", "\u{2022}\u{a0}four"]);
    let markers: Vec<_> = tree
        .markers()
        .iter()
        .map(|(at, leaf)| (*at, leaf.text()))
        .collect();
    assert_eq!(markers, vec![(1, "1.".to_owned()), (3, "ii.".to_owned())]);
    // A block first child no longer gets pushed below a marker line.
    let root = node("page", vec![node("li", vec![node("p", vec![])])]);
    let tree = BoxTree::new(root, "#li { display:list-item; } #p { height:5px; }").unwrap();
    assert_eq!(tree.markers()[&1].text(), "\u{2022}");
    let g = tree.layout(100.0, None).unwrap();
    assert_eq!(g[2].unwrap().border.top, 0.0);
}

#[test]
fn a_skipped_subtree_does_not_close_open_elements() {
    let root = node(
        "page",
        vec![node("gone", vec![text("inner", "x")]), node("kept", vec![])],
    );
    let tree = BoxTree::new(
        root,
        "#gone { display:none; }
         #kept { height:5px; }
         #page:after { content:\"end\"; }",
    )
    .unwrap();
    let g = tree.layout_text(100.0, None, &mut |_, _| 7.0).unwrap();
    // #page's content ends after #kept, so its :after sits below it.
    assert_eq!(g[0].unwrap().content.bottom, 12.0);
    assert_eq!(g[3].unwrap().border.top, 0.0);
}

#[test]
fn pseudo_takes_its_own_text_properties_and_keyword_sizes() {
    let root = node("page", vec![node("a", vec![])]);
    let tree = BoxTree::new(
        root,
        "#a:before { content:\"x y\"; font-size:x-large; font-family:Verdana;
                     text-transform:uppercase; background-color:#ff0000; }",
    )
    .unwrap();
    let leaf = tree.text_leaves()[0].1;
    assert_eq!(leaf.runs[0].text, "X Y");
    assert_eq!(leaf.runs[0].size, 24.0);
    assert_eq!(leaf.runs[0].family.as_deref(), Some("Verdana"));
    assert_eq!(leaf.runs[0].background, Some([1.0, 0.0, 0.0]));
}

#[test]
fn display_none_pseudo_generates_nothing() {
    let root = node("page", vec![node("a", vec![])]);
    let tree = BoxTree::new(root, "#a:before { content:\"x\"; display:none; }").unwrap();
    assert!(tree.text_leaves().is_empty());
}

#[test]
fn absolute_after_a_text_run_keeps_its_static_position_below_it() {
    let root = node("page", vec![text("copy", "words"), node("abs", vec![])]);
    let tree = BoxTree::new(root, "#abs { position:absolute; width:5px; height:5px; }").unwrap();
    let g = tree.layout_text(100.0, None, &mut |_, _| 20.0).unwrap();
    assert_eq!(g[2].unwrap().border.top, 20.0);
}

#[test]
fn inline_runs_remember_their_owner_for_hit_tests() {
    let root = node("page", vec![text("a", "A"), text("b", "B")]);
    let tree = BoxTree::new(root, "#a:before { content:\"x\"; }").unwrap();
    let leaf = tree.text_leaves()[0].1.clone();
    assert_eq!(leaf.text(), "xAB");
    // Source nodes are postorder: a0 b1 page2.
    let owners: Vec<_> = leaf.runs.iter().map(|run| run.owner).collect();
    assert_eq!(owners, vec![0, 0, 1]);
}

#[test]
fn generated_content_in_table_parts_gets_an_anonymous_cell() {
    let table = || node("t", vec![node("r", vec![node("c", vec![])])]);
    let base = "#t { display:table; } #r { display:table-row; } #c { display:table-cell; }";
    let on = |selector: &str| format!("{base} {selector}:before {{ content:\"x\"; }}");
    for selector in ["#t", "#r", "#c"] {
        let tree = BoxTree::new(table(), &on(selector)).unwrap();
        assert_eq!(tree.text_leaves()[0].1.text(), "x");
        assert!(tree.layout_text(100.0, None, &mut |_, _| 5.0).is_ok());
    }
}

#[test]
fn missing_table_parts_are_generated_around_misplaced_ones() {
    // Lone cells in a block: one anonymous table and row around them both.
    let root = node(
        "page",
        vec![node("a", vec![]), node("b", vec![]), node("after", vec![])],
    );
    let tree = BoxTree::new(
        root,
        "#a, #b { display:table-cell; width:20px; height:5px; } #after { height:1px; }",
    )
    .unwrap();
    let g = tree.layout(100.0, None).unwrap();
    assert_eq!(g[1].unwrap().border.left, 0.0);
    assert_eq!(g[2].unwrap().border.left, 20.0);
    assert_eq!(g[1].unwrap().border.top, g[2].unwrap().border.top);
    assert_eq!(g[3].unwrap().border.top, 5.0);
    // A block and text in a row become cells; blank text between parts is nothing.
    let root = node(
        "t",
        vec![
            text("gap", "   "),
            node(
                "r",
                vec![node("block", vec![]), text("words", "w"), text("more", "x")],
            ),
        ],
    );
    let tree = BoxTree::new(
        root,
        "#t { display:table; } #r { display:table-row; } #block { width:10px; height:3px; }",
    )
    .unwrap();
    let leaves = tree.text_leaves();
    assert_eq!(leaves.len(), 1);
    assert_eq!(leaves[0].1.text(), "wx");
    let g = tree.layout_text(100.0, None, &mut |_, _| 3.0).unwrap();
    assert_eq!(g[3].unwrap().border.right, 10.0);
    // A row directly in a block gets a table, and a row in a row a cell and table.
    let root = node("page", vec![node("row", vec![node("c", vec![])])]);
    let tree = BoxTree::new(
        root,
        "#row { display:table-row; } #c { display:table-cell; height:4px; }",
    )
    .unwrap();
    assert_eq!(
        tree.layout(100.0, None).unwrap()[2].unwrap().border.bottom,
        4.0
    );
}

#[test]
fn vertical_align_and_caption_side_reach_the_table() {
    let root = node(
        "t",
        vec![
            node("cap", vec![]),
            node(
                "r",
                vec![node("tall", vec![]), node("mid", vec![node("in", vec![])])],
            ),
        ],
    );
    let tree = BoxTree::new(
        root,
        "#t { display:table; width:100px; }
         #cap { display:table-caption; caption-side:bottom; height:5px; }
         #r { display:table-row; }
         #tall { display:table-cell; height:40px; }
         #mid { display:table-cell; vertical-align:middle; }
         #in { height:10px; }",
    )
    .unwrap();
    let g = tree.layout(100.0, None).unwrap();
    assert_eq!(g[5].unwrap().border.top, 15.0);
    assert_eq!(g[1].unwrap().border.top, 40.0);
}

#[test]
fn row_and_text_alignment_come_from_the_block_container() {
    let root = node("page", vec![node("a", vec![]), node("b", vec![])]);
    let tree = BoxTree::new(
        root,
        "#a, #b { display:inline-block; width:10px; height:1px; text-align:center; }",
    )
    .unwrap();
    let g = tree.layout(100.0, None).unwrap();
    assert_eq!(g[1].unwrap().border.left, 0.0);
    let root = node("page", vec![node("a", vec![]), node("b", vec![])]);
    let tree = BoxTree::new(
        root,
        "#page { text-align:right; } #a, #b { display:inline-block; width:10px; height:1px; }",
    )
    .unwrap();
    let g = tree.layout(100.0, None).unwrap();
    assert_eq!(g[2].unwrap().border.right, 100.0);
    // A span's own text-align does nothing to the line it sits in.
    let root = node("page", vec![node("span", vec![text("t", "x")])]);
    let tree = BoxTree::new(root, "#span { display:inline; text-align:right; }").unwrap();
    assert_eq!(tree.text_leaves()[0].1.align, "left");
}

#[test]
fn inline_generated_text_beside_inline_blocks_is_refused_and_blank_text_is_not() {
    let root = node("page", vec![node("a", vec![])]);
    let css = "#a { display:inline-block; } #page:after { content:\"x\"; }";
    assert!(
        BoxTree::new(root, css)
            .unwrap_err()
            .to_string()
            .contains("content")
    );
    let root = node("page", vec![node("a", vec![])]);
    let css = "#a { display:inline-block; } #page:after { content:\"x\"; display:block; }";
    assert_eq!(
        BoxTree::new(root, css).unwrap().text_leaves()[0].1.text(),
        "x"
    );
    let root = node("page", vec![text("gap", "  "), node("a", vec![])]);
    assert!(BoxTree::new(root, "#a { display:inline-block; }").is_ok());
}

#[test]
fn counters_count_in_document_order_with_scopes() {
    let section = |id: &str, inner: Vec<Element>| node(id, inner);
    let root = node(
        "page",
        vec![
            section("s1", vec![node("a", vec![]), node("b", vec![])]),
            section("s2", vec![node("c", vec![])]),
            node("hidden", vec![]),
            section("s3", vec![]),
        ],
    );
    let tree = BoxTree::new(
        root,
        "#page { counter-reset:chapter; }
         #s1, #s2, #s3 { counter-increment:chapter; counter-reset:item 4; }
         #s1:before, #s2:before, #s3:before { content:\"Chapter \" counter(chapter, upper-roman) \": \"; }
         #a, #b, #c { counter-increment:item; }
         #a:before, #b:before, #c:before { content:counters(chapter, \".\") \"-\" counter(item); }
         #hidden { display:none; counter-increment:chapter 100; }",
    )
    .unwrap();
    let texts: Vec<_> = tree.text_leaves().iter().map(|(_, l)| l.text()).collect();
    assert_eq!(
        texts,
        vec![
            "Chapter I:",
            "1-5",
            "1-6",
            "Chapter II:",
            "2-5",
            "Chapter III:",
        ]
    );
}

#[test]
fn after_sees_its_subtree_and_pseudo_actions_apply() {
    let root = node(
        "page",
        vec![node("list", vec![node("a", vec![]), node("b", vec![])])],
    );
    let tree = BoxTree::new(
        root,
        "#list { counter-reset:n; }
         #a, #b { counter-increment:n 2; }
         #list:after { content:\"total \" counter(n, decimal-leading-zero); }
         #b:before { counter-increment:n 10; content:counter(n, lower-alpha); }",
    )
    .unwrap();
    let texts: Vec<_> = tree.text_leaves().iter().map(|(_, l)| l.text()).collect();
    // #b gets +2 then its pseudo +10, so 14 reads as n; the total sees all of it.
    assert_eq!(texts, vec!["n", "total 14"]);
}

#[test]
fn an_undeclared_counter_reads_zero_and_quotes_nest() {
    let root = node("page", vec![node("outer", vec![node("inner", vec![])])]);
    let tree = BoxTree::new(
        root,
        "#outer:before { content:open-quote counter(missing); }
         #inner:before { content:open-quote; }
         #inner:after { content:close-quote; }
         #outer:after { content:close-quote; }",
    )
    .unwrap();
    let texts: Vec<_> = tree.text_leaves().iter().map(|(_, l)| l.text()).collect();
    assert_eq!(texts, vec!["\u{201c}0", "\u{2018}\u{2019}", "\u{201d}"]);
}

#[test]
fn a_reset_reaches_later_siblings_and_counters_nest_outermost_first() {
    let root = node(
        "page",
        vec![
            node("a", vec![]),
            node("b", vec![]),
            node("o", vec![node("i", vec![])]),
            node("z", vec![]),
        ],
    );
    let tree = BoxTree::new(
        root,
        "#a { counter-reset:x 5; }
         #b:before { content:counter(x, decimal-leading-zero); }
         #o { counter-reset:n 1; }
         #i { counter-reset:n 2; }
         #i:before { content:counters(n, \".\"); }
         #z { counter-increment:fresh 3; }
         #z:before { content:counter(fresh); }",
    )
    .unwrap();
    let texts: Vec<_> = tree.text_leaves().iter().map(|(_, l)| l.text()).collect();
    assert_eq!(texts, vec!["05", "1.2", "3"]);
}

#[test]
fn colspan_and_rowspan_come_from_source_attributes() {
    let attrs = AttributeLimits {
        entries: NonZeroUsize::new(100).unwrap(),
        retained_bytes: NonZeroUsize::new(100_000).unwrap(),
        work: NonZeroUsize::new(100_000).unwrap(),
    };
    let wide = node("wide", vec![])
        .style_attribute("colspan", "2", attrs)
        .unwrap();
    let root = node(
        "t",
        vec![
            node("r1", vec![wide]),
            node("r2", vec![node("a", vec![]), node("b", vec![])]),
        ],
    );
    let tree = BoxTree::new(
        root,
        "#t { display:table; width:100px; }
         #r1, #r2 { display:table-row; }
         #wide, #a, #b { display:table-cell; height:5px; }
         #a { width:30px; } #b { width:70px; }",
    )
    .unwrap();
    let g = tree.layout(100.0, None).unwrap();
    assert_eq!(g[2].unwrap().border.right, 100.0);
    assert_eq!(g[5].unwrap().border.left, 30.0);
}

#[test]
fn the_root_scope_encloses_its_childrens_and_out_of_scope_reads_instantiate() {
    let root = node("page", vec![node("a", vec![])]);
    let tree = BoxTree::new(
        root,
        "#page { counter-reset:n 1; } #a { counter-reset:n 2; }
         #a:before { content:counters(n, \".\"); }",
    )
    .unwrap();
    assert_eq!(tree.text_leaves()[0].1.text(), "1.2");
    let root = node(
        "page",
        vec![node(
            "a",
            vec![
                node("b", vec![node("c", vec![])]),
                node("d", vec![node("e", vec![])]),
            ],
        )],
    );
    let tree = BoxTree::new(
        root,
        "#a:before { content:counter(x); }
         #c, #e { counter-increment:x; }
         #e:before { content:counter(x); }",
    )
    .unwrap();
    let texts: Vec<_> = tree.text_leaves().iter().map(|(_, l)| l.text()).collect();
    assert_eq!(texts, vec!["0", "2"]);
}

#[test]
fn quotes_follow_the_property_and_an_unmatched_close_prints_nothing() {
    let root = node("page", vec![node("a", vec![]), node("b", vec![])]);
    let tree = BoxTree::new(
        root,
        "#a:before { content:\"x\" close-quote; }
         #b { quotes:\"<\" \">\" \"[\" \"]\"; }
         #b:before { content:open-quote open-quote open-quote; }
         #b:after { content:close-quote close-quote close-quote; }",
    )
    .unwrap();
    let texts: Vec<_> = tree.text_leaves().iter().map(|(_, l)| l.text()).collect();
    assert_eq!(texts, vec!["x", "<[[]]>"]);
    let root = node("page", vec![node("a", vec![])]);
    let tree = BoxTree::new(
        root,
        "#page { quotes:none; } #a:before { content:\"x\" open-quote; }",
    )
    .unwrap();
    assert_eq!(tree.text_leaves()[0].1.text(), "x");
}

#[test]
fn hidden_pseudo_with_an_inside_marker_does_not_count_and_counter_inherit_copies() {
    let root = node("page", vec![node("li", vec![]), node("next", vec![])]);
    let tree = BoxTree::new(
        root,
        "#page { counter-reset:n; }
         #li { display:list-item; list-style-position:inside; }
         #li:before { display:none; counter-increment:n 5; content:\"gone\"; }
         #next:before { content:counter(n); }",
    )
    .unwrap();
    let texts: Vec<_> = tree.text_leaves().iter().map(|(_, l)| l.text()).collect();
    assert_eq!(texts[1], "0");
    let root = node("page", vec![node("a", vec![])]);
    let tree = BoxTree::new(
        root,
        "#page { counter-reset:n 7; } #a { counter-reset:inherit; }
         #a:before { content:counters(n, \".\"); }",
    )
    .unwrap();
    assert_eq!(tree.text_leaves()[0].1.text(), "7.7");
}

#[test]
fn anonymous_parts_cover_nested_tables_rows_of_inline_blocks_and_blank_text() {
    // A table directly in a table sits in an anonymous cell, with its own content.
    let root = node("t", vec![node("inner", vec![node("c", vec![])])]);
    let tree = BoxTree::new(
        root,
        "#t, #inner { display:table; } #c { display:table-cell; width:9px; height:4px; }",
    )
    .unwrap();
    let g = tree.layout(100.0, None).unwrap();
    assert_eq!(g[2].unwrap().border.bottom, 4.0);
    assert_eq!(g[0].unwrap().border.bottom, 4.0);
    // Inline-blocks in a row share one anonymous cell, in one line.
    let root = node("r", vec![node("a", vec![]), node("b", vec![])]);
    let tree = BoxTree::new(
        root,
        "#r { display:table-row; }
         #a, #b { display:inline-block; width:10px; height:2px; }",
    )
    .unwrap();
    let g = tree.layout(100.0, None).unwrap();
    assert_eq!(
        (g[1].unwrap().border.left, g[2].unwrap().border.left),
        (0.0, 10.0)
    );
    assert_eq!(g[1].unwrap().border.top, g[2].unwrap().border.top);
    // Blank text between loose cells doesn't split their table.
    let root = node(
        "page",
        vec![node("a", vec![]), text("gap", "  "), node("b", vec![])],
    );
    let tree = BoxTree::new(
        root,
        "#a, #b { display:table-cell; width:10px; height:2px; }",
    )
    .unwrap();
    let g = tree.layout(100.0, None).unwrap();
    assert_eq!(g[3].unwrap().border.left, 10.0);
    assert_eq!(g[3].unwrap().border.top, 0.0);
    // White space between inline runs inside an anonymous cell is kept.
    let root = node(
        "r",
        vec![text("a", "one"), text("gap", " "), text("b", "two")],
    );
    let tree = BoxTree::new(root, "#r { display:table-row; }").unwrap();
    assert_eq!(tree.text_leaves()[0].1.text(), "one two");
    // A float after a loose table stays after it.
    let root = node("page", vec![node("a", vec![]), node("f", vec![])]);
    let tree = BoxTree::new(
        root,
        "#a { display:table-cell; width:10px; height:20px; }
         #f { float:left; width:5px; height:5px; }",
    )
    .unwrap();
    let g = tree.layout(100.0, None).unwrap();
    assert_eq!(g[2].unwrap().border.top, 20.0);
}

#[test]
fn collapsed_borders_share_a_line_and_keep_the_table_flush() {
    let root = node(
        "t",
        vec![
            node("r1", vec![node("a", vec![]), node("b", vec![])]),
            node("r2", vec![node("c", vec![]), node("d", vec![])]),
        ],
    );
    let tree = BoxTree::new(
        root,
        "#t { display:table; border-collapse:collapse; border-spacing:9px; border:7px solid; }
         #r1, #r2 { display:table-row; }
         #a, #b, #c, #d { display:table-cell; width:10px; height:5px; border:2px solid; }",
    )
    .unwrap();
    let g = tree.layout(200.0, None).unwrap();
    let edge = |index: usize| g[index].unwrap().border;
    // Cells are 14 wide by 9 tall, and overlap by their shared 2px border.
    assert_eq!((edge(2).left, edge(2).right), (0.0, 14.0));
    assert_eq!((edge(3).left, edge(3).right), (12.0, 26.0));
    assert_eq!((edge(2).top, edge(5).top), (0.0, 7.0));
    assert_eq!((edge(0).right, edge(0).bottom), (26.0, 16.0));
    // The table's own border gives way to its outer cells'.
    assert_eq!(tree.paints()[0].border[0], None);
    assert_eq!(tree.paints()[2].border[0], Some([0.0, 0.0, 0.0]));
}

#[test]
fn collapse_without_cell_borders_keeps_the_table_border_and_no_spacing() {
    let root = node(
        "t",
        vec![node("r", vec![node("a", vec![]), node("b", vec![])])],
    );
    let tree = BoxTree::new(
        root,
        "#t { display:table; border-collapse:collapse; border-spacing:9px; border:3px solid; }
         #r { display:table-row; }
         #a, #b { display:table-cell; width:10px; height:5px; }",
    )
    .unwrap();
    let g = tree.layout(200.0, None).unwrap();
    assert_eq!(g[2].unwrap().border.left, 3.0);
    assert_eq!(g[3].unwrap().border.left, 13.0);
    assert!(tree.paints()[0].border[0].is_some());
}

#[test]
fn collapse_overlaps_per_axis_and_a_loose_cell_keeps_its_paint() {
    let root = node(
        "t",
        vec![
            node("r1", vec![node("a", vec![])]),
            node("r2", vec![node("b", vec![])]),
        ],
    );
    let tree = BoxTree::new(
        root,
        "#t { display:table; border-collapse:collapse; width:100%; padding:20px; }
         #r1, #r2 { display:table-row; }
         #a, #b { display:table-cell; border-left:10px solid; height:5px; }",
    )
    .unwrap();
    let g = tree.layout(200.0, None).unwrap();
    assert_eq!(g[4].unwrap().border.top, 5.0);
    assert_eq!(g[0].unwrap().border.right, 200.0);
    assert_eq!(g[2].unwrap().border.left, 0.0);
    let root = node("page", vec![node("a", vec![])]);
    let tree = BoxTree::new(
        root,
        "#page { border-collapse:collapse; }
         #a { display:table-cell; border:2px solid #ff0000; }",
    )
    .unwrap();
    assert_eq!(tree.paints()[1].border[0], Some([1.0, 0.0, 0.0]));
}

#[test]
fn background_image_properties_cascade_and_remote_urls_are_ignored() {
    let root = node(
        "page",
        vec![node("a", vec![]), node("b", vec![]), node("c", vec![])],
    );
    let tree = BoxTree::new(
        root,
        "#a { background:url(tile.png) repeat-x fixed 10px 50%; }
         #b { background-image:url(http://example.com/x.png); }
         #c { background-image:url(data:image/png;base64,AAAA); }",
    )
    .unwrap();
    let images = tree.background_images();
    assert_eq!(images.len(), 1);
    let (at, value) = images[0];
    assert_eq!(at, 1);
    assert_eq!(value.uri, "tile.png");
    assert!(value.repeat_x && !value.repeat_y && value.fixed);
    use iced_css::evaluation::tree_layout::BackgroundOffset;
    assert_eq!(
        value.position,
        [BackgroundOffset::Px(10.0), BackgroundOffset::Percent(50.0)]
    );
    // Only files under the base: no absolute paths and no climbing out.
    for escape in ["/etc/x.png", "../x.png", "a/../../x.png", "c:x.png"] {
        let root = node("page", vec![node("a", vec![])]);
        let css = format!("#a {{ background-image:url({escape}); }}");
        assert!(
            BoxTree::new(root, &css)
                .unwrap()
                .background_images()
                .is_empty()
        );
    }
    let root = node("page", vec![node("a", vec![])]);
    let css = "#a { background-image:url(./img/x.png); }";
    assert_eq!(
        BoxTree::new(root, css).unwrap().background_images().len(),
        1
    );
    // Inherit reads the parent's values even if the parent has no image.
    let root = node("page", vec![node("a", vec![])]);
    let css = "#page { background-repeat:no-repeat; }
        #a { background-repeat:inherit; background-image:url(x.png); }";
    let tree = BoxTree::new(root, css).unwrap();
    let inherited = tree.background_images()[0].1.clone();
    assert!(!inherited.repeat_x && !inherited.repeat_y);
    // display none has no image to load.
    let root = node("page", vec![node("a", vec![])]);
    let tree = BoxTree::new(root, "#a { display:none; background-image:url(x.png); }").unwrap();
    assert!(tree.background_images().is_empty());
}

#[test]
fn inline_tables_line_up_with_inline_blocks() {
    let root = node(
        "page",
        vec![
            node("b", vec![]),
            node("t", vec![node("r", vec![node("c", vec![])])]),
        ],
    );
    let tree = BoxTree::new(
        root,
        "#b { display:inline-block; width:20px; height:4px; }
         #t { display:inline-table; }
         #r { display:table-row; }
         #c { display:table-cell; width:15px; height:9px; }",
    )
    .unwrap();
    let g = tree.layout(100.0, None).unwrap();
    assert_eq!(
        (g[2].unwrap().border.left, g[2].unwrap().border.right),
        (20.0, 35.0)
    );
    assert_eq!(g[0].unwrap().border.bottom, 9.0);
    assert_eq!(g[1].unwrap().border.top, 5.0);
}

#[test]
fn restyling_swaps_sheets_later_over_earlier_and_keeps_the_tree() {
    let root = node("page", vec![node("a", vec![]), node("b", vec![])]);
    let tree = BoxTree::new(root, "#a { height:5px; }").unwrap();
    assert_eq!(
        tree.layout(100.0, None).unwrap()[1].unwrap().border.bottom,
        5.0
    );
    let next = tree
        .restyled(&[
            "#a { height:9px; } #b { height:1px; }",
            "#a { height:20px; }",
        ])
        .unwrap();
    let g = next.layout(100.0, None).unwrap();
    assert_eq!(g[1].unwrap().border.bottom, 20.0);
    assert_eq!(g[2].unwrap().border.bottom, 21.0);
    // The first tree still lays out as it did, and importance beats order.
    assert_eq!(
        tree.layout(100.0, None).unwrap()[1].unwrap().border.bottom,
        5.0
    );
    let next = tree
        .restyled(&["#a { height:9px !important; }", "#a { height:20px; }"])
        .unwrap();
    assert_eq!(
        next.layout(100.0, None).unwrap()[1].unwrap().border.bottom,
        9.0
    );
    // No sheets is legal: the user agent's defaults remain.
    assert!(tree.restyled(&[]).is_ok());
}
