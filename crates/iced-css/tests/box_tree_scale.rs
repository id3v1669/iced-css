//! How the tree's costs grow. Not a limit, a measurement: run with `-- --ignored --nocapture`.
use iced::Theme;
use iced_css::{
    box_tree::{BoxTree, capture_limits},
    capture::{CapturedElement, adapters},
};
use iced_css_core::source::AttributeLimits;
use std::{num::NonZeroUsize, time::Instant};

type Node = CapturedElement<'static, u8, Theme, iced::Renderer>;

fn attrs() -> AttributeLimits {
    let bound = NonZeroUsize::new(1000).unwrap();
    AttributeLimits {
        entries: bound,
        retained_bytes: bound,
        work: bound,
    }
}
fn node(id: String, children: Vec<Node>) -> Node {
    adapters::column_from_vec(children, capture_limits())
        .unwrap()
        .style_id(&id, attrs())
        .unwrap()
        .erase()
}

#[test]
#[ignore = "a measurement"]
fn cost_by_size() {
    for (count, rules) in [
        (250_usize, 2_usize),
        (1000, 2),
        (4000, 2),
        (16000, 2),
        (40000, 2),
        (1000, 100),
        (1000, 1000),
    ] {
        let cards: Vec<Node> = (0..count)
            .map(|at| {
                let title = adapters::text("a title", capture_limits())
                    .unwrap()
                    .style_id(&format!("t{at}"), attrs())
                    .unwrap()
                    .erase();
                node(format!("c{at}"), vec![title])
            })
            .collect();
        let root = node("page".to_owned(), cards);
        let mut css = "#page { padding:8px; }".to_owned();
        for rule in 0..rules {
            css.push_str(&format!("#c{rule} {{ margin:4px; color:#102030; }}"));
        }
        let started = Instant::now();
        let tree = BoxTree::new(root, &css).unwrap();
        let built = started.elapsed();
        let started = Instant::now();
        let geometry = tree
            .layout_text(800.0, Some(600.0), &mut |_, _| 20.0)
            .unwrap();
        let laid = started.elapsed();
        assert!(geometry.len() > count);
        println!("{count:>6} cards {rules:>5} rules: build {built:>12.2?}  layout {laid:>12.2?}");
    }
}
