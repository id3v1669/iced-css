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
