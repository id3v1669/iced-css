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
/// The Vulkan loader's first driver scan is not safe to run from several threads: parallel
/// tests crashed in it, inside `loader_scanned_icd_add`. Renderers are made one at a time.
fn simulator(view: Element<'_, u8>) -> iced_test::Simulator<'_, u8> {
    static CREATING: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _one_at_a_time = CREATING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    iced_test::simulator(view)
}

fn click(view: Element<'_, u8>, at: Point) -> Vec<u8> {
    let mut ui = simulator(view);
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
    let mut ui = simulator(view);
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
    let mut ui = simulator(view);
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
    let mut ui = simulator(view);
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
    let mut ui = simulator(view);
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
    let mut ui = simulator(view);
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
    let mut ui = simulator(view);
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
    let mut ui = simulator(view);
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
    let mut ui = simulator(view);
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
    let mut ui = simulator(view);
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
    let mut ui = simulator(view);
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

#[test]
fn justified_text_draws_differently_from_left_aligned() {
    const COPY: &str = "several words of ordinary text wrap across more than one line here so \
         that the lines have to stretch out to meet the widest of them while the last one does not";
    let draw = |align: &str, picture: &std::path::Path| {
        let root = node("page", vec![text("copy", COPY)]);
        let css = format!(
            "#page {{ background-color:white; color:black; width:300px; }}
             #copy {{ display:block; text-align:{align}; }}"
        );
        let tree = BoxTree::new(root, &css).unwrap();
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(picture)
            .unwrap()
    };
    let picture = std::env::temp_dir().join(format!("justify-{}.png", std::process::id()));
    assert!(draw("justify", &picture));
    assert!(draw("justify", &picture));
    assert!(!draw("left", &picture));
}

#[test]
fn clip_rect_cuts_an_absolute_box() {
    let root = node("page", vec![node("abs", vec![node("kid", vec![])])]);
    let tree = BoxTree::new(
        root,
        "#page { background-color:white; }
         #abs { position:absolute; top:10px; left:20px; width:100px; height:100px;
                background-color:#2a4d69; clip:rect(10px, 60px, auto, 20px); }
         #kid { width:300px; height:50px; background-color:#d9534f; }",
    )
    .unwrap();
    let view: Element<'_, u8> = tree.view();
    let mut ui = simulator(view);
    let name = format!("clip-rect-{}.png", std::process::id());
    assert!(
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(name))
            .unwrap()
    );
}

#[test]
fn blank_text_adds_no_scroll_range() {
    let root = node(
        "page",
        vec![node("list", vec![node("full", vec![]), text("gap", " ")])],
    );
    let tree = BoxTree::new(
        root,
        "#page { background-color:white; }
         #list { overflow:auto; height:50px; width:80px; background-color:#2a4d69; }
         #full { height:50px; background-color:#d9534f; }",
    )
    .unwrap();
    let view: Element<'_, u8> = tree.view();
    let mut ui = simulator(view);
    let picture = std::env::temp_dir().join(format!("blank-{}.png", std::process::id()));
    assert!(
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(&picture)
            .unwrap()
    );
    ui.point_at(Point::new(10.0, 10.0));
    let _ = ui.simulate([iced::Event::Mouse(iced::mouse::Event::WheelScrolled {
        delta: iced::mouse::ScrollDelta::Pixels { x: 0.0, y: -10.0 },
    })]);
    assert!(
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(&picture)
            .unwrap()
    );
}

#[test]
fn a_fixed_child_stays_inside_its_ancestors_clip() {
    let picture = |css: &str, name: &str| {
        let root = node("page", vec![node("abs", vec![node("kid", vec![])])]);
        let css = format!(
            "#page {{ background-color:white; }}
             #abs {{ position:absolute; width:10px; height:10px; {css} }}
             #kid {{ position:fixed; top:0; left:0; width:100px; height:100px; background-color:#d9534f; }}"
        );
        let tree = BoxTree::new(root, &css).unwrap();
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(name))
            .unwrap()
    };
    let name = format!("fixed-clip-{}.png", std::process::id());
    assert!(picture("clip:rect(0, 10px, 10px, 0);", &name));
    assert!(!picture("", &name));
}

#[test]
fn clip_does_nothing_on_a_static_box() {
    let picture = |css: &str, name: &str| {
        let root = node("page", vec![node("still", vec![])]);
        let css = format!(
            "#page {{ background-color:white; }}
             #still {{ width:100px; height:100px; background-color:#d9534f; {css} }}"
        );
        let tree = BoxTree::new(root, &css).unwrap();
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(name))
            .unwrap()
    };
    let name = format!("static-clip-{}.png", std::process::id());
    assert!(picture("", &name));
    assert!(picture("clip:rect(0, 10px, 10px, 0);", &name));
}

#[test]
fn text_indent_and_small_caps_draw() {
    let root = node(
        "page",
        vec![text(
            "copy",
            "Indented small capitals wrap across more than one line in a narrow box",
        )],
    );
    let tree = BoxTree::new(
        root,
        "#page { background-color:white; color:black; width:300px; }
         #copy { display:block; text-indent:40px; font-variant:small-caps; }",
    )
    .unwrap();
    let view: Element<'_, u8> = tree.view();
    let mut ui = simulator(view);
    let name = format!("indent-{}.png", std::process::id());
    assert!(
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(name))
            .unwrap()
    );
}

#[test]
fn tab_reaches_hosted_text_inputs_in_box_order() {
    let root = node(
        "page",
        vec![
            node("one", vec![]),
            node("card", vec![]),
            node("two", vec![]),
        ],
    );
    let tree = BoxTree::new(root, "#one, #card, #two { height:30px; width:200px; }").unwrap();
    let input = |message: u8| iced::widget::text_input("", "").on_input(move |_| message);
    let view: Element<'_, u8> = tree
        .interactive()
        .widget("one", input(1))
        .widget("two", input(2))
        .on_press("card", 9)
        .into();
    let mut ui = simulator(view);
    use iced::keyboard::key::Named;
    let _ = ui.tap_key(Named::Tab);
    let _ = ui.typewrite("a");
    let _ = ui.tap_key(Named::Tab);
    let _ = ui.tap_key(Named::Enter);
    let _ = ui.tap_key(Named::Tab);
    let _ = ui.typewrite("b");
    // Past the last, Tab comes round to the first.
    let _ = ui.tap_key(Named::Tab);
    let _ = ui.typewrite("c");
    let messages: Vec<u8> = ui.into_messages().collect();
    assert_eq!(messages, vec![1, 9, 2, 1]);
}

#[cfg(feature = "image")]
#[test]
fn list_style_images_replace_outside_markers_and_fall_back_to_text() {
    let path = std::env::temp_dir().join(format!("iced-css-markers-{}", std::process::id()));
    std::fs::create_dir_all(&path).unwrap();
    std::fs::write(path.join("dot.png"), checker_png(16)).unwrap();
    let matches = |css: &str, name: &str| {
        let root = node("page", vec![node("one", vec![text("a", "item")])]);
        let sheet = format!(
            "#page {{ background-color:white; color:black; padding:30px; }}
             #one {{ display:list-item; {css} }}"
        );
        let tree = BoxTree::with_base(root, &sheet, &path).unwrap();
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(path.join(name))
            .unwrap()
    };
    // The plain bullet first, which a missing image falls back to.
    assert!(matches("", "bullet.png"));
    assert!(matches("list-style-image:url(gone.png);", "bullet.png"));
    assert!(!matches("list-style-image:url(dot.png);", "bullet.png"));
    // The image draws without a marker type, and not at all when hidden.
    assert!(matches("list-style-image:url(dot.png);", "image.png"));
    assert!(matches(
        "list-style-type:none; list-style-image:url(dot.png);",
        "image.png"
    ));
    assert!(matches("list-style-type:none;", "none.png"));
    assert!(!matches(
        "list-style-type:none; list-style-image:url(dot.png);",
        "none.png"
    ));
    assert!(matches("visibility:hidden;", "hidden.png"));
    assert!(matches(
        "visibility:hidden; list-style-image:url(dot.png);",
        "hidden.png"
    ));
}

#[cfg(feature = "image")]
#[test]
fn inside_marker_images_take_room_in_the_line_and_fall_back_to_text() {
    let path = std::env::temp_dir().join(format!("iced-css-inside-{}", std::process::id()));
    std::fs::create_dir_all(&path).unwrap();
    std::fs::write(path.join("dot.png"), checker_png(16)).unwrap();
    let matches_in = |page: &str, css: &str, name: &str| {
        let root = node("page", vec![node("one", vec![text("a", "item")])]);
        let sheet = format!(
            "#page {{ background-color:white; color:black; padding:30px; {page} }}
             #one {{ display:list-item; list-style-position:inside; {css} }}"
        );
        let tree = BoxTree::with_base(root, &sheet, &path).unwrap();
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(path.join(name))
            .unwrap()
    };
    let matches = |css: &str, name: &str| matches_in("", css, name);
    assert!(matches("", "bullet.png"));
    assert!(matches("list-style-image:url(gone.png);", "bullet.png"));
    assert!(!matches("list-style-image:url(dot.png);", "bullet.png"));
    // The image replaces the marker type's text, whatever it is.
    assert!(matches("list-style-image:url(dot.png);", "image.png"));
    assert!(matches(
        "list-style-type:upper-roman; list-style-image:url(dot.png);",
        "image.png"
    ));
    assert!(matches(
        "list-style-type:none; list-style-image:url(dot.png);",
        "image.png"
    ));
    // The text after it moves past the image and half an em: 16 + 8 pixels.
    assert!(matches(
        "list-style-type:none; list-style-image:url(dot.png); line-height:16px;",
        "room.png"
    ));
    assert!(matches(
        "list-style-type:none; line-height:16px; text-indent:24px; background-image:url(dot.png);
         background-repeat:no-repeat;",
        "room.png"
    ));
    // Right to left, the image sits at the line's right end.
    assert!(matches(
        "direction:rtl; list-style-type:none; list-style-image:url(dot.png);
         line-height:16px;",
        "rtl-room.png"
    ));
    assert!(matches(
        "direction:rtl; list-style-type:none; line-height:16px; text-indent:24px;
         background-image:url(dot.png); background-repeat:no-repeat;
         background-position:right top;",
        "rtl-room.png"
    ));
    // A percentage indent's spaces come before the marker's, and the mark after them.
    assert!(matches(
        "direction:rtl; list-style-type:none; list-style-image:url(dot.png); line-height:16px;
         text-indent:10%;",
        "rtl-indent.png"
    ));
    // 10% of the 964 pixels across, as a fixed indent draws it.
    assert!(matches(
        "direction:rtl; list-style-type:none; list-style-image:url(dot.png); line-height:16px;
         text-indent:96.4px;",
        "rtl-indent.png"
    ));
    assert!(!matches(
        "direction:rtl; list-style-type:none; list-style-image:url(dot.png); line-height:16px;
         text-indent:20%;",
        "rtl-indent.png"
    ));
    // Measured to the right: the image's right edge 96 pixels in, the text 24 past it.
    assert!(matches(
        "direction:rtl; list-style-type:none; line-height:16px; text-indent:120px;
         background-image:url(dot.png); background-repeat:no-repeat;
         background-position:852px 0;",
        "rtl-indent.png"
    ));
    // Centered on a taller line: 20.8 pixels, so 2.4 above the image.
    assert!(matches(
        "list-style-type:none; list-style-image:url(dot.png);",
        "centered.png"
    ));
    assert!(matches(
        "list-style-type:none; text-indent:24px; background-image:url(dot.png);
         background-repeat:no-repeat; background-position:0 2.4px;",
        "centered.png"
    ));
    // Its room stays with the word after it when a float shrinks to fit.
    let narrow = "width:20px;";
    assert!(matches_in(
        narrow,
        "float:left; list-style-type:none; list-style-image:url(dot.png); line-height:16px;",
        "narrow.png"
    ));
    assert!(matches_in(
        narrow,
        "float:left; list-style-type:none; line-height:16px; text-indent:24px;
         background-image:url(dot.png); background-repeat:no-repeat;",
        "narrow.png"
    ));
    // Hidden, it keeps its room and draws nothing.
    assert!(matches(
        "visibility:hidden; list-style-image:url(dot.png);",
        "hidden.png"
    ));
    assert!(matches("visibility:hidden;", "hidden.png"));
}

#[test]
fn right_to_left_text_keeps_a_percentage_indent_through_a_new_layout() {
    let picture = |css: &str| {
        let root = node(
            "page",
            vec![node(
                "box",
                vec![node("one", vec![text("a", "item")]), node("tall", vec![])],
            )],
        );
        let sheet = format!(
            "#page {{ background-color:white; color:black; }}
             #box {{ height:100px; overflow:auto; direction:rtl; }}
             #one {{ text-indent:10%; {css} }} #tall {{ height:400px; }}"
        );
        let tree = BoxTree::new(root, &sheet).unwrap();
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        let before = ui.snapshot(&Theme::Light).unwrap();
        // Down and back: two layouts of the same tree.
        ui.point_at(Point::new(10.0, 10.0));
        for y in [-40.0, 40.0] {
            let _ = ui.simulate([iced::Event::Mouse(iced::mouse::Event::WheelScrolled {
                delta: iced::mouse::ScrollDelta::Pixels { x: 0.0, y },
            })]);
        }
        (before, ui.snapshot(&Theme::Light).unwrap())
    };
    let path = std::env::temp_dir().join(format!("iced-css-rtl-indent-{}", std::process::id()));
    std::fs::create_dir_all(&path).unwrap();
    let (blank, _) = picture("color:white;");
    assert!(blank.matches_image(path.join("blank.png")).unwrap());
    let (before, after) = picture("");
    assert!(before.matches_image(path.join("text.png")).unwrap());
    assert!(!before.matches_image(path.join("blank.png")).unwrap());
    assert!(after.matches_image(path.join("text.png")).unwrap());
}

#[test]
fn keys_in_a_hosted_input_stay_its_own() {
    use iced::keyboard::key::Named;
    let root = node("page", vec![node("card", vec![node("one", vec![])])]);
    let tree = BoxTree::new(
        root,
        "#card { padding:5px; } #one { height:30px; width:200px; }",
    )
    .unwrap();
    let view: Element<'_, u8> = tree
        .interactive()
        .widget("one", iced::widget::text_input("", "").on_input(|_| 1))
        .on_press("card", 9)
        .into();
    let mut ui = simulator(view);
    // The press box comes first in Tab order, then the input.
    let _ = ui.tap_key(Named::Tab);
    let _ = ui.tap_key(Named::Tab);
    let _ = ui.tap_key(Named::Enter);
    let messages: Vec<u8> = ui.into_messages().collect();
    assert!(!messages.contains(&9), "{messages:?}");
}

#[test]
fn narrow_columns_hold_whole_small_caps_words() {
    // A word's minimum width is its whole width, so a shrunken column matches an unwrapped one.
    let draw = |css: &str, name: &str| {
        let root = node(
            "t",
            vec![node(
                "r",
                vec![
                    node("c", vec![text("w", "Extraordinary")]),
                    node("d", vec![]),
                ],
            )],
        );
        let sheet = format!(
            "#t {{ display:table; width:1px; color:black; }}
             #r {{ display:table-row; }}
             #c {{ display:table-cell; font-variant:small-caps; {css} }}
             #d {{ display:table-cell; width:50px; height:30px; background-color:#d9534f; }}"
        );
        let tree = BoxTree::new(root, &sheet).unwrap();
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(name))
            .unwrap()
    };
    let name = format!("small-caps-column-{}.png", std::process::id());
    assert!(draw("white-space:nowrap;", &name));
    assert!(draw("", &name));
}

#[test]
fn inline_boxes_draw_their_sides_across_lines() {
    let draw = |chip: &str, picture: &std::path::Path| {
        let root = node(
            "page",
            vec![
                text("a", "before "),
                node(
                    "chip",
                    vec![text(
                        "b",
                        "a chip with enough words to break onto the next line",
                    )],
                ),
                text("c", " after"),
            ],
        );
        let css = format!(
            "#page {{ background-color:white; color:black; width:300px; line-height:2; }}
             #chip {{ display:inline; {chip} }}"
        );
        let tree = BoxTree::new(root, &css).unwrap();
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(picture)
            .unwrap()
    };
    let picture = std::env::temp_dir().join(format!("inline-box-{}.png", std::process::id()));
    let boxed =
        "padding:3px 8px; margin:0 8px; background-color:#adcbe3; border:2px solid #2a4d69;";
    assert!(draw(boxed, &picture));
    assert!(draw(boxed, &picture));
    assert!(!draw("background-color:#adcbe3;", &picture));
    // Styles reach an inline box's top, bottom and side borders alike.
    let solid = "border-top:6px solid #2a4d69;";
    let name = std::env::temp_dir().join(format!("inline-top-{}.png", std::process::id()));
    assert!(draw(solid, &name));
    assert!(!draw("border-top:6px dashed #2a4d69;", &name));
    let solid = "border-left:12px solid #2a4d69;";
    let name = std::env::temp_dir().join(format!("inline-side-{}.png", std::process::id()));
    assert!(draw(solid, &name));
    assert!(!draw("border-left:12px double #2a4d69;", &name));
    assert!(!draw("", &picture));
}

#[test]
fn enter_still_presses_a_box_whose_hosted_input_lacks_focus() {
    use iced::keyboard::key::Named;
    let root = node("page", vec![node("one", vec![])]);
    let tree = BoxTree::new(root, "#one { height:30px; width:200px; }").unwrap();
    let view: Element<'_, u8> = tree
        .interactive()
        .widget(
            "one",
            iced::widget::text_input("", "").on_input(|_| 1).width(50),
        )
        .on_press("one", 9)
        .into();
    let mut ui = simulator(view);
    // Past the 50px input, inside the box.
    ui.point_at(Point::new(150.0, 10.0));
    let _ = ui.simulate(iced_test::simulator::click());
    let _ = ui.tap_key(Named::Enter);
    let messages: Vec<u8> = ui.into_messages().collect();
    assert_eq!(messages, vec![9, 9]);
}

#[test]
fn unbroken_text_keeps_its_column_whole() {
    let draw = |css: &str, name: &str| {
        let root = node(
            "t",
            vec![node(
                "r",
                vec![node("c", vec![text("w", "aaa bbb")]), node("d", vec![])],
            )],
        );
        let sheet = format!(
            "#t {{ display:table; {css} color:black; }}
             #r {{ display:table-row; }}
             #c {{ display:table-cell; white-space:nowrap; }}
             #d {{ display:table-cell; width:50px; height:30px; background-color:#d9534f; }}"
        );
        let tree = BoxTree::new(root, &sheet).unwrap();
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(name))
            .unwrap()
    };
    let name = format!("nowrap-column-{}.png", std::process::id());
    assert!(draw("", &name));
    assert!(draw("width:1px;", &name));
}

#[test]
fn a_hidden_child_leaves_its_parents_paint_whole() {
    // Text in the background's own color looks just like hidden text.
    let draw = |css: &str, name: &str| {
        let root = node(
            "page",
            vec![node(
                "o",
                vec![
                    text("a", "a"),
                    node("i", vec![text("b", "b")]),
                    text("c", "c"),
                ],
            )],
        );
        let sheet = format!(
            "#page {{ background-color:white; color:white; padding:20px; }}
             #o {{ display:inline; background-color:#0000ff; padding:2px 8px; border:1px solid #ff0000; }}
             #i {{ display:inline; padding:0 8px; color:#0000ff; {css} }}"
        );
        let tree = BoxTree::new(root, &sheet).unwrap();
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(name))
            .unwrap()
    };
    let name = format!("hidden-child-{}.png", std::process::id());
    assert!(draw("", &name));
    assert!(draw("visibility:hidden;", &name));
}

#[test]
fn a_hidden_child_leaves_a_parent_without_sides_whole() {
    let draw = |css: &str, name: &str| {
        let root = node(
            "page",
            vec![node(
                "o",
                vec![
                    text("a", "a"),
                    node("i", vec![text("b", "b")]),
                    text("c", "c"),
                ],
            )],
        );
        let sheet = format!(
            "#page {{ background-color:white; color:#0000ff; padding:20px; }}
             #o {{ display:inline; background-color:#0000ff; }}
             #i {{ display:inline; {css} }}"
        );
        let tree = BoxTree::new(root, &sheet).unwrap();
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(name))
            .unwrap()
    };
    let name = format!("hidden-unsided-{}.png", std::process::id());
    assert!(draw("", &name));
    assert!(draw("visibility:hidden;", &name));
}

#[test]
fn authored_wide_spaces_still_break_a_columns_minimum() {
    let draw = |words: &'static str, name: &str| {
        let root = node(
            "t",
            vec![node(
                "r",
                vec![node("c", vec![text("w", words)]), node("d", vec![])],
            )],
        );
        let sheet = "#t { display:table; width:1px; color:black; }
             #r { display:table-row; }
             #c { display:table-cell; }
             #d { display:table-cell; width:50px; height:30px; background-color:#d9534f; }";
        let tree = BoxTree::new(root, sheet).unwrap();
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(name))
            .unwrap()
    };
    // The neighbour sits past the wider word in both: the space counts for nothing.
    let name = format!("wide-space-{}.png", std::process::id());
    assert!(draw("aaaa\u{2003}bbbb", &name));
    assert!(!draw("aaaa\u{a0}bbbb", &name));
}

/// What a press at `x`, in the first line, sends: 1 for `#outer`, 2 for `#inner`.
fn pressed_at(css: &str, x: f32) -> Vec<u8> {
    let root = node(
        "page",
        vec![node("outer", vec![node("inner", vec![text("t", "text")])])],
    );
    let sheet = format!(
        "#page {{ width:600px; font-size:16px; }}
         #outer, #inner {{ display:inline; }}
         {css}"
    );
    let tree = BoxTree::new(root, &sheet).unwrap();
    let view: Element<'_, u8> = tree
        .interactive()
        .on_press("outer", 1)
        .on_press("inner", 2)
        .into();
    click(view, Point::new(x, 10.0))
}

