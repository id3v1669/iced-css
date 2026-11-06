//! Manual check: a page layout drawn from a stylesheet. Resize the window: percentage
//! widths, float avoidance and the absolute badge follow without rebuilding the tree.
use iced::{Element, Theme};
use iced_css::{
    box_tree::{BoxTree, capture_limits},
    capture::{CapturedElement, adapters},
};
use iced_css_core::source::AttributeLimits;
use std::num::NonZeroUsize;

type Node = CapturedElement<'static, (), Theme, iced::Renderer>;

const CSS: &str = "
    #page    { background-color:#f4f1ea; padding:16px; height:100%; }
    #sidebar { float:left; width:25%; height:320px; margin-right:16px;
               background-color:#2a4d69; }
    #header  { height:64px; margin-bottom:16px; background-color:#4b86b4;
               border-bottom:4px solid #2a4d69; }
    #content { overflow:hidden; padding:12px; background-color:#ffffff;
               border:2px solid #adcbe3; }
    #card    { height:96px; margin-bottom:12px; background-color:#e7eff6; }
    #wide    { height:48px; width:60%; margin:0 auto; background-color:#63ace5; }
    #badge   { position:absolute; top:24px; right:24px; width:40px; height:40px;
               background-color:#d9534f; }
";

fn node(id: &str, children: Vec<Node>) -> Node {
    let bound = NonZeroUsize::new(1000).expect("positive");
    adapters::column_from_vec(children, capture_limits())
        .expect("capture")
        .style_id(
            id,
            AttributeLimits {
                entries: bound,
                retained_bytes: bound,
                work: bound,
            },
        )
        .expect("style id")
        .erase()
}

fn tree() -> BoxTree {
    let root = node(
        "page",
        vec![
            node("header", vec![]),
            node("sidebar", vec![]),
            node("content", vec![node("card", vec![]), node("wide", vec![])]),
            node("badge", vec![]),
        ],
    );
    BoxTree::new(root, CSS).expect("stylesheet")
}

fn view(tree: &BoxTree) -> Element<'_, ()> {
    tree.clone().view()
}

fn main() -> iced::Result {
    iced::application(tree, |_: &mut BoxTree, (): ()| {}, view)
        .title("iced-css box tree")
        .run()
}
