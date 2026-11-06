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
fn renders_a_table_from_a_stylesheet() {
    let row = |id: &str, a: &'static str, b: &'static str, c: &'static str| {
        node(
            id,
            vec![
                node(&format!("{id}a"), vec![text(&format!("{id}at"), a)]),
                node(&format!("{id}b"), vec![text(&format!("{id}bt"), b)]),
                node(&format!("{id}c"), vec![text(&format!("{id}ct"), c)]),
            ],
        )
    };
    let root = node(
        "page",
        vec![node(
            "table",
            vec![
                node("cap", vec![text("captext", "Quarterly results")]),
                node(
                    "foot",
                    vec![row(
                        "f",
                        "Total",
                        "12",
                        "A much longer note in the last column",
                    )],
                ),
                node(
                    "body",
                    vec![
                        row("r1", "North", "5", "Up"),
                        row("r2", "South", "7", "Down"),
                    ],
                ),
                node("head", vec![row("h", "Region", "Sales", "Note")]),
            ],
        )],
    );
    let cells = ["h", "r1", "r2", "f"]
        .iter()
        .flat_map(|r| ["a", "b", "c"].map(|c| format!("#{r}{c}")))
        .collect::<Vec<_>>()
        .join(", ");
    let css = format!(
        "#page {{ padding:16px; background-color:#ffffff; color:#222222; }}
         #table {{ display:table; width:70%; border-spacing:4px 2px; background-color:#cfd8dc; }}
         #cap {{ display:table-caption; font-weight:bold; padding:6px; }}
         #head {{ display:table-header-group; font-weight:bold; background-color:#90a4ae; }}
         #body {{ display:table-row-group; }}
         #foot {{ display:table-footer-group; font-style:italic; }}
         #h, #r1, #r2, #f {{ display:table-row; }}
         #r2 {{ background-color:#fff3e0; }}
         {cells} {{ display:table-cell; padding:4px 8px; border:1px solid #607d8b; }}
         #r1b, #r2b, #fb {{ text-align:right; }}"
    );
    let view: Element<'_, u8> = BoxTree::new(root, &css).unwrap().view();
    let mut ui = iced_test::simulator(view);
    let snapshot = ui.snapshot(&Theme::Light).unwrap();
    let path = std::env::temp_dir().join("iced-css-box-tree-table");
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    assert!(snapshot.matches_image(path.join("table.png")).unwrap());
}

#[test]
fn inline_blocks_lay_out_as_a_navigation_row() {
    let item = |id: &str, label: &'static str| node(id, vec![text(&format!("{id}t"), label)]);
    let root = node(
        "nav",
        vec![
            item("home", "Home"),
            item("docs", "Documentation"),
            item("about", "About"),
        ],
    );
    let css = "#nav { text-align:center; background-color:#263238; padding:8px; }
        #home, #docs, #about { display:inline-block; padding:6px 12px; margin:0 4px;
                               background-color:#455a64; color:#ffffff; }
        #docs:hover { background-color:#607d8b; }";
    let view: Element<'_, u8> = BoxTree::new(root, css)
        .unwrap()
        .interactive()
        .on_press("about", 1)
        .into();
    let mut ui = iced_test::simulator(view);
    let snapshot = ui.snapshot(&Theme::Light).unwrap();
    let path = std::env::temp_dir().join("iced-css-box-tree-nav");
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    assert!(snapshot.matches_image(path.join("nav.png")).unwrap());
    let spaced = node(
        "page",
        vec![node("b", vec![]), text("t", "   "), node("c", vec![])],
    );
    assert!(BoxTree::new(spaced, "#b, #c { display:inline-block; }").is_ok());
}

#[test]
fn text_widens_past_a_shorter_float_on_the_real_renderer() {
    let long = "This paragraph is long enough to run beside the short float for a few lines, \
        and then, once it has passed the float's bottom edge, the following lines should use \
        the whole width of the page again instead of staying in the narrow band they started \
        in, which is what real line boxes do. The words keep going so that a good number of \
        lines land below the float and the difference is plain to see at a glance.";
    let root = node(
        "page",
        vec![
            node("side", vec![]),
            text("copy", long),
            node("after", vec![]),
        ],
    );
    let css = "#page { width:420px; padding:8px; }
        #side { float:left; width:120px; height:60px; margin-right:8px; background-color:#ffcc80; }
        #after { height:8px; background-color:#90caf9; }";
    let view: Element<'_, u8> = BoxTree::new(root, css)
        .unwrap()
        .interactive()
        .on_press("copy", 1)
        .into();
    let mut ui = iced_test::simulator(view);
    let snapshot = ui.snapshot(&Theme::Light).unwrap();
    let path = std::env::temp_dir().join("iced-css-box-tree-widen");
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    assert!(snapshot.matches_image(path.join("widen.png")).unwrap());
    let view = || -> Element<'_, u8> {
        BoxTree::new(
            node(
                "page",
                vec![
                    node("side", vec![]),
                    text("copy", long),
                    node("after", vec![]),
                ],
            ),
            css,
        )
        .unwrap()
        .interactive()
        .on_press("copy", 1)
        .on_press("side", 2)
        .into()
    };
    // Left of where the float's edge was, the widened lines reach the text.
    assert_eq!(click(view(), Point::new(20.0, 100.0)), vec![1]);
    // The last line sits below all of them, so the second part really starts lower down.
    assert_eq!(click(view(), Point::new(20.0, 183.0)), vec![1]);
    // The line that overlaps the float's bottom stays narrow: no text runs over the float.
    assert_eq!(click(view(), Point::new(20.0, 60.0)), vec![2]);
}