#[test]
fn inline_sides_answer_for_their_own_box() {
    // The inner box's padding is its own.
    assert_eq!(pressed_at("#inner { padding:0 48px; }", 10.0), vec![2]);
    // An outer box's padding is its own too, though the first text inside is the inner box's.
    assert_eq!(pressed_at("#outer { padding:0 48px; }", 10.0), vec![1]);
    // Its margin belongs to the box around it.
    assert_eq!(pressed_at("#inner { margin:0 48px; }", 10.0), vec![1]);
    // A hidden box around takes no presses, so the margin answers for nothing here.
    assert_eq!(
        pressed_at(
            "#outer { visibility:hidden; } #inner { visibility:visible; margin:0 48px; }",
            10.0
        ),
        Vec::<u8>::new()
    );
    // An indent is blank space of the block, not of the first word.
    assert_eq!(
        pressed_at("#page { text-indent:48px; }", 10.0),
        Vec::<u8>::new()
    );
    assert_eq!(pressed_at("#page { text-indent:48px; }", 56.0), vec![2]);
}

#[test]
fn a_columns_minimum_holds_a_word_with_its_sides() {
    let draw = |css: &str, name: &str| {
        let root = node(
            "t",
            vec![node(
                "r",
                vec![
                    node("c", vec![node("s", vec![text("w", "word")])]),
                    node("d", vec![]),
                ],
            )],
        );
        let sheet = format!(
            "#t {{ display:table; width:1px; color:black; }}
             #r {{ display:table-row; }}
             #c {{ display:table-cell; {css} }}
             #s {{ display:inline; padding:0 16px; background-color:#adcbe3; }}
             #d {{ display:table-cell; width:50px; height:30px; background-color:#d9534f; }}"
        );
        let tree = BoxTree::new(root, &sheet).unwrap();
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(name))
            .unwrap()
    };
    let name = format!("word-with-sides-{}.png", std::process::id());
    assert!(draw("white-space:nowrap;", &name));
    assert!(draw("", &name));
}

#[test]
fn small_letters_keep_the_full_line_height_on_screen() {
    // Text the color of the page, so only the block below it shows where the line ends.
    let draw = |words: &'static str, name: &str| {
        let root = node("page", vec![text("t", words), node("below", vec![])]);
        let sheet = "#page { background-color:white; color:white; font-size:40px; font-variant:small-caps; }
             #t { display:block; }
             #below { height:10px; background-color:#d9534f; }";
        let tree = BoxTree::new(root, sheet).unwrap();
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(name))
            .unwrap()
    };
    let name = format!("small-line-{}.png", std::process::id());
    assert!(draw("HI", &name));
    assert!(draw("hi", &name));
}

#[test]
fn escape_in_a_hosted_input_drops_focus_styling_with_its_focus() {
    use iced::keyboard::key::Named;
    let root = node("page", vec![node("one", vec![])]);
    let tree = BoxTree::new(
        root,
        "#page { background-color:white; padding:20px; }
         #one { height:30px; width:200px; }
         #one:focus { outline:4px solid #ff0000; }",
    )
    .unwrap();
    let view: Element<'_, u8> = tree
        .interactive()
        .widget("one", iced::widget::text_input("", "").on_input(|_| 1))
        .into();
    let mut ui = simulator(view);
    let picture = std::env::temp_dir().join(format!("escape-{}.png", std::process::id()));
    assert!(
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(&picture)
            .unwrap()
    );
    let _ = ui.tap_key(Named::Tab);
    assert!(
        !ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(&picture)
            .unwrap()
    );
    let _ = ui.tap_key(Named::Escape);
    assert!(
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(&picture)
            .unwrap()
    );
}

#[test]
fn enter_in_a_hosted_input_presses_no_box_around_it() {
    use iced::keyboard::key::Named;
    let run = |enter: bool| {
        let root = node("page", vec![node("outer", vec![node("inner", vec![])])]);
        let tree = BoxTree::new(
            root,
            "#outer { padding:5px; } #inner { height:30px; width:200px; }",
        )
        .unwrap();
        let view: Element<'_, u8> = tree
            .interactive()
            .widget("inner", iced::widget::text_input("", "").on_input(|_| 1))
            .on_press("outer", 9)
            .into();
        let mut ui = simulator(view);
        ui.point_at(Point::new(50.0, 15.0));
        let _ = ui.simulate(iced_test::simulator::click());
        if enter {
            let _ = ui.tap_key(Named::Enter);
        }
        ui.into_messages().collect::<Vec<u8>>()
    };
    assert_eq!(run(true), run(false));
}

#[test]
fn border_styles_draw() {
    let root = node(
        "page",
        ["a", "b", "c", "d", "e", "f", "g"]
            .into_iter()
            .map(|id| node(id, vec![]))
            .collect(),
    );
    let tree = BoxTree::new(
        root,
        "#page { background-color:white; padding:8px; }
         #a, #b, #c, #d, #e, #f, #g { width:200px; height:24px; margin:8px;
             border:6px solid #2a4d69; background-color:#e7eff6; }
         #a { border-style:dashed; } #b { border-style:dotted; } #c { border-style:double; }
         #d { border-style:groove; } #e { border-style:ridge; } #f { border-style:inset; }
         #g { border-style:outset; outline:3px dashed #d9534f; }",
    )
    .unwrap();
    let view: Element<'_, u8> = tree.view();
    let mut ui = simulator(view);
    let name = format!("border-styles-{}.png", std::process::id());
    assert!(
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(name))
            .unwrap()
    );
}

#[test]
fn unicode_bidi_and_direction_reorder_text() {
    let draw = |words: &'static str, css: &str, name: &str| {
        let root = node("page", vec![node("s", vec![text("w", words)])]);
        let sheet = format!(
            "#page {{ background-color:white; color:black; width:300px; }}
             #s {{ display:inline; }} #w {{ display:inline; }} {css}"
        );
        let tree = BoxTree::new(root, &sheet).unwrap();
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(name))
            .unwrap()
    };
    let pid = std::process::id();
    // 9.10: an override sets the characters' direction: the letters come out reversed.
    let name = format!("bidi-override-{pid}.png");
    assert!(draw("cba", "", &name));
    assert!(draw(
        "abc",
        "#s { direction:rtl; unicode-bidi:bidi-override; }",
        &name
    ));
    assert!(!draw("abc", "", &name));
    // A paragraph's base direction puts the neutral mark to the left of the word. On an inner
    // block: a narrower box with `direction:rtl` also moves to its container's right.
    let block = "#s { display:block; }";
    let rtl = "#s { display:block; direction:rtl; }";
    let name = format!("bidi-base-{pid}.png");
    assert!(draw(
        "!word",
        &format!("{block} #s {{ text-align:right; }}"),
        &name
    ));
    assert!(draw("word!", rtl, &name));
    assert!(draw(
        "word!",
        &format!("{rtl} #s {{ text-align:right; }}"),
        &name
    ));
    // Hebrew right-aligned used to vanish: a right-to-left paragraph draws wherever it is put.
    let name = format!("bidi-left-{pid}.png");
    assert!(draw("!word", block, &name));
    assert!(draw(
        "word!",
        &format!("{rtl} #s {{ text-align:left; }}"),
        &name
    ));
    let name = format!("bidi-hebrew-{pid}.png");
    assert!(draw("\u{5d0}\u{5d1}\u{5d2}", rtl, &name));
    assert!(draw(
        "\u{5d0}\u{5d1}\u{5d2}",
        &format!("{rtl} #s {{ text-align:right; }}"),
        &name
    ));
    assert!(!draw("x", rtl, &name));
}

#[test]
fn collapsed_lines_survive_cell_backgrounds() {
    // White cells on a white page look like no backgrounds at all, if the lines stay on top.
    let draw = |css: &str, name: &str| {
        let root = node(
            "t",
            vec![
                node("r1", vec![node("a", vec![]), node("b", vec![])]),
                node("r2", vec![node("c", vec![]), node("d", vec![])]),
            ],
        );
        let sheet = format!(
            "#t {{ display:table; border-collapse:collapse; background-color:white; }}
             #r1, #r2 {{ display:table-row; }}
             #a, #b, #c, #d {{ display:table-cell; width:40px; height:40px; border:2px solid #000000; }}
             {css}"
        );
        let tree = BoxTree::new(root, &sheet).unwrap();
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(name))
            .unwrap()
    };
    let name = format!("collapse-lines-{}.png", std::process::id());
    assert!(draw("", &name));
    assert!(draw("#a, #b, #c, #d { background-color:white; }", &name));
    assert!(draw("#r1, #r2 { background-color:white; }", &name));
    assert!(draw(
        "#a { position:relative; background-color:white; }",
        &name
    ));
}

#[test]
fn justified_right_to_left_text_starts_at_the_right() {
    let draw = |css: &str, name: &str| {
        let root = node("page", vec![node("s", vec![text("w", "word!")])]);
        let sheet = format!(
            "#page {{ background-color:white; color:black; width:300px; }}
             #s {{ display:block; direction:rtl; }} #w {{ display:inline; }} {css}"
        );
        let tree = BoxTree::new(root, &sheet).unwrap();
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(name))
            .unwrap()
    };
    let name = format!("rtl-justify-{}.png", std::process::id());
    assert!(draw("", &name));
    assert!(draw("#s { text-align:justify; }", &name));
}

#[test]
fn a_later_rows_background_stays_under_a_spanning_cell() {
    let draw = |css: &str, name: &str| {
        let tall = node("a", vec![])
            .style_attribute("rowspan", "2", limits())
            .unwrap();
        let root = node(
            "t",
            vec![
                node("r1", vec![tall, node("b", vec![])]),
                node("r2", vec![node("c", vec![])]),
            ],
        );
        let sheet = format!(
            "#t {{ display:table; background-color:white; }} #r1, #r2 {{ display:table-row; }}
             #a, #b, #c {{ display:table-cell; width:40px; height:30px; }}
             #a {{ background-color:#d9534f; }} {css}"
        );
        let tree = BoxTree::new(root, &sheet).unwrap();
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(name))
            .unwrap()
    };
    // A white row over a white table looks like none, if the red cell stays on top.
    let name = format!("row-layer-{}.png", std::process::id());
    assert!(draw("", &name));
    assert!(draw("#r2 { background-color:white; }", &name));
}

#[test]
fn a_columns_background_paints_under_its_cells() {
    let draw = |css: &str, name: &str| {
        let root = node(
            "t",
            vec![
                node("c1", vec![]),
                node("c2", vec![]),
                node("r1", vec![node("a", vec![]), node("b", vec![])]),
                node("r2", vec![node("c", vec![]), node("d", vec![])]),
            ],
        );
        let sheet = format!(
            "#t {{ display:table; background-color:white; }}
             #c1, #c2 {{ display:table-column; }}
             #r1, #r2 {{ display:table-row; }}
             #a, #b, #c, #d {{ display:table-cell; width:40px; height:30px; }} {css}"
        );
        let tree = BoxTree::new(root, &sheet).unwrap();
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(name))
            .unwrap()
    };
    let name = format!("column-background-{}.png", std::process::id());
    assert!(draw("#b, #d { background-color:#d9534f; }", &name));
    assert!(draw("#c2 { background-color:#d9534f; }", &name));
    // Under the cells: a cell's own background covers its column's.
    let name = format!("column-under-{}.png", std::process::id());
    assert!(draw("#b, #d { background-color:#2a4d69; }", &name));
    assert!(draw(
        "#c2 { background-color:#d9534f; } #b, #d { background-color:#2a4d69; }",
        &name
    ));
}

#[test]
fn a_column_fills_an_anonymous_cell_not_its_content() {
    let draw = |css: &str, name: &str| {
        let root = node(
            "t",
            vec![
                node("c1", vec![]),
                node("r", vec![node("a", vec![]), node("b", vec![])]),
            ],
        );
        let sheet = format!(
            "#t {{ display:table; background-color:white; }}
             #c1 {{ display:table-column; }}
             #r {{ display:table-row; }}
             #a {{ height:30px; }}
             #b {{ display:table-cell; width:40px; height:30px; }} {css}"
        );
        let tree = BoxTree::new(root, &sheet).unwrap();
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(name))
            .unwrap()
    };
    let name = format!("column-anonymous-{}.png", std::process::id());
    // `#a` sits in an anonymous cell 40px wide; the column covers that cell, margins and all.
    assert!(draw("#a { width:40px; background-color:#d9534f; }", &name));
    assert!(draw(
        "#a { width:20px; margin:0 10px; } #c1 { background-color:#d9534f; }",
        &name
    ));
}

#[test]
fn a_widget_in_an_empty_table_row_is_drawn() {
    let view = |hosted: bool| -> Element<'static, u8> {
        let root = node("t", vec![node("r", vec![])]);
        let tree = BoxTree::new(
            root,
            "#t { display:table; width:100px; background-color:white; }
             #r { display:table-row; height:40px; }",
        )
        .unwrap();
        let mut interactive = tree.interactive();
        if hosted {
            interactive = interactive.widget(
                "r",
                iced::widget::button(iced::widget::text("go"))
                    .on_press(7)
                    .width(80)
                    .height(30),
            );
        }
        interactive.into()
    };
    let name = std::env::temp_dir().join(format!("empty-row-widget-{}.png", std::process::id()));
    assert!(
        simulator(view(false))
            .snapshot(&Theme::Light)
            .unwrap()
            .matches_image(&name)
            .unwrap()
    );
    assert!(
        !simulator(view(true))
            .snapshot(&Theme::Light)
            .unwrap()
            .matches_image(&name)
            .unwrap()
    );
    assert_eq!(click(view(true), Point::new(10.0, 10.0)), vec![7]);
}

#[test]
fn a_widget_in_an_empty_row_group_is_drawn() {
    let view = |hosted: bool| -> Element<'static, u8> {
        let root = node("t", vec![node("g", vec![])]);
        let tree = BoxTree::new(
            root,
            "#t { display:table; width:100px; background-color:white; }
             #g { display:table-row-group; height:40px; }",
        )
        .unwrap();
        let mut interactive = tree.interactive();
        if hosted {
            interactive = interactive.widget(
                "g",
                iced::widget::button(iced::widget::text("go"))
                    .on_press(7)
                    .width(80)
                    .height(30),
            );
        }
        interactive.into()
    };
    let name = std::env::temp_dir().join(format!("empty-group-widget-{}.png", std::process::id()));
    assert!(
        simulator(view(false))
            .snapshot(&Theme::Light)
            .unwrap()
            .matches_image(&name)
            .unwrap()
    );
    assert!(
        !simulator(view(true))
            .snapshot(&Theme::Light)
            .unwrap()
            .matches_image(&name)
            .unwrap()
    );
}

#[test]
fn a_spanning_cell_does_not_widen_a_column_and_a_hidden_column_paints_nothing() {
    let draw = |css: &str, name: &str| {
        let wide = node("a", vec![])
            .style_attribute("colspan", "2", limits())
            .unwrap();
        let root = node(
            "t",
            vec![
                node("c1", vec![]),
                node("c2", vec![]),
                node("r1", vec![wide]),
                node("r2", vec![node("c", vec![]), node("d", vec![])]),
            ],
        );
        let sheet = format!(
            "#t {{ display:table; background-color:white; }}
             #c1, #c2 {{ display:table-column; }}
             #r1, #r2 {{ display:table-row; }}
             #a, #c, #d {{ display:table-cell; height:30px; }}
             #c, #d {{ width:40px; }} {css}"
        );
        let tree = BoxTree::new(root, &sheet).unwrap();
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(name))
            .unwrap()
    };
    let name = format!("column-span-{}.png", std::process::id());
    assert!(draw("#c1, #c2 { background-color:#d9534f; }", &name));
    assert!(!draw("#c1 { background-color:#d9534f; }", &name));
    let name = format!("column-hidden-{}.png", std::process::id());
    assert!(draw("", &name));
    assert!(draw(
        "#c2 { background-color:#d9534f; visibility:hidden; }",
        &name
    ));
    assert!(!draw("#c2 { background-color:#d9534f; }", &name));
}

#[test]
fn a_column_group_paints_under_its_columns() {
    let draw = |css: &str, name: &str| {
        let root = node(
            "t",
            vec![
                node("c1", vec![]),
                node(
                    "g",
                    vec![node("c2", vec![]), node("c3", vec![]), node("x", vec![])],
                ),
                node(
                    "r1",
                    vec![node("a", vec![]), node("b", vec![]), node("c", vec![])],
                ),
                node(
                    "r2",
                    vec![node("d", vec![]), node("e", vec![]), node("f", vec![])],
                ),
            ],
        );
        let sheet = format!(
            "#t {{ display:table; background-color:white; }}
             #c1, #c2, #c3 {{ display:table-column; }}
             #g {{ display:table-column-group; }}
             #x {{ display:block; height:500px; background-color:black; }}
             #r1, #r2 {{ display:table-row; }}
             #a, #b, #c, #d, #e, #f {{ display:table-cell; width:40px; height:30px; }} {css}"
        );
        let tree = BoxTree::new(root, &sheet).unwrap();
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(name))
            .unwrap()
    };
    let name = format!("column-group-{}.png", std::process::id());
    assert!(draw("#b, #c, #e, #f { background-color:#d9534f; }", &name));
    assert!(draw("#c2, #c3 { background-color:#d9534f; }", &name));
    assert!(draw("#g { background-color:#d9534f; }", &name));
    // Columns paint over their group, and a hidden group paints nothing.
    let name = format!("column-over-group-{}.png", std::process::id());
    assert!(draw(
        "#c2 { background-color:#2a4d69; } #c3 { background-color:#d9534f; }",
        &name
    ));
    assert!(draw(
        "#g { background-color:#d9534f; } #c2 { background-color:#2a4d69; }",
        &name
    ));
    let name = format!("column-group-hidden-{}.png", std::process::id());
    assert!(draw("", &name));
    assert!(draw(
        "#g { background-color:#d9534f; visibility:hidden; }",
        &name
    ));
}

#[test]
fn a_fixed_table_reads_widths_of_grouped_columns() {
    let draw = |columns: Vec<Node>, css: &str, name: &str| {
        let mut children = columns;
        children.push(node("r", vec![node("a", vec![]), node("b", vec![])]));
        let root = node("t", children);
        let sheet = format!(
            "#t {{ display:table; table-layout:fixed; width:200px; background-color:white; }}
             #r {{ display:table-row; }}
             #a, #b {{ display:table-cell; height:30px; }}
             #a {{ background-color:#d9534f; }} {css}"
        );
        let tree = BoxTree::new(root, &sheet).unwrap();
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(name))
            .unwrap()
    };
    let name = format!("grouped-widths-{}.png", std::process::id());
    let css = "#c1, #c2 { display:table-column; } #c1 { width:150px; } #g { display:table-column-group; }";
    assert!(draw(
        vec![node("c1", vec![]), node("c2", vec![])],
        css,
        &name
    ));
    assert!(draw(
        vec![node("g", vec![node("c1", vec![]), node("c2", vec![])])],
        css,
        &name
    ));
    assert!(!draw(vec![], css, &name));
}

#[test]
fn a_column_group_keeps_generated_content_after_the_table() {
    let draw = |group: bool, name: &str| {
        let mut parts = Vec::new();
        if group {
            parts.push(node("g", vec![node("c", vec![])]));
        }
        parts.push(node("r", vec![node("a", vec![text("x", "cell")])]));
        let root = node(
            "w",
            vec![node("t", parts), node("p", vec![text("y", "tail")])],
        );
        let sheet = "#w { background-color:white; color:black; }
             #t { display:table; } #g { display:table-column-group; } #c { display:table-column; }
             #r { display:table-row; } #a { display:table-cell; }
             #t:after { content:\"Y\"; } #w:after { content:\"Z\"; }";
        let tree = BoxTree::new(root, sheet).unwrap();
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(name))
            .unwrap()
    };
    let name = format!("group-after-{}.png", std::process::id());
    assert!(draw(false, &name));
    assert!(draw(true, &name));
}

#[test]
fn an_rtl_scroller_scrolls_left_with_the_wheel() {
    let root = node(
        "page",
        vec![node("box", vec![node("wide", vec![node("mark", vec![])])])],
    );
    let tree = BoxTree::new(
        root,
        "#page { background:white; }
         #box { direction:rtl; overflow:auto; width:100px; height:40px; }
         #wide { width:300px; height:40px; }
         #mark { width:60px; height:40px; background-color:#d9534f; }",
    )
    .unwrap();
    let view: Element<'_, u8> = tree.view();
    let mut ui = simulator(view);
    let picture = std::env::temp_dir().join(format!("rtl-scroll-{}.png", std::process::id()));
    assert!(
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(&picture)
            .unwrap()
    );
    ui.point_at(Point::new(10.0, 10.0));
    // Toward the start there is nothing: the picture stays.
    let _ = ui.simulate([iced::Event::Mouse(iced::mouse::Event::WheelScrolled {
        delta: iced::mouse::ScrollDelta::Pixels { x: -40.0, y: 0.0 },
    })]);
    assert!(
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(&picture)
            .unwrap()
    );
    let _ = ui.simulate([iced::Event::Mouse(iced::mouse::Event::WheelScrolled {
        delta: iced::mouse::ScrollDelta::Pixels { x: 40.0, y: 0.0 },
    })]);
    assert!(
        !ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(&picture)
            .unwrap()
    );
}

