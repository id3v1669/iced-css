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
    let error = BoxTree::new(page(), "#note { display:inline; }").unwrap_err();
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
        .layout_text(100.0, None, &mut |text, size, width| {
            seen.push((text.to_owned(), size, width));
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
        .layout_text(100.0, None, &mut |_, _, width| {
            assert_eq!(width, 50.0);
            20.0
        })
        .unwrap();
    assert_eq!(g[1].unwrap().content.top, 16.0);
    assert_eq!(g[1].unwrap().border.bottom, 45.0);
}

#[test]
fn adjacent_inline_text_is_refused_until_line_layout_exists() {
    let root = node("page", vec![text("first", "a"), text("second", "b")]);
    let error = BoxTree::new(root, "").unwrap_err();
    assert!(error.to_string().contains("display"));
    let root = node(
        "page",
        vec![text("first", "a"), node("gap", vec![]), text("second", "b")],
    );
    assert!(BoxTree::new(root, "").is_ok());
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
        .layout_text(100.0, None, &mut |content, _, width| {
            widths.push((content.to_owned(), width));
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