#[test]
fn centered_text_still_splits_and_bidirectional_text_stays_narrow() {
    let long = "This paragraph is long enough to run beside the short float for a few lines, \
        and then, once it has passed the float's bottom edge, the following lines should use \
        the whole width of the page again, and it keeps going for a good while after that.";
    let css = |align: &str| {
        format!(
            "#page {{ width:420px; padding:8px; text-align:{align}; }}
             #side {{ float:left; width:120px; height:60px; margin-right:8px; }}"
        )
    };
    let view = |content: &'static str, align: &str| -> Element<'_, u8> {
        BoxTree::new(
            node("page", vec![node("side", vec![]), text("copy", content)]),
            &css(align),
        )
        .unwrap()
        .interactive()
        .on_press("copy", 1)
        .on_press("page", 3)
        .into()
    };
    // Centered lines below the float come back to full width, past the float's right edge
    // to its left: a press well left of the old band's start lands in text.
    assert_eq!(
        click(view(long, "center"), Point::new(40.0, 110.0)),
        vec![1]
    );
    let mixed = "\u{5e9}\u{5dc}\u{5d5}\u{5dd} words words words words words words words words \
        words words words words words words words words words words words words words words \
        words words words words words words words words words words words words words words";
    assert_eq!(click(view(mixed, "left"), Point::new(40.0, 110.0)), vec![3]);
}