#[test]
fn rtl_text_wider_than_its_scroller_can_be_scrolled_to() {
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
         #list { direction:rtl; overflow:auto; width:60px; height:40px; white-space:nowrap; }
         #line { color:black; }",
    )
    .unwrap();
    let view: Element<'_, u8> = tree.view();
    let mut ui = simulator(view);
    let picture = std::env::temp_dir().join(format!("rtl-scroll-text-{}.png", std::process::id()));
    assert!(
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(&picture)
            .unwrap()
    );
    ui.point_at(Point::new(10.0, 10.0));
    let _ = ui.simulate([iced::Event::Mouse(iced::mouse::Event::WheelScrolled {
        delta: iced::mouse::ScrollDelta::Pixels { x: 40.0, y: 0.0 },
    })]);
    assert!(
        !ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(&picture)
            .unwrap()
    );
}

#[test]
fn an_rtl_scrollers_bar_starts_at_its_right_end() {
    let draw = |direction: &str, scroll: f32, name: &str| {
        let root = node("page", vec![node("box", vec![node("wide", vec![])])]);
        let sheet = format!(
            "#page {{ background:white; }}
             #box {{ direction:{direction}; overflow:auto; width:100px; height:40px; }}
             #wide {{ width:300px; height:40px; background-color:#2a4d69; }}"
        );
        let tree = BoxTree::new(root, &sheet).unwrap();
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.point_at(Point::new(10.0, 10.0));
        let _ = ui.simulate([iced::Event::Mouse(iced::mouse::Event::WheelScrolled {
            delta: iced::mouse::ScrollDelta::Pixels { x: scroll, y: 0.0 },
        })]);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(name))
            .unwrap()
    };
    // At its start an rtl box shows what an ltr one shows at its end, bar included.
    let name = format!("rtl-bar-{}.png", std::process::id());
    assert!(draw("ltr", -1000.0, &name));
    assert!(draw("rtl", 0.0, &name));
    assert!(!draw("ltr", 0.0, &name));
}

#[test]
fn tab_scrolls_an_rtl_box_to_an_input_at_its_left_end() {
    let ui = |hosted: bool| {
        let root = node(
            "page",
            vec![node("box", vec![node("wide", vec![node("field", vec![])])])],
        );
        let tree = BoxTree::new(
            root,
            "#page { background:white; }
             #box { direction:rtl; overflow:auto; width:100px; height:40px; }
             #wide { width:300px; height:40px; }
             #field { width:60px; height:30px; margin-right:240px; }",
        )
        .unwrap();
        let mut view = tree.interactive();
        if hosted {
            view = view.widget(
                "field",
                iced::widget::text_input("", "here").on_input(|_| 1),
            );
        }
        simulator(view.into())
    };
    let wheel = |ui: &mut iced_test::Simulator<'_, u8>| {
        ui.point_at(Point::new(10.0, 10.0));
        let _ = ui.simulate([iced::Event::Mouse(iced::mouse::Event::WheelScrolled {
            delta: iced::mouse::ScrollDelta::Pixels { x: 1000.0, y: 0.0 },
        })]);
    };
    let id = std::process::id();
    // The field starts out of view, past the box's left end.
    let hidden = std::env::temp_dir().join(format!("rtl-tab-hidden-{id}.png"));
    assert!(
        ui(false)
            .snapshot(&Theme::Light)
            .unwrap()
            .matches_image(&hidden)
            .unwrap()
    );
    assert!(
        ui(true)
            .snapshot(&Theme::Light)
            .unwrap()
            .matches_image(&hidden)
            .unwrap()
    );
    // Tab scrolls exactly to it: the same as scrolling to the left end and then focusing it.
    let shown = std::env::temp_dir().join(format!("rtl-tab-shown-{id}.png"));
    let mut reference = ui(true);
    wheel(&mut reference);
    let _ = reference.tap_key(iced::keyboard::key::Named::Tab);
    assert!(
        reference
            .snapshot(&Theme::Light)
            .unwrap()
            .matches_image(&shown)
            .unwrap()
    );
    let mut tabbed = ui(true);
    let _ = tabbed.tap_key(iced::keyboard::key::Named::Tab);
    assert!(
        tabbed
            .snapshot(&Theme::Light)
            .unwrap()
            .matches_image(&shown)
            .unwrap()
    );
    assert!(
        !ui(true)
            .snapshot(&Theme::Light)
            .unwrap()
            .matches_image(&shown)
            .unwrap()
    );
}

#[test]
fn a_row_of_hidden_empty_cells_takes_no_height() {
    let draw = |rows: &[&str], name: &str| {
        let row = |id: &str| {
            let cells = (0..2)
                .map(|at| {
                    let inner = if id == "v" {
                        vec![text(&format!("{id}{at}x"), "x")]
                    } else {
                        vec![]
                    };
                    node(&format!("{id}{at}"), inner)
                })
                .collect();
            node(id, cells)
        };
        // In a page, so the table's own height shows.
        let root = node(
            "page",
            vec![node("t", rows.iter().map(|id| row(id)).collect())],
        );
        let sheet = "#page { background-color:white; }
             #t { display:table; border-spacing:10px; background-color:#2a4d69; }
             #a, #b, #c, #d, #v { display:table-row; }
             #v0, #v1 { display:table-cell; width:26px; height:30px; padding:5px;
                        border:2px solid red; empty-cells:hide; visibility:hidden; }
             #a0, #a1, #c0, #c1 { display:table-cell; width:40px; height:30px; background-color:white; }
             #b0, #b1, #d0, #d1 { display:table-cell; width:26px; height:30px; padding:5px;
                        border:2px solid red; empty-cells:hide; }";
        let tree = BoxTree::new(root, sheet).unwrap();
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(name))
            .unwrap()
    };
    let id = std::process::id();
    // Its cells still size their columns (26px + padding + borders = 40px). Between rows,
    // at the end, and alone: as if the row were not there, one spacing gone.
    let name = format!("hidden-row-middle-{id}.png");
    assert!(draw(&["a", "c"], &name));
    assert!(draw(&["a", "b", "c"], &name));
    // Hidden cells with text show nothing either.
    assert!(draw(&["a", "v", "c"], &name));
    let name = format!("hidden-row-last-{id}.png");
    assert!(draw(&["a"], &name));
    assert!(draw(&["a", "b"], &name));
    assert!(draw(&["b", "a"], &name));
    // Each hidden row takes one spacing, however many follow each other.
    assert!(draw(&["a", "b", "d"], &name));
    assert!(draw(&["b", "d", "a"], &name));
    let name = format!("hidden-row-two-{id}.png");
    assert!(draw(&["a", "c"], &name));
    assert!(draw(&["a", "b", "d", "c"], &name));
    // A lone hidden row leaves one spacing: the table is 10px tall.
    let lone = BoxTree::new(
        node(
            "t",
            vec![node("b", vec![node("b0", vec![]), node("b1", vec![])])],
        ),
        "#t { display:table; border-spacing:10px; } #b { display:table-row; }
         #b0, #b1 { display:table-cell; width:26px; height:30px; empty-cells:hide; }",
    )
    .unwrap();
    let table = lone.layout(800.0, None).unwrap()[0].unwrap().border;
    assert_eq!(table.bottom - table.top, 10.0);
}

#[test]
fn a_hidden_empty_cell_hosting_a_widget_keeps_its_row() {
    let draw = |hide: &str, name: &str| {
        let root = node(
            "t",
            vec![
                node("r", vec![node("w", vec![])]),
                node("s", vec![node("x", vec![text("v", "below")])]),
            ],
        );
        let sheet = format!(
            "#t {{ display:table; border-spacing:10px; background-color:white; color:black; }}
             #r, #s {{ display:table-row; }}
             #w, #x {{ display:table-cell; }}
             #w {{ border:2px solid red; empty-cells:{hide}; }}"
        );
        let tree = BoxTree::new(root, &sheet).unwrap();
        let view: Element<'_, u8> = tree
            .interactive()
            .widget(
                "w",
                iced::widget::button(iced::widget::text("go"))
                    .width(80)
                    .height(30),
            )
            .into();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(name))
            .unwrap()
    };
    // A hosted widget's cell draws as if shown, and its row stays as tall as the widget,
    // so the text below stays below it.
    let name = format!("hidden-widget-row-{}.png", std::process::id());
    assert!(draw("show", &name));
    assert!(draw("hide", &name));
}

#[test]
fn an_anonymous_table_inherits_from_its_parent_not_its_cell() {
    let draw = |cell: &str, name: &str| {
        let root = node("w", vec![node("c", vec![]), node("d", vec![])]);
        let sheet = format!(
            "#w {{ background-color:white; }}
             #c, #d {{ display:table-cell; width:40px; height:30px; background-color:#d9534f;
                     border:4px solid #2a4d69; }} #c {{ {cell} }}"
        );
        let tree = BoxTree::new(root, &sheet).unwrap();
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(name))
            .unwrap()
    };
    // `border-spacing` and `border-collapse` on a cell do nothing: they apply to tables, and
    // the anonymous table around these cells takes them from `#w`.
    let name = format!("anonymous-table-{}.png", std::process::id());
    assert!(draw("", &name));
    assert!(draw("border-spacing:20px;", &name));
    assert!(draw("border-collapse:collapse;", &name));
}

#[test]
fn an_anonymous_table_in_an_inline_element_inherits_from_it() {
    let draw = |span: &str, name: &str| {
        let root = node(
            "w",
            vec![node("s", vec![node("c", vec![]), node("d", vec![])])],
        );
        let sheet = format!(
            "#w {{ background-color:white; }} #s {{ display:inline; {span} }}
             #c, #d {{ display:table-cell; width:40px; height:30px; background-color:#d9534f; }}"
        );
        let tree = BoxTree::new(root, &sheet).unwrap();
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(name))
            .unwrap()
    };
    // The inline element has no box of its own, yet its spacing reaches the table.
    let name = format!("anonymous-table-inline-{}.png", std::process::id());
    assert!(draw("", &name));
    assert!(!draw("border-spacing:20px;", &name));
}

#[test]
fn a_block_before_takes_its_own_box_like_an_element() {
    let frame = "display:block; width:60px; height:20px; padding:5px 8px;
                 margin:4px 0 10px 6px; border:3px solid #2a4d69;
                 background-color:#d9534f; color:black;";
    let draw = |generated: bool, extra: &str, name: &str| {
        let mut children = vec![text("after", "text")];
        if !generated {
            children.insert(0, node("g", vec![text("gx", "x")]));
        }
        let root = node("p", children);
        let sheet = if generated {
            format!(
                "#p {{ background-color:white; color:black; }} #p:before {{ content:\"x\"; {frame} {extra} }}"
            )
        } else {
            format!("#p {{ background-color:white; color:black; }} #g {{ {frame} {extra} }}")
        };
        let tree = BoxTree::new(root, &sheet).unwrap();
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(name))
            .unwrap()
    };
    let name = format!("generated-box-{}.png", std::process::id());
    assert!(draw(false, "", &name));
    assert!(draw(true, "", &name));
    // Lengths in its own font size, and a hidden one paints nothing but keeps its room.
    let name = format!("generated-box-em-{}.png", std::process::id());
    assert!(draw(false, "font-size:20px; padding-left:1em;", &name));
    assert!(draw(true, "font-size:20px; padding-left:1em;", &name));
    let name = format!("generated-box-hidden-{}.png", std::process::id());
    assert!(draw(false, "visibility:hidden;", &name));
    assert!(draw(true, "visibility:hidden;", &name));
}

#[test]
fn generated_blocks_keep_the_right_background_in_text_and_inline_owners() {
    let snap = |root: Node, sheet: &str, name: &str| {
        let tree = BoxTree::new(root, sheet).unwrap();
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(name))
            .unwrap()
    };
    let id = std::process::id();
    // A text leaf element: block or inline, the generated text joins its runs, highlighted.
    let leaf = |display: &str| {
        (
            node("w", vec![text("copy", "copy")]),
            format!(
                "#w {{ background-color:white; color:black; }} #copy {{ display:block; }}
                 #copy:before {{ content:\">\"; display:{display}; background-color:#d9534f; }}"
            ),
        )
    };
    let name = format!("generated-leaf-{id}.png");
    let (root, sheet) = leaf("inline");
    assert!(snap(root, &sheet, &name));
    let (root, sheet) = leaf("block");
    assert!(snap(root, &sheet, &name));
    // In an inline element, the block shows its own background, not the inline's.
    let name = format!("generated-inline-owner-{id}.png");
    let base = "#w { background-color:white; color:black; } #s { display:inline; background-color:#f0ad4e; }";
    let boxed = "display:block; width:40px; height:20px; background-color:#d9534f;";
    assert!(snap(
        node(
            "w",
            vec![node(
                "s",
                vec![node("g", vec![text("gx", "x")]), text("y", "after")]
            )]
        ),
        &format!("{base} #g {{ {boxed} }}"),
        &name,
    ));
    assert!(snap(
        node("w", vec![node("s", vec![text("y", "after")])]),
        &format!("{base} #s:before {{ content:\"x\"; {boxed} }}"),
        &name,
    ));
    // `inherit` takes the element's border style; the width is still the initial medium.
    let name = format!("generated-inherit-{id}.png");
    let base = "#w { background-color:white; color:black; border:2px dashed #2a4d69; }";
    assert!(snap(
        node("w", vec![node("g", vec![text("gx", "x")])]),
        &format!("{base} #g {{ display:block; border:3px dashed #2a4d69; }}"),
        &name,
    ));
    assert!(snap(
        node("w", vec![]),
        &format!(
            "{base} #w:before {{ content:\"x\"; display:block; border-style:inherit;
             border-color:inherit; }}"
        ),
        &name,
    ));
}

#[test]
fn a_percentage_text_indent_takes_its_share_of_the_width() {
    let draw = |indent: &str, name: &str| {
        let root = node("p", vec![text("t", "indented words")]);
        let sheet = format!(
            "#p {{ width:200px; background-color:white; color:black; text-indent:{indent}; }}"
        );
        let tree = BoxTree::new(root, &sheet).unwrap();
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(name))
            .unwrap()
    };
    // A quarter of the 200px it lays out in is 50px.
    let name = format!("indent-percent-{}.png", std::process::id());
    assert!(draw("50px", &name));
    assert!(draw("25%", &name));
    assert!(!draw("50%", &name));
    // On a line of inline-blocks it takes none, rather than a share of a piece's own width.
    let row = |indent: &str, name: &str| {
        let root = node(
            "p",
            vec![
                text("a", "Hello there my good friends and "),
                node("b", vec![text("bx", "X")]),
                text("c", " world"),
            ],
        );
        let sheet = format!(
            "#p {{ width:400px; background-color:white; color:black; text-indent:{indent}; }}
             #b {{ display:inline-block; }}"
        );
        let tree = BoxTree::new(root, &sheet).unwrap();
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(name))
            .unwrap()
    };
    let name = format!("indent-percent-row-{}.png", std::process::id());
    assert!(row("0", &name));
    assert!(row("25%", &name));
}

#[test]
fn an_inline_outline_draws_around_its_pieces() {
    let draw = |css: &str, name: &str| {
        let root = node(
            "p",
            vec![
                text("a", "before "),
                node(
                    "s",
                    vec![text("b", "outlined words that wrap onto a second line")],
                ),
                text("c", " after"),
            ],
        );
        let sheet = format!(
            "#p {{ width:220px; background-color:white; color:black; }}
             #s {{ display:inline; {css} }}"
        );
        let tree = BoxTree::new(root, &sheet).unwrap();
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(name))
            .unwrap()
    };
    let name = format!("inline-outline-{}.png", std::process::id());
    assert!(draw("", &name));
    // It draws, and takes no room: a hidden one draws nothing and moves nothing.
    assert!(!draw("outline:2px solid #d9534f;", &name));
    // Its vertical padding takes no room in the line, but the outline goes around it.
    let name = format!("inline-outline-padded-{}.png", std::process::id());
    assert!(draw("outline:2px solid #d9534f;", &name));
    assert!(!draw("outline:2px solid #d9534f; padding:6px 0;", &name));
    // A positioned box or a float inside it has a box of its own: no outline there, the
    // same as beside it.
    for (kind, css) in [
        ("abs", "position:absolute; top:60px; left:10px;"),
        ("float", "float:right;"),
    ] {
        let outlined = |inside: bool, name: &str| {
            let other = node("o", vec![text("ot", "elsewhere")]);
            let mut span = vec![text("b", "outlined")];
            let mut row = vec![text("a", "before ")];
            if inside {
                span.push(other);
            } else {
                row.push(other);
            }
            row.push(node("s", span));
            let sheet = format!(
                "#p {{ width:220px; height:100px; background-color:white; color:black; }}
                 #s {{ display:inline; outline:2px solid #d9534f; }} #o {{ {css} }}"
            );
            let tree = BoxTree::new(node("p", row), &sheet).unwrap();
            let view: Element<'_, u8> = tree.view();
            let mut ui = simulator(view);
            ui.snapshot(&Theme::Light)
                .unwrap()
                .matches_image(std::env::temp_dir().join(name))
                .unwrap()
        };
        let name = format!("inline-outline-{kind}-{}.png", std::process::id());
        assert!(outlined(false, &name));
        assert!(outlined(true, &name), "{kind}");
    }
    let name = format!("inline-outline-hidden-{}.png", std::process::id());
    assert!(draw("visibility:hidden;", &name));
    assert!(draw("visibility:hidden; outline:2px solid #d9534f;", &name));
}

#[test]
fn an_empty_inline_with_sides_still_draws_its_box() {
    let draw = |span: Option<&str>, name: &str| {
        let mut children = vec![text("a", "before ")];
        if span.is_some() {
            children.push(node("s", vec![]));
        }
        children.push(text("c", " after"));
        let sheet = format!(
            "#p {{ width:300px; background-color:white; color:black; }}
             #s {{ display:inline; {} }}",
            span.unwrap_or("")
        );
        let tree = BoxTree::new(node("p", children), &sheet).unwrap();
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(name))
            .unwrap()
    };
    let name = format!("empty-inline-{}.png", std::process::id());
    assert!(draw(None, &name));
    // Without sides it is nothing; with them its padding, border and background show.
    assert!(draw(Some("background-color:#d9534f;"), &name));
    assert!(!draw(
        Some("padding:0 8px; border:2px solid #2a4d69; background-color:#d9534f;"),
        &name
    ));
    // Its margin alone moves the text after it.
    assert!(!draw(Some("margin-left:20px;"), &name));
}

#[test]
fn an_inline_holding_only_an_empty_inline_draws_its_box() {
    let draw = |inner: bool, name: &str| {
        let inside = if inner {
            vec![node("e", vec![node("f", vec![])])]
        } else {
            vec![]
        };
        let root = node(
            "p",
            vec![text("a", "before "), node("s", inside), text("c", " after")],
        );
        let tree = BoxTree::new(
            root,
            "#p { width:300px; background-color:white; color:black; }
             #s { display:inline; padding:0 8px; border:2px solid #2a4d69;
                  background-color:#d9534f; }
             #e, #f { display:inline; }",
        )
        .unwrap();
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(name))
            .unwrap()
    };
    let name = format!("nested-empty-inline-{}.png", std::process::id());
    let blank = format!("nested-empty-none-{}.png", std::process::id());
    // The same box as an empty one, not the bare text either side.
    assert!(draw(false, &name));
    assert!(draw(true, &name));
    let root = node("p", vec![text("a", "before "), text("c", " after")]);
    let tree = BoxTree::new(
        root,
        "#p { width:300px; background-color:white; color:black; }",
    )
    .unwrap();
    let view: Element<'_, u8> = tree.view();
    let mut ui = simulator(view);
    assert!(
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(&blank))
            .unwrap()
    );
    assert!(!draw(true, &blank));
}

#[test]
fn generated_inline_boxes_draw_as_inline_elements_do() {
    // The `:before` with `css`, or an element with it first in its place.
    let draw = |css: &str, content: &str, element: bool, name: &str| {
        let mut inside = vec![text("x", "text")];
        if element {
            let made = if content.is_empty() {
                vec![]
            } else {
                vec![text("m", "ab")]
            };
            inside.insert(0, node("i", made));
        }
        let root = node(
            "p",
            vec![text("a", "before "), node("s", inside), text("c", " after")],
        );
        let rule = if element { "#i" } else { "#s:before" };
        let made = if element {
            String::new()
        } else {
            format!("content:\"{content}\";")
        };
        let sheet = format!(
            "#p {{ width:300px; background-color:white; color:black; }}
             #s {{ display:inline; }}
             {rule} {{ display:inline; {made} {css} }}"
        );
        let tree = BoxTree::new(root, &sheet).unwrap();
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(name))
            .unwrap()
    };
    let boxed = "padding:0 8px; border:2px solid #2a4d69; background-color:#d9534f;";
    for (content, tag) in [("", "empty"), ("ab", "text")] {
        let name = format!("generated-box-{tag}-{}.png", std::process::id());
        assert!(draw(boxed, content, true, &name));
        assert!(draw(boxed, content, false, &name), "{content:?}");
        // Without its sides it is not that box.
        assert!(!draw("", content, false, &name));
    }
    // A margin alone moves what follows.
    let name = format!("generated-margin-{}.png", std::process::id());
    assert!(draw("margin-left:20px;", "", true, &name));
    assert!(draw("margin-left:20px;", "", false, &name));
}

