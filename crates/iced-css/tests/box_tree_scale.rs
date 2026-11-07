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

#[test]
#[ignore = "a measurement"]
fn widget_cost_by_size() {
    use iced::{Element, Point, mouse};
    use iced_css::{animation::clock::AnimationTiming, box_tree::BoxTreeAnimations};
    use std::time::Duration;
    for count in [100_usize, 500, 2000] {
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
        let css = "#page { padding:8px; } #c1:hover { color:#ff0000; }";
        let tree = BoxTree::new(root, css).unwrap();
        // The renderer's first start is not part of what is measured.
        let view: Element<'_, u8> = tree.clone().view();
        let mut ui = iced_test::simulator(view);
        drop(ui.snapshot(&Theme::Light).unwrap());
        drop(ui);

        let started = Instant::now();
        let view: Element<'_, u8> = tree.clone().view();
        let mut ui = iced_test::simulator(view);
        let opened = started.elapsed();
        let started = Instant::now();
        drop(ui.snapshot(&Theme::Light).unwrap());
        let drawn = started.elapsed();
        let started = Instant::now();
        for step in 0..20 {
            ui.point_at(Point::new(50.0, 5.0 + step as f32));
            let _ = ui.simulate([iced::Event::Mouse(mouse::Event::CursorMoved {
                position: Point::new(50.0, 5.0 + step as f32),
            })]);
        }
        let moved = started.elapsed() / 20;
        drop(ui);

        let to = tree.restyled(&["#c1 { color:#ff0000; }"]).unwrap();
        let mut animations = BoxTreeAnimations::new();
        let start = Instant::now();
        let timing = AnimationTiming::new(Duration::from_secs(10), Duration::ZERO);
        animations
            .request(&tree, &to, "c1", &["color"], timing, start)
            .unwrap();
        let started = Instant::now();
        animations.advance(start + Duration::from_secs(1), &to);
        let shown = animations.presented(&to);
        shown
            .layout_text(800.0, Some(600.0), &mut |_, _| 20.0)
            .unwrap();
        let frame = started.elapsed();
        println!(
            "{count:>5} cards: open {opened:>10.2?}  draw {drawn:>10.2?}  move {moved:>10.2?}  presented layout {frame:>10.2?}"
        );
    }
}

#[test]
#[ignore = "a measurement"]
fn parse_cost_by_sheet_size() {
    for rules in [1000_usize, 4000, 16_000, 64_000] {
        let css = "a{color:red}".repeat(rules);
        let started = Instant::now();
        BoxTree::new(node("page".to_owned(), vec![]), &css).unwrap();
        println!("{rules:>6} rules: {:>10.2?}", started.elapsed());
    }
}