/// A small RGB PNG, written with stored deflate blocks so the test needs no encoder.
#[cfg(feature = "image")]
fn checker_png(size: u32) -> Vec<u8> {
    fn crc(data: &[u8]) -> u32 {
        let mut value = !0_u32;
        for byte in data {
            value ^= u32::from(*byte);
            for _ in 0..8 {
                value = if value & 1 == 1 {
                    (value >> 1) ^ 0xEDB8_8320
                } else {
                    value >> 1
                };
            }
        }
        !value
    }
    fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], body: &[u8]) {
        out.extend((body.len() as u32).to_be_bytes());
        let mut tagged = kind.to_vec();
        tagged.extend_from_slice(body);
        out.extend_from_slice(&tagged);
        out.extend(crc(&tagged).to_be_bytes());
    }
    let mut raw = Vec::new();
    for y in 0..size {
        raw.push(0);
        for x in 0..size {
            let on = (x < size / 2) == (y < size / 2);
            raw.extend(if on { [230, 74, 25] } else { [255, 224, 178] });
        }
    }
    let mut stored = vec![0x78, 0x01];
    let blocks: Vec<&[u8]> = raw.chunks(65_535).collect();
    for (index, block) in blocks.iter().enumerate() {
        stored.push(u8::from(index + 1 == blocks.len()));
        stored.extend((block.len() as u16).to_le_bytes());
        stored.extend((!(block.len() as u16)).to_le_bytes());
        stored.extend_from_slice(block);
    }
    let (mut a, mut b) = (1_u32, 0_u32);
    for byte in &raw {
        a = (a + u32::from(*byte)) % 65_521;
        b = (b + a) % 65_521;
    }
    stored.extend(((b << 16) | a).to_be_bytes());
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut header = Vec::new();
    header.extend(size.to_be_bytes());
    header.extend(size.to_be_bytes());
    header.extend([8, 2, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &header);
    chunk(&mut out, b"IDAT", &stored);
    chunk(&mut out, b"IEND", &[]);
    out
}

#[cfg(feature = "image")]
#[test]
fn renders_background_images_from_a_stylesheet() {
    let path = std::env::temp_dir().join("iced-css-box-tree-images");
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    std::fs::write(path.join("tile.png"), checker_png(32)).unwrap();
    let root = node(
        "page",
        vec![
            node("repeat", vec![text("t1", "repeat")]),
            node("across", vec![text("t2", "repeat-x, centred")]),
            node("single", vec![node("child", vec![])]),
            node(
                "missing",
                vec![text("t3", "a missing file shows the color")],
            ),
        ],
    );
    let css = "#page { padding:16px; background:#fafafa url(tile.png) repeat-y; }
        #repeat, #across, #single, #missing { height:96px; margin-bottom:12px; width:400px;
                                              border:2px solid #37474f; }
        #repeat { background-image:url(tile.png); background-color:#ffffff; }
        #t1 { display:inline; color:#ffffff; background-color:#37474f;
              text-decoration:underline; font-weight:bold; }
        #across { background:url(tile.png) repeat-x 50% 50%; }
        #single { background:#e0f2f1 url(tile.png) no-repeat right bottom; }
        #child { width:40px; height:40px; background-color:#00796b; }
        #missing { background:#fff9c4 url(no-such-file.png); }";
    let view: Element<'_, u8> = BoxTree::with_base(root, css, &path).unwrap().view();
    let mut ui = iced_test::simulator(view);
    let snapshot = ui.snapshot(&Theme::Light).unwrap();
    assert!(snapshot.matches_image(path.join("images.png")).unwrap());
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

#[test]
fn unbreakable_text_can_be_scrolled_to() {
    let root = node(
        "page",
        vec![node(
            "list",
            vec![text("line", "a line that is much wider than its box")],
        )],
    );
    let tree = BoxTree::new(
        root,
        "#page { background:white; }
         #list { overflow:auto; width:60px; height:40px; white-space:nowrap; }
         #line { color:black; }",
    )
    .unwrap();
    let view: Element<'_, u8> = tree.view();
    let mut ui = iced_test::simulator(view);
    // A first comparison records the picture, so a second one sees any movement.
    let picture = std::env::temp_dir().join(format!("scroll-text-{}.png", std::process::id()));
    assert!(
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(&picture)
            .unwrap()
    );
    ui.point_at(Point::new(10.0, 10.0));
    let _ = ui.simulate([iced::Event::Mouse(iced::mouse::Event::WheelScrolled {
        delta: iced::mouse::ScrollDelta::Pixels { x: -40.0, y: 0.0 },
    })]);
    assert!(
        !ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(&picture)
            .unwrap()
    );
}