#[test]
fn a_box_fading_out_with_visibility_stays_drawn_until_the_end() {
    use iced_css::{animation::clock::AnimationTiming, box_tree::BoxTreeAnimations};
    use std::time::{Duration, Instant};
    // A block with text, inline boxes (sided, generated and plain), an outside-marker item,
    // a generated block, and a hidden one that never shows.
    let source = || {
        node(
            "page",
            vec![node(
                "bar",
                vec![
                    text("line", "words "),
                    node("in", vec![text("i", "boxed")]),
                    text("gap", " "),
                    node("plain", vec![text("p", "plain")]),
                    node("li", vec![text("item", "item")]),
                ],
            )],
        )
    };
    let css = |shown: &str| {
        format!(
            "#page {{ background-color:white; padding:20px; }}
             #bar {{ width:200px; background-color:#ff0000; color:#000000;
                     border:4px solid #0000ff; visibility:{shown}; }}
             #bar:before {{ content:\"x\"; display:block; height:10px;
                            background-color:#00ff00; }}
             #bar:after {{ content:\".\"; display:block; height:6px;
                           background-color:#ff00ff; visibility:hidden; }}
             #in {{ display:inline; padding:0 4px; border:2px solid #000000;
                    background-color:#00ffff; }}
             #in:after {{ content:\"y\"; padding:0 6px; background-color:#ffff00;
                          border-left:3px solid #000000; }}
             #plain {{ display:inline; background-color:#ff8800; }}
             #li {{ display:list-item; margin-left:30px; }}"
        )
    };
    let shown = BoxTree::new(source(), &css("visible")).unwrap();
    let hidden = shown.restyled(&[&css("hidden")]).unwrap();
    let picture = |tree: &BoxTree, name: &str| {
        let view: Element<'_, u8> = tree.clone().view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(format!("{name}-{}.png", std::process::id())))
            .unwrap()
    };
    assert!(picture(&shown, "visibility-shown"));
    assert!(picture(&hidden, "visibility-hidden"));
    assert!(!picture(&hidden, "visibility-shown"));
    let start = Instant::now();
    let timing = AnimationTiming::new(Duration::from_secs(1), Duration::ZERO);
    let mut animations = BoxTreeAnimations::new();
    animations
        .request(&shown, &hidden, "bar", &["visibility"], timing, start)
        .unwrap();
    // Halfway it draws as shown: box, border, text, inline boxes, marker and generated
    // block.
    animations.advance(start + Duration::from_millis(500), &hidden);
    assert!(picture(&animations.presented(&hidden), "visibility-shown"));
    // Done, it is hidden.
    animations.advance(start + Duration::from_millis(1500), &hidden);
    assert!(picture(&animations.presented(&hidden), "visibility-hidden"));
    // Coming back after a delay, it draws nothing until it moves.
    let delayed = AnimationTiming::new(Duration::from_secs(1), Duration::from_secs(1));
    let mut animations = BoxTreeAnimations::new();
    animations
        .request(&hidden, &shown, "bar", &["visibility"], delayed, start)
        .unwrap();
    animations.advance(start + Duration::from_millis(500), &shown);
    assert!(picture(&animations.presented(&shown), "visibility-hidden"));
    animations.advance(start + Duration::from_millis(1100), &shown);
    assert!(picture(&animations.presented(&shown), "visibility-shown"));
    for name in ["visibility-shown", "visibility-hidden"] {
        for renderer in ["wgpu", "tiny-skia"] {
            let _ = std::fs::remove_file(
                std::env::temp_dir().join(format!("{name}-{}-{renderer}.png", std::process::id())),
            );
        }
    }
}

#[test]
fn a_hidden_inline_box_keeps_only_its_room() {
    // Hidden, a padded inline with a background draws nothing of its own, not even its
    // bands above and below, and its shown child draws as in no inline box, its background
    // the whole tall line: as between empty spans whose margins make the same room.
    let draw = |hidden: bool, name: &str| {
        let child = || node("c", vec![text("x", "child")]);
        let middle = if hidden {
            vec![node("s", vec![child()])]
        } else {
            vec![node("l", vec![]), child(), node("r", vec![])]
        };
        let mut children = vec![text("a", "before ")];
        children.extend(middle);
        children.push(text("b", " after"));
        let tree = BoxTree::new(
            node("p", children),
            "#p { width:300px; background-color:white; color:black; line-height:40px; }
             #s { display:inline; visibility:hidden; padding:6px 8px;
                  background-color:#0000ff; }
             #l { display:inline; margin-right:8px; }
             #r { display:inline; margin-left:8px; }
             #c { display:inline; visibility:visible; background-color:#ffff00; }",
        )
        .unwrap();
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(name))
            .unwrap()
    };
    let name = format!("hidden-room-{}.png", std::process::id());
    assert!(draw(false, &name));
    assert!(draw(true, &name));
    let _ = std::fs::remove_file(
        std::env::temp_dir().join(format!("hidden-room-{}-wgpu.png", std::process::id())),
    );
}

#[cfg(feature = "image")]
#[test]
fn a_fading_block_keeps_its_image_under_generated_text() {
    use iced_css::{animation::clock::AnimationTiming, box_tree::BoxTreeAnimations};
    use std::time::{Duration, Instant};
    let path = std::env::temp_dir().join(format!("iced-css-fade-image-{}", std::process::id()));
    std::fs::create_dir_all(&path).unwrap();
    std::fs::write(path.join("dot.png"), checker_png(16)).unwrap();
    let css = |shown: &str| {
        format!(
            "#page {{ background-color:white; padding:20px; color:#000000; }}
             #bar {{ width:200px; background-color:#ff0000; background-image:url(dot.png);
                     background-repeat:no-repeat; visibility:{shown}; }}
             #bar:before {{ content:\"x\"; }}"
        )
    };
    let root = || node("page", vec![node("bar", vec![text("t", "words")])]);
    let shown = BoxTree::with_base(root(), &css("visible"), &path).unwrap();
    let hidden = shown.restyled(&[&css("hidden")]).unwrap();
    let picture = |tree: &BoxTree, name: &str| {
        let view: Element<'_, u8> = tree.clone().view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(path.join(name))
            .unwrap()
    };
    assert!(picture(&shown, "shown.png"));
    let start = Instant::now();
    let mut animations = BoxTreeAnimations::new();
    animations
        .request(
            &shown,
            &hidden,
            "bar",
            &["visibility"],
            AnimationTiming::new(Duration::from_secs(1), Duration::ZERO),
            start,
        )
        .unwrap();
    animations.advance(start + Duration::from_millis(500), &hidden);
    // The text over the image paints no block background of its own.
    assert!(picture(&animations.presented(&hidden), "shown.png"));
    let _ = std::fs::remove_dir_all(&path);
}

#[test]
fn font_size_halfway_lays_out_as_its_size_then() {
    use iced_css::{animation::clock::AnimationTiming, box_tree::BoxTreeAnimations};
    use std::time::{Duration, Instant};
    // Text that wraps, a list item's marker, and an inline with sides in pixels.
    let source = || {
        node(
            "page",
            vec![node(
                "box",
                vec![
                    text("t", "words that wrap across the box "),
                    node("in", vec![text("i", "inline")]),
                    node("li", vec![text("l", "item")]),
                ],
            )],
        )
    };
    let css = |size: &str| {
        format!(
            "#page {{ background-color:white; padding:20px; color:#000000; }}
             #box {{ width:160px; font-size:{size}; background-color:#ddeeff; }}
             #in {{ display:inline; padding:0 4px; background-color:#ffff00; }}
             #li {{ display:list-item; margin-left:30px; font-size:0.75em; }}"
        )
    };
    let picture = |tree: BoxTree, name: &str| {
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(format!("{name}-{}.png", std::process::id())))
            .unwrap()
    };
    let from = BoxTree::new(source(), &css("16px")).unwrap();
    let to = from.restyled(&[&css("32px")]).unwrap();
    let between = BoxTree::new(source(), &css("24px")).unwrap();
    assert!(picture(between, "font-between"));
    assert!(!picture(to.clone(), "font-between"));
    let start = Instant::now();
    let mut animations = BoxTreeAnimations::new();
    animations
        .request(
            &from,
            &to,
            "box",
            &["font-size"],
            AnimationTiming::new(Duration::from_secs(1), Duration::ZERO),
            start,
        )
        .unwrap();
    animations.advance(start + Duration::from_millis(500), &to);
    assert!(picture(animations.presented(&to), "font-between"));
    for renderer in ["wgpu", "tiny-skia"] {
        let _ = std::fs::remove_file(std::env::temp_dir().join(format!(
            "font-between-{}-{renderer}.png",
            std::process::id()
        )));
    }
    // A float that shrinks to fit, with an indent and an inline's sides in pixels, and an
    // inline animated itself, with a margin: halfway, as at 24 pixels.
    let source = || {
        node(
            "page",
            vec![
                node(
                    "float",
                    vec![
                        text("t", "words beside "),
                        node("pad", vec![text("p", "padded")]),
                        node("fixed", vec![text("f", "fixed")]),
                    ],
                ),
                node(
                    "line",
                    vec![
                        text("a", "before "),
                        node("span", vec![text("s", "span")]),
                        text("b", " after"),
                    ],
                ),
            ],
        )
    };
    let css = |float: &str, span: &str| {
        format!(
            "#page {{ background-color:white; padding:20px; color:#000000; }}
             #float {{ float:left; font-size:{float}; text-indent:20px;
                       background-color:#ddeeff; }}
             #pad {{ display:inline; padding:0 20px; background-color:#ffff00; }}
             #fixed {{ display:inline; font-size:10px; padding:0 6px;
                       background-color:#ff8800; }}
             #line {{ clear:both; }}
             #span {{ display:inline; font-size:{span}; margin:0 8px;
                      background-color:#ffcc00; }}"
        )
    };
    let from = BoxTree::new(source(), &css("16px", "16px")).unwrap();
    let to = from.restyled(&[&css("32px", "32px")]).unwrap();
    let between = BoxTree::new(source(), &css("24px", "24px")).unwrap();
    assert!(picture(between, "font-between-2"));
    let mut animations = BoxTreeAnimations::new();
    let timing = AnimationTiming::new(Duration::from_secs(1), Duration::ZERO);
    for id in ["float", "span"] {
        animations
            .request(&from, &to, id, &["font-size"], timing, start)
            .unwrap();
    }
    animations.advance(start + Duration::from_millis(500), &to);
    assert!(picture(animations.presented(&to), "font-between-2"));
    for renderer in ["wgpu", "tiny-skia"] {
        let _ = std::fs::remove_file(std::env::temp_dir().join(format!(
            "font-between-2-{}-{renderer}.png",
            std::process::id()
        )));
    }
    // With visibility at once: a `:before` hidden of its own, with sides, stays hidden;
    // 5 pixels make other spaces at 24 pixels than at 32.
    let source = || node("page", vec![node("box", vec![text("t", "words")])]);
    let css = |size: &str, shown: &str| {
        format!(
            "#page {{ background-color:white; padding:20px; color:#000000; }}
             #box {{ font-size:{size}; visibility:{shown}; }}
             #box:before {{ content:\"x\"; visibility:hidden; padding:0 5px;
                            background-color:#ffff00; }}"
        )
    };
    let from = BoxTree::new(source(), &css("16px", "visible")).unwrap();
    let to = from.restyled(&[&css("32px", "hidden")]).unwrap();
    let between = BoxTree::new(source(), &css("24px", "visible")).unwrap();
    assert!(picture(between, "font-between-3"));
    let mut animations = BoxTreeAnimations::new();
    animations
        .request(
            &from,
            &to,
            "box",
            &["font-size", "visibility"],
            timing,
            start,
        )
        .unwrap();
    animations.advance(start + Duration::from_millis(500), &to);
    assert!(picture(animations.presented(&to), "font-between-3"));
    for renderer in ["wgpu", "tiny-skia"] {
        let _ = std::fs::remove_file(std::env::temp_dir().join(format!(
            "font-between-3-{}-{renderer}.png",
            std::process::id()
        )));
    }
}

#[cfg(feature = "image")]
#[test]
fn a_list_image_keeps_its_gap_with_an_animated_font_size() {
    use iced_css::{animation::clock::AnimationTiming, box_tree::BoxTreeAnimations};
    use std::time::{Duration, Instant};
    let path = std::env::temp_dir().join(format!("iced-css-font-image-{}", std::process::id()));
    std::fs::create_dir_all(&path).unwrap();
    std::fs::write(path.join("dot.png"), checker_png(8)).unwrap();
    let source = || {
        node(
            "page",
            vec![node("box", vec![node("li", vec![text("t", "item")])])],
        )
    };
    let css = |size: &str, item: &str| {
        format!(
            "#page {{ background-color:white; padding:20px 60px; color:#000000; }}
             #box {{ font-size:{size}; }}
             #li {{ display:list-item; font-size:{item}; list-style-image:url(dot.png); }}"
        )
    };
    let picture = |tree: BoxTree, name: &str| {
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(path.join(name))
            .unwrap()
    };
    let from = BoxTree::with_base(source(), &css("16px", "1em"), &path).unwrap();
    let to = from.restyled(&[&css("32px", "1em")]).unwrap();
    let start = Instant::now();
    let timing = AnimationTiming::new(Duration::from_secs(1), Duration::ZERO);
    // Halfway its gap is half of 24 pixels.
    assert!(picture(
        BoxTree::with_base(source(), &css("24px", "1em"), &path).unwrap(),
        "between.png"
    ));
    let mut animations = BoxTreeAnimations::new();
    animations
        .request(&from, &to, "box", &["font-size"], timing, start)
        .unwrap();
    animations.advance(start + Duration::from_millis(500), &to);
    assert!(picture(animations.presented(&to), "between.png"));
    // An item that moves on to 20 pixels without a request keeps them.
    let changed = to.restyled(&[&css("32px", "20px")]).unwrap();
    assert!(picture(
        BoxTree::with_base(source(), &css("24px", "20px"), &path).unwrap(),
        "changed.png"
    ));
    animations.advance(start + Duration::from_millis(500), &changed);
    assert!(picture(animations.presented(&changed), "changed.png"));
    let _ = std::fs::remove_dir_all(&path);
}

#[test]
fn lengths_and_percentages_mix_halfway() {
    use iced_css::{animation::clock::AnimationTiming, box_tree::BoxTreeAnimations};
    use std::time::{Duration, Instant};
    let source = || node("page", vec![node("bar", vec![text("t", "words")])]);
    let css = |bar: &str| {
        format!(
            "#page {{ background-color:white; width:400px; color:#000000; }}
             #bar {{ height:30px; background-color:#ff0000; position:relative; {bar} }}"
        )
    };
    let picture = |tree: BoxTree, name: &str| {
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(format!("{name}-{}.png", std::process::id())))
            .unwrap()
    };
    let from = BoxTree::new(source(), &css("width:25%; margin-left:0px; left:0px;")).unwrap();
    let to = from
        .restyled(&[&css("width:200px; margin-left:20%; left:10%;")])
        .unwrap();
    // Halfway: 100 pixels and 12.5% of 400 wide, 10% of 400 in, and 5% of 400 across.
    let between =
        BoxTree::new(source(), &css("width:150px; margin-left:40px; left:20px;")).unwrap();
    assert!(picture(between, "mix-between"));
    assert!(!picture(to.clone(), "mix-between"));
    let start = Instant::now();
    let mut animations = BoxTreeAnimations::new();
    let count = animations
        .request(
            &from,
            &to,
            "bar",
            &["width", "margin-left", "left"],
            AnimationTiming::new(Duration::from_secs(1), Duration::ZERO),
            start,
        )
        .unwrap();
    assert_eq!(count, 3);
    animations.advance(start + Duration::from_millis(500), &to);
    assert!(picture(animations.presented(&to), "mix-between"));
    // Turned back halfway, it starts from the mix: at once it is where it was.
    let back = to
        .restyled(&[&css("width:25%; margin-left:0px; left:0px;")])
        .unwrap();
    let now = start + Duration::from_millis(500);
    animations
        .request(
            &to,
            &back,
            "bar",
            &["width", "margin-left", "left"],
            AnimationTiming::new(Duration::from_secs(1), Duration::ZERO),
            now,
        )
        .unwrap();
    animations.advance(now, &back);
    assert!(picture(animations.presented(&back), "mix-between"));
    // A quarter of the way back, mixes blend: 75 pixels and 15.625% wide, 7.5% in, 3.75%
    // across.
    animations.advance(now + Duration::from_millis(250), &back);
    let quarter = BoxTree::new(
        source(),
        &css("width:137.5px; margin-left:30px; left:15px;"),
    )
    .unwrap();
    assert!(picture(quarter, "mix-quarter"));
    assert!(picture(animations.presented(&back), "mix-quarter"));
    for renderer in ["wgpu", "tiny-skia"] {
        let _ = std::fs::remove_file(
            std::env::temp_dir().join(format!("mix-quarter-{}-{renderer}.png", std::process::id())),
        );
    }
    for renderer in ["wgpu", "tiny-skia"] {
        let _ = std::fs::remove_file(
            std::env::temp_dir().join(format!("mix-between-{}-{renderer}.png", std::process::id())),
        );
    }
}

#[test]
fn an_inline_elements_sides_animate_both_ways() {
    use iced_css::{animation::clock::AnimationTiming, box_tree::BoxTreeAnimations};
    use std::time::{Duration, Instant};
    // An inline with a background, around text and a bordered inline of its own.
    let source = || {
        node(
            "page",
            vec![node(
                "p",
                vec![
                    text("a", "before "),
                    node(
                        "span",
                        vec![text("s", "span "), node("kid", vec![text("k", "kid")])],
                    ),
                    text("b", " after"),
                ],
            )],
        )
    };
    let css = |sides: &str| {
        format!(
            "#page {{ background-color:white; padding:20px; color:#000000; }}
             #p {{ line-height:40px; }}
             #span {{ display:inline; background-color:#ffcc00; {sides} }}
             #kid {{ display:inline; border-left:2px solid #0000ff; padding:0 3px; }}"
        )
    };
    let picture = |tree: BoxTree, name: &str| {
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(format!("{name}-{}.png", std::process::id())))
            .unwrap()
    };
    let none = BoxTree::new(source(), &css("")).unwrap();
    let full = none
        .restyled(&[&css("padding:20px; margin-left:10px;")])
        .unwrap();
    let half = BoxTree::new(source(), &css("padding:10px; margin-left:5px;")).unwrap();
    assert!(picture(half, "sides-half"));
    assert!(!picture(none.clone(), "sides-half"));
    let start = Instant::now();
    let timing = AnimationTiming::new(Duration::from_secs(1), Duration::ZERO);
    // Growing, the target has the spaces; shrinking to none, it has no spaces or box.
    for (from, to) in [(&none, &full), (&full, &none)] {
        let mut animations = BoxTreeAnimations::new();
        let count = animations
            .request(from, to, "span", &["padding", "margin-left"], timing, start)
            .unwrap();
        assert_eq!(count, 5);
        animations.advance(start + Duration::from_millis(500), to);
        assert!(picture(animations.presented(to), "sides-half"));
    }
    for renderer in ["wgpu", "tiny-skia"] {
        let _ = std::fs::remove_file(
            std::env::temp_dir().join(format!("sides-half-{}-{renderer}.png", std::process::id())),
        );
    }
}

#[test]
fn inline_sides_animate_in_every_place_they_fall() {
    use iced_css::{animation::clock::AnimationTiming, box_tree::BoxTreeAnimations};
    use std::time::{Duration, Instant};
    let picture = |tree: BoxTree, name: &str| {
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(format!("{name}-{}.png", std::process::id())))
            .unwrap()
    };
    // `#s` between `ends`, both ways: halfway it draws as `half`.
    let check = |source: &dyn Fn() -> Node,
                 css: &dyn Fn(&str) -> String,
                 ends: [&str; 2],
                 half: &str,
                 properties: &[&str],
                 tag: &str| {
        let name = format!("inline-sides-{tag}");
        assert!(picture(BoxTree::new(source(), &css(half)).unwrap(), &name));
        let one = BoxTree::new(source(), &css(ends[0])).unwrap();
        let two = one.restyled(&[&css(ends[1])]).unwrap();
        assert!(!picture(one.clone(), &name), "{tag}");
        let start = Instant::now();
        let timing = AnimationTiming::new(Duration::from_secs(1), Duration::ZERO);
        for (from, to) in [(&one, &two), (&two, &one)] {
            let mut animations = BoxTreeAnimations::new();
            animations
                .request(from, to, "s", properties, timing, start)
                .unwrap();
            animations.advance(start + Duration::from_millis(500), to);
            assert!(picture(animations.presented(to), &name), "{tag}");
        }
        for renderer in ["wgpu", "tiny-skia"] {
            let _ = std::fs::remove_file(
                std::env::temp_dir().join(format!("{name}-{}-{renderer}.png", std::process::id())),
            );
        }
    };
    let style = |rules: &'static str| {
        move |sides: &str| {
            format!(
                "#page {{ background-color:white; padding:20px; color:#000000; }}
                 #p {{ line-height:40px; width:400px; }}
                 #s {{ display:inline; background-color:#ffcc00; {sides} }} {rules}"
            )
        }
    };
    let sides = ["", "padding:20px; margin-left:10px;"];
    let half = "padding:10px; margin-left:5px;";
    let both = &["padding", "margin-left"];
    // Opening its line: its colors go under its text.
    check(
        &|| {
            node(
                "page",
                vec![node(
                    "p",
                    vec![node("s", vec![text("t", "span")]), text("b", " tail")],
                )],
            )
        },
        &style(""),
        sides,
        half,
        both,
        "opening",
    );
    // A `:before` with a box of its own inside it.
    check(
        &|| {
            node(
                "page",
                vec![node(
                    "p",
                    vec![text("a", "head "), node("s", vec![text("t", "span")])],
                )],
            )
        },
        &style("#s:before { content:\"x\"; padding:0 4px; background-color:#ff0000; }"),
        sides,
        half,
        both,
        "generated",
    );
    // A block and an inline-block inside it: its sides stay on its own lines.
    check(
        &|| {
            node(
                "page",
                vec![node(
                    "p",
                    vec![node(
                        "s",
                        vec![
                            node("d", vec![text("b", "block")]),
                            text("c", "after "),
                            node("i", vec![text("y", "row")]),
                        ],
                    )],
                )],
            )
        },
        &style("#d { display:block; } #i { display:inline-block; background-color:#ddeeff; }"),
        sides,
        half,
        both,
        "nested",
    );
    // Right to left: the paragraph's mark stays first.
    check(
        &|| {
            node(
                "page",
                vec![node(
                    "p",
                    vec![node("s", vec![text("t", "span")]), text("b", " tail")],
                )],
            )
        },
        &style("#p { direction:rtl; }"),
        sides,
        half,
        both,
        "rtl",
    );
    // Inside an inline with a background: its padding shows its own.
    check(
        &|| {
            node(
                "page",
                vec![node(
                    "p",
                    vec![node(
                        "o",
                        vec![text("a", "outer "), node("s", vec![text("t", "span")])],
                    )],
                )],
            )
        },
        &style("#o { display:inline; background-color:#ff0000; }"),
        sides,
        half,
        both,
        "outer",
    );
    // A share of the line's width.
    check(
        &|| {
            node(
                "page",
                vec![node(
                    "p",
                    vec![text("a", "head "), node("s", vec![text("t", "span")])],
                )],
            )
        },
        &style(""),
        ["", "padding:0 10%;"],
        "padding:0 5%;",
        &["padding"],
        "percent",
    );
    // With its font size at once.
    check(
        &|| {
            node(
                "page",
                vec![node(
                    "p",
                    vec![text("a", "head "), node("s", vec![text("t", "span")])],
                )],
            )
        },
        &style(""),
        ["font-size:16px;", "font-size:32px; padding:20px;"],
        "font-size:24px; padding:10px;",
        &["padding", "font-size"],
        "sized",
    );
}

