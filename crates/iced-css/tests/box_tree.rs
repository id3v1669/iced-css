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