#[test]
fn a_frame_of_an_animation_draws_between_its_ends() {
    use iced_css::{animation::clock::AnimationTiming, box_tree::BoxTreeAnimations};
    use std::time::{Duration, Instant};
    let root = node("page", vec![node("bar", vec![text("line", "words")])]);
    let from = BoxTree::new(
        root,
        "#page { background-color:white; }
         #bar { width:100px; height:40px; background-color:#ff0000; color:#000000; border:4px solid; }",
    )
    .unwrap();
    let to = from
        .restyled(&[
            "#page { background-color:white; }
             #bar { width:300px; height:40px; background-color:#0000ff; color:#ffffff; border:4px solid; }",
        ])
        .unwrap();
    let mut animations = BoxTreeAnimations::new();
    let start = Instant::now();
    let timing = AnimationTiming::new(Duration::from_secs(1), Duration::ZERO);
    let count = animations
        .request(
            &from,
            &to,
            "bar",
            &["width", "background-color", "color"],
            timing,
            start,
        )
        .unwrap();
    assert_eq!(count, 3);
    animations.advance(start + Duration::from_millis(500), &to);
    let view: Element<'_, u8> = animations.presented(&to).view();
    let mut ui = iced_test::simulator(view);
    assert!(
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(
                std::env::temp_dir().join(format!("animation-frame-{}.png", std::process::id()))
            )
            .unwrap()
    );
}

#[test]
fn text_sits_in_a_line_beside_inline_blocks() {
    let root = node(
        "page",
        vec![
            text("before", "before"),
            node("chip", vec![]),
            text("after", "after"),
        ],
    );
    let tree = BoxTree::new(
        root,
        "#page { background-color:white; color:black; }
         #chip { display:inline-block; width:40px; height:30px; background-color:#2a4d69; }",
    )
    .unwrap();
    let view: Element<'_, u8> = tree.view();
    let mut ui = iced_test::simulator(view);
    assert!(
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(
                std::env::temp_dir().join(format!("row-text-{}.png", std::process::id()))
            )
            .unwrap()
    );
}

#[test]
fn a_scrollers_own_text_can_be_scrolled_to() {
    let root = node(
        "page",
        vec![text("line", "a line that is much wider than its box")],
    );
    let tree = BoxTree::new(
        root,
        "#page { background:white; } #line { display:block; overflow:auto; width:60px; white-space:nowrap; color:black; }",
    )
    .unwrap();
    let view: Element<'_, u8> = tree.view();
    let mut ui = iced_test::simulator(view);
    let picture = std::env::temp_dir().join(format!(
        "a_scrollers_own_text_can_be_scrolled_to-{}.png",
        std::process::id()
    ));
    assert!(
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(&picture)
            .unwrap()
    );
    ui.point_at(Point::new(10.0, 10.0));
    let _ = ui.simulate([iced::Event::Mouse(iced::mouse::Event::WheelScrolled {
        delta: iced::mouse::ScrollDelta::Pixels { x: -40.0, y: 0.0 },
    })]);
    assert!(
        !ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(&picture)
            .unwrap()
    );
}

#[test]
fn text_a_child_clips_does_not_extend_the_scroller() {
    let root = node(
        "page",
        vec![node(
            "list",
            vec![text("line", "a line that is much wider than its box")],
        )],
    );
    let tree = BoxTree::new(
        root,
        "#page { background:white; } #list { overflow:auto; width:100px; white-space:nowrap; color:black; } #line { display:block; overflow:hidden; }",
    )
    .unwrap();
    let view: Element<'_, u8> = tree.view();
    let mut ui = iced_test::simulator(view);
    let picture = std::env::temp_dir().join(format!(
        "text_a_child_clips_does_not_extend_the_scroller-{}.png",
        std::process::id()
    ));
    assert!(
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(&picture)
            .unwrap()
    );
    ui.point_at(Point::new(10.0, 10.0));
    let _ = ui.simulate([iced::Event::Mouse(iced::mouse::Event::WheelScrolled {
        delta: iced::mouse::ScrollDelta::Pixels { x: -40.0, y: 0.0 },
    })]);
    assert!(
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(&picture)
            .unwrap()
    );
}