#[test]
fn right_to_left_text_keeps_its_mark_before_percentage_sides() {
    // 10% of 400 is 40 pixels: the same line either way.
    let draw = |sides: &str, name: &str| {
        let root = node(
            "page",
            vec![node(
                "p",
                vec![
                    node("s", vec![node("i", vec![text("t", "span")])]),
                    text("b", " tail"),
                ],
            )],
        );
        let sheet = format!(
            "#page {{ background-color:white; padding:20px; color:#000000; }}
             #p {{ width:400px; direction:rtl; }}
             #s {{ display:inline; background-color:#ffcc00; {sides} }}
             #i {{ display:inline; padding:0 4px; background-color:#ff8800; }}"
        );
        let tree = BoxTree::new(root, &sheet).unwrap();
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(name))
            .unwrap()
    };
    // Its own padding, its margin before a nested inline's padding: the mark leads them all.
    let name = format!("rtl-percent-{}.png", std::process::id());
    assert!(draw("padding:0 40px;", &name));
    assert!(draw("padding:0 10%;", &name));
    let margin = format!("rtl-percent-margin-{}.png", std::process::id());
    assert!(draw("margin:0 40px;", &margin));
    assert!(draw("margin:0 10%;", &margin));
    let _ = std::fs::remove_file(std::env::temp_dir().join(format!(
        "rtl-percent-margin-{}-wgpu.png",
        std::process::id()
    )));
    for renderer in ["wgpu", "tiny-skia"] {
        let _ = std::fs::remove_file(
            std::env::temp_dir().join(format!("rtl-percent-{}-{renderer}.png", std::process::id())),
        );
    }
}

#[test]
fn em_lengths_follow_an_animated_font_size() {
    use iced_css::{animation::clock::AnimationTiming, box_tree::BoxTreeAnimations};
    use std::time::{Duration, Instant};
    let source = || {
        node(
            "page",
            vec![node(
                "box",
                vec![
                    text("t", "words in a box "),
                    node("in", vec![text("i", "em")]),
                    text("g", " "),
                    node("px", vec![text("p", "px")]),
                    // A block inheriting the indent, as `text-indent` does.
                    node("kid", vec![text("k", "inherits")]),
                    node("z", vec![text("zz", "zero")]),
                ],
            )],
        )
    };
    // A top border in pixels that the same styling doubles, as the size: no `em`, so it
    // takes its end at once.
    let css = |size: &str, top: &str| {
        format!(
            "#page {{ background-color:white; padding:20px; color:#000000; }}
             #box {{ font-size:{size}; width:10em; padding:0.5em; margin-left:1em;
                     border-left:0.25em solid #0000ff; line-height:1.5em;
                     border-top:{top} solid #00aa00; text-indent:1em;
                     background-color:#ddeeff; }}
             #in {{ display:inline; padding:0 0.5em; background-color:#ffcc00;
                    border-left:0.25em solid #000000; border-top:0.25em solid #ff0000; }}
             #z {{ display:inline; padding-left:0em; background-color:#ccffcc; }}
             #px {{ display:inline; padding:0 6px; background-color:#ff8800; }}"
        )
    };
    let picture = |tree: BoxTree, name: &str| {
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(format!("{name}-{}.png", std::process::id())))
            .unwrap()
    };
    let from = BoxTree::new(source(), &css("16px", "2px")).unwrap();
    let to = from.restyled(&[&css("32px", "4px")]).unwrap();
    assert!(picture(
        BoxTree::new(source(), &css("24px", "4px")).unwrap(),
        "em-between"
    ));
    let start = Instant::now();
    let mut animations = BoxTreeAnimations::new();
    animations
        .request(
            &from,
            &to,
            "box",
            &["font-size"],
            AnimationTiming::new(Duration::from_secs(1), Duration::ZERO),
            start,
        )
        .unwrap();
    animations.advance(start + Duration::from_millis(500), &to);
    assert!(picture(animations.presented(&to), "em-between"));
    for renderer in ["wgpu", "tiny-skia"] {
        let _ = std::fs::remove_file(
            std::env::temp_dir().join(format!("em-between-{}-{renderer}.png", std::process::id())),
        );
    }
}

#[test]
fn an_inline_background_fades_to_and_from_transparent() {
    use iced_css::{animation::clock::AnimationTiming, box_tree::BoxTreeAnimations};
    use std::time::{Duration, Instant};
    let source = || {
        node(
            "page",
            vec![node(
                "p",
                vec![
                    text("a", "before "),
                    node(
                        "o",
                        vec![text("b", "outer "), node("s", vec![text("t", "span")])],
                    ),
                    text("c", " after"),
                ],
            )],
        )
    };
    let css = |color: &str| {
        format!(
            "#page {{ background-color:white; padding:20px; color:#000000; }}
             #p {{ line-height:40px; }}
             #o {{ display:inline; background-color:#9999ff; }}
             #s {{ display:inline; padding:4px 8px; background-color:{color};
                   border:3px solid {color}; }}"
        )
    };
    let picture = |tree: BoxTree, name: &str| {
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(format!("{name}-{}.png", std::process::id())))
            .unwrap()
    };
    let clear = BoxTree::new(source(), &css("transparent")).unwrap();
    let red = clear.restyled(&[&css("#ff0000")]).unwrap();
    assert!(picture(clear.clone(), "fade-clear"));
    assert!(picture(red.clone(), "fade-red"));
    let start = Instant::now();
    let timing = AnimationTiming::new(Duration::from_secs(1), Duration::ZERO);
    let colors = &["background-color", "border-color"];
    for (from, to, first, last) in [
        (&clear, &red, "fade-clear", "fade-red"),
        (&red, &clear, "fade-red", "fade-clear"),
    ] {
        let mut animations = BoxTreeAnimations::new();
        animations
            .request(from, to, "s", colors, timing, start)
            .unwrap();
        // At its start it shows where it comes from; halfway neither end.
        animations.advance(start, to);
        assert!(picture(animations.presented(to), first), "{first}");
        animations.advance(start + Duration::from_millis(500), to);
        assert!(!picture(animations.presented(to), first), "{first}");
        assert!(!picture(animations.presented(to), last), "{last}");
        let _ = picture(animations.presented(to), &format!("{first}-half"));
        animations.advance(start + Duration::from_millis(1500), to);
        assert!(picture(animations.presented(to), last), "{last}");
    }
}

#[test]
fn nested_inline_backgrounds_fade_together() {
    use iced_css::{animation::clock::AnimationTiming, box_tree::BoxTreeAnimations};
    use std::time::{Duration, Instant};
    let source = || {
        node(
            "page",
            vec![node(
                "p",
                vec![
                    text("a", "before "),
                    node(
                        "o",
                        vec![text("b", "outer "), node("s", vec![text("t", "span")])],
                    ),
                ],
            )],
        )
    };
    let css = |outer: &str, own: &str| {
        format!(
            "#page {{ background-color:white; padding:20px; color:#000000; }}
             #p {{ line-height:40px; }}
             #o {{ display:inline; background-color:{outer}; }}
             #s {{ display:inline; padding:4px 8px; background-color:{own}; }}"
        )
    };
    let picture = |tree: BoxTree, name: &str| {
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(format!("{name}-{}.png", std::process::id())))
            .unwrap()
    };
    let clear = BoxTree::new(source(), &css("transparent", "transparent")).unwrap();
    let full = clear.restyled(&[&css("#0000ff", "#ff0000")]).unwrap();
    assert!(picture(clear.clone(), "nested-clear"));
    assert!(picture(full.clone(), "nested-full"));
    let start = Instant::now();
    let timing = AnimationTiming::new(Duration::from_secs(1), Duration::ZERO);
    for (from, to, last) in [
        (&clear, &full, "nested-full"),
        (&full, &clear, "nested-clear"),
    ] {
        let mut animations = BoxTreeAnimations::new();
        for id in ["s", "o"] {
            animations
                .request(from, to, id, &["background-color"], timing, start)
                .unwrap();
        }
        // Halfway the same both ways; at the end, the end.
        animations.advance(start + Duration::from_millis(500), to);
        assert!(picture(animations.presented(to), "nested-half"), "{last}");
        animations.advance(start + Duration::from_millis(1500), to);
        assert!(picture(animations.presented(to), last), "{last}");
    }
    assert!(!picture(clear, "nested-half"));
    // The colors themselves, over the white page: the outer blue at half, the red at
    // half over that in the span, and the red alone in its padding above the line.
    let prefix = format!("nested-half-{}-", std::process::id());
    let file = std::fs::read_dir(std::env::temp_dir())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with(&prefix))
        })
        .unwrap();
    let mut reader = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(file).unwrap()))
        .read_info()
        .unwrap();
    let mut bytes = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut bytes).unwrap();
    let pixels: Vec<[u8; 3]> = bytes[..info.buffer_size()]
        .chunks(4)
        .map(|pixel| [pixel[0], pixel[1], pixel[2]])
        .collect();
    let count = |color: [u8; 3]| {
        pixels
            .iter()
            .filter(|pixel| (0..3).all(|at| pixel[at].abs_diff(color[at]) <= 2))
            .count()
    };
    let outer = count([128, 128, 255]);
    let span = count([191, 64, 128]);
    let padding = count([255, 128, 128]);
    assert!(
        outer > 100 && span > 100 && padding > 100,
        "{outer} {span} {padding}"
    );
    // Not the inner layer twice with the outer between.
    assert_eq!(count([191, 32, 96]), 0);
}

#[test]
fn an_inline_fade_shows_behind_hidden_text_in_it() {
    use iced_css::{animation::clock::AnimationTiming, box_tree::BoxTreeAnimations};
    use std::time::{Duration, Instant};
    let source = || {
        node(
            "page",
            vec![node(
                "p",
                vec![node("s", vec![node("h", vec![text("t", "hidden words")])])],
            )],
        )
    };
    let css = |own: &str| {
        format!(
            "#page {{ background-color:white; padding:20px; color:#000000; }}
             #p {{ line-height:40px; }}
             #s {{ display:inline; background-color:{own}; }}
             #h {{ display:inline; visibility:hidden; }}"
        )
    };
    let picture = |tree: BoxTree, name: &str| {
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(format!("{name}-{}.png", std::process::id())))
            .unwrap()
    };
    let clear = BoxTree::new(source(), &css("transparent")).unwrap();
    let full = clear.restyled(&[&css("#ff0000")]).unwrap();
    assert!(picture(clear.clone(), "hidden-text-clear"));
    let start = Instant::now();
    let mut animations = BoxTreeAnimations::new();
    animations
        .request(
            &clear,
            &full,
            "s",
            &["background-color"],
            AnimationTiming::new(Duration::from_secs(1), Duration::ZERO),
            start,
        )
        .unwrap();
    animations.advance(start + Duration::from_millis(500), &full);
    // The box paints though its text is hidden: not the clear page.
    assert!(!picture(animations.presented(&full), "hidden-text-clear"));
}

#[test]
fn an_inline_outline_animates_where_it_draws() {
    use iced_css::{animation::clock::AnimationTiming, box_tree::BoxTreeAnimations};
    use std::time::{Duration, Instant};
    let source = || {
        node(
            "page",
            vec![node(
                "p",
                vec![text("a", "before "), node("s", vec![text("t", "outlined")])],
            )],
        )
    };
    let css = |color: &str, width: &str| {
        format!(
            "#page {{ background-color:white; padding:20px; color:#000000; }}
             #p {{ line-height:40px; }}
             #s {{ display:inline; outline:{width} solid {color}; }}"
        )
    };
    let picture = |tree: BoxTree, name: &str| {
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(format!("{name}-{}.png", std::process::id())))
            .unwrap()
    };
    let from = BoxTree::new(source(), &css("#ff0000", "2px")).unwrap();
    let to = from.restyled(&[&css("#0000ff", "6px")]).unwrap();
    // Halfway: as wide and as colored as the static outline between them.
    let mid = from.restyled(&[&css("rgb(50%, 0%, 50%)", "4px")]).unwrap();
    assert!(picture(mid, "outline-half"));
    assert!(picture(to.clone(), "outline-end"));
    let start = Instant::now();
    let mut animations = BoxTreeAnimations::new();
    animations
        .request(
            &from,
            &to,
            "s",
            &["outline-color", "outline-width"],
            AnimationTiming::new(Duration::from_secs(1), Duration::ZERO),
            start,
        )
        .unwrap();
    animations.advance(start + Duration::from_millis(500), &to);
    assert!(picture(animations.presented(&to), "outline-half"));
    animations.advance(start + Duration::from_millis(1500), &to);
    assert!(picture(animations.presented(&to), "outline-end"));
    // The colors themselves: the halfway purple, and neither end.
    let prefix = format!("outline-half-{}-", std::process::id());
    let file = std::fs::read_dir(std::env::temp_dir())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with(&prefix))
        })
        .unwrap();
    let mut reader = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(file).unwrap()))
        .read_info()
        .unwrap();
    let mut bytes = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut bytes).unwrap();
    let count = |color: [u8; 3]| {
        bytes[..info.buffer_size()]
            .chunks(4)
            .filter(|pixel| (0..3).all(|at| pixel[at].abs_diff(color[at]) <= 2))
            .count()
    };
    assert!(count([128, 0, 128]) > 100, "{}", count([128, 0, 128]));
    assert_eq!((count([255, 0, 0]), count([0, 0, 255])), (0, 0));
}

#[test]
fn font_weight_animates_through_the_weights_between() {
    use iced_css::{animation::clock::AnimationTiming, box_tree::BoxTreeAnimations};
    use std::time::{Duration, Instant};
    let source = || {
        node(
            "page",
            vec![node(
                "p",
                vec![text("a", "heavier "), node("s", vec![text("t", "text")])],
            )],
        )
    };
    let css = |weight: &str| {
        format!(
            "#page {{ background-color:white; padding:20px; color:#000000; }}
             #p {{ font-weight:{weight}; }}
             #s {{ display:inline; }}"
        )
    };
    let picture = |tree: BoxTree, name: &str| {
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(format!("{name}-{}.png", std::process::id())))
            .unwrap()
    };
    let from = BoxTree::new(source(), &css("400")).unwrap();
    let to = from.restyled(&[&css("800")]).unwrap();
    let mid = from.restyled(&[&css("600")]).unwrap();
    assert!(picture(mid, "weight-half"));
    // The weights draw apart here, so a match says something.
    assert!(!picture(from.clone(), "weight-half"));
    assert!(!picture(to.clone(), "weight-half"));
    let start = Instant::now();
    let mut animations = BoxTreeAnimations::new();
    animations
        .request(
            &from,
            &to,
            "p",
            &["font-weight"],
            AnimationTiming::new(Duration::from_secs(1), Duration::ZERO),
            start,
        )
        .unwrap();
    animations.advance(start + Duration::from_millis(500), &to);
    assert!(picture(animations.presented(&to), "weight-half"));
    animations.advance(start + Duration::from_millis(1500), &to);
    assert!(picture(to.clone(), "weight-end"));
    assert!(picture(animations.presented(&to), "weight-end"));
}

#[test]
fn a_box_border_fades_over_what_lies_beneath() {
    use iced_css::{animation::clock::AnimationTiming, box_tree::BoxTreeAnimations};
    use std::time::{Duration, Instant};
    let source = || node("page", vec![node("b", vec![text("t", "boxed")])]);
    let css = |color: &str| {
        format!(
            "#page {{ background-color:white; padding:20px; color:#000000; }}
             #b {{ border:10px solid {color}; border-left-style:dashed; height:60px; }}"
        )
    };
    let picture = |tree: BoxTree, name: &str| {
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(format!("{name}-{}.png", std::process::id())))
            .unwrap()
    };
    let clear = BoxTree::new(source(), &css("transparent")).unwrap();
    let full = clear.restyled(&[&css("#ff0000")]).unwrap();
    assert!(picture(clear.clone(), "border-clear"));
    assert!(picture(full.clone(), "border-full"));
    let start = Instant::now();
    let timing = AnimationTiming::new(Duration::from_secs(1), Duration::ZERO);
    for (from, to, last) in [
        (&clear, &full, "border-full"),
        (&full, &clear, "border-clear"),
    ] {
        let mut animations = BoxTreeAnimations::new();
        animations
            .request(from, to, "b", &["border-color"], timing, start)
            .unwrap();
        // Halfway the same both ways; at the end, the end.
        animations.advance(start + Duration::from_millis(500), to);
        assert!(picture(animations.presented(to), "border-half"), "{last}");
        animations.advance(start + Duration::from_millis(1500), to);
        assert!(picture(animations.presented(to), last), "{last}");
    }
    // The red at half over the white page, its corners no darker, nothing at full.
    let prefix = format!("border-half-{}-", std::process::id());
    let file = std::fs::read_dir(std::env::temp_dir())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with(&prefix))
        })
        .unwrap();
    let mut reader = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(file).unwrap()))
        .read_info()
        .unwrap();
    let mut bytes = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut bytes).unwrap();
    let count = |color: [u8; 3]| {
        bytes[..info.buffer_size()]
            .chunks(4)
            .filter(|pixel| (0..3).all(|at| pixel[at].abs_diff(color[at]) <= 2))
            .count()
    };
    assert!(count([255, 128, 128]) > 500, "{}", count([255, 128, 128]));
    assert_eq!((count([255, 64, 64]), count([255, 0, 0])), (0, 0));
}

#[test]
fn a_fade_across_a_display_change_paints_as_the_new_display() {
    use iced_css::{animation::clock::AnimationTiming, box_tree::BoxTreeAnimations};
    use std::time::{Duration, Instant};
    let source = || {
        node(
            "page",
            vec![node(
                "p",
                vec![text("a", "before "), node("s", vec![text("t", "fading")])],
            )],
        )
    };
    let css = |display: &str, color: &str| {
        format!(
            "#page {{ background-color:white; padding:20px; color:#000000; }}
             #s {{ display:{display}; background-color:{color}; border:3px solid {color}; }}"
        )
    };
    let picture = |tree: BoxTree, name: &str| {
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(format!("{name}-{}.png", std::process::id())))
            .unwrap()
    };
    let start = Instant::now();
    let halfway = |from: &str, to: &str| {
        let from = BoxTree::new(source(), &css(from, "transparent")).unwrap();
        let to = from.restyled(&[&css(to, "#ff0000")]).unwrap();
        let mut animations = BoxTreeAnimations::new();
        animations
            .request(
                &from,
                &to,
                "s",
                &["background-color", "border-color"],
                AnimationTiming::new(Duration::from_secs(1), Duration::ZERO),
                start,
            )
            .unwrap();
        animations.advance(start + Duration::from_millis(500), &to);
        animations.presented(&to)
    };
    // Halfway as a fade within the new display, which is what shows.
    for (display, other) in [("inline", "block"), ("block", "inline")] {
        let name = format!("display-{display}-half");
        assert!(picture(halfway(display, display), &name));
        assert!(
            picture(halfway(other, display), &name),
            "{other} to {display}"
        );
        let full = BoxTree::new(source(), &css(display, "#ff0000")).unwrap();
        assert!(!picture(full, &name));
    }
}

