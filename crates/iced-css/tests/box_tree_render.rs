//! The tree through iced's real renderer and text shaper.
use iced::{Element, Point, Theme};
use iced_css::{
    box_tree::{BoxTree, capture_limits},
    capture::{CapturedElement, adapters},
};
use iced_css_core::source::AttributeLimits;
use std::num::NonZeroUsize;

type Node = CapturedElement<'static, u8, Theme, iced::Renderer>;

fn limits() -> AttributeLimits {
    let bound = NonZeroUsize::new(1000).unwrap();
    AttributeLimits {
        entries: bound,
        retained_bytes: bound,
        work: bound,
    }
}
fn node(id: &str, children: Vec<Node>) -> Node {
    adapters::column_from_vec(children, capture_limits())
        .unwrap()
        .style_id(id, limits())
        .unwrap()
        .erase()
}
fn text(id: &str, content: &'static str) -> Node {
    adapters::text(content, capture_limits())
        .unwrap()
        .style_id(id, limits())
        .unwrap()
        .erase()
}
fn click(view: Element<'_, u8>, at: Point) -> Vec<u8> {
    let mut ui = iced_test::simulator(view);
    ui.point_at(at);
    let _ = ui.simulate(iced_test::simulator::click());
    ui.into_messages().collect()
}

#[test]
fn a_press_on_inline_text_goes_to_the_run_under_the_point() {
    let view = || -> Element<'_, u8> {
        BoxTree::new(
            node(
                "page",
                vec![text("a", "AAAAAAAAAA"), text("b", "BBBBBBBBBB")],
            ),
            "#page { height:100px; }",
        )
        .unwrap()
        .interactive()
        .on_press("a", 1)
        .on_press("b", 2)
        .on_press("page", 3)
        .into()
    };
    assert_eq!(click(view(), Point::new(5.0, 10.0)), vec![1]);
    assert_eq!(click(view(), Point::new(160.0, 10.0)), vec![2]);
    // Past the end of the line and below it, the point is only on the parent box.
    assert_eq!(click(view(), Point::new(600.0, 10.0)), vec![3]);
    assert_eq!(click(view(), Point::new(5.0, 80.0)), vec![3]);
}

#[test]
fn centered_and_right_aligned_text_sits_where_it_is_hit() {
    let view = |align: &str| -> Element<'_, u8> {
        BoxTree::new(
            node("page", vec![text("a", "AAAAAAAAAA")]),
            &format!("#page {{ text-align:{align}; height:100px; }}"),
        )
        .unwrap()
        .interactive()
        .on_press("a", 1)
        .on_press("page", 2)
        .into()
    };
    // The simulator window is 1024 wide, so the centered run straddles 512.
    assert_eq!(click(view("center"), Point::new(5.0, 10.0)), vec![2]);
    assert_eq!(click(view("center"), Point::new(512.0, 10.0)), vec![1]);
    assert_eq!(click(view("center"), Point::new(1000.0, 10.0)), vec![2]);
    assert_eq!(click(view("right"), Point::new(990.0, 10.0)), vec![1]);
    assert_eq!(click(view("right"), Point::new(512.0, 10.0)), vec![2]);
    assert_eq!(click(view("left"), Point::new(5.0, 10.0)), vec![1]);
}

#[test]
fn renders_inline_runs_lists_floats_clips_and_stacking() {
    let root = node(
        "page",
        vec![
            node("side", vec![text("sidetext", "A float")]),
            text("one", "Inline run "),
            node("span", vec![text("two", "bold red underlined ")]),
            text(
                "three",
                "then plain text that keeps going until it wraps beside the float and past it, because a paragraph is one flow of runs.",
            ),
            node(
                "list",
                vec![
                    node("i1", vec![text("t1", "First item")]),
                    node("i2", vec![text("t2", "Second item")]),
                    node("i3", vec![text("t3", "Third, inside marker")]),
                ],
            ),
            node("clip", vec![node("tall", vec![])]),
            node("under", vec![]),
            node("over", vec![]),
            node("quote", vec![text("q", "generated quotes")]),
        ],
    );
    let css = "
        #page { padding:16px 16px 16px 48px; background-color:#fafafa; color:#222222; }
        #side { float:right; width:180px; height:120px; margin-left:12px; padding:8px;
                background-color:#c8e6c9; border:2px solid #2e7d32; }
        #span { display:inline; color:#c62828; font-weight:bold; text-decoration:underline;
                background-color:#fff59d; }
        #list { margin:16px 0; }
        #i1, #i2 { display:list-item; list-style-type:decimal; }
        #i3 { display:list-item; list-style-type:square; list-style-position:inside; }
        #clip { overflow:hidden; height:30px; width:200px; background-color:#bbdefb; }
        #tall { height:100px; margin:8px; background-color:#1565c0; }
        #under { position:relative; z-index:2; top:8px; left:40px; width:120px; height:40px;
                 background-color:#ef9a9a; outline:3px solid #b71c1c; }
        #over { position:relative; z-index:1; width:120px; height:40px;
                background-color:#90caf9; }
        #quote { margin-top:12px; text-align:center; font-style:italic; font-size:20px; }
        #q:before { content:open-quote; color:#6a1b9a; }
        #q:after { content:close-quote; color:#6a1b9a; }
    ";
    let view: Element<'_, u8> = BoxTree::new(root, css).unwrap().view();
    let mut ui = iced_test::simulator(view);
    let snapshot = ui.snapshot(&Theme::Light).unwrap();
    let path = std::env::temp_dir().join("iced-css-box-tree-render");
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    assert!(snapshot.matches_image(path.join("page.png")).unwrap());
}