#[test]
fn collapsed_borders_fade_where_they_win() {
    use iced_css::{animation::clock::AnimationTiming, box_tree::BoxTreeAnimations};
    use std::time::{Duration, Instant};
    let source = || {
        node(
            "page",
            vec![node(
                "t",
                vec![node(
                    "r",
                    vec![
                        node("a", vec![text("x", "wide")]),
                        node("b", vec![text("y", "narrow")]),
                    ],
                )],
            )],
        )
    };
    let css = |wide: &str| {
        format!(
            "#page {{ background-color:white; padding:20px; color:#000000; }}
             #t {{ display:table; border-collapse:collapse; }} #r {{ display:table-row; }}
             #a {{ display:table-cell; border:8px solid {wide}; height:40px; }}
             #b {{ display:table-cell; border:2px solid #0000ff; }}"
        )
    };
    let picture = |tree: BoxTree, name: &str| {
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(format!("{name}-{}.png", std::process::id())))
            .unwrap()
    };
    let clear = BoxTree::new(source(), &css("transparent")).unwrap();
    let full = clear.restyled(&[&css("#ff0000")]).unwrap();
    assert!(picture(clear.clone(), "collapsed-clear"));
    assert!(picture(full.clone(), "collapsed-full"));
    let start = Instant::now();
    let timing = AnimationTiming::new(Duration::from_secs(1), Duration::ZERO);
    for (from, to, last) in [
        (&clear, &full, "collapsed-full"),
        (&full, &clear, "collapsed-clear"),
    ] {
        let mut animations = BoxTreeAnimations::new();
        animations
            .request(from, to, "a", &["border-color"], timing, start)
            .unwrap();
        animations.advance(start + Duration::from_millis(500), to);
        assert!(
            picture(animations.presented(to), "collapsed-half"),
            "{last}"
        );
        animations.advance(start + Duration::from_millis(1500), to);
        assert!(picture(animations.presented(to), last), "{last}");
    }
    // The wide cell's red at half over white, the edge it wins too; no red at full.
    let prefix = format!("collapsed-half-{}-", std::process::id());
    let file = std::fs::read_dir(std::env::temp_dir())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with(&prefix))
        })
        .unwrap();
    let mut reader = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(file).unwrap()))
        .read_info()
        .unwrap();
    let mut bytes = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut bytes).unwrap();
    let count = |color: [u8; 3]| {
        bytes[..info.buffer_size()]
            .chunks(4)
            .filter(|pixel| (0..3).all(|at| pixel[at].abs_diff(color[at]) <= 2))
            .count()
    };
    assert!(count([255, 128, 128]) > 500, "{}", count([255, 128, 128]));
    assert_eq!(count([255, 0, 0]), 0);
    assert!(count([0, 0, 255]) > 0);
}

#[test]
fn a_border_a_hover_gives_fades_its_color_there() {
    use iced_css::{animation::clock::AnimationTiming, box_tree::BoxTreeAnimations};
    use std::time::{Duration, Instant};
    let source = || node("page", vec![node("b", vec![text("t", "hovered")])]);
    let css = |color: &str| {
        format!(
            "#page {{ background-color:white; padding:20px; color:#000000; }}
             #b {{ border:0 solid {color}; height:60px; }}
             #b:hover {{ border-width:10px; }}"
        )
    };
    let hovered = |tree: BoxTree| {
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        let on = iced::Point::new(60.0, 50.0);
        ui.point_at(on);
        let _ = ui.simulate([iced::Event::Mouse(iced::mouse::Event::CursorMoved {
            position: on,
        })]);
        let snapshot = ui.snapshot(&Theme::Light).unwrap();
        let path = std::env::temp_dir().join(format!("hover-border-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        assert!(snapshot.matches_image(path.join("frame.png")).unwrap());
        let file = std::fs::read_dir(&path)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let mut reader =
            png::Decoder::new(std::io::BufReader::new(std::fs::File::open(file).unwrap()))
                .read_info()
                .unwrap();
        let mut bytes = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut bytes).unwrap();
        let count = |color: [u8; 3]| {
            bytes[..info.buffer_size()]
                .chunks(4)
                .filter(|pixel| (0..3).all(|at| pixel[at].abs_diff(color[at]) <= 2))
                .count()
        };
        (count([255, 128, 128]), count([255, 0, 0]))
    };
    let clear = BoxTree::new(source(), &css("transparent")).unwrap();
    let full = clear.restyled(&[&css("#ff0000")]).unwrap();
    // Hovered at the end: the border at full.
    let (_, red) = hovered(full.clone());
    assert!(red > 500, "{red}");
    let start = Instant::now();
    let mut animations = BoxTreeAnimations::new();
    assert_eq!(
        animations
            .request(
                &clear,
                &full,
                "b",
                &["border-color"],
                AnimationTiming::new(Duration::from_secs(1), Duration::ZERO),
                start,
            )
            .unwrap(),
        4
    );
    animations.advance(start + Duration::from_millis(500), &full);
    // Hovered halfway: the red at half, none at full.
    let (half, red) = hovered(animations.presented(&full));
    assert!(half > 500 && red == 0, "{half} {red}");
}

#[test]
fn a_negative_inline_margin_draws_its_box_over_the_text_before() {
    let source = || {
        node(
            "page",
            vec![node(
                "p",
                vec![
                    text("a", "before "),
                    node("s", vec![text("t", "in")]),
                    text("b", " after"),
                ],
            )],
        )
    };
    // The leftmost red and rightmost dark pixel of a page.
    let measure = |css: &str, name: &str| {
        let css = format!("#page {{ background-color:white; padding:20px; color:#000000; }} {css}");
        let tree = BoxTree::new(source(), &css).unwrap();
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        let path = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        assert!(
            ui.snapshot(&Theme::Light)
                .unwrap()
                .matches_image(path.join("frame.png"))
                .unwrap()
        );
        let file = std::fs::read_dir(&path)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let decoder =
            png::Decoder::new(std::io::BufReader::new(std::fs::File::open(file).unwrap()));
        let mut reader = decoder.read_info().unwrap();
        let width = reader.info().width as usize;
        let mut bytes = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut bytes).unwrap();
        let mut red = (usize::MAX, 0);
        let mut dark = 0;
        let mut blue = (usize::MAX, 0);
        for (at, pixel) in bytes[..info.buffer_size()].chunks(4).enumerate() {
            let x = at % width;
            if pixel[0] > 200 && pixel[1] < 60 && pixel[2] < 60 {
                red = (red.0.min(x), red.1.max(x));
            }
            if pixel[0] < 80 && pixel[1] < 80 && pixel[2] < 80 {
                dark = dark.max(x);
            }
            if pixel[0] < 60 && pixel[1] < 60 && pixel[2] > 200 {
                blue = (blue.0.min(x), blue.1 + 1);
            }
        }
        (red, dark, blue)
    };
    let span = |side: &str| format!("#s {{ display:inline; background-color:#ff0000; {side} }}");
    // Pulled by 8 within its 12 pixels of padding: its text where 4 of padding put it, its
    // box 8 further out, over the word before. Snapshots draw at a scale of 2.
    let (reference_red, reference_dark, _) = measure(&span("padding:0 4px;"), "negative-reference");
    let (red, dark, _) = measure(&span("padding:0 12px; margin:0 -8px;"), "negative-pulled");
    assert!(
        (reference_red.0 as i32 - red.0 as i32 - 16).abs() <= 3,
        "{reference_red:?} {red:?}"
    );
    assert!(
        (red.1 as i32 - reference_red.1 as i32 - 16).abs() <= 3,
        "{reference_red:?} {red:?}"
    );
    assert!(
        (reference_dark as i32 - dark as i32).abs() <= 3,
        "{reference_dark} {dark}"
    );
    // Pulled past its padding: the spaces beside it go, so what follows comes nearer.
    let (past_red, past, _) = measure(&span("padding:0 4px; margin:0 -12px;"), "negative-past");
    assert!(past + 2 < reference_dark, "{past} {reference_dark}");
    // Its box still draws from where its padding would start, past the word before.
    assert!(
        past_red.0 + 2 < reference_red.0,
        "{past_red:?} {reference_red:?}"
    );
    // A start border draws at the box's pulled edge: 8 further out.
    let (_, _, plain) = measure(&span("border-left:4px solid #0000ff;"), "negative-bar");
    let (_, _, pulled) = measure(
        &span("border-left:4px solid #0000ff; padding-left:8px; margin-left:-8px;"),
        "negative-bar-pulled",
    );
    assert!(
        (plain.0 as i32 - pulled.0 as i32 - 16).abs() <= 3,
        "{plain:?} {pulled:?}"
    );
    // Beside a percentage padding, out of that padding: box and text both 8 further out
    // than with the padding alone, the space before them kept.
    let (percent_red, percent_dark, _) = measure(&span("padding:0 0 0 10%;"), "negative-percent");
    let (pulled_red, pulled_dark, _) = measure(
        &span("padding:0 0 0 10%; margin-left:-8px;"),
        "negative-percent-pulled",
    );
    assert!(
        (percent_red.0 as i32 - pulled_red.0 as i32 - 16).abs() <= 3,
        "{percent_red:?} {pulled_red:?}"
    );
    assert!(
        (percent_dark as i32 - pulled_dark as i32 - 16).abs() <= 3,
        "{percent_dark} {pulled_dark}"
    );
    // A padding narrower than the pull: all of it, no more, the box drawn from where its
    // text now starts.
    let (thin_red, _, _) = measure(&span("padding:0 0 0 0.5%;"), "negative-thin");
    let (taken_red, _, _) = measure(
        &span("padding:0 0 0 0.5%; margin-left:-8px;"),
        "negative-thin-pulled",
    );
    let moved = thin_red.0 as i32 - taken_red.0 as i32;
    assert!((6..=13).contains(&moved), "{thin_red:?} {taken_red:?}");
    // One bar, at the pulled edge: as many blue pixels as the bar unpulled.
    assert!(pulled.1 <= plain.1 + plain.1 / 8, "{plain:?} {pulled:?}");
}

#[test]
fn a_negative_inline_margin_pulls_only_its_own_box_and_outward() {
    // Extents of red, blue and dark pixels: (leftmost, rightmost) of each.
    let measure = |source: Node, css: &str, name: &str| {
        let css = format!("#page {{ background-color:white; padding:20px; color:#000000; }} {css}");
        let tree = BoxTree::new(source, &css).unwrap();
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        let path = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        assert!(
            ui.snapshot(&Theme::Light)
                .unwrap()
                .matches_image(path.join("frame.png"))
                .unwrap()
        );
        let file = std::fs::read_dir(&path)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let decoder =
            png::Decoder::new(std::io::BufReader::new(std::fs::File::open(file).unwrap()));
        let mut reader = decoder.read_info().unwrap();
        let width = reader.info().width as usize;
        let mut bytes = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut bytes).unwrap();
        let mut extents = [(usize::MAX, 0); 3];
        for (at, pixel) in bytes[..info.buffer_size()].chunks(4).enumerate() {
            let x = at % width;
            let which = if pixel[0] > 200 && pixel[1] < 60 && pixel[2] < 60 {
                0
            } else if pixel[0] < 60 && pixel[1] < 60 && pixel[2] > 200 {
                1
            } else if pixel[0] < 80 && pixel[1] < 80 && pixel[2] < 80 {
                2
            } else {
                continue;
            };
            extents[which] = (extents[which].0.min(x), extents[which].1.max(x));
        }
        extents
    };
    let nested = || {
        node(
            "page",
            vec![node(
                "p",
                vec![
                    text("a", "before "),
                    node(
                        "x",
                        vec![node("s", vec![text("t", "in")]), text("b", " after")],
                    ),
                ],
            )],
        )
    };
    // A box around it keeps its own edges: the pull is the inner box's alone.
    let outer = "#x { display:inline; border:2px solid #0000ff; }";
    let plain = measure(
        nested(),
        &format!("{outer} #s {{ display:inline; }}"),
        "pull-outer",
    );
    let pulled = measure(
        nested(),
        &format!(
            "{outer} #s {{ display:inline; background-color:#ff0000; padding-left:8px; margin-left:-8px; }}"
        ),
        "pull-outer-pulled",
    );
    assert!(
        (plain[1].0 as i32 - pulled[1].0 as i32).abs() <= 2,
        "{plain:?} {pulled:?}"
    );
    // Nor when it is hidden: nothing of it draws, and the box around it stays.
    let hidden = measure(
        nested(),
        &format!(
            "{outer} #s {{ display:inline; visibility:hidden; padding-left:8px; margin-left:-8px; }}"
        ),
        "pull-outer-hidden",
    );
    assert!(
        (plain[1].0 as i32 - hidden[1].0 as i32).abs() <= 2,
        "{plain:?} {hidden:?}"
    );
    // Its own top band draws past its spaces too.
    let banded =
        |side: &str| format!("#s {{ display:inline; border-top:2px solid #0000ff; {side} }}");
    let flat = measure(nested(), &banded("padding-left:4px;"), "pull-band");
    let pulled = measure(
        nested(),
        &banded("padding-left:12px; margin-left:-8px;"),
        "pull-band-pulled",
    );
    assert!(
        (flat[1].0 as i32 - pulled[1].0 as i32 - 16).abs() <= 3,
        "{flat:?} {pulled:?}"
    );
    // Its end pulled into what follows inside a box with a background: still drawn.
    let outer = "#x { display:inline; background-color:#0000ff; color:#ffffff; }";
    let end =
        |side: &str| format!("{outer} #s {{ display:inline; background-color:#ff0000; {side} }}");
    let reference = measure(nested(), &end("padding-right:4px;"), "pull-end");
    let pulled = measure(
        nested(),
        &end("padding-right:12px; margin-right:-8px;"),
        "pull-end-pulled",
    );
    assert!(
        (pulled[0].1 as i32 - reference[0].1 as i32 - 16).abs() <= 3,
        "{reference:?} {pulled:?}"
    );
    // In a right-to-left paragraph, left-to-right words keep their order: its start is
    // still on the left of its text, and the box draws further left, not over its text.
    let rtl = || {
        node(
            "page",
            vec![node(
                "p",
                vec![
                    text("a", "before "),
                    node("s", vec![text("t", "in")]),
                    text("b", " after"),
                ],
            )],
        )
    };
    let side = |side: &str| {
        format!("#p {{ direction:rtl; }} #s {{ display:inline; background-color:#ff0000; {side} }}")
    };
    let reference = measure(rtl(), &side("padding-left:4px;"), "pull-rtl");
    let pulled = measure(
        rtl(),
        &side("padding-left:12px; margin-left:-8px;"),
        "pull-rtl-pulled",
    );
    assert!(
        (reference[0].0 as i32 - pulled[0].0 as i32 - 16).abs() <= 3,
        "{reference:?} {pulled:?}"
    );
    assert!(
        (pulled[0].1 as i32 - reference[0].1 as i32).abs() <= 2,
        "{reference:?} {pulled:?}"
    );
}

#[test]
fn a_negative_inline_margin_animates_as_its_box_pulls() {
    use iced_css::{animation::clock::AnimationTiming, box_tree::BoxTreeAnimations};
    use std::time::{Duration, Instant};
    let source = || {
        node(
            "page",
            vec![node(
                "p",
                vec![
                    text("a", "before "),
                    node("s", vec![text("t", "in")]),
                    text("b", " after"),
                ],
            )],
        )
    };
    let css = |margin: &str| {
        format!(
            "#page {{ background-color:white; padding:20px; color:#000000; }}
             #s {{ display:inline; background-color:#ff0000; border-left:2px solid #0000ff;
                   padding:0 12px; margin:0 {margin}; }}"
        )
    };
    let picture = |tree: BoxTree, name: &str| {
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(format!("{name}-{}.png", std::process::id())))
            .unwrap()
    };
    let from = BoxTree::new(source(), &css("-8px")).unwrap();
    let to = from.restyled(&[&css("0")]).unwrap();
    let mid = from.restyled(&[&css("-4px")]).unwrap();
    assert!(picture(mid, "pull-half"));
    assert!(!picture(to.clone(), "pull-half"));
    let start = Instant::now();
    for (one, two) in [(&from, &to), (&to, &from)] {
        let mut animations = BoxTreeAnimations::new();
        animations
            .request(
                one,
                two,
                "s",
                &["margin-left", "margin-right"],
                AnimationTiming::new(Duration::from_secs(1), Duration::ZERO),
                start,
            )
            .unwrap();
        // Halfway, either way: the box pulled by 4, as a page with -4 draws it.
        animations.advance(start + Duration::from_millis(500), two);
        assert!(picture(animations.presented(two), "pull-half"));
    }
}

#[test]
fn a_before_animates_its_own_colors_where_it_draws() {
    use iced_css::{animation::clock::AnimationTiming, box_tree::BoxTreeAnimations};
    use std::time::{Duration, Instant};
    let source = || {
        node(
            "page",
            vec![node("p", vec![node("s", vec![text("t", "words")])])],
        )
    };
    let picture = |tree: BoxTree, name: &str| {
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(format!("{name}-{}.png", std::process::id())))
            .unwrap()
    };
    let start = Instant::now();
    let timing = AnimationTiming::new(Duration::from_secs(1), Duration::ZERO);
    // Its text color halfway, as a page with the color between draws it.
    let inline = |color: &str| {
        format!(
            "#page {{ background-color:white; padding:20px; color:#000000; }}
             #s {{ display:inline; }} #s:before {{ content:\"before \"; color:{color}; }}"
        )
    };
    let from = BoxTree::new(source(), &inline("#ff0000")).unwrap();
    let to = from.restyled(&[&inline("#0000ff")]).unwrap();
    let mid = from.restyled(&[&inline("rgb(50%, 0%, 50%)")]).unwrap();
    assert!(picture(mid, "before-color-half"));
    assert!(!picture(to.clone(), "before-color-half"));
    let mut animations = BoxTreeAnimations::new();
    animations
        .request(&from, &to, "s::before", &["color"], timing, start)
        .unwrap();
    animations.advance(start + Duration::from_millis(500), &to);
    assert!(picture(animations.presented(&to), "before-color-half"));
    // A block one's background and border, from transparent, at half over the white page:
    // half inside, and its border at half over its background at half (backgrounds reach
    // under the border).
    let block = |color: &str| {
        format!(
            "#page {{ background-color:white; padding:20px; color:#000000; }}
             #s:before {{ content:\"block\"; display:block; height:40px;
                          background-color:{color}; border:6px solid {color}; }}"
        )
    };
    let clear = BoxTree::new(source(), &block("transparent")).unwrap();
    let full = clear.restyled(&[&block("#ff0000")]).unwrap();
    let mut animations = BoxTreeAnimations::new();
    animations
        .request(
            &clear,
            &full,
            "s::before",
            &["background-color", "border-color"],
            timing,
            start,
        )
        .unwrap();
    animations.advance(start + Duration::from_millis(500), &full);
    assert!(picture(animations.presented(&full), "before-block-half"));
    let prefix = format!("before-block-half-{}-", std::process::id());
    let file = std::fs::read_dir(std::env::temp_dir())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with(&prefix))
        })
        .unwrap();
    let mut reader = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(file).unwrap()))
        .read_info()
        .unwrap();
    let mut bytes = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut bytes).unwrap();
    let count = |color: [u8; 3]| {
        bytes[..info.buffer_size()]
            .chunks(4)
            .filter(|pixel| (0..3).all(|at| pixel[at].abs_diff(color[at]) <= 2))
            .count()
    };
    assert!(count([255, 128, 128]) > 1000, "{}", count([255, 128, 128]));
    assert!(count([255, 64, 64]) > 1000, "{}", count([255, 64, 64]));
    assert_eq!(count([255, 0, 0]), 0);
    // An `:after` block's own, beside a `:before` block that keeps its color.
    let both = |color: &str| {
        format!(
            "#page {{ background-color:white; padding:20px; color:#000000; }}
             #s:before {{ content:\"one\"; display:block; height:40px; background-color:#00ff00; }}
             #s:after {{ content:\"two\"; display:block; height:40px; background-color:{color}; }}"
        )
    };
    let clear = BoxTree::new(source(), &both("transparent")).unwrap();
    let full = clear.restyled(&[&both("#ff0000")]).unwrap();
    assert!(picture(clear.clone(), "after-block-clear"));
    let mut animations = BoxTreeAnimations::new();
    animations
        .request(
            &clear,
            &full,
            "s::after",
            &["background-color"],
            timing,
            start,
        )
        .unwrap();
    animations.advance(start + Duration::from_millis(500), &full);
    assert!(!picture(animations.presented(&full), "after-block-clear"));
    let snapshot = {
        let view: Element<'_, u8> = animations.presented(&full).view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light).unwrap()
    };
    let path = std::env::temp_dir().join(format!("after-block-half-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    assert!(snapshot.matches_image(path.join("frame.png")).unwrap());
    let file = std::fs::read_dir(&path)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let mut reader = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(file).unwrap()))
        .read_info()
        .unwrap();
    let mut bytes = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut bytes).unwrap();
    let count = |color: [u8; 3]| {
        bytes[..info.buffer_size()]
            .chunks(4)
            .filter(|pixel| (0..3).all(|at| pixel[at].abs_diff(color[at]) <= 2))
            .count()
    };
    // The `:before` still all green; the `:after` at half red.
    assert!(count([0, 255, 0]) > 1000 && count([255, 128, 128]) > 1000);
    assert_eq!(count([128, 255, 128]), 0);
}

#[test]
fn a_block_before_animates_its_own_box() {
    use iced_css::{animation::clock::AnimationTiming, box_tree::BoxTreeAnimations};
    use std::time::{Duration, Instant};
    let source = || node("page", vec![node("s", vec![text("t", "after it")])]);
    let picture = |tree: BoxTree, name: &str| {
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(format!("{name}-{}.png", std::process::id())))
            .unwrap()
    };
    let css = |width: &str, side: &str, size: &str| {
        format!(
            "#page {{ background-color:white; padding:20px; color:#000000; }}
             #s:before {{ content:\"block\"; display:block; background-color:#ff0000;
                          border:{side} solid #0000ff; padding:{side}; margin:{side};
                          width:{width}; height:{width}; font-size:{size}; }}"
        )
    };
    let from = BoxTree::new(source(), &css("100px", "4px", "16px")).unwrap();
    let to = from.restyled(&[&css("200px", "12px", "32px")]).unwrap();
    let mid = from.restyled(&[&css("150px", "8px", "24px")]).unwrap();
    assert!(picture(mid, "before-box-half"));
    assert!(!picture(to.clone(), "before-box-half"));
    let start = Instant::now();
    for (one, two) in [(&from, &to), (&to, &from)] {
        let mut animations = BoxTreeAnimations::new();
        let count = animations
            .request(
                one,
                two,
                "s::before",
                &[
                    "width",
                    "height",
                    "padding",
                    "border-width",
                    "margin",
                    "font-size",
                ],
                AnimationTiming::new(Duration::from_secs(1), Duration::ZERO),
                start,
            )
            .unwrap();
        assert_eq!(count, 15);
        // Halfway, either way: as a page with the lengths between draws it.
        animations.advance(start + Duration::from_millis(500), two);
        assert!(picture(animations.presented(two), "before-box-half"));
    }
}

#[test]
fn a_before_grows_with_its_line_and_small_capitals() {
    use iced_css::{animation::clock::AnimationTiming, box_tree::BoxTreeAnimations};
    use std::time::{Duration, Instant};
    let source = || {
        node(
            "page",
            vec![node("p", vec![node("s", vec![text("t", "words")])])],
        )
    };
    let picture = |tree: BoxTree, name: &str| {
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(format!("{name}-{}.png", std::process::id())))
            .unwrap()
    };
    let start = Instant::now();
    // A sided inline one in small capitals (generated content takes them from its element),
    // and a block one under an inherited indent.
    for (name, css) in [
        (
            "grow-inline",
            "#s { display:inline; font-variant:small-caps; }
             #s:before { content:\"Gen \"; padding:0 5px; background-color:#ff0000; font-size:SIZE; }",
        ),
        (
            "grow-block",
            "#p { text-indent:20px; } #s:before { content:\"Block\"; display:block;
             background-color:#ff0000; font-size:SIZE; }",
        ),
    ] {
        let page = |size: &str| {
            format!(
                "#page {{ background-color:white; padding:20px; color:#000000; }} {}",
                css.replace("SIZE", size)
            )
        };
        let from = BoxTree::new(source(), &page("16px")).unwrap();
        let to = from.restyled(&[&page("32px")]).unwrap();
        let mid = from.restyled(&[&page("24px")]).unwrap();
        let half = format!("{name}-half");
        assert!(picture(mid, &half));
        assert!(!picture(to.clone(), &half));
        let mut animations = BoxTreeAnimations::new();
        animations
            .request(
                &from,
                &to,
                "s::before",
                &["font-size"],
                AnimationTiming::new(Duration::from_secs(1), Duration::ZERO),
                start,
            )
            .unwrap();
        animations.advance(start + Duration::from_millis(500), &to);
        assert!(picture(animations.presented(&to), &half), "{name}");
    }
}

#[test]
fn an_inline_before_animates_its_sides_on_its_line() {
    use iced_css::{animation::clock::AnimationTiming, box_tree::BoxTreeAnimations};
    use std::time::{Duration, Instant};
    let source = || {
        node(
            "page",
            vec![node(
                "p",
                vec![text("a", "before "), node("s", vec![text("t", "words")])],
            )],
        )
    };
    let picture = |tree: BoxTree, name: &str| {
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(format!("{name}-{}.png", std::process::id())))
            .unwrap()
    };
    let css = |padding: &str, border: &str, margin: &str| {
        format!(
            "#page {{ background-color:white; padding:20px; color:#000000; }}
             #p {{ line-height:40px; }}
             #s {{ display:inline; }}
             #s:before {{ content:\"GEN\"; background-color:#ff0000;
                          padding:{padding}; border:{border} solid #0000ff; margin:0 {margin}; }}"
        )
    };
    let from = BoxTree::new(source(), &css("2px 6px", "2px", "4px")).unwrap();
    let to = from.restyled(&[&css("6px 18px", "6px", "12px")]).unwrap();
    let mid = from.restyled(&[&css("4px 12px", "4px", "8px")]).unwrap();
    assert!(picture(mid, "before-sides-half"));
    assert!(!picture(to.clone(), "before-sides-half"));
    let start = Instant::now();
    for (one, two) in [(&from, &to), (&to, &from)] {
        let mut animations = BoxTreeAnimations::new();
        let count = animations
            .request(
                one,
                two,
                "s::before",
                &["padding", "border-width", "margin-left", "margin-right"],
                AnimationTiming::new(Duration::from_secs(1), Duration::ZERO),
                start,
            )
            .unwrap();
        assert_eq!(count, 10);
        // Halfway, either way: as a page with the sides between draws it.
        animations.advance(start + Duration::from_millis(500), two);
        assert!(picture(animations.presented(two), "before-sides-half"));
    }
}

#[test]
fn an_inline_before_fades_its_background_on_its_line() {
    use iced_css::{animation::clock::AnimationTiming, box_tree::BoxTreeAnimations};
    use std::time::{Duration, Instant};
    let source = || {
        node(
            "page",
            vec![node(
                "p",
                vec![text("a", "before "), node("s", vec![text("t", "words")])],
            )],
        )
    };
    let start = Instant::now();
    for (name, sides) in [("fade-sided", "padding:4px 8px;"), ("fade-plain", "")] {
        let css = |color: &str| {
            format!(
                "#page {{ background-color:white; padding:20px; color:#000000; }}
                 #p {{ line-height:40px; }} #s {{ display:inline; }}
                 #s:before {{ content:\"GEN \"; {sides} background-color:{color}; }}"
            )
        };
        let clear = BoxTree::new(source(), &css("transparent")).unwrap();
        let full = clear.restyled(&[&css("#ff0000")]).unwrap();
        let half = |one: &BoxTree, two: &BoxTree, path: &std::path::Path| {
            let mut animations = BoxTreeAnimations::new();
            assert_eq!(
                animations
                    .request(
                        one,
                        two,
                        "s::before",
                        &["background-color"],
                        AnimationTiming::new(Duration::from_secs(1), Duration::ZERO),
                        start,
                    )
                    .unwrap(),
                1
            );
            animations.advance(start + Duration::from_millis(500), two);
            let view: Element<'_, u8> = animations.presented(two).view();
            let mut ui = simulator(view);
            ui.snapshot(&Theme::Light)
                .unwrap()
                .matches_image(path)
                .unwrap()
        };
        let path = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        let frame = path.join("frame.png");
        // Halfway the same both ways.
        assert!(half(&clear, &full, &frame));
        assert!(half(&full, &clear, &frame), "{name}");
        let file = std::fs::read_dir(&path)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let mut reader =
            png::Decoder::new(std::io::BufReader::new(std::fs::File::open(file).unwrap()))
                .read_info()
                .unwrap();
        let mut bytes = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut bytes).unwrap();
        let count = |color: [u8; 3]| {
            bytes[..info.buffer_size()]
                .chunks(4)
                .filter(|pixel| (0..3).all(|at| pixel[at].abs_diff(color[at]) <= 2))
                .count()
        };
        // The red at half over the white page, nothing at full.
        assert!(
            count([255, 128, 128]) > 200,
            "{name} {}",
            count([255, 128, 128])
        );
        assert_eq!(count([255, 0, 0]), 0, "{name}");
    }
}

#[test]
fn a_block_before_carries_its_current_color_border() {
    use iced_css::{animation::clock::AnimationTiming, box_tree::BoxTreeAnimations};
    use std::time::{Duration, Instant};
    let source = || node("page", vec![node("s", vec![text("t", "after it")])]);
    let picture = |tree: BoxTree, name: &str| {
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(format!("{name}-{}.png", std::process::id())))
            .unwrap()
    };
    let css = |color: &str| {
        format!(
            "#page {{ background-color:white; padding:20px; }}
             #s:before {{ content:\"block\"; display:block; height:40px; color:{color}; border:6px solid; }}"
        )
    };
    let from = BoxTree::new(source(), &css("#ff0000")).unwrap();
    let to = from.restyled(&[&css("#0000ff")]).unwrap();
    let mid = from.restyled(&[&css("rgb(50%, 0%, 50%)")]).unwrap();
    assert!(picture(mid, "current-border-half"));
    let start = Instant::now();
    let mut animations = BoxTreeAnimations::new();
    animations
        .request(
            &from,
            &to,
            "s::before",
            &["color"],
            AnimationTiming::new(Duration::from_secs(1), Duration::ZERO),
            start,
        )
        .unwrap();
    animations.advance(start + Duration::from_millis(500), &to);
    // Its text and its border both at the color between.
    assert!(picture(animations.presented(&to), "current-border-half"));
}

#[test]
fn an_inline_block_sits_on_the_line_baseline() {
    let source = || {
        node(
            "page",
            vec![node(
                "p",
                vec![
                    text("a", "xxx "),
                    node("b", vec![text("t", "xxx")]),
                    text("c", " xxx"),
                ],
            )],
        )
    };
    // The lowest row of black and of blue pixels.
    let measured = |source: Node, css: &str, name: &str| {
        let css = format!("#page {{ background-color:white; padding:20px; color:#000000; }} {css}");
        let tree = BoxTree::new(source, &css).unwrap();
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        let path = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        assert!(
            ui.snapshot(&Theme::Light)
                .unwrap()
                .matches_image(path.join("frame.png"))
                .unwrap()
        );
        let file = std::fs::read_dir(&path)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let decoder =
            png::Decoder::new(std::io::BufReader::new(std::fs::File::open(file).unwrap()));
        let mut reader = decoder.read_info().unwrap();
        let width = reader.info().width as usize;
        let mut bytes = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut bytes).unwrap();
        let (mut black, mut blue) = (0, 0);
        for (at, pixel) in bytes[..info.buffer_size()].chunks(4).enumerate() {
            let y = at / width;
            let (r, g, b) = (
                i32::from(pixel[0]),
                i32::from(pixel[1]),
                i32::from(pixel[2]),
            );
            // Dark gray, and clearly blue, antialiased edges too.
            if r < 128 && g < 128 && b < 128 && (b - r).abs() < 40 {
                black = black.max(y);
            }
            if b > 150 && r < 110 && g < 110 {
                blue = blue.max(y);
            }
        }
        (black, blue)
    };
    let bottoms = |css: &str, name: &str| measured(source(), css, name);
    // A bigger font and a deep bottom padding: its text still on the line's baseline.
    let (black, blue) = bottoms(
        "#b { display:inline-block; color:#0000ff; font-size:28px; padding-bottom:30px; }",
        "baseline-block",
    );
    assert!(black.abs_diff(blue) <= 2, "{black} {blue}");
    // Aligned to the bottom instead, its box fills the line and the text beside it sits at
    // the line's top: off the shared baseline.
    let (black, blue) = bottoms(
        "#b { display:inline-block; color:#0000ff; font-size:28px; padding-bottom:30px;
              vertical-align:bottom; }",
        "bottom-block",
    );
    assert!(blue > black + 10, "{black} {blue}");
    // A run whose font family is missing gives its line nothing; the rest of it does.
    let missing = node(
        "page",
        vec![node(
            "p",
            vec![
                text("a", "xxx "),
                node(
                    "b",
                    vec![node("odd", vec![text("t", "xx")]), text("u", "xx")],
                ),
                text("c", " xxx"),
            ],
        )],
    );
    let (black, blue) = measured(
        missing,
        "#b { display:inline-block; color:#0000ff; padding-bottom:30px; }
         #odd { display:inline; font-family:NoSuchFamilyAnywhere; }",
        "missing-block",
    );
    assert!(black.abs_diff(blue) <= 2, "{black} {blue}");
    // In another family, whose faces' metrics differ: the faces the text is drawn in, as
    // the ink shows, share it.
    for family in ["monospace", "serif"] {
        let (black, blue) = bottoms(
            &format!(
                "#b {{ display:inline-block; color:#0000ff; font-size:28px; font-family:{family};
                      padding-bottom:30px; }}"
            ),
            &format!("{family}-block"),
        );
        assert!(black.abs_diff(blue) <= 2, "{family} {black} {blue}");
    }
    // At a weight a family may not have.
    let (black, blue) = bottoms(
        "#b { display:inline-block; color:#0000ff; font-size:28px; font-weight:900;
              padding-bottom:30px; }",
        "heavy-block",
    );
    assert!(black.abs_diff(blue) <= 2, "{black} {blue}");
    // Inside an absolutely positioned box too.
    let (black, blue) = bottoms(
        "#p { position:absolute; left:20px; top:20px; }
         #b { display:inline-block; color:#0000ff; font-size:28px; padding-bottom:30px; }",
        "absolute-block",
    );
    assert!(black.abs_diff(blue) <= 2, "{black} {blue}");
    // A tall line height, which centers each line's text in more room: still on it.
    let (black, blue) = bottoms(
        "#b { display:inline-block; color:#0000ff; font-size:28px; line-height:3; }",
        "tall-block",
    );
    assert!(black.abs_diff(blue) <= 2, "{black} {blue}");
    // Its last line in a smaller font than its first: that line's baseline.
    let mixed = node(
        "page",
        vec![node(
            "p",
            vec![
                text("a", "xxx "),
                node(
                    "b",
                    vec![node("big", vec![text("t", "XX")]), text("u", " xx")],
                ),
                text("c", " xxx"),
            ],
        )],
    );
    let (black, blue) = measured(
        mixed,
        "#b { display:inline-block; color:#0000ff; width:60px; }
         #big { display:inline; font-size:40px; }",
        "mixed-block",
    );
    assert!(black.abs_diff(blue) <= 2, "{black} {blue}");
    // An inline-block with no line, or that clips: its bottom margin edge on the baseline.
    let (black, empty) = bottoms(
        "#b { display:inline-block; color:#0000ff; font-size:28px; overflow:hidden;
              padding-bottom:30px; }",
        "clipping-block",
    );
    assert!(empty < black, "{empty} {black}");
}

#[test]
fn layout_with_a_renderer_matches_the_widget() {
    use iced::advanced::renderer::Headless;
    let source = || {
        node(
            "page",
            vec![node(
                "p",
                vec![
                    text("a", "xxx "),
                    node("b", vec![text("t", "xxx")]),
                    text("c", " xxx"),
                ],
            )],
        )
    };
    let css = "#page { background-color:white; padding:20px; color:#000000; }
               #b { display:inline-block; background-color:#ff0000; font-size:28px;
                    padding-bottom:30px; }";
    let tree = BoxTree::new(source(), css).unwrap();
    let renderer = iced_test::futures::futures::executor::block_on(
        iced_test::renderer::Renderer::new(iced::Font::default(), iced::Pixels(16.0), None),
    )
    .unwrap();
    // Page, paragraph, its first text, then the inline-block.
    let laid = tree.layout_with(&renderer, 1024.0, None).unwrap();
    let boxed = laid[3].unwrap().border;
    // Where the widget draws it: its red, at the snapshot's scale of 2.
    let view: Element<'_, u8> = tree.clone().view();
    let mut ui = simulator(view);
    let path = std::env::temp_dir().join(format!("layout-with-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    assert!(
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(path.join("frame.png"))
            .unwrap()
    );
    let file = std::fs::read_dir(&path)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(file).unwrap()));
    let mut reader = decoder.read_info().unwrap();
    let width = reader.info().width as usize;
    let mut bytes = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut bytes).unwrap();
    let (mut top, mut bottom) = (usize::MAX, 0);
    for (at, pixel) in bytes[..info.buffer_size()].chunks(4).enumerate() {
        if pixel[0] > 200 && pixel[1] < 60 && pixel[2] < 60 {
            top = top.min(at / width);
            bottom = bottom.max(at / width);
        }
    }
    assert!(
        (boxed.top * 2.0 - top as f32).abs() <= 2.0
            && (boxed.bottom * 2.0 - bottom as f32).abs() <= 3.0,
        "{boxed:?} {top} {bottom}"
    );
    // A percentage indent, sized once the block's width is known: as tall as the widget
    // draws it.
    let long = || {
        node(
            "page",
            vec![node(
                "p",
                vec![text(
                    "a",
                    "words that wrap onto more lines once a wide first-line indent pushes them",
                )],
            )],
        )
    };
    let indented = BoxTree::new(
        long(),
        "#page { background-color:white; width:300px; }
         #p { background-color:#ff0000; text-indent:60%; }",
    )
    .unwrap();
    let block = indented.layout_with(&renderer, 1024.0, None).unwrap()[1]
        .unwrap()
        .border;
    let view: Element<'_, u8> = indented.view();
    let mut ui = simulator(view);
    let path = std::env::temp_dir().join(format!("layout-with-indent-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    assert!(
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(path.join("frame.png"))
            .unwrap()
    );
    let file = std::fs::read_dir(&path)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(file).unwrap()));
    let mut reader = decoder.read_info().unwrap();
    let width = reader.info().width as usize;
    let mut bytes = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut bytes).unwrap();
    let red_bottom = bytes[..info.buffer_size()]
        .chunks(4)
        .enumerate()
        .filter(|(_, pixel)| pixel[0] > 200 && pixel[1] < 60 && pixel[2] < 60)
        .map(|(at, _)| at / width)
        .max()
        .unwrap();
    assert!(
        (block.bottom * 2.0 - red_bottom as f32).abs() <= 3.0,
        "{block:?} {red_bottom}"
    );
    // With taller text beside it, bottoms up (as `layout_text` lines them) it sits lower
    // than on the baseline.
    let small = BoxTree::new(
        source(),
        "#p { font-size:40px; } #b { display:inline-block; font-size:16px; }",
    )
    .unwrap();
    let on_baseline = small.layout_with(&renderer, 1024.0, None).unwrap()[3]
        .unwrap()
        .border;
    let bottoms = small
        .layout_text(1024.0, None, &mut |leaf, _| {
            leaf.runs
                .iter()
                .map(|run| run.size * 1.3)
                .fold(0.0, f32::max)
        })
        .unwrap()[3]
        .unwrap()
        .border;
    assert!(on_baseline.top < bottoms.top, "{on_baseline:?} {bottoms:?}");
}

#[test]
fn every_inline_box_fades_as_it_shows() {
    use iced_css::{animation::clock::AnimationTiming, box_tree::BoxTreeAnimations};
    use std::time::{Duration, Instant};
    let picture = |tree: &BoxTree, name: &str| {
        let view: Element<'_, u8> = tree.clone().view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(format!("{name}-{}.png", std::process::id())))
            .unwrap()
    };
    // `id` fades out, then after a delay back in: halfway out it draws as shown, through the
    // delay as hidden, then as shown again.
    let fade = |source: &dyn Fn() -> Node, css: &dyn Fn(&str) -> String, id: &str, tag: &str| {
        let shown = BoxTree::new(source(), &css("visible")).unwrap();
        let hidden = shown.restyled(&[&css("hidden")]).unwrap();
        let (on, off) = (format!("fade-{tag}-shown"), format!("fade-{tag}-hidden"));
        assert!(picture(&shown, &on));
        assert!(picture(&hidden, &off));
        assert!(!picture(&hidden, &on), "{tag}");
        let start = Instant::now();
        let second = AnimationTiming::new(Duration::from_secs(1), Duration::ZERO);
        let mut animations = BoxTreeAnimations::new();
        animations
            .request(&shown, &hidden, id, &["visibility"], second, start)
            .unwrap();
        animations.advance(start + Duration::from_millis(500), &hidden);
        assert!(
            picture(&animations.presented(&hidden), &on),
            "{tag} halfway out"
        );
        let delayed = AnimationTiming::new(Duration::from_secs(1), Duration::from_secs(1));
        let mut animations = BoxTreeAnimations::new();
        animations
            .request(&hidden, &shown, id, &["visibility"], delayed, start)
            .unwrap();
        animations.advance(start + Duration::from_millis(500), &shown);
        assert!(
            picture(&animations.presented(&shown), &off),
            "{tag} delayed"
        );
        animations.advance(start + Duration::from_millis(1100), &shown);
        assert!(
            picture(&animations.presented(&shown), &on),
            "{tag} coming in"
        );
        for name in [on, off] {
            for renderer in ["wgpu", "tiny-skia"] {
                let _ = std::fs::remove_file(
                    std::env::temp_dir()
                        .join(format!("{name}-{}-{renderer}.png", std::process::id())),
                );
            }
        }
    };
    // A block: an inline box with a `:before` hidden of its own and a hidden box around a
    // shown one inside it, and a plain inline background around a child with sides.
    let block = || {
        node(
            "page",
            vec![node(
                "bar",
                vec![
                    node(
                        "in",
                        vec![
                            text("i", "boxed "),
                            node("d", vec![node("c", vec![text("cc", "kept")])]),
                        ],
                    ),
                    text("gap", " "),
                    node("plain", vec![node("x", vec![text("xx", "x")])]),
                ],
            )],
        )
    };
    let block_css = |shown: &str| {
        format!(
            "#page {{ background-color:white; padding:20px; color:#000000; }}
             #bar {{ width:300px; visibility:{shown}; }}
             #in {{ display:inline; padding:0 4px; border:2px solid #000000;
                    background-color:#00ffff; }}
             #in:before {{ content:\"z\"; visibility:hidden; background-color:#ff00ff; }}
             #d {{ display:inline; visibility:hidden; background-color:#0000aa; }}
             #c {{ display:inline; visibility:visible; }}
             #plain {{ display:inline; background-color:#ff8800; }}
             #x {{ display:inline; margin:0 10px; padding:0 4px; }}"
        )
    };
    fade(&block, &block_css, "bar", "block");
    // An inline in a shown inline's background, with a `:before` hidden of its own whose
    // side spaces match its own, and an `:after` with sides of its own that follows it.
    let inline = || {
        node(
            "page",
            vec![node(
                "o",
                vec![
                    text("a", "outer "),
                    node("in2", vec![text("b", "inner")]),
                    text("c", " end"),
                ],
            )],
        )
    };
    let inline_css = |shown: &str| {
        format!(
            "#page {{ background-color:white; padding:20px; color:#000000; }}
             #o {{ display:inline; background-color:#ff0000; }}
             #in2 {{ display:inline; padding:0 4px; border:2px solid #000000;
                     visibility:{shown}; }}
             #in2:before {{ content:\"q\"; visibility:hidden; padding:0 4px;
                            border-left:2px solid #000000; }}
             #in2:after {{ content:\"w\"; padding:0 6px; background-color:#ffff00; }}"
        )
    };
    fade(&inline, &inline_css, "in2", "inline");
    // An inside marker stays outside its `:before`'s box and background.
    let item = || node("page", vec![node("li", vec![text("t", "item")])]);
    let item_css = |shown: &str| {
        format!(
            "#page {{ background-color:white; padding:20px; color:#000000; }}
             #li {{ display:list-item; list-style-position:inside; visibility:{shown}; }}
             #li:before {{ content:\"x\"; padding:0 4px; background-color:#ffff00; }}"
        )
    };
    fade(&item, &item_css, "li", "marker");
}

#[test]
fn inline_percentage_sides_take_their_share_of_the_block() {
    let draw = |css: &str, name: &str| {
        let root = node(
            "p",
            vec![
                text("a", "before "),
                node("s", vec![text("b", "inside")]),
                text("c", " after"),
            ],
        );
        let sheet = format!(
            "#p {{ width:300px; background-color:white; color:black; }}
             #s {{ display:inline; background-color:#d9534f; {css} }}"
        );
        let tree = BoxTree::new(root, &sheet).unwrap();
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(name))
            .unwrap()
    };
    // A tenth of the 300px block is 30px, for padding and for margin.
    let name = format!("inline-percent-padding-{}.png", std::process::id());
    assert!(draw("padding-left:30px;", &name));
    assert!(draw("padding-left:10%;", &name));
    assert!(!draw("padding-left:20%;", &name));
    let name = format!("inline-percent-margin-{}.png", std::process::id());
    assert!(draw("margin-right:30px;", &name));
    assert!(draw("margin-right:10%;", &name));
    assert!(!draw("margin-right:20%;", &name));
    // The opening margin and the closing padding too.
    let name = format!("inline-percent-sides-{}.png", std::process::id());
    assert!(draw("margin-left:30px; padding-right:15px;", &name));
    assert!(draw("margin-left:10%; padding-right:5%;", &name));
    // In rtl, with a border on its opening side, the paragraph keeps its direction.
    let rtl = |css: &str, name: &str| {
        let root = node(
            "p",
            vec![node("s", vec![text("b", "abc")]), text("c", " def")],
        );
        let sheet = format!(
            "#p {{ width:300px; direction:rtl; background-color:white; color:black; }}
             #s {{ display:inline; border-left:1px solid black; {css} }}"
        );
        let tree = BoxTree::new(root, &sheet).unwrap();
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(name))
            .unwrap()
    };
    let name = format!("inline-percent-rtl-{}.png", std::process::id());
    assert!(rtl("padding-left:30px;", &name));
    assert!(rtl("padding-left:10%;", &name));
    // Between inline-blocks it takes no share of a piece's own width.
    let row = |css: &str, name: &str| {
        let root = node(
            "p",
            vec![
                node("ib", vec![text("x", "x")]),
                text("a", " before "),
                node("s", vec![text("b", "inside")]),
                text("c", " after"),
            ],
        );
        let sheet = format!(
            "#p {{ width:300px; background-color:white; color:black; }}
             #ib {{ display:inline-block; }} #s {{ display:inline; {css} }}"
        );
        let tree = BoxTree::new(root, &sheet).unwrap();
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(name))
            .unwrap()
    };
    let name = format!("inline-percent-row-{}.png", std::process::id());
    assert!(row("", &name));
    assert!(row("padding-left:20%;", &name));
    // Room that wraps a line grows the block, moving what follows: a second layout.
    let wrapped = |css: &str, name: &str| {
        let root = node(
            "w",
            vec![
                node(
                    "p",
                    vec![
                        node("s", vec![text("b", "inside words here")]),
                        text("c", " after"),
                    ],
                ),
                node("q", vec![]),
            ],
        );
        let sheet = format!(
            "#w {{ background-color:white; color:black; }} #p {{ width:300px; }}
             #q {{ height:20px; background-color:#2a4d69; }}
             #s {{ display:inline; {css} }}"
        );
        let tree = BoxTree::new(root, &sheet).unwrap();
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(name))
            .unwrap()
    };
    let name = format!("inline-percent-wrap-{}.png", std::process::id());
    assert!(wrapped("padding-left:240px;", &name));
    assert!(wrapped("padding-left:80%;", &name));
    assert!(!wrapped("", &name));
}

#[test]
fn a_spanning_cells_edge_is_resolved_per_stretch() {
    let draw = |spanned: bool, name: &str| {
        let tall = node("a", vec![])
            .style_attribute("rowspan", "2", limits())
            .unwrap();
        let root = if spanned {
            node(
                "t",
                vec![
                    node("r1", vec![tall, node("b", vec![])]),
                    node("r2", vec![node("c", vec![])]),
                ],
            )
        } else {
            node(
                "t",
                vec![
                    node("r1", vec![node("a1", vec![]), node("b", vec![])]),
                    node("r2", vec![node("a2", vec![]), node("c", vec![])]),
                ],
            )
        };
        let sheet = "#t { display:table; border-collapse:collapse; background-color:white; }
             #r1, #r2 { display:table-row; }
             #a, #a1, #a2, #b, #c { display:table-cell; width:40px; height:30px; }
             #a, #a1, #a2 { border:1px solid #d9534f; }
             #a1 { border-bottom:none; } #a2 { border-top:none; }
             #b, #c { border:1px solid black; }
             #b { border-left:6px solid #2a4d69; border-bottom:none; }
             #c { border-left:1px solid #5cb85c; border-top:none; }";
        let tree = BoxTree::new(root, sheet).unwrap();
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(name))
            .unwrap()
    };
    // Blue beats the spanning cell's red beside the first row; red keeps the second, where
    // it meets a like border further left: as two cells stacked without a line between.
    // (No line between the rows on the right either: where lines cross is left open.)
    let name = format!("span-stretch-{}.png", std::process::id());
    assert!(draw(false, &name));
    assert!(draw(true, &name));
}

#[test]
fn a_spanning_loser_after_its_winners_and_a_hidden_edge_settle_per_stretch() {
    let draw = |spanned: bool, sheet: &str, name: &str| {
        let tall = node("a", vec![])
            .style_attribute("rowspan", "2", limits())
            .unwrap();
        let root = if spanned {
            node(
                "t",
                vec![
                    node("r1", vec![node("b", vec![]), tall]),
                    node("r2", vec![node("c", vec![])]),
                ],
            )
        } else {
            node(
                "t",
                vec![
                    node("r1", vec![node("b", vec![]), node("a1", vec![])]),
                    node("r2", vec![node("c", vec![]), node("a2", vec![])]),
                ],
            )
        };
        let sheet = format!(
            "#t {{ display:table; border-collapse:collapse; background-color:white; }}
             #r1, #r2 {{ display:table-row; }}
             #a, #a1, #a2, #b, #c {{ display:table-cell; width:40px; height:30px; }}
             {sheet}
             #a1 {{ border-bottom:none; }} #a2 {{ border-top:none; }}
             #b {{ border-bottom:none; }} #c {{ border-top:none; }}"
        );
        let tree = BoxTree::new(root, &sheet).unwrap();
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(name))
            .unwrap()
    };
    let id = std::process::id();
    // The spanning cell, painted after both, loses each stretch to its left neighbour.
    // Only the shared edge has borders: where lines cross is left open.
    let losing = "#a, #a1, #a2 { border-left:1px solid #d9534f; }
         #b { border-right:6px solid #2a4d69; } #c { border-right:1px solid #5cb85c; }";
    let name = format!("span-loser-{id}.png");
    assert!(draw(false, losing, &name));
    assert!(draw(true, losing, &name));
    // `hidden` beside it hides the neighbours' part too.
    let hidden = "#a, #a1, #a2 { border-left:hidden; } #b, #c { border-right:2px solid #2a4d69; }";
    let name = format!("span-hidden-{id}.png");
    assert!(draw(false, hidden, &name));
    assert!(draw(true, hidden, &name));
}

#[test]
fn a_column_spanning_cells_edge_is_resolved_per_stretch() {
    let draw = |spanned: bool, name: &str| {
        let wide = node("a", vec![])
            .style_attribute("colspan", "2", limits())
            .unwrap();
        let first = if spanned {
            node("r1", vec![wide])
        } else {
            node("r1", vec![node("a1", vec![]), node("a2", vec![])])
        };
        let root = node(
            "t",
            vec![
                first,
                node("r2", vec![node("b", vec![]), node("c", vec![])]),
            ],
        );
        let sheet = "#t { display:table; border-collapse:collapse; background-color:white; }
             #r1, #r2 { display:table-row; }
             #a, #a1, #a2, #b, #c { display:table-cell; height:30px; }
             #b, #c { width:40px; }
             #a, #a1, #a2 { border-bottom:1px solid #d9534f; }
             #b { border-top:6px solid #2a4d69; } #c { border-top:1px solid #5cb85c; }";
        let tree = BoxTree::new(root, sheet).unwrap();
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        ui.snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(name))
            .unwrap()
    };
    // Blue beats the wide cell's red over the first column; red keeps the second.
    let name = format!("span-columns-{}.png", std::process::id());
    assert!(draw(false, &name));
    assert!(draw(true, &name));
}

#[test]
fn a_negative_text_indent_hangs_the_first_line() {
    use iced::advanced::renderer::Headless;
    const WORDS: &str = "words that wrap onto more lines once a hanging first line pulls them out";
    let source = |words: &'static str| node("page", vec![node("p", vec![text("a", words)])]);
    let sheet = |css: &str| {
        format!(
            "#page {{ background-color:white; padding-left:60px; width:200px; color:black; }}
             #p {{ line-height:20px; {css} }}"
        )
    };
    // Each line's leftmost and rightmost dark pixels, at the snapshot's scale of 2, and
    // whether it draws as `reference` first did.
    type Lines = Vec<Option<(usize, usize)>>;
    let draw = |words: &'static str, css: &str, name: &str, reference: &str| -> (Lines, bool) {
        let tree = BoxTree::new(source(words), &sheet(css)).unwrap();
        let view: Element<'_, u8> = tree.view();
        let mut ui = simulator(view);
        let path = std::env::temp_dir().join(format!("hang-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        let shot = ui.snapshot(&Theme::Light).unwrap();
        let same = shot
            .matches_image(
                std::env::temp_dir().join(format!("hang-{}-{reference}.png", std::process::id())),
            )
            .unwrap();
        assert!(shot.matches_image(path.join("frame.png")).unwrap());
        let file = std::fs::read_dir(&path)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let decoder =
            png::Decoder::new(std::io::BufReader::new(std::fs::File::open(file).unwrap()));
        let mut reader = decoder.read_info().unwrap();
        let width = reader.info().width as usize;
        let mut bytes = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut bytes).unwrap();
        let mut lines: Lines = vec![None; 6];
        for (at, pixel) in bytes[..info.buffer_size()].chunks(4).enumerate() {
            let line = at / width / 40;
            if pixel[0] < 100 && pixel[1] < 100 && pixel[2] < 100 && line < lines.len() {
                let x = at % width;
                lines[line] =
                    Some(lines[line].map_or((x, x), |(near, far)| (near.min(x), far.max(x))));
            }
        }
        let _ = std::fs::remove_dir_all(&path);
        (lines, same)
    };
    let starts = |lines: &Lines| {
        lines
            .iter()
            .flatten()
            .map(|line| line.0)
            .collect::<Vec<_>>()
    };
    let ends = |lines: &Lines| {
        lines
            .iter()
            .flatten()
            .map(|line| line.1)
            .collect::<Vec<_>>()
    };
    // Without an indent every line starts at the content edge, 60px in.
    let (plain, _) = draw(WORDS, "", "plain", "plain");
    let plain = starts(&plain);
    assert!(plain.len() >= 3, "{plain:?}");
    assert!(plain.iter().all(|x| (118..=126).contains(x)), "{plain:?}");
    // At -40px the first line starts 40px left of it, the rest stay.
    let (hung, _) = draw(WORDS, "text-indent:-40px;", "pixels", "hung");
    let hung = starts(&hung);
    assert!((38..=46).contains(&hung[0]), "{hung:?}");
    assert!(
        hung[1..].iter().all(|x| (118..=126).contains(x)),
        "{hung:?}"
    );
    // A percentage is of the block's width: -20% of 200px is the same 40px.
    let (_, same) = draw(WORDS, "text-indent:-20%;", "percent", "hung");
    assert!(same);
    // A right-to-left block's lines end at its right edge, 260px in; its first line hangs
    // out past it to the right.
    let (flush, _) = draw(WORDS, "direction:rtl;", "rtl-plain", "rtl");
    let flush = ends(&flush);
    assert!(flush.iter().all(|x| (512..=520).contains(x)), "{flush:?}");
    let (right, same) = draw(WORDS, "direction:rtl; text-indent:-40px;", "rtl", "rtl");
    assert!(!same);
    let right = ends(&right);
    assert!((592..=600).contains(&right[0]), "{right:?}");
    assert!(
        right[1..].iter().all(|x| (512..=520).contains(x)),
        "{right:?}"
    );
    // So does right-to-left text, whose lines read from their right: the second starts
    // where the first ends.
    const HEBREW: &str = "שלום עולם זה טקסט ארוך שעובר לשורות רבות כשהשורה הראשונה תלויה";
    let (flush, _) = draw(HEBREW, "direction:rtl;", "hebrew-plain", "hebrew");
    let flush = ends(&flush);
    assert!(flush.len() >= 3, "{flush:?}");
    assert!(flush.iter().all(|x| (510..=520).contains(x)), "{flush:?}");
    let (right, _) = draw(
        HEBREW,
        "direction:rtl; text-indent:-40px;",
        "hebrew",
        "hebrew",
    );
    let right = ends(&right);
    assert!((590..=600).contains(&right[0]), "{right:?}");
    assert!(
        right[1..].iter().all(|x| (510..=520).contains(x)),
        "{right:?}"
    );
    // Its lines after the first keep the block's direction: a line of English words and a
    // last Hebrew one reads from the right, so the Hebrew word, red here, sits at its left.
    let mixed = BoxTree::new(
        node(
            "page",
            vec![node(
                "p",
                vec![
                    text("a", "שלום one two three four five six seven eight nine "),
                    node("r", vec![text("b", "עולם")]),
                ],
            )],
        ),
        &sheet("direction:rtl; text-indent:-40px; } #r { display:inline; color:#ff0000;"),
    )
    .unwrap();
    let view: Element<'_, u8> = mixed.view();
    let path = std::env::temp_dir().join(format!("hang-{}-mixed", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    assert!(
        simulator(view)
            .snapshot(&Theme::Light)
            .unwrap()
            .matches_image(path.join("frame.png"))
            .unwrap()
    );
    let file = std::fs::read_dir(&path)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(file).unwrap()));
    let mut reader = decoder.read_info().unwrap();
    let width = reader.info().width as usize;
    let mut bytes = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut bytes).unwrap();
    let pixels: Vec<(usize, usize, &[u8])> = bytes[..info.buffer_size()]
        .chunks(4)
        .enumerate()
        .map(|(at, pixel)| (at % width, at / width / 40, pixel))
        .collect();
    let red = |pixel: &[u8]| pixel[0] > 200 && pixel[1] < 80 && pixel[2] < 80;
    let dark = |pixel: &[u8]| pixel[0] < 100 && pixel[1] < 100 && pixel[2] < 100;
    let red_line = pixels.iter().find(|(_, _, pixel)| red(pixel)).unwrap().1;
    assert!(red_line > 0, "the red word wraps");
    let on_line = |keep: &dyn Fn(&[u8]) -> bool| {
        pixels
            .iter()
            .filter(|(_, line, pixel)| *line == red_line && keep(pixel))
            .map(|(x, _, _)| *x)
            .collect::<Vec<_>>()
    };
    let (reds, darks) = (on_line(&red), on_line(&dark));
    assert!(!darks.is_empty(), "English words share the red word's line");
    assert!(
        reds.iter().max() < darks.iter().min(),
        "{:?} {:?}",
        reds.iter().max(),
        darks.iter().min()
    );
    let _ = std::fs::remove_dir_all(&path);
    // Laid out outside the widget, as tall as the widget draws it.
    let renderer = iced_test::futures::futures::executor::block_on(
        iced_test::renderer::Renderer::new(iced::Font::default(), iced::Pixels(16.0), None),
    )
    .unwrap();
    let height = |css: &str| {
        let tree = BoxTree::new(source(WORDS), &sheet(css)).unwrap();
        let block = tree.layout_with(&renderer, 1024.0, None).unwrap()[1]
            .unwrap()
            .border;
        block.bottom - block.top
    };
    let lines = |height: f32| (height / 20.0).round() as usize;
    assert_eq!(lines(height("")), plain.len());
    assert_eq!(lines(height("text-indent:-40px;")), hung.len());
    // Text that takes two lines in its box fits on one when it hangs: a percentage's hang,
    // sized once the width is known, lays out again.
    let short = || node("page", vec![node("p", vec![text("a", "a few words here")])]);
    let float = BoxTree::new(short(), "#p { float:left; }").unwrap();
    let long = float.layout_with(&renderer, 1024.0, None).unwrap()[1]
        .unwrap()
        .content;
    let long = long.right - long.left;
    let fit = |css: &str| {
        let sheet = format!(
            "#page {{ width:{}px; }} #p {{ line-height:20px; {css} }}",
            (long * 0.8).floor()
        );
        let block = BoxTree::new(short(), &sheet)
            .unwrap()
            .layout_with(&renderer, 1024.0, None)
            .unwrap()[1]
            .unwrap()
            .border;
        block.bottom - block.top
    };
    assert_eq!(fit(""), 40.0);
    assert_eq!(fit("text-indent:-50%;"), 20.0);
    // Beside a shorter float its lines keep the band beside it, as without the float in a
    // box that narrow.
    let beside = |page: &str, float: bool| {
        let mut children = Vec::new();
        if float {
            children.push(node("f", vec![]));
        }
        children.push(node(
            "p",
            vec![text(
                "a",
                "words that wrap onto many more lines once a hanging first line pulls them \
                 out beside a short float that ends after the first of them",
            )],
        ));
        let sheet = format!(
            "#page {{ width:{page}; }} #f {{ float:left; width:60px; height:10px; }}
             #p {{ line-height:20px; text-indent:-20px; }}"
        );
        let laid = BoxTree::new(node("page", children), &sheet)
            .unwrap()
            .layout_with(&renderer, 1024.0, None)
            .unwrap();
        let block = laid[laid.len() - 2].unwrap().border;
        block.bottom - block.top
    };
    assert!(beside("200px", false) > 60.0);
    assert_eq!(beside("260px", true), beside("200px", false));
    // The widget draws it as tall: rows its red background covers.
    let drawn = |page: &str, float: bool, name: &str| {
        let mut children = Vec::new();
        if float {
            children.push(node("f", vec![]));
        }
        children.push(node(
            "p",
            vec![text(
                "a",
                "words that wrap onto many more lines once a hanging first line pulls them \
                 out beside a short float that ends after the first of them",
            )],
        ));
        let sheet = format!(
            "#page {{ width:{page}; background-color:white; }}
             #f {{ float:left; width:60px; height:10px; }}
             #p {{ line-height:20px; text-indent:-20px; background-color:#ff0000; }}"
        );
        let view: Element<'_, u8> = BoxTree::new(node("page", children), &sheet).unwrap().view();
        let path = std::env::temp_dir().join(format!("hang-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        assert!(
            simulator(view)
                .snapshot(&Theme::Light)
                .unwrap()
                .matches_image(path.join("frame.png"))
                .unwrap()
        );
        let file = std::fs::read_dir(&path)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let decoder =
            png::Decoder::new(std::io::BufReader::new(std::fs::File::open(file).unwrap()));
        let mut reader = decoder.read_info().unwrap();
        let width = reader.info().width as usize;
        let mut bytes = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut bytes).unwrap();
        let mut rows = std::collections::BTreeSet::new();
        for (at, pixel) in bytes[..info.buffer_size()].chunks(4).enumerate() {
            if pixel[0] > 200 && pixel[1] < 60 && pixel[2] < 60 {
                rows.insert(at / width);
            }
        }
        let _ = std::fs::remove_dir_all(&path);
        rows.len()
    };
    assert_eq!(
        drawn("260px", true, "float"),
        drawn("200px", false, "narrow")
    );
    // Text with a forced break does not hang.
    let broken = |css: &str, name: &str| {
        let root = node("p", vec![text("a", "one line\nand another")]);
        let sheet = format!(
            "#p {{ background-color:white; padding-left:60px; width:200px; color:black;
                   white-space:pre; {css} }}"
        );
        let view: Element<'_, u8> = BoxTree::new(root, &sheet).unwrap().view();
        simulator(view)
            .snapshot(&Theme::Light)
            .unwrap()
            .matches_image(std::env::temp_dir().join(name))
            .unwrap()
    };
    let name = format!("hang-{}-broken.png", std::process::id());
    assert!(broken("", &name));
    assert!(broken("text-indent:-40px;", &name));
    // An inline-block whose first line hangs lines up on its last line's baseline: the
    // text beside it sits by its bottom line.
    let row = BoxTree::new(
        node(
            "p",
            vec![
                text("a", "x "),
                node(
                    "b",
                    vec![text("t", "words that wrap onto more lines in a narrow box")],
                ),
            ],
        ),
        "#p { width:400px; line-height:20px; }
         #b { display:inline-block; width:120px; text-indent:-20px; }",
    )
    .unwrap();
    let laid = row.layout_with(&renderer, 1024.0, None).unwrap();
    let (beside, boxed) = (laid[1].unwrap().border, laid[2].unwrap().border);
    assert!(boxed.bottom - boxed.top > 40.0, "{boxed:?}");
    assert!(
        (beside.bottom - boxed.bottom).abs() < 6.0,
        "{beside:?} {boxed:?}"
    );
    for reference in ["plain", "hung", "rtl", "broken", "hebrew"] {
        let file = format!("hang-{}-{reference}.png", std::process::id());
        let _ = std::fs::remove_file(std::env::temp_dir().join(file));
    }
}
