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
fn selectors_level3_style_boxes_by_place_negation_and_siblings() {
    let root = || {
        node(
            "page",
            vec![
                node("a", vec![]),
                text("b", "text"),
                node("c", vec![]),
                node("d", vec![]),
            ],
        )
    };
    let colors = |css: &str| -> Vec<Option<[f32; 3]>> {
        let tree = BoxTree::new(root(), css).unwrap();
        (1..=4).map(|at| tree.paints()[at].background).collect()
    };
    let red = Some([1.0, 0.0, 0.0]);
    assert_eq!(
        colors("#page > :nth-child(odd) { background:#ff0000; }"),
        [red, None, red, None]
    );
    assert_eq!(
        colors("#page > :nth-last-child(-n+2) { background:#ff0000; }"),
        [None, None, red, red]
    );
    assert_eq!(
        colors("#page > :last-child, #page > :first-child { background:#ff0000; }"),
        [red, None, None, red]
    );
    assert_eq!(
        colors("#page > :not(#b):not(:last-child) { background:#ff0000; }"),
        [red, None, red, None]
    );
    assert_eq!(
        colors("#b ~ * { background:#ff0000; }"),
        [None, None, red, red]
    );
    // A box with text of its own is not empty; the page with children is neither.
    assert_eq!(
        colors(":empty { background:#ff0000; }"),
        [red, None, red, red]
    );
    assert_eq!(
        colors("[id^=c], [id$=d], [id*=a] { background:#ff0000; }"),
        [red, None, red, red]
    );
    // `:root` is the page; its children are untouched.
    let tree = BoxTree::new(root(), ":root { background:#ff0000; }").unwrap();
    assert_eq!(tree.paints()[0].background, red);
    assert_eq!(tree.paints()[1].background, None);
    // A negation counts as its argument: one id beats the class-level `:nth-child`.
    assert_eq!(
        colors(
            "#page > :not(#zz) { background:#ff0000; } #page > :nth-child(1) { background:#00ff00; }"
        )[0],
        red
    );
}

#[test]
fn box_sizing_border_box_sizes_the_border_box() {
    let root = || node("page", vec![node("k", vec![node("inner", vec![])])]);
    let size = |css: &str| {
        let tree = BoxTree::new(root(), css).unwrap();
        let geometry = tree.layout(800.0, Some(600.0)).unwrap();
        let border = geometry[1].as_ref().unwrap().border;
        let inner = geometry[2].as_ref().unwrap().border;
        (
            border.right - border.left,
            border.bottom - border.top,
            inner.right - inner.left,
        )
    };
    let frame = "#k { width:100px; height:50px; padding:10px; border:2px solid #000000; }";
    assert_eq!(size(frame), (124.0, 74.0, 100.0));
    assert_eq!(
        size(&format!("{frame} #k {{ box-sizing:border-box; }}")),
        (100.0, 50.0, 76.0)
    );
    // The initial value again, and any other value drops the declaration.
    assert_eq!(
        size(&format!(
            "{frame} #k {{ box-sizing:border-box; box-sizing:content-box; }}"
        )),
        (124.0, 74.0, 100.0)
    );
    assert_eq!(
        size(&format!(
            "{frame} #k {{ box-sizing:border-box; box-sizing:padding-box; }}"
        )),
        (100.0, 50.0, 76.0)
    );
    // Not inherited, unless asked.
    let nested = "#page { box-sizing:border-box; } #k { width:100px; padding:10px; }";
    assert_eq!(size(nested).0, 120.0);
    assert_eq!(
        size(&format!("{nested} #k {{ box-sizing:inherit; }}")).0,
        100.0
    );
    // A percentage width, and a maximum, are border-box sizes too.
    assert_eq!(
        size("#k { box-sizing:border-box; width:50%; padding:10px; }").0,
        400.0
    );
    assert_eq!(
        size("#k { box-sizing:border-box; width:500px; max-width:200px; padding:10px; }").0,
        200.0
    );
    // An absolutely positioned box.
    assert_eq!(
        size(
            "#k { position:absolute; box-sizing:border-box; width:100px; height:50px; padding:10px; }"
        ),
        (100.0, 50.0, 80.0)
    );
}

#[test]
fn rem_measures_the_root_font_size_and_ch_half_the_font_size() {
    let root = || node("page", vec![node("k", vec![node("inner", vec![])])]);
    let size = |css: &str| {
        let tree = BoxTree::new(root(), css).unwrap();
        let geometry = tree.layout(800.0, Some(600.0)).unwrap();
        let border = geometry[1].as_ref().unwrap().border;
        let inner = geometry[2].as_ref().unwrap().border;
        (
            border.right - border.left,
            border.bottom - border.top,
            inner.right - inner.left,
        )
    };
    // The initial font size is 16px.
    assert_eq!(
        size("#k { width:10rem; height:2rem; }"),
        (160.0, 32.0, 160.0)
    );
    // The root's font size, whatever the element's own.
    let sized = "#page { font-size:20px; } #k { font-size:10px; width:10rem; height:2em; }";
    assert_eq!(size(sized), (200.0, 20.0, 200.0));
    // On the root's own font-size, `rem` is the initial size; elsewhere the root's.
    assert_eq!(
        size("#page { font-size:2rem; } #k { width:5rem; }").0,
        160.0
    );
    assert_eq!(
        size("#page { font-size:20px; } #k { font-size:2rem; width:1em; }").0,
        40.0
    );
    // Padding, borders, margins and line boxes take it too.
    assert_eq!(
        size("#page { font-size:10px; } #k { width:100px; padding:0 1rem; border:1rem solid #000000; }").0,
        140.0
    );
    assert_eq!(
        size("#page { font-size:10px; } #k { width:100px; } #inner { margin-left:2rem; }").2,
        80.0
    );
    // `ch` is half the element's font size.
    assert_eq!(
        size("#k { font-size:20px; width:10ch; height:1CH; }"),
        (100.0, 10.0, 100.0)
    );
    // A negative `rem` is as invalid as any negative width.
    assert_eq!(size("#k { width:50px; width:-1rem; }").0, 50.0);
}

#[test]
fn css_color_3_values_paint_boxes() {
    let root = || node("page", vec![node("k", vec![])]);
    let paint = |css: &str| {
        let tree = BoxTree::new(root(), css).unwrap();
        tree.paints()[1]
    };
    let red = Some([1.0, 0.0, 0.0]);
    // An opacity below 1 is the background's fade; an opaque one has none.
    let half = paint("#k { background-color:rgba(255, 0, 0, 0.5); }");
    assert_eq!(half.background, red);
    assert!((half.background_fade.unwrap() - 0.5).abs() < 0.01);
    assert_eq!(
        paint("#k { background:#ff000080; }").background_fade,
        half.background_fade
    );
    let opaque = paint("#k { background-color:rgba(255, 0, 0, 1); }");
    assert_eq!((opaque.background, opaque.background_fade), (red, None));
    assert_eq!(
        paint("#k { background-color:hsl(0, 100%, 50%); }").background,
        red
    );
    // Borders, side by side.
    let sides =
        paint("#k { border:2px solid #ff0000; border-left-color:hsla(0, 100%, 50%, 0.25); }");
    assert_eq!(sides.border, [red; 4]);
    assert_eq!(sides.border_fade[0], None);
    assert!((sides.border_fade[3].unwrap() - 0.25).abs() < 0.01);
    // `currentColor` is the element's own color, and inherits as `color` does.
    let current = paint("#page { color:#ff0000; } #k { background-color:currentColor; }");
    assert_eq!(current.background, red);
    assert_eq!(
        paint("#page { color:#ff0000; } #k { color:#00ff00; color:currentColor; }").color,
        red
    );
    // `transparent` is a color everywhere: text with none draws nothing.
    assert_eq!(paint("#k { color:transparent; }").color, None);
    assert_eq!(paint("#k { color:rgba(0, 0, 0, 0); }").color, None);
    // Text has no opacity of its own: a see-through color draws opaque.
    assert_eq!(paint("#k { color:rgba(255, 0, 0, 0.5); }").color, red);
}

#[test]
fn calc_mixes_lengths_and_percentages() {
    let root = || node("page", vec![node("k", vec![node("inner", vec![])])]);
    let size = |css: &str| {
        let tree = BoxTree::new(root(), css).unwrap();
        let geometry = tree.layout(800.0, Some(600.0)).unwrap();
        let border = geometry[1].as_ref().unwrap().border;
        let inner = geometry[2].as_ref().unwrap().border;
        (
            border.right - border.left,
            border.bottom - border.top,
            inner.left - border.left,
        )
    };
    assert_eq!(
        size("#k { width:calc(100% - 200px); height:calc(10px * 3); }"),
        (600.0, 30.0, 0.0)
    );
    assert_eq!(
        size("#k { width:calc(50% + 2em); font-size:10px; }").0,
        420.0
    );
    assert_eq!(
        size("#page { font-size:20px; } #k { width:calc(1rem + 1px); }").0,
        21.0
    );
    assert_eq!(size("#k { width:calc((100% - 100px) / 2); }").0, 350.0);
    // Margins, padding and max sizes; in shorthands too.
    assert_eq!(
        size("#k { width:100px; } #inner { margin-left:calc(10% + 5px); }").2,
        15.0
    );
    assert_eq!(
        size("#k { width:100px; padding:0 calc(5px * 2); }").0,
        120.0
    );
    assert_eq!(
        size("#k { width:500px; max-width:calc(50% - 100px); }").0,
        300.0
    );
    assert_eq!(
        size("#k { width:100px; } #inner { margin:0 0 0 calc(2px + 3px); }").2,
        5.0
    );
    // A negative result clips to nothing where a negative is not allowed, and stays one
    // where it is.
    assert_eq!(size("#k { width:calc(10% - 200px); }").0, 0.0);
    assert_eq!(size("#k { width:calc(10px - 200px); }").0, 0.0);
    assert_eq!(
        size("#k { width:100px; } #inner { margin-left:calc(10px - 30px); }").2,
        -20.0
    );
    // Invalid sums drop the declaration.
    for bad in [
        "calc(100% - 20)",
        "calc(100%-20px)",
        "calc(1px * 1px)",
        "calc()",
    ] {
        assert_eq!(
            size(&format!("#k {{ width:50px; width:{bad}; }}")).0,
            50.0,
            "{bad}"
        );
    }
    // Border widths take a sum of lengths; a percentage there is invalid.
    assert_eq!(
        size("#k { width:100px; border:1px solid #000000; border-left-width:calc(1px + 4px); }").0,
        106.0
    );
    assert_eq!(
        size("#k { width:100px; border:1px solid #000000; border-left-width:calc(1px + 4%); }").0,
        102.0
    );
}

#[test]
fn custom_properties_cascade_inherit_and_substitute() {
    let root = || node("page", vec![node("k", vec![node("inner", vec![])])]);
    let read = |css: &str| {
        let tree = BoxTree::new(root(), css).unwrap();
        let geometry = tree.layout(800.0, Some(600.0)).unwrap();
        let width = |at: usize| {
            let border = geometry[at].as_ref().unwrap().border;
            border.right - border.left
        };
        (
            width(1),
            width(2),
            tree.paints()[1].background,
            tree.paints()[2].background,
        )
    };
    let red = Some([1.0, 0.0, 0.0]);
    let lime = Some([0.0, 1.0, 0.0]);
    // Declared on an ancestor, used below it.
    let (k, inner, ..) =
        read("#page { --w:120px; } #k { width:var(--w); } #inner { width:var(--w); }");
    assert_eq!((k, inner), (120.0, 120.0));
    // The nearer declaration wins for the element and what is under it.
    let (k, inner, ..) =
        read("#page { --w:120px; } #k { --w:200px; width:var(--w); } #inner { width:var(--w); }");
    assert_eq!((k, inner), (200.0, 200.0));
    // A fallback where the property is not set; none, and the declaration is as unset.
    assert_eq!(read("#k { width:var(--none, 77px); }").0, 77.0);
    assert_eq!(read("#k { width:50px; width:var(--none); }").0, 800.0);
    // Specificity and `!important` decide between declarations of one name.
    assert_eq!(
        read("#k { --w:10px; } * { --w:20px; } #k { width:var(--w); }").0,
        10.0
    );
    assert_eq!(
        read("* { --w:20px !important; } #k { --w:10px; width:var(--w); }").0,
        20.0
    );
    // In a shorthand, in `calc()`, and from one custom property into another.
    let (.., background, _) = read("#page { --c:#ff0000; } #k { background:var(--c); }");
    assert_eq!(background, red);
    assert_eq!(
        read("#page { --gap:10px; } #k { width:calc(100px + var(--gap) * 2); }").0,
        120.0
    );
    assert_eq!(
        read("#page { --a:30px; --b:var(--a); } #k { width:var(--b); }").0,
        30.0
    );
    let (k, ..) = read("#page { --pad:0 15px; } #k { width:100px; padding:var(--pad); }");
    assert_eq!(k, 130.0);
    // A value that does not parse once substituted is as unset: inherited or initial,
    // not the earlier declaration.
    let (.., background, _) =
        read("#page { --c:12px; } #k { background-color:#00ff00; background-color:var(--c); }");
    assert_eq!(background, None);
    assert_eq!(
        read("#page { --w:red; } #k { width:50px; width:var(--w); }").0,
        800.0
    );
    // A cycle makes its properties invalid: a reference to one takes its own fallback.
    assert_eq!(
        read("#page { --a:var(--b); --b:var(--a); } #k { width:var(--a, 5px); }").0,
        5.0
    );
    // An inherited property that takes a variable still inherits its computed value.
    let css = "#page { --c:#00ff00; } #k { color:var(--c); }";
    let tree = BoxTree::new(root(), css).unwrap();
    assert_eq!(tree.paints()[2].color, lime);
    // Names are case-sensitive.
    assert_eq!(
        read("#page { --W:10px; } #k { width:var(--w, 20px); }").0,
        20.0
    );
}

#[test]
fn media_queries_follow_the_environment_given() {
    use iced_css::box_tree::MediaEnvironment;
    let root = node("page", vec![node("k", vec![])]);
    let css = "#k { width:100px; }
        @media (min-width: 600px) { #k { width:200px; } }
        @media screen and (min-width: 900px) and (orientation: landscape) { #k { width:300px; } }
        @media (prefers-color-scheme: dark) { #k { background-color:#ff0000; } }
        @media print { #k { width:1px; } }";
    let tree = BoxTree::new(root, css).unwrap();
    let width = |tree: &BoxTree| {
        let geometry = tree.layout(2000.0, Some(600.0)).unwrap();
        let border = geometry[1].as_ref().unwrap().border;
        border.right - border.left
    };
    let at = |width: f32, height: f32, dark: bool| MediaEnvironment {
        viewport: Some((width, height)),
        dark: Some(dark),
    };
    // No viewport: no size feature matches.
    assert_eq!(tree.media(), MediaEnvironment::default());
    assert_eq!(width(&tree), 100.0);
    assert_eq!(
        width(&tree.with_media(at(500.0, 400.0, false)).unwrap()),
        100.0
    );
    assert_eq!(
        width(&tree.with_media(at(700.0, 400.0, false)).unwrap()),
        200.0
    );
    let wide = tree.with_media(at(1000.0, 400.0, false)).unwrap();
    assert_eq!(width(&wide), 300.0);
    assert_eq!(wide.paints()[1].background, None);
    // Taller than wide: the landscape rule drops out.
    assert_eq!(
        width(&tree.with_media(at(1000.0, 1200.0, false)).unwrap()),
        200.0
    );
    let dark = tree.with_media(at(1000.0, 400.0, true)).unwrap();
    assert_eq!(dark.paints()[1].background, Some([1.0, 0.0, 0.0]));
    // A restyle keeps the environment.
    let restyled = dark
        .restyled(&["@media (min-width: 900px) { #k { width:50px; } }"])
        .unwrap();
    assert_eq!(restyled.media(), at(1000.0, 400.0, true));
    assert_eq!(width(&restyled), 50.0);
}

#[test]
fn supports_groups_apply_when_their_condition_holds() {
    let root = || node("page", vec![node("k", vec![])]);
    let width = |condition: &str| {
        let css =
            format!("#k {{ width:100px; }} @supports {condition} {{ #k {{ width:200px; }} }}");
        let tree = BoxTree::new(root(), &css).unwrap();
        let geometry = tree.layout(800.0, Some(600.0)).unwrap();
        let border = geometry[1].as_ref().unwrap().border;
        border.right - border.left
    };
    for (condition, held) in [
        ("(width: 10px)", true),
        ("(box-sizing: border-box)", true),
        ("(width: calc(1px + 2%))", true),
        ("(--anything: goes here)", true),
        ("(width: var(--x))", true),
        ("(width: red)", false),
        ("(no-such-property: 1)", false),
        ("(display: grid)", false),
        ("not (display: grid)", true),
        ("not (width: 10px)", false),
        ("(width: 10px) and (height: 10px)", true),
        ("(width: 10px) and (width: red)", false),
        ("(width: red) or (height: 10px)", true),
        ("(width: red) or (height: red)", false),
        ("((width: 10px) and (height: 1px)) or (width: red)", true),
        ("(not (width: red)) and (height: 1px)", true),
        ("selector(a > b)", false),
        // Malformed conditions drop the whole group.
        ("(width: 10px) and (height: 10px) or (color: red)", false),
        ("width: 10px", false),
        ("(width: 10px) (height: 1px)", false),
        ("", false),
    ] {
        assert_eq!(
            width(condition),
            if held { 200.0 } else { 100.0 },
            "{condition}"
        );
    }
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
    // The inline box's sides take spaces across: margin 7px, about a third, and padding 9px, up to a half.
    let text = "\u{2004}\u{2002}one two three four\u{2002}\u{2004}";
    assert_eq!(seen, vec![(text.to_owned(), 20.0, 80.0)]);
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
fn a_negative_inline_margin_pulls_its_box_over_what_is_beside_it() {
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
    let runs = |css: &str| {
        let tree = BoxTree::new(source(), css).unwrap();
        tree.text_leaves()[0].1.runs.clone()
    };
    let side = |runs: &[iced_css::box_tree::TextRun], across: usize| {
        runs.iter()
            .find(|run| {
                run.side_of.is_some_and(|side| side.1 == across)
                    && run.percent_room.is_some_and(|room| room.2)
            })
            .map(|run| (run.percent_room.unwrap().0, run.overhang, run.pull))
    };
    // Within its border and padding: their spaces narrow by it, its box draws past them.
    let pulled = runs("#s { display:inline; padding:0 12px; margin:0 -8px; }");
    assert_eq!(side(&pulled, 3), Some((4.0, 8.0, 0.0)));
    assert_eq!(side(&pulled, 1), Some((4.0, 8.0, 0.0)));
    // The words beside it keep their spaces.
    assert!(pulled.iter().any(|run| run.text == "before "));
    assert!(pulled.iter().any(|run| run.text == " after"));
    // Past them: the space beside narrows, here all of it.
    let past = runs("#s { display:inline; padding:0 4px; margin:0 -12px; }");
    assert_eq!(side(&past, 3), Some((0.0, 4.0, 8.0)));
    // With none of its border and padding's room left, a zero-width space to draw from.
    assert!(
        past.iter()
            .any(|run| run.side_of.is_some() && run.text == "\u{200B}")
    );
    assert!(past.iter().any(|run| run.text == "before"));
    assert!(past.iter().any(|run| run.text == "after"));
    // A little past them: narrower, not gone: a sixth of the 16 pixel font is left.
    let little = runs("#s { display:inline; margin:0 -1.2px; }");
    assert!(
        little.iter().any(|run| run.text == "before\u{2006}"),
        "{:?}",
        little.iter().map(|run| &run.text).collect::<Vec<_>>()
    );
    // The space nearest it narrows, not an earlier one.
    let nested = || {
        node(
            "page",
            vec![node(
                "p",
                vec![
                    text("a", "one "),
                    node("x", vec![text("c", "two ")]),
                    node("s", vec![text("t", "in")]),
                ],
            )],
        )
    };
    let tree = BoxTree::new(
        nested(),
        "#x, #s { display:inline; } #s { margin-left:-8px; }",
    )
    .unwrap();
    let texts: Vec<String> = tree.text_leaves()[0]
        .1
        .runs
        .iter()
        .map(|run| run.text.clone())
        .collect();
    assert!(
        texts.contains(&"one ".to_owned()) && texts.contains(&"two".to_owned()),
        "{texts:?}"
    );
    // Animating to or from a negative margin, or its border and padding beside one: its
    // box's start, (room, overhang), as the presented sides pull it.
    let start = std::time::Instant::now();
    for (one, two, property, quarter, half) in [
        // Back out from -8 over 12 of padding: -6, then -4.
        (
            "padding-left:12px; margin-left:-8px;",
            "padding-left:12px; margin-left:0;",
            "margin-left",
            (6.0, 6.0),
            (8.0, 4.0),
        ),
        // Across 0 over 4 of padding: -4, all of the padding, then 0.
        (
            "padding-left:4px; margin-left:-8px;",
            "padding-left:4px; margin-left:8px;",
            "margin-left",
            (0.0, 4.0),
            (4.0, 0.0),
        ),
        // Its padding shrinking beside -8: 10 then 8, of which 8 pulled.
        (
            "padding-left:12px; margin-left:-8px;",
            "padding-left:4px; margin-left:-8px;",
            "padding-left",
            (2.0, 8.0),
            (0.0, 8.0),
        ),
        // Its border widening beside -8, over 4 of padding: 7 then 8.
        (
            "padding-left:4px; border-left:2px solid red; margin-left:-8px;",
            "padding-left:4px; border-left:6px solid red; margin-left:-8px;",
            "border-left-width",
            (0.0, 7.0),
            (0.0, 8.0),
        ),
    ] {
        let css = |side: &str| format!("#s {{ display:inline; {side} }}");
        let from = BoxTree::new(source(), &css(one)).unwrap();
        let to = from.restyled(&[&css(two)]).unwrap();
        let mut animations = iced_css::box_tree::BoxTreeAnimations::new();
        let timing = iced_css::animation::clock::AnimationTiming::new(
            std::time::Duration::from_secs(1),
            std::time::Duration::ZERO,
        );
        assert_eq!(
            animations
                .request(&from, &to, "s", &[property], timing, start)
                .unwrap(),
            1,
            "{one} {two}"
        );
        for (at, expected) in [(250, quarter), (500, half)] {
            animations.advance(start + std::time::Duration::from_millis(at), &to);
            let shown = animations.presented(&to);
            let leaves = shown.text_leaves();
            let found = side(&leaves[0].1.runs, 3).map(|(room, overhang, _)| (room, overhang));
            assert_eq!(found, Some(expected), "{one} {two} at {at}");
        }
    }
    // No margin spaces made while the margin pulls; beside a percentage padding, the pull
    // goes to layout, halfway at half.
    let css =
        |margin: &str| format!("#s {{ display:inline; padding-left:10%; margin-left:{margin}; }}");
    let from = BoxTree::new(source(), &css("-8px")).unwrap();
    let to = from.restyled(&[&css("0")]).unwrap();
    let mut animations = iced_css::box_tree::BoxTreeAnimations::new();
    let timing = iced_css::animation::clock::AnimationTiming::new(
        std::time::Duration::from_secs(1),
        std::time::Duration::ZERO,
    );
    animations
        .request(&from, &to, "s", &["margin-left"], timing, start)
        .unwrap();
    animations.advance(start + std::time::Duration::from_millis(500), &to);
    let shown = animations.presented(&to);
    let starts: Vec<(bool, f32)> = shown.text_leaves()[0]
        .1
        .runs
        .iter()
        .filter(|run| run.side_of.is_some_and(|side| side.1 == 3))
        .map(|run| (run.percent_room.unwrap().2, run.pulled))
        .collect();
    assert_eq!(starts, vec![(true, 4.0)]);
    // Sized again as its font size animates: still a place to draw from.
    let css = |size: &str| {
        format!("#s {{ display:inline; font-size:{size}; padding:0 4px; margin:0 -12px; }}")
    };
    let from = BoxTree::new(source(), &css("16px")).unwrap();
    let to = from.restyled(&[&css("32px")]).unwrap();
    let mut animations = iced_css::box_tree::BoxTreeAnimations::new();
    let timing = iced_css::animation::clock::AnimationTiming::new(
        std::time::Duration::from_secs(1),
        std::time::Duration::ZERO,
    );
    animations
        .request(&from, &to, "s", &["font-size"], timing, start)
        .unwrap();
    animations.advance(start + std::time::Duration::from_millis(500), &to);
    let shown = animations.presented(&to);
    let sides: Vec<String> = shown.text_leaves()[0]
        .1
        .runs
        .iter()
        .filter(|run| run.side_of.is_some())
        .map(|run| run.text.clone())
        .collect();
    assert_eq!(sides, vec!["\u{200B}".to_owned(); 2]);
    // Beside a percentage padding, taken out of its room at layout: the words keep their
    // space.
    let percent = runs("#s { display:inline; padding:0 10%; margin:0 -8px; }");
    let spacers: Vec<(f32, f32)> = percent
        .iter()
        .filter(|run| run.side_of.is_some())
        .map(|run| (run.pulled, run.pull))
        .collect();
    assert_eq!(spacers, vec![(8.0, 0.0); 2]);
    assert!(percent.iter().any(|run| run.text == "before "));
    // Until layout, and where no width applies, out of the fixed part: here its border.
    let bordered = runs(
        "#s { display:inline; border-left:4px solid red; padding-left:10%; margin-left:-8px; }",
    );
    assert_eq!(side(&bordered, 3), Some((4.0, 4.0, 0.0)));
    // A box around it whose padding animates takes nothing of its pull.
    let wrapped = || {
        node(
            "page",
            vec![node(
                "p",
                vec![
                    text("c", "before "),
                    node(
                        "a",
                        vec![node("s", vec![text("t", "in")]), text("d", " after")],
                    ),
                ],
            )],
        )
    };
    let css = |padding: &str| {
        format!(
            "#a {{ display:inline; padding-left:{padding}; }}
             #s {{ display:inline; padding-left:10%; margin-left:-8px; }}"
        )
    };
    // Animating out: its spaces made anew from the inner box's, the first in it.
    let from = BoxTree::new(wrapped(), &css("10px")).unwrap();
    let to = from.restyled(&[&css("0")]).unwrap();
    let mut animations = iced_css::box_tree::BoxTreeAnimations::new();
    let timing = iced_css::animation::clock::AnimationTiming::new(
        std::time::Duration::from_secs(1),
        std::time::Duration::ZERO,
    );
    let begin = std::time::Instant::now();
    animations
        .request(&from, &to, "a", &["padding-left"], timing, begin)
        .unwrap();
    animations.advance(begin + std::time::Duration::from_millis(500), &to);
    let shown = animations.presented(&to);
    // Its own start spaces carry the pull; the animated box's carry none.
    let mut pulls: Vec<f32> = shown.text_leaves()[0]
        .1
        .runs
        .iter()
        .filter(|run| run.side_of.is_some_and(|side| side.1 == 3))
        .map(|run| run.pulled)
        .collect();
    pulls.sort_by(f32::total_cmp);
    assert_eq!(pulls, vec![0.0, 8.0]);
    // A positive margin is spaces of its own, as before.
    let positive = runs("#s { display:inline; margin:0 8px; }");
    assert!(
        positive
            .iter()
            .all(|run| run.overhang == 0.0 && run.pull == 0.0)
    );
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
    // CSS Color 3: `color` accepts transparent, and the text draws nothing.
    let root = node("page", vec![text("copy", "words")]);
    let tree = BoxTree::new(root, "#copy { color:transparent; }").unwrap();
    assert_eq!(tree.paints()[1].color, None);
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
    // The text after the `pre` run may break, so the leaf wraps.
    assert!(leaves[0].1.wraps);
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
fn a_carried_decoration_keeps_the_color_and_size_of_its_box() {
    let root = node(
        "p",
        vec![text("a", "red "), node("child", vec![text("b", "blue")])],
    );
    let tree = BoxTree::new(
        root,
        "#p { text-decoration:underline; color:#ff0000; font-size:10px; }
         #child { display:inline; color:#0000ff; font-size:30px; }",
    )
    .unwrap();
    let leaves = tree.text_leaves();
    let runs = &leaves[0].1.runs;
    let run = |wanted: &str| runs.iter().find(|run| run.text.trim() == wanted).unwrap();
    // The box that sets the line draws it in its own color and size, under both runs.
    let line = |wanted: &str| {
        let run = run(wanted);
        assert!(run.underline);
        run.underline_from
            .map_or((run.color.unwrap(), run.size), |(_, color, size)| {
                (color, size)
            })
    };
    assert_eq!(line("red"), ([1.0, 0.0, 0.0], 10.0));
    assert_eq!(line("blue"), ([1.0, 0.0, 0.0], 10.0));
    assert_eq!(run("blue").color, Some([0.0, 0.0, 1.0]));
    assert_eq!(run("blue").line_through_from, None);
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
            self.send_event(Event::Mouse(event), at)
        }
        fn send_event(&mut self, event: Event, at: Point) -> Vec<u8> {
            let mut messages = Vec::new();
            let mut shell = Shell::new(&mut messages);
            self.element.as_widget_mut().update(
                &mut self.tree,
                &event,
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
    fn a_second_widget_for_the_same_id_replaces_the_first() {
        let root = node("page", vec![node("slot", vec![])]);
        let tree = BoxTree::new(root, "").unwrap();
        let view: View<'_> = tree
            .interactive()
            .widget("slot", space(25.0))
            .widget("slot", space(7.0))
            .into();
        let rig = Rig::new(view);
        assert_eq!(rig.node.children().len(), 1);
        assert_eq!(rig.slot().height, 7.0);
    }

    #[test]
    fn a_click_into_a_hosted_widget_gives_its_box_focus() {
        let root = node("page", vec![node("top", vec![]), node("field", vec![])]);
        let tree = BoxTree::new(
            root,
            "#top { height:10px; } #field:focus { margin-top:10px; }",
        )
        .unwrap();
        let view: View<'_> = tree
            .interactive()
            .widget("field", iced::widget::text_input("", ""))
            .into();
        let mut rig = Rig::new(view);
        let slot = rig.slot();
        assert_eq!(slot.y, 10.0);
        assert!(slot.height > 2.0);
        let press = mouse::Event::ButtonPressed(mouse::Button::Left);
        let release = mouse::Event::ButtonReleased(mouse::Button::Left);
        let inside = Point::new(5.0, slot.y + 1.0);
        rig.send(press, inside);
        rig.send(release, inside);
        assert_eq!(rig.slot().y, 20.0, "the focused input's box has :focus");
        // A click on nothing unfocuses the input, and its box with it.
        let off = Point::new(5.0, 190.0);
        rig.send(press, off);
        rig.send(release, off);
        assert_eq!(rig.slot().y, 10.0);
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
    fn a_custom_property_set_on_hover_restyles_what_uses_it() {
        let root = node("page", vec![node("top", vec![]), node("slot", vec![])]);
        let tree = BoxTree::new(
            root,
            "#top { --h:10px; height:var(--h); } #top:hover { --h:30px; }",
        )
        .unwrap();
        let view: View<'_> = tree.interactive().widget("slot", space(1.0)).into();
        let mut rig = Rig::new(view);
        assert_eq!(rig.slot().y, 10.0);
        let on = Point::new(5.0, 5.0);
        rig.send(mouse::Event::CursorMoved { position: on }, on);
        assert_eq!(rig.slot().y, 30.0);
        let off = Point::new(5.0, 150.0);
        rig.send(mouse::Event::CursorMoved { position: off }, off);
        assert_eq!(rig.slot().y, 10.0);
    }

    #[test]
    fn a_finger_presses_and_lifts_as_the_left_button_does() {
        use iced::touch;
        let root = node("page", vec![node("top", vec![]), node("slot", vec![])]);
        let tree =
            BoxTree::new(root, "#top { height:10px; } #top:active { height:50px; }").unwrap();
        let view: View<'_> = tree
            .interactive()
            .widget("slot", space(1.0))
            .on_press("top", 7)
            .into();
        let mut rig = Rig::new(view);
        let on = Point::new(5.0, 5.0);
        let id = touch::Finger(0);
        let pressed = Event::Touch(touch::Event::FingerPressed { id, position: on });
        assert_eq!(rig.send_event(pressed.clone(), on), vec![7]);
        assert!(rig.captured);
        assert_eq!(rig.slot().y, 50.0);
        let lifted = Event::Touch(touch::Event::FingerLifted { id, position: on });
        assert!(rig.send_event(lifted, on).is_empty());
        assert_eq!(rig.slot().y, 10.0);
        rig.send_event(pressed, on);
        assert_eq!(rig.slot().y, 50.0);
        let lost = Event::Touch(touch::Event::FingerLost { id, position: on });
        rig.send_event(lost, on);
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
    fn an_outside_focus_operation_skips_widgets_whose_box_is_hidden() {
        use iced::advanced::widget::{Id, Operation, operation::Focusable};
        // What iced's `focus_next` walks: each focusable widget it reaches.
        struct Count(usize);
        impl Operation for Count {
            fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation)) {
                operate(self);
            }
            fn focusable(&mut self, _: Option<&Id>, _: Rectangle, _: &mut dyn Focusable) {
                self.0 += 1;
            }
        }
        let root = node(
            "page",
            vec![
                node("gone", vec![]),
                node("unseen", vec![]),
                node("shown", vec![]),
            ],
        );
        let tree = BoxTree::new(
            root,
            "#gone { display:none; } #unseen { visibility:hidden; height:10px; }",
        )
        .unwrap();
        let view: View<'_> = tree
            .interactive()
            .widget("gone", iced::widget::text_input("", ""))
            .widget("unseen", iced::widget::text_input("", ""))
            .widget("shown", iced::widget::text_input("", ""))
            .into();
        let mut rig = Rig::new(view);
        let mut count = Count(0);
        rig.element
            .as_widget_mut()
            .operate(&mut rig.tree, Layout::new(&rig.node), &(), &mut count);
        assert_eq!(count.0, 1);
    }

    #[test]
    fn a_boxless_widget_with_a_child_still_operates_and_overlays() {
        struct Walk;
        impl iced::advanced::widget::Operation for Walk {
            fn traverse(
                &mut self,
                operate: &mut dyn FnMut(&mut dyn iced::advanced::widget::Operation),
            ) {
                operate(self);
            }
        }
        let root = node("page", vec![node("top", vec![]), node("menu", vec![])]);
        let tree = BoxTree::new(root, "#menu { display:none; }").unwrap();
        let view: View<'_> = tree
            .interactive()
            .widget("menu", iced::widget::button(space(5.0)).on_press(1_u8))
            .widget("missing", iced::widget::container(space(5.0)))
            .widget("page", iced::widget::button(space(5.0)))
            .into();
        let mut rig = Rig::new(view);
        assert!(
            rig.node
                .children()
                .iter()
                .all(|child| child.size() == Size::ZERO)
        );
        let viewport = Rectangle::new(Point::ORIGIN, Size::new(100.0, 200.0));
        rig.element
            .as_widget_mut()
            .operate(&mut rig.tree, Layout::new(&rig.node), &(), &mut Walk);
        assert!(
            rig.element
                .as_widget_mut()
                .overlay(
                    &mut rig.tree,
                    Layout::new(&rig.node),
                    &(),
                    &viewport,
                    iced::Vector::ZERO,
                )
                .is_none()
        );
        // Tab walks every hosted widget, the boxless ones too.
        let mut messages = Vec::new();
        let mut shell = Shell::new(&mut messages);
        rig.element.as_widget_mut().update(
            &mut rig.tree,
            &Event::Keyboard(iced::keyboard::Event::KeyPressed {
                key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Tab),
                modified_key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Tab),
                physical_key: iced::keyboard::key::Physical::Code(iced::keyboard::key::Code::Tab),
                location: iced::keyboard::Location::Standard,
                modifiers: iced::keyboard::Modifiers::default(),
                text: None,
                repeat: false,
            }),
            Layout::new(&rig.node),
            mouse::Cursor::Unavailable,
            &(),
            &mut clipboard::Null,
            &mut shell,
            &viewport,
        );
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
        key_with(rig, named, modifiers, false)
    }

    fn key_with(
        rig: &mut Rig<'_>,
        named: iced::keyboard::key::Named,
        modifiers: iced::keyboard::Modifiers,
        repeat: bool,
    ) -> Vec<u8> {
        let event = Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Named(named),
            modified_key: iced::keyboard::Key::Named(named),
            physical_key: iced::keyboard::key::Physical::Unidentified(
                iced::keyboard::key::NativeCode::Unidentified,
            ),
            location: iced::keyboard::Location::Standard,
            modifiers,
            text: None,
            repeat,
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
        // A held Space presses once; a held Enter goes on pressing.
        let none = iced::keyboard::Modifiers::empty();
        assert!(key_with(&mut rig, Named::Space, none, true).is_empty());
        assert_eq!(key_with(&mut rig, Named::Enter, none, true), vec![5]);
        // With Ctrl, Alt or Logo the keys are shortcuts: the app's, uncaptured.
        for chord in [
            iced::keyboard::Modifiers::CTRL,
            iced::keyboard::Modifiers::ALT,
            iced::keyboard::Modifiers::LOGO,
        ] {
            assert!(key_with(&mut rig, Named::Enter, chord, false).is_empty());
            assert!(!rig.captured);
            assert!(key_with(&mut rig, Named::Tab, chord, false).is_empty());
            assert!(!rig.captured);
        }
        // Focus stayed where it was.
        assert_eq!(key(&mut rig, Named::Enter, false), vec![5]);
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
    fn focus_and_scroll_follow_their_elements_into_a_rebuilt_view() {
        use iced::keyboard::key::Named;
        let css = "#list { overflow:auto; height:50px; }
                   #tall { height:100px; }
                   #new, #more, #a, #b { height:10px; }";
        let root = node(
            "page",
            vec![
                node("a", vec![]),
                node("b", vec![]),
                node("list", vec![node("tall", vec![]), node("slot", vec![])]),
            ],
        );
        let tree = BoxTree::new(root, css).unwrap();
        let view: View<'_> = tree
            .interactive()
            .widget("slot", space(10.0))
            .on_press("a", 1)
            .on_press("b", 2)
            .into();
        let mut rig = Rig::new(view);
        assert_eq!(rig.slot().y, 120.0);
        rig.send(wheel(-30.0), Point::new(5.0, 30.0));
        assert_eq!(rig.slot().y, 90.0);
        key(&mut rig, Named::Tab, false);
        assert_eq!(key(&mut rig, Named::Enter, false), vec![1]);
        // Source nodes are postorder: a box before #a and one more in #list renumber
        // #a, #list and the root.
        let root = node(
            "page",
            vec![
                node("new", vec![]),
                node("a", vec![]),
                node("b", vec![]),
                node(
                    "list",
                    vec![
                        node("tall", vec![]),
                        node("slot", vec![]),
                        node("more", vec![]),
                    ],
                ),
            ],
        );
        let grown = BoxTree::new(root, css).unwrap();
        let mut next: View<'_> = grown
            .interactive()
            .widget("slot", space(10.0))
            .on_press("new", 3)
            .on_press("a", 1)
            .on_press("b", 2)
            .into();
        rig.tree.diff(&next);
        rig.node = next.as_widget_mut().layout(&mut rig.tree, &(), &limits());
        rig.element = next;
        // #list is still scrolled by 30, below the new 10px box.
        assert_eq!(rig.slot().y, 100.0);
        // And #a still has the focus, not the box that took its number.
        assert_eq!(key(&mut rig, Named::Enter, false), vec![1]);
        key(&mut rig, Named::Tab, false);
        assert_eq!(key(&mut rig, Named::Enter, false), vec![2]);
    }

    #[test]
    fn another_page_in_the_same_place_takes_no_focus_from_the_last() {
        use iced::keyboard::key::Named;
        let css = "#a, #b, #x, #y { height:10px; }";
        let tree = BoxTree::new(
            node("page", vec![node("a", vec![]), node("b", vec![])]),
            css,
        )
        .unwrap();
        let view: View<'_> = tree.interactive().on_press("a", 1).on_press("b", 2).into();
        let mut rig = Rig::new(view);
        key(&mut rig, Named::Tab, false);
        assert_eq!(key(&mut rig, Named::Enter, false), vec![1]);
        // Same shape, other elements: the focused node's number names #x now.
        let other = BoxTree::new(
            node("page", vec![node("x", vec![]), node("y", vec![])]),
            css,
        )
        .unwrap();
        let mut next: View<'_> = other.interactive().on_press("x", 3).on_press("y", 4).into();
        rig.tree.diff(&next);
        rig.node = next.as_widget_mut().layout(&mut rig.tree, &(), &limits());
        rig.element = next;
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
    fn the_wheel_over_a_fixed_box_scrolls_the_page_past_its_scrollers() {
        let root = node(
            "page",
            vec![
                node(
                    "box",
                    vec![
                        node("in", vec![]),
                        node("deep", vec![]),
                        node("bar", vec![]),
                    ],
                ),
                node("slot", vec![]),
                node("tall", vec![]),
            ],
        );
        let tree = BoxTree::new(
            root,
            "#page { overflow:auto; }
             #box { overflow:auto; height:50px; }
             #in { height:10px; }
             #deep { height:100px; }
             #bar { position:fixed; top:150px; left:0; width:20px; height:20px; }
             #slot { height:10px; }
             #tall { height:1000px; }",
        )
        .unwrap();
        let view: View<'_> = tree
            .interactive()
            .widget("in", space(10.0))
            .widget("slot", space(10.0))
            .into();
        let mut rig = Rig::new(view);
        assert_eq!(rig.node.children()[1].bounds().y, 50.0);
        rig.send(wheel(-30.0), Point::new(5.0, 155.0));
        // The page moved; the box between the fixed bar and the root did not.
        assert!(rig.captured);
        assert_eq!(rig.node.children()[1].bounds().y, 20.0);
        assert_eq!(rig.node.children()[0].bounds().y, -30.0);
    }

    #[test]
    fn hover_follows_the_boxes_a_wheel_moves_under_a_still_pointer() {
        let root = node(
            "page",
            vec![node(
                "list",
                vec![
                    node("a", vec![]),
                    node("b", vec![]),
                    node("slot", vec![]),
                    node("tall", vec![]),
                ],
            )],
        );
        let tree = BoxTree::new(
            root,
            "#list { overflow:auto; height:50px; }
             #a, #b { height:20px; }
             #a:hover { height:25px; }
             #tall { height:100px; }",
        )
        .unwrap();
        let view: View<'_> = tree.interactive().widget("slot", space(10.0)).into();
        let mut rig = Rig::new(view);
        let at = Point::new(5.0, 15.0);
        rig.send(mouse::Event::CursorMoved { position: at }, at);
        assert_eq!(rig.slot().y, 45.0);
        rig.send(wheel(-20.0), at);
        assert_eq!(rig.slot().y, 25.0);
        // The redraw after the scroll finds #b under the pointer, and #a loses its hover.
        let redraw = Event::Window(iced::window::Event::RedrawRequested(
            iced::time::Instant::now(),
        ));
        rig.send_event(redraw.clone(), at);
        assert_eq!(rig.slot().y, 20.0);
        // Once: a later redraw with nothing scrolled changes nothing.
        rig.send_event(redraw, at);
        assert_eq!(rig.slot().y, 20.0);
    }

    #[test]
    fn a_static_root_scrolls_absolute_children_with_the_page() {
        // 11.1.1: the root's initial `visible` scrolls the viewport as `auto` would.
        for page in ["#page { overflow:auto; }", ""] {
            let root = node("page", vec![node("slot", vec![]), node("far", vec![])]);
            let css = format!(
                "{page}
                 #slot {{ height:10px; }}
                 #far {{ position:absolute; top:500px; left:0; width:5px; height:5px; }}"
            );
            root_scrolls_to_far(BoxTree::new(root, &css).unwrap());
        }
        // `hidden` on the root does not scroll.
        let root = node("page", vec![node("slot", vec![]), node("far", vec![])]);
        let tree = BoxTree::new(
            root,
            "#page { overflow:hidden; }
             #slot { height:10px; }
             #far { position:absolute; top:500px; left:0; width:5px; height:5px; }",
        )
        .unwrap();
        let view: View<'_> = tree.interactive().widget("slot", space(10.0)).into();
        let mut rig = Rig::new(view);
        rig.send(wheel(-10000.0), Point::new(5.0, 5.0));
        assert_eq!(rig.slot().y, 0.0);
    }

    fn root_scrolls_to_far(tree: BoxTree) {
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
    fn tab_scrolls_across_too_and_a_fixed_target_moves_nothing() {
        use iced::keyboard::key::Named;
        let root = node(
            "page",
            vec![node(
                "list",
                vec![node("slot", vec![]), node("far", vec![])],
            )],
        );
        let tree = BoxTree::new(
            root,
            "#list { overflow:auto; height:50px; width:60px; }
             #slot { height:5px; }
             #far { margin-left:200px; width:10px; height:5px; }",
        )
        .unwrap();
        let view: View<'_> = tree
            .interactive()
            .widget("slot", space(5.0))
            .on_press("far", 2)
            .into();
        let mut rig = Rig::new(view);
        key(&mut rig, Named::Tab, false);
        // #far ends at 210; to show it in a 60px box the list scrolls by 150.
        assert_eq!(rig.slot().x, -150.0);

        let root = node(
            "page",
            vec![node(
                "list",
                vec![
                    node("slot", vec![]),
                    node("spacer", vec![]),
                    node("far", vec![]),
                ],
            )],
        );
        let tree = BoxTree::new(
            root,
            "#list { overflow:auto; height:50px; }
             #slot { height:5px; }
             #spacer { height:200px; }
             #far { position:fixed; top:300px; height:5px; width:5px; }",
        )
        .unwrap();
        let view: View<'_> = tree
            .interactive()
            .widget("slot", space(5.0))
            .on_press("far", 2)
            .into();
        let mut rig = Rig::new(view);
        key(&mut rig, Named::Tab, false);
        assert_eq!(rig.slot().y, 0.0);
    }

    #[test]
    fn a_diagonal_wheel_moves_each_axis_on_its_own_box() {
        let root = node(
            "page",
            vec![node(
                "outer",
                vec![node("inner", vec![node("slot", vec![])])],
            )],
        );
        let tree = BoxTree::new(
            root,
            "#outer { overflow:auto; height:50px; width:200px; }
             #inner { overflow:auto; width:60px; }
             #slot { width:100px; height:90px; }",
        )
        .unwrap();
        let view: View<'_> = tree.interactive().widget("slot", space(90.0)).into();
        let mut rig = Rig::new(view);
        rig.send(
            mouse::Event::WheelScrolled {
                delta: mouse::ScrollDelta::Pixels { x: -10.0, y: -20.0 },
            },
            Point::new(5.0, 5.0),
        );
        // The inner box only goes across; the outer one takes the vertical part.
        assert_eq!(rig.slot().x, -10.0);
        assert_eq!(rig.slot().y, -20.0);
    }

    #[test]
    fn css_cursor_beats_the_press_pointer_and_inherits() {
        let root = node(
            "page",
            vec![node("a", vec![]), node("b", vec![]), node("c", vec![])],
        );
        let tree = BoxTree::new(
            root,
            "#page { cursor:crosshair; }
             #a, #b, #c { height:10px; }
             #b { cursor:auto; }
             #c { cursor:url(x.cur), move; }",
        )
        .unwrap();
        let view: View<'_> = tree.interactive().on_press("b", 1).into();
        let rig = Rig::new(view);
        let shown = |y: f32| {
            rig.element.as_widget().mouse_interaction(
                &rig.tree,
                Layout::new(&rig.node),
                mouse::Cursor::Available(Point::new(5.0, y)),
                &Rectangle::new(Point::ORIGIN, iced::Size::new(800.0, 600.0)),
                &(),
            )
        };
        assert_eq!(shown(5.0), mouse::Interaction::Crosshair);
        assert_eq!(shown(15.0), mouse::Interaction::Pointer);
        assert_eq!(shown(25.0), mouse::Interaction::Move);
    }

    #[test]
    fn a_diagonal_wheel_moves_one_box_both_ways() {
        let root = node("page", vec![node("list", vec![node("slot", vec![])])]);
        let tree = BoxTree::new(
            root,
            "#list { overflow:auto; height:50px; width:60px; }
             #slot { width:100px; height:90px; }",
        )
        .unwrap();
        let view: View<'_> = tree.interactive().widget("slot", space(90.0)).into();
        let mut rig = Rig::new(view);
        rig.send(
            mouse::Event::WheelScrolled {
                delta: mouse::ScrollDelta::Pixels { x: -10.0, y: -20.0 },
            },
            Point::new(5.0, 5.0),
        );
        assert_eq!((rig.slot().x, rig.slot().y), (-10.0, -20.0));
    }

    #[test]
    fn an_absolute_box_escaping_a_static_clip_still_extends_the_scroller() {
        let root = node(
            "page",
            vec![node(
                "list",
                vec![
                    node("clip", vec![node("far", vec![])]),
                    node("slot", vec![]),
                ],
            )],
        );
        let tree = BoxTree::new(
            root,
            "#list { overflow:auto; position:relative; height:50px; width:100px; }
             #clip { overflow:hidden; height:5px; }
             #far { position:absolute; top:200px; height:5px; width:5px; }
             #slot { height:5px; }",
        )
        .unwrap();
        let view: View<'_> = tree.interactive().widget("slot", space(5.0)).into();
        let mut rig = Rig::new(view);
        rig.send(wheel(-5.0), Point::new(50.0, 40.0));
        assert!(
            rig.slot().y < 5.0,
            "the far box should make the list scroll"
        );
    }

    #[test]
    fn a_presented_value_survives_an_interaction_restyle() {
        let root = || node("page", vec![node("slot", vec![]), node("other", vec![])]);
        let sheet = |width: u32| {
            format!(
                "#slot {{ width:{width}px; height:10px; }} #other {{ height:10px; }} #other:hover {{ margin-top:5px; }}"
            )
        };
        let from = BoxTree::new(root(), &sheet(100)).unwrap();
        let to = from.restyled(&[&sheet(200)]).unwrap();
        let mut animations = iced_css::box_tree::BoxTreeAnimations::new();
        let start = std::time::Instant::now();
        animations
            .request(
                &from,
                &to,
                "slot",
                &["width"],
                iced_css::animation::clock::AnimationTiming::new(
                    std::time::Duration::from_secs(1),
                    std::time::Duration::ZERO,
                ),
                start,
            )
            .unwrap();
        animations.advance(start + std::time::Duration::from_millis(500), &to);
        let view: View<'_> = animations
            .presented(&to)
            .interactive()
            .widget("slot", space(10.0))
            .into();
        let mut rig = Rig::new(view);
        assert_eq!(rig.slot().width, 150.0);
        rig.send(
            mouse::Event::CursorMoved {
                position: Point::new(5.0, 15.0),
            },
            Point::new(5.0, 15.0),
        );
        assert_eq!(rig.slot().width, 150.0);
    }

    #[test]
    fn an_important_interaction_value_holds_over_a_presented_one() {
        let root = || node("page", vec![node("slot", vec![])]);
        let sheet = |width: u32| {
            format!(
                "#slot {{ width:{width}px; height:10px; }} #slot:hover {{ width:200px !important; }}"
            )
        };
        let from = BoxTree::new(root(), &sheet(100)).unwrap();
        let to = from.restyled(&[&sheet(200)]).unwrap();
        let mut animations = iced_css::box_tree::BoxTreeAnimations::new();
        let start = std::time::Instant::now();
        animations
            .request(
                &from,
                &to,
                "slot",
                &["width"],
                iced_css::animation::clock::AnimationTiming::new(
                    std::time::Duration::from_secs(1),
                    std::time::Duration::ZERO,
                ),
                start,
            )
            .unwrap();
        animations.advance(start + std::time::Duration::from_millis(500), &to);
        let view: View<'_> = animations
            .presented(&to)
            .interactive()
            .widget("slot", space(10.0))
            .into();
        let mut rig = Rig::new(view);
        assert_eq!(rig.slot().width, 150.0);
        let on = Point::new(5.0, 5.0);
        rig.send(mouse::Event::CursorMoved { position: on }, on);
        assert_eq!(rig.slot().width, 200.0, "the same value, now important");
        let off = Point::new(5.0, 150.0);
        rig.send(mouse::Event::CursorMoved { position: off }, off);
        assert_eq!(rig.slot().width, 150.0);
    }

    #[test]
    fn a_hosted_widget_draws_within_its_clipping_box() {
        struct Probe(std::rc::Rc<std::cell::Cell<Option<Rectangle>>>);
        impl iced::advanced::Widget<u8, Theme, ()> for Probe {
            fn size(&self) -> Size<iced::Length> {
                Size::new(iced::Length::Fill, iced::Length::Fixed(20.0))
            }
            fn layout(
                &mut self,
                _: &mut Tree,
                _: &(),
                limits: &Limits,
            ) -> iced::advanced::layout::Node {
                iced::advanced::layout::Node::new(limits.resolve(
                    iced::Length::Fill,
                    20.0,
                    Size::ZERO,
                ))
            }
            fn draw(
                &self,
                _: &Tree,
                _: &mut (),
                _: &Theme,
                _: &iced::advanced::renderer::Style,
                _: Layout<'_>,
                _: mouse::Cursor,
                viewport: &Rectangle,
            ) {
                self.0.set(Some(*viewport));
            }
        }
        let root = node("page", vec![node("clip", vec![node("slot", vec![])])]);
        let tree = BoxTree::new(root, "#clip { overflow:hidden; height:5px; }").unwrap();
        let seen = std::rc::Rc::new(std::cell::Cell::new(None));
        let view: View<'_> = tree
            .interactive()
            .widget("slot", iced::Element::new(Probe(seen.clone())))
            .into();
        let rig = Rig::new(view);
        let slot = rig.slot();
        assert_eq!(slot.height, 20.0);
        rig.element.as_widget().draw(
            &rig.tree,
            &mut (),
            &Theme::Light,
            &iced::advanced::renderer::Style::default(),
            Layout::new(&rig.node),
            mouse::Cursor::Unavailable,
            &Rectangle::new(Point::ORIGIN, Size::new(100.0, 200.0)),
        );
        let viewport = seen.get().expect("the widget is drawn");
        assert_eq!(viewport.y, slot.y);
        assert_eq!(viewport.height, 5.0);
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
fn inside_marker_images_take_a_run_of_their_own() {
    let leaf_of = |css: &str| {
        let root = node("page", vec![node("one", vec![text("a", "item")])]);
        let sheet = format!("#one {{ display:list-item; list-style-position:inside; {css} }}");
        let tree = BoxTree::new(root, &sheet).unwrap();
        let leaves = tree.text_leaves();
        assert_eq!(leaves.len(), 1);
        leaves[0]
            .1
            .runs
            .iter()
            .map(|run| (run.text.clone(), run.marker_image.is_some()))
            .collect::<Vec<_>>()
    };
    let plain = |text: &str| (text.to_owned(), false);
    let marker = |text: &str| (text.to_owned(), true);
    assert_eq!(leaf_of(""), vec![plain("\u{2022}\u{a0}"), plain("item")]);
    assert_eq!(
        leaf_of("list-style-image:url(dot.png);"),
        vec![marker("\u{2022}\u{a0}"), plain("item")]
    );
    // The marker's own run, apart from the `:before` content after it.
    let root = node("page", vec![node("one", vec![text("a", "item")])]);
    let tree = BoxTree::new(
        root,
        "#one { display:list-item; list-style:inside lower-roman url(dot.png);
                font-variant:small-caps; }
         #one:before { content:\"x\"; }",
    )
    .unwrap();
    let runs = &tree.text_leaves()[0].1.runs;
    // Small capitals leave the marker as it is, in one run.
    assert_eq!(runs[0].text, "i.\u{a0}");
    assert!(runs[0].marker_image.is_some());
    assert!(runs[1..].iter().all(|run| run.marker_image.is_none()));
    assert_eq!(tree.text_leaves()[0].1.text(), "i.\u{a0}XITEM");
    // With no marker type the image still has a run, empty until it loads.
    assert_eq!(
        leaf_of("list-style-type:none; list-style-image:url(dot.png);"),
        vec![marker(""), plain("item")]
    );
    assert_eq!(leaf_of("list-style-type:none;"), vec![plain("item")]);
    // Only a local file can load; outside markers draw on their own.
    assert_eq!(
        leaf_of("list-style-image:url(http://example.com/dot.png);"),
        vec![plain("\u{2022}\u{a0}"), plain("item")]
    );
    assert_eq!(
        leaf_of("list-style-position:outside; list-style-image:url(dot.png);"),
        vec![plain("item")]
    );
}

#[test]
fn the_core_types_in_the_signatures_are_named_here() {
    let limits: iced_css::capture::AttributeLimits = iced_css::box_tree::attribute_limits();
    let refused: Result<_, iced_css::capture::AttributeError> =
        adapters::text::<iced::Theme, ()>("x", capture_limits())
            .unwrap()
            .style_id("x", limits);
    assert!(refused.is_ok());
    let tree = BoxTree::new(node("page", vec![]), "#page { height:10px; }").unwrap();
    let boxes: Vec<Option<iced_css::box_tree::FlowGeometry>> = tree.layout(100.0, None).unwrap();
    let border: iced_css::box_tree::FlowRect = boxes[0].unwrap().border;
    assert_eq!(border.bottom - border.top, 10.0);
}

#[test]
fn trees_differing_in_marker_images_or_sheets_are_not_equal() {
    let tree = |css: &str, base: &str| {
        let root = node("page", vec![node("one", vec![text("a", "item")])]);
        BoxTree::with_base(root, css, std::path::Path::new(base)).unwrap()
    };
    let css = "#one { display:list-item; list-style-image:url(dot.png); }";
    assert!(tree(css, "a") == tree(css, "a"));
    // Only the outside marker's image differs.
    assert!(tree(css, "a") != tree(css, "b"));
    // A `:hover` rule that matches nothing yet changes what the tree can become.
    let plain = tree("#one { display:block; }", "a");
    let hover = plain
        .restyled(&["#one { display:block; } #one:hover { color:red; }"])
        .unwrap();
    assert!(plain != hover);
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
    // A block and a cell directly in a table share one anonymous row, in either order.
    for (first, second) in [("div", "c"), ("c", "div")] {
        let root = node("t", vec![node(first, vec![]), node(second, vec![])]);
        let tree = BoxTree::new(
            root,
            "#t { display:table; } #c { display:table-cell; }
             #div, #c { width:20px; height:5px; }",
        )
        .unwrap();
        let g = tree.layout(100.0, None).unwrap();
        assert_eq!(g[1].unwrap().border.top, g[2].unwrap().border.top);
        assert_eq!(g[2].unwrap().border.left, 20.0);
    }
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
fn nowrap_keeps_a_line_of_inline_blocks_on_one_line() {
    let page = || {
        node(
            "page",
            vec![node("a", vec![]), node("b", vec![]), node("c", vec![])],
        )
    };
    let boxes = "#a, #b, #c { display:inline-block; width:40px; height:10px; }";
    let g = BoxTree::new(page(), boxes)
        .unwrap()
        .layout(100.0, None)
        .unwrap();
    assert_eq!(
        (g[3].unwrap().border.left, g[3].unwrap().border.top),
        (0.0, 10.0)
    );
    let css = format!("#page {{ white-space:nowrap; }} {boxes}");
    let g = BoxTree::new(page(), &css)
        .unwrap()
        .layout(100.0, None)
        .unwrap();
    assert_eq!(
        (g[3].unwrap().border.left, g[3].unwrap().border.top),
        (80.0, 0.0)
    );
    assert_eq!(g[0].unwrap().border.bottom, 10.0);
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
fn text_beside_inline_blocks_takes_a_box_in_the_line_and_blank_text_does_not() {
    let root = node("page", vec![node("a", vec![])]);
    let css =
        "#a { display:inline-block; width:40px; height:10px; } #page:after { content:\"x\"; }";
    let tree = BoxTree::new(root, css).unwrap();
    assert_eq!(tree.text_leaves()[0].1.text(), "x");
    let root = node(
        "page",
        vec![text("one", "one"), node("a", vec![]), text("two", "two")],
    );
    let tree = BoxTree::new(
        root,
        "#a { display:inline-block; width:40px; height:10px; }",
    )
    .unwrap();
    let mut widths = tree.clone();
    widths.set_text_widths(|_| (30.0, 30.0));
    let geometry = widths
        .layout_text(800.0, Some(600.0), &mut |_, _| 10.0)
        .unwrap();
    let chip = geometry[2].as_ref().unwrap();
    assert_eq!((chip.border.left, chip.border.right), (30.0, 70.0));
    assert_eq!(geometry[3].as_ref().unwrap().border.left, 70.0);
    assert_eq!(geometry[1].as_ref().unwrap().border.left, 0.0);
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
fn counter_styles_format_as_core_does() {
    let root = node(
        "page",
        vec![
            node("a", vec![]),
            node("b", vec![]),
            node("c", vec![]),
            node("d", vec![]),
            node("e", vec![]),
        ],
    );
    let tree = BoxTree::new(
        root,
        "#page { counter-reset:n 3 z 27 m -1; }
         #a:before { content:counter(n, lower-greek); }
         #b:before { content:counter(n, armenian); }
         #c:before { content:counter(n, georgian); }
         #d:before { content:counter(z, lower-alpha); }
         #e:before { content:counter(m, decimal-leading-zero); }",
    )
    .unwrap();
    let texts: Vec<_> = tree.text_leaves().iter().map(|(_, l)| l.text()).collect();
    // Latin past 26 falls back to decimal, and a negative leading-zero value pads.
    assert_eq!(texts, vec!["\u{3b3}", "\u{533}", "\u{10d2}", "27", "-01"]);
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
fn content_less_pseudo_with_an_inside_marker_does_not_count() {
    // No `content`, so no pseudo-element: its counter directives apply whatever the marker.
    for position in ["inside", "outside"] {
        let root = node("page", vec![node("li", vec![]), node("next", vec![])]);
        let sheet = format!(
            "#page {{ counter-reset:n; }}
             #li {{ display:list-item; list-style-position:{position}; }}
             #li:before {{ counter-increment:n 5; }}
             #next:before {{ content:\"[\" counter(n) \"]\"; }}"
        );
        let tree = BoxTree::new(root, &sheet).unwrap();
        let texts: Vec<_> = tree.text_leaves().iter().map(|(_, l)| l.text()).collect();
        assert!(texts.iter().any(|t| t == "[0]"), "{position}: {texts:?}");
    }
}

#[test]
fn a_block_pseudo_takes_its_own_line_height_alignment_and_indent() {
    let root = node("e", vec![text("t", "body")]);
    let tree = BoxTree::new(
        root,
        "#e { font-size:10px; }
         #e:before { content:\"Title\"; display:block; text-align:center; line-height:40px;
                     text-indent:10px; }
         #e:after { content:\"End\"; display:block; text-indent:inherit; }",
    )
    .unwrap();
    let leaves = tree.text_leaves();
    let title = &leaves[0].1;
    assert_eq!(title.text(), "\u{2003}Title");
    assert_eq!(title.align, "center");
    assert_eq!(
        title.line_height,
        iced_css::evaluation::tree_layout::TextLineHeight::Px(40.0)
    );
    // The element's own text keeps its alignment, as does a later block that sets none.
    assert_eq!(leaves[1].1.text(), "body");
    assert_eq!(leaves[1].1.align, "left");
    assert_eq!(leaves[2].1.text(), "End");
    assert_eq!(leaves[2].1.align, "left");
    // An inline one does not align or indent: the element's lines do.
    let root = node("e", vec![text("t", "body")]);
    let tree = BoxTree::new(
        root,
        "#e:before { content:\"x\"; text-align:center; text-indent:10px; }",
    )
    .unwrap();
    let leaf = &tree.text_leaves()[0].1;
    assert_eq!(leaf.align, "left");
    assert_eq!(leaf.text(), "xbody");
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
    // 17.6.2.1: on the outer edges the table's wider 7px border wins, and the outer cells
    // draw it; inside they overlap by their shared 2px border.
    assert_eq!((edge(2).left, edge(2).right), (0.0, 19.0));
    assert_eq!((edge(3).left, edge(3).right), (17.0, 36.0));
    assert_eq!((edge(2).top, edge(5).top), (0.0, 12.0));
    assert_eq!((edge(0).right, edge(0).bottom), (36.0, 26.0));
    assert_eq!(tree.paints()[0].border[0], None);
    assert_eq!(tree.paints()[2].border[0], Some([0.0, 0.0, 0.0]));
}

#[test]
fn collapse_without_cell_borders_draws_the_table_border_on_its_cells_and_no_spacing() {
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
    // The outer cells draw the table's 3px border, which wins over none.
    assert_eq!(g[2].unwrap().border.left, 0.0);
    assert_eq!(g[2].unwrap().content.left, 3.0);
    assert_eq!(g[3].unwrap().border.left, 13.0);
    assert!(tree.paints()[2].border[3].is_some());
    assert!(tree.paints()[0].border[0].is_none());
}

#[test]
fn collapse_draws_row_group_and_column_borders_on_the_cells_along_them() {
    // 17.6.2.1: a row's border lies on its cells' sides, and wins over none there.
    let rows = |extra: &str| {
        let root = node(
            "t",
            vec![
                node("r1", vec![node("a", vec![]), node("b", vec![])]),
                node("r2", vec![node("c", vec![]), node("d", vec![])]),
            ],
        );
        BoxTree::new(
            root,
            &format!(
                "#t {{ display:table; border-collapse:collapse; }}
                 #r1, #r2 {{ display:table-row; }}
                 #r1 {{ border-bottom:4px solid #ff0000; }}
                 #a, #b, #c, #d {{ display:table-cell; width:10px; height:5px; }}
                 {extra}"
            ),
        )
        .unwrap()
    };
    let tree = rows("");
    let g = tree.layout(200.0, None).unwrap();
    let red = Some([1.0, 0.0, 0.0]);
    assert_eq!(
        (tree.paints()[2].border[2], tree.paints()[3].border[2]),
        (red, red)
    );
    assert_eq!(
        (tree.paints()[2].border[1], tree.paints()[5].border[0]),
        (None, None)
    );
    // The border takes room: the next row starts below it.
    assert_eq!(
        (g[2].unwrap().border.bottom, g[5].unwrap().content.top),
        (9.0, 9.0)
    );
    assert_eq!(g[0].unwrap().border.bottom, 14.0);
    // A cell's border as wide and of the same style keeps the edge, and a wider row's
    // takes it back.
    let tree = rows("#a { border-bottom:4px solid #00ff00; }");
    assert_eq!(tree.paints()[2].border[2], Some([0.0, 1.0, 0.0]));
    let tree = rows("#a { border-bottom:2px solid #00ff00; }");
    assert_eq!(tree.paints()[2].border[2], red);
    // `hidden` on the row leaves none there, of the cells' either.
    let tree = rows("#r1 { border-bottom-style:hidden; } #c { border-top:2px solid; }");
    assert_eq!(
        (tree.paints()[2].border[2], tree.paints()[5].border[0]),
        (None, None)
    );
    // Separated borders: a row has none.
    let tree = rows("#t { border-collapse:separate; }");
    assert_eq!(tree.paints()[2].border_style[2], "none");
    assert_eq!(
        tree.layout(200.0, None).unwrap()[0].unwrap().border.bottom,
        10.0
    );

    // A row group's border goes around its rows, a column's down its cells.
    let root = node(
        "t",
        vec![
            node("col", vec![]),
            node(
                "g",
                vec![
                    node("r1", vec![node("a", vec![]), node("b", vec![])]),
                    node("r2", vec![node("c", vec![]), node("d", vec![])]),
                ],
            ),
        ],
    );
    let tree = BoxTree::new(
        root,
        "#t { display:table; border-collapse:collapse; }
         #col { display:table-column; border-right:3px solid #00ff00; }
         #g { display:table-row-group; border:2px solid #0000ff; }
         #r1, #r2 { display:table-row; }
         #a, #b, #c, #d { display:table-cell; width:10px; height:5px; }",
    )
    .unwrap();
    let g = tree.layout(200.0, None).unwrap();
    let (green, blue) = (Some([0.0, 1.0, 0.0]), Some([0.0, 0.0, 1.0]));
    let paints = tree.paints();
    // a: the group's top and left, the column's right, nothing below inside the group.
    assert_eq!(paints[4].border, [blue, green, None, blue]);
    assert_eq!(paints[8].border, [None, blue, blue, None]);
    assert_eq!(paints[7].border[1], green);
    // The column's 3px sits between the cells, the group's 2px around them.
    assert_eq!(
        (g[4].unwrap().content.left, g[5].unwrap().content.left),
        (2.0, 15.0)
    );
    assert_eq!(
        (g[0].unwrap().border.right, g[0].unwrap().border.bottom),
        (27.0, 14.0)
    );
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
    // Negative pixel offsets, the sprite technique, keep their sign.
    let root = node("page", vec![node("a", vec![])]);
    let tree = BoxTree::new(
        root,
        "#a { background:url(tile.png) no-repeat -10px -20px; }",
    )
    .unwrap();
    assert_eq!(
        tree.background_images()[0].1.position,
        [BackgroundOffset::Px(-10.0), BackgroundOffset::Px(-20.0)]
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

mod animation {
    use super::*;
    use iced_css::{animation::clock::AnimationTiming, box_tree::BoxTreeAnimations};
    use std::time::{Duration, Instant};

    fn second() -> AnimationTiming {
        AnimationTiming::new(Duration::from_secs(1), Duration::ZERO)
    }
    fn near(left: [f32; 3], right: [f32; 3]) -> bool {
        left.iter().zip(right).all(|(a, b)| (a - b).abs() < 0.01)
    }
    fn tree(css: &str) -> BoxTree {
        let root = node(
            "page",
            vec![node("box", vec![node("kid", vec![]), node("own", vec![])])],
        );
        BoxTree::new(root, css).unwrap()
    }
    fn index(tree: &BoxTree, _name: &str, at: usize) -> Option<[f32; 3]> {
        tree.paints()[at].background
    }

    #[test]
    fn a_requested_color_runs_from_old_to_new_and_lets_go() {
        let from = tree("#box { background-color:#ff0000; }");
        let to = from
            .restyled(&["#box { background-color:#0000ff; }"])
            .unwrap();
        let mut animations = BoxTreeAnimations::new();
        let start = Instant::now();
        let count = animations
            .request(&from, &to, "box", &["background-color"], second(), start)
            .unwrap();
        assert_eq!(count, 1);
        assert!(animations.is_animating());
        let shown = animations.presented(&to);
        assert!(near(index(&shown, "box", 1).unwrap(), [1.0, 0.0, 0.0]));
        animations.advance(start + Duration::from_millis(500), &to);
        let shown = animations.presented(&to);
        assert!(
            near(index(&shown, "box", 1).unwrap(), [0.5, 0.0, 0.5]),
            "{shown:?}"
        );
        // The committed tree is never touched.
        assert_eq!(index(&to, "box", 1), Some([0.0, 0.0, 1.0]));
        animations.advance(start + Duration::from_secs(1), &to);
        assert!(!animations.is_animating());
        assert_eq!(animations.presented(&to), to);
    }

    #[test]
    fn lengths_animate_through_layout_and_mixed_kinds_mix() {
        let from = tree(
            "#box { width:100px; margin-left:10px; padding-top:50%; height:10px; top:0px; position:relative; }",
        );
        let to = from
            .restyled(&["#box { width:200px; margin-left:30px; padding-top:10px; height:auto; top:40px; position:relative; }"])
            .unwrap();
        let mut animations = BoxTreeAnimations::new();
        let start = Instant::now();
        // Shorthands expand. padding-top mixes a percentage and a length; height meets auto.
        let count = animations
            .request(
                &from,
                &to,
                "box",
                &["width", "margin", "padding", "height", "top"],
                second(),
                start,
            )
            .unwrap();
        assert_eq!(count, 4);
        animations.advance(start + Duration::from_millis(500), &to);
        let shown = animations.presented(&to);
        let geometry = shown.layout(800.0, Some(600.0)).unwrap();
        let moved = geometry[1].as_ref().unwrap();
        assert_eq!(moved.border.right - moved.border.left, 150.0);
        assert_eq!(moved.border.left, 20.0);
        assert_eq!(moved.border.top, 20.0);
        // Halfway from 50% to 10 pixels: 5 pixels and 25% of the 800 wide.
        assert_eq!(moved.padding.top, 20.0);
        assert_eq!(moved.content.top - moved.padding.top, 205.0);
    }

    #[test]
    fn a_fixed_box_mixing_a_percentage_height_is_auto_without_a_viewport_height() {
        let from = tree("#box { position:fixed; height:100px; }");
        let to = from
            .restyled(&["#box { position:fixed; height:50%; }"])
            .unwrap();
        let mut animations = BoxTreeAnimations::new();
        let start = Instant::now();
        animations
            .request(&from, &to, "box", &["height"], second(), start)
            .unwrap();
        animations.advance(start + Duration::from_millis(500), &to);
        let shown = animations.presented(&to);
        let height = |tree: &BoxTree, viewport: Option<f32>| {
            let geometry = tree.layout(800.0, viewport).unwrap();
            let moved = geometry[1].as_ref().unwrap();
            moved.border.bottom - moved.border.top
        };
        // With no height to take a share of, the mix is auto, as the percentage is.
        assert_eq!(height(&shown, None), height(&to, None));
        // With one, it is halfway: 50 pixels and 25% of 600.
        assert_eq!(height(&shown, Some(600.0)), 200.0);
    }

    #[test]
    fn an_initial_block_without_a_viewport_height_gives_no_height_basis() {
        let height = |css: &str| {
            let geometry = tree(css).layout(800.0, None).unwrap();
            let placed = geometry[1].as_ref().unwrap();
            (placed.border.top, placed.border.bottom - placed.border.top)
        };
        // Absolute with no positioned ancestor shares the fixed box's initial block.
        assert_eq!(
            height("#box { position:absolute; height:50%; }"),
            height("#box { position:fixed; height:50%; }")
        );
        assert_eq!(
            height("#box { position:absolute; height:50%; }"),
            height("#box { position:absolute; }")
        );
        // `bottom` has no edge to measure from, so the box keeps its static position.
        assert_eq!(
            height("#box { position:fixed; bottom:0; height:40px; }"),
            (0.0, 40.0)
        );
        // A percentage max-height is none rather than a zero cap.
        assert_eq!(
            height("#box { position:absolute; height:40px; max-height:80%; }"),
            (0.0, 40.0)
        );
    }

    #[test]
    fn a_relative_offset_mixes_against_a_definite_height() {
        let css = |top: &str| {
            format!("#page {{ height:400px; }} #box {{ position:relative; top:{top}; }}")
        };
        let from = tree(&css("0px"));
        let to = from.restyled(&[&css("10%")]).unwrap();
        let mut animations = BoxTreeAnimations::new();
        let start = Instant::now();
        animations
            .request(&from, &to, "box", &["top"], second(), start)
            .unwrap();
        animations.advance(start + Duration::from_millis(500), &to);
        let geometry = animations.presented(&to).layout(800.0, None).unwrap();
        // Halfway: 5% of the 400 high.
        assert_eq!(geometry[1].as_ref().unwrap().border.top, 20.0);
    }

    #[test]
    fn an_inline_margin_made_for_an_animation_answers_as_the_styling_would() {
        let source = || {
            node(
                "page",
                vec![node("p", vec![node("s", vec![text("t", "x")])])],
            )
        };
        let css = |sides: &str| format!("#s {{ display:inline; {sides} }}");
        let wide = BoxTree::new(source(), &css("margin-left:10px;")).unwrap();
        let none = wide.restyled(&[&css("")]).unwrap();
        let half = BoxTree::new(source(), &css("margin-left:5px;")).unwrap();
        let margin = |tree: &BoxTree| {
            tree.text_leaves()
                .iter()
                .flat_map(|(_, leaf)| leaf.runs.iter())
                .find(|run| run.side_of.is_some() && run.percent_room.is_some_and(|r| !r.2))
                .map(|run| (run.owner, run.text.clone()))
        };
        let mut animations = BoxTreeAnimations::new();
        let start = Instant::now();
        animations
            .request(&wide, &none, "s", &["margin-left"], second(), start)
            .unwrap();
        animations.advance(start + Duration::from_millis(500), &none);
        // Made where the target has none, its spaces answer for the block around it.
        assert_eq!(margin(&animations.presented(&none)), margin(&half));
        assert!(margin(&half).is_some());
        // An empty inline whose sides go to none has no place for them: at once.
        let empty = || node("page", vec![node("p", vec![node("s", vec![])])]);
        let padded = BoxTree::new(empty(), &css("padding:20px;")).unwrap();
        let bare = padded.restyled(&[&css("")]).unwrap();
        let mut animations = BoxTreeAnimations::new();
        let count = animations
            .request(&padded, &bare, "s", &["padding"], second(), start)
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn an_interruption_starts_from_what_is_shown() {
        let from = tree("#box { background-color:#000000; }");
        let middle = from
            .restyled(&["#box { background-color:#ffffff; }"])
            .unwrap();
        let back = from
            .restyled(&["#box { background-color:#000000; }"])
            .unwrap();
        let mut animations = BoxTreeAnimations::new();
        let start = Instant::now();
        animations
            .request(
                &from,
                &middle,
                "box",
                &["background-color"],
                second(),
                start,
            )
            .unwrap();
        animations.advance(start + Duration::from_millis(500), &middle);
        let halfway = start + Duration::from_millis(500);
        animations
            .request(
                &middle,
                &back,
                "box",
                &["background-color"],
                second(),
                halfway,
            )
            .unwrap();
        let shown = animations.presented(&back);
        assert!(
            near(index(&shown, "box", 1).unwrap(), [0.5, 0.5, 0.5]),
            "{shown:?}"
        );
        animations.advance(halfway + Duration::from_millis(500), &back);
        let shown = animations.presented(&back);
        assert!(
            near(index(&shown, "box", 1).unwrap(), [0.25, 0.25, 0.25]),
            "{shown:?}"
        );
    }

    #[test]
    fn nothing_starts_without_a_changed_animatable_target() {
        let from = tree("#box { width:10px; background-color:#000000; }");
        let to = from
            .restyled(&["#box { width:10px; display:inline-block; }"])
            .unwrap();
        let mut animations = BoxTreeAnimations::new();
        let now = Instant::now();
        let mut count = |names: &[&str], id: &str| {
            animations
                .request(&from, &to, id, names, second(), now)
                .unwrap()
        };
        assert_eq!(count(&["width"], "box"), 0);
        assert_eq!(count(&["display"], "box"), 0);
        assert_eq!(count(&["width"], "absent"), 0);
        assert!(!animations.is_animating());
        assert!(
            animations
                .request(&from, &to, "box", &["no-such"], second(), now)
                .is_err()
        );
    }

    #[test]
    fn an_unrequested_style_change_wins_at_once_and_cancels() {
        let from = tree("#box { width:100px; }");
        let to = from.restyled(&["#box { width:200px; }"]).unwrap();
        let other = from.restyled(&["#box { width:300px; }"]).unwrap();
        let mut animations = BoxTreeAnimations::new();
        let start = Instant::now();
        animations
            .request(&from, &to, "box", &["width"], second(), start)
            .unwrap();
        animations.advance(start + Duration::from_millis(300), &to);
        assert_eq!(animations.presented(&other), other);
        // A frame sees the tree moved on: going back later does not revive the animation.
        animations.advance(start + Duration::from_millis(400), &other);
        assert!(!animations.is_animating());
        assert_eq!(animations.presented(&to), to);
    }

    #[test]
    fn another_capture_with_the_same_id_inherits_nothing() {
        let from = tree("#box { width:100px; }");
        let to = from.restyled(&["#box { width:200px; }"]).unwrap();
        let fresh = tree("#box { width:200px; }");
        let mut animations = BoxTreeAnimations::new();
        let start = Instant::now();
        animations
            .request(&from, &to, "box", &["width"], second(), start)
            .unwrap();
        animations.advance(start + Duration::from_millis(500), &to);
        assert_eq!(animations.presented(&fresh), fresh);
        animations.advance(start + Duration::from_millis(600), &fresh);
        assert!(!animations.is_animating());
        let count = animations
            .request(&from, &fresh, "box", &["width"], second(), start)
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn a_background_fades_to_and_from_transparent() {
        let from = tree("#box { background-color:transparent; }");
        let to = from
            .restyled(&["#box { background-color:#ff0000; }"])
            .unwrap();
        let mut animations = BoxTreeAnimations::new();
        let start = Instant::now();
        let count = animations
            .request(&from, &to, "box", &["background-color"], second(), start)
            .unwrap();
        assert_eq!(count, 1);
        animations.advance(start + Duration::from_millis(250), &to);
        let shown = animations.presented(&to);
        let fade = shown.paints()[1];
        // Premultiplied: the color stays the far end's while it fades in.
        assert!(near(fade.background.unwrap(), [1.0, 0.0, 0.0]), "{fade:?}");
        assert!(
            (fade.background_fade.unwrap() - 0.25).abs() < 0.01,
            "{fade:?}"
        );
        // Interrupted, it fades back out from what shows.
        let halfway = start + Duration::from_millis(250);
        animations
            .request(&to, &from, "box", &["background-color"], second(), halfway)
            .unwrap();
        animations.advance(halfway + Duration::from_millis(500), &from);
        let fade = animations.presented(&from).paints()[1];
        assert!(near(fade.background.unwrap(), [1.0, 0.0, 0.0]), "{fade:?}");
        assert!(
            (fade.background_fade.unwrap() - 0.125).abs() < 0.01,
            "{fade:?}"
        );
        animations.advance(halfway + Duration::from_secs(1), &from);
        assert_eq!(animations.presented(&from), from);
    }

    #[test]
    fn an_interruption_from_another_capture_starts_from_its_own_value() {
        let from = tree("#box { width:100px; }");
        let to = from.restyled(&["#box { width:200px; }"]).unwrap();
        let fresh = tree("#box { width:200px; }");
        let far = fresh.restyled(&["#box { width:300px; }"]).unwrap();
        let mut animations = BoxTreeAnimations::new();
        let start = Instant::now();
        animations
            .request(&from, &to, "box", &["width"], second(), start)
            .unwrap();
        let halfway = start + Duration::from_millis(500);
        animations
            .request(&fresh, &far, "box", &["width"], second(), halfway)
            .unwrap();
        let shown = animations.presented(&far);
        let geometry = shown.layout(800.0, Some(600.0)).unwrap();
        let border = geometry[1].as_ref().unwrap().border;
        assert_eq!(border.right - border.left, 200.0);
    }

    #[test]
    fn an_inline_element_animates_its_background_but_no_box_of_its_own() {
        let source = || node("page", vec![node("span", vec![text("line", "words")])]);
        let from = BoxTree::new(
            source(),
            "#span { display:inline; background-color:#000000; }",
        )
        .unwrap();
        let to = from
            .restyled(&["#span { display:inline; background-color:#ffffff; margin-left:5px; }"])
            .unwrap();
        let mut animations = BoxTreeAnimations::new();
        let start = Instant::now();
        let count = animations
            .request(
                &from,
                &to,
                "span",
                &["background-color", "margin-left"],
                second(),
                start,
            )
            .unwrap();
        // The background runs under the text, and the margin across its line.
        assert_eq!(count, 2);
        animations.advance(start + Duration::from_millis(500), &to);
        let shown = animations.presented(&to);
        let under = |tree: &BoxTree| {
            tree.text_leaves()
                .iter()
                .flat_map(|(_, leaf)| leaf.runs.iter())
                .find(|run| run.text.contains("words"))
                .and_then(|run| run.background)
        };
        assert!(
            near(under(&shown).unwrap(), [0.5, 0.5, 0.5]),
            "{:?}",
            under(&shown)
        );
        assert_eq!(under(&to), Some([1.0, 1.0, 1.0]));
        // Its paint carries the presented color too.
        let paint = shown.paints()[1];
        assert!(near(
            paint.under_text.or(paint.background).unwrap(),
            [0.5, 0.5, 0.5]
        ));
        // Its padded box's own sides take the same color.
        let padded = |color: &str| {
            format!("#span {{ display:inline; padding:3px; background-color:{color}; }}")
        };
        let dark = BoxTree::new(source(), &padded("#000000")).unwrap();
        let light = dark.restyled(&[&padded("#ffffff")]).unwrap();
        let mut padding = BoxTreeAnimations::new();
        padding
            .request(
                &dark,
                &light,
                "span",
                &["background-color"],
                second(),
                start,
            )
            .unwrap();
        padding.advance(start + Duration::from_millis(500), &light);
        let sides = |tree: &BoxTree| {
            tree.text_leaves()
                .iter()
                .flat_map(|(_, leaf)| leaf.runs.iter())
                .flat_map(|run| run.inline_boxes.iter())
                .find_map(|sides| sides.background)
        };
        assert!(near(
            sides(&padding.presented(&light)).unwrap(),
            [0.5, 0.5, 0.5]
        ));
        // From transparent there is no in-between under text: it applies at once.
        let bare = BoxTree::new(source(), "#span { display:inline; }").unwrap();
        let count = animations
            .request(&bare, &to, "span", &["background-color"], second(), start)
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn an_important_target_applies_at_once() {
        let from = tree("#box { width:100px; height:10px; }");
        let to = from
            .restyled(&["#box { width:200px !important; height:20px; }"])
            .unwrap();
        let mut animations = BoxTreeAnimations::new();
        let count = animations
            .request(
                &from,
                &to,
                "box",
                &["width", "height"],
                second(),
                Instant::now(),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn a_target_that_becomes_important_ends_its_running_animation() {
        let from = tree("#box { width:100px; }");
        let to = from.restyled(&["#box { width:200px; }"]).unwrap();
        let important = from
            .restyled(&["#box { width:200px !important; }"])
            .unwrap();
        let width = |shown: &BoxTree| {
            let geometry = shown.layout(800.0, Some(600.0)).unwrap();
            let border = geometry[1].as_ref().unwrap().border;
            border.right - border.left
        };
        let start = Instant::now();
        let half = start + Duration::from_millis(500);
        // A restyle the animations only see through `advance`.
        let mut animations = BoxTreeAnimations::new();
        animations
            .request(&from, &to, "box", &["width"], second(), start)
            .unwrap();
        animations.advance(half, &important);
        assert!(!animations.is_animating());
        assert_eq!(width(&animations.presented(&important)), 200.0);
        // The same target requested again, now important.
        let mut animations = BoxTreeAnimations::new();
        animations
            .request(&from, &to, "box", &["width"], second(), start)
            .unwrap();
        let count = animations
            .request(&to, &important, "box", &["width"], second(), start)
            .unwrap();
        assert_eq!(count, 0);
        animations.advance(half, &important);
        assert!(!animations.is_animating());
        assert_eq!(width(&animations.presented(&important)), 200.0);
    }

    #[test]
    fn an_easing_shapes_progress_and_stays_between_the_ends() {
        use iced::animation::Easing;
        use iced_css::box_tree::Transition;
        let from = tree("#box { width:100px; }");
        let to = from.restyled(&["#box { width:200px; }"]).unwrap();
        let width = |shown: &BoxTree| {
            let geometry = shown.layout(800.0, Some(600.0)).unwrap();
            let border = geometry[1].as_ref().unwrap().border;
            border.right - border.left
        };
        let start = Instant::now();
        let mut animations = BoxTreeAnimations::new();
        let quad = Transition {
            timing: second(),
            easing: Easing::EaseInQuad,
        };
        animations
            .request(&from, &to, "box", &["width"], quad, start)
            .unwrap();
        animations.advance(start + Duration::from_millis(500), &to);
        assert!((width(&animations.presented(&to)) - 125.0).abs() < 0.5);
        // EaseInBack dips below its start early on.
        let mut animations = BoxTreeAnimations::new();
        let back = Transition {
            timing: second(),
            easing: Easing::EaseInBack,
        };
        animations
            .request(&from, &to, "box", &["width"], back, start)
            .unwrap();
        animations.advance(start + Duration::from_millis(200), &to);
        assert_eq!(width(&animations.presented(&to)), 100.0);
    }

    #[test]
    fn a_later_inline_sibling_animates_its_background_and_border_colors() {
        let source = || {
            node(
                "page",
                vec![
                    node("first", vec![text("one", "one")]),
                    node("span", vec![text("two", "two")]),
                ],
            )
        };
        let css = |color: &str| {
            format!(
                "#first, #span {{ display:inline; }} #span {{ background-color:{color}; outline:1px solid {color}; border:1px solid {color}; }}"
            )
        };
        let from = BoxTree::new(source(), &css("#000000")).unwrap();
        let to = from.restyled(&[&css("#ffffff")]).unwrap();
        let mut animations = BoxTreeAnimations::new();
        let start = Instant::now();
        let count = animations
            .request(
                &from,
                &to,
                "span",
                &["background-color", "outline-color", "border-color"],
                second(),
                start,
            )
            .unwrap();
        // Its background, border and outline colors paint on its line.
        assert_eq!(count, 6);
        animations.advance(start + Duration::from_millis(500), &to);
        let shown = animations.presented(&to);
        let under = |text: &str| {
            shown
                .text_leaves()
                .iter()
                .flat_map(|(_, leaf)| leaf.runs.iter())
                .find(|run| run.text.contains(text))
                .and_then(|run| run.background)
        };
        assert!(near(under("two").unwrap(), [0.5, 0.5, 0.5]));
        assert_eq!(under("one"), None);
        // Its border bars, through its own paint rather than a saved one, move too.
        let bars: Vec<[f32; 3]> = shown
            .text_leaves()
            .iter()
            .flat_map(|(_, leaf)| leaf.runs.iter())
            .filter_map(|run| run.edge.map(|edge| edge.2))
            .collect();
        assert!(!bars.is_empty() && bars.iter().all(|color| near(*color, [0.5, 0.5, 0.5])));
        // Page, first, its text, then the span.
        assert!(near(shown.paints()[3].outline.unwrap().1, [0.5, 0.5, 0.5]));
    }

    /// Runs whose text contains `text`, and their backgrounds, in `tree`.
    fn shown_under(tree: &BoxTree, text: &str) -> Vec<Option<[f32; 3]>> {
        tree.text_leaves()
            .iter()
            .flat_map(|(_, leaf)| leaf.runs.iter())
            .filter(|run| run.text.contains(text))
            .map(|run| run.background)
            .collect()
    }

    #[test]
    fn only_the_runs_showing_an_inlines_background_follow_it() {
        let source = || {
            node(
                "page",
                vec![node(
                    "a",
                    vec![text("x", "outer"), node("b", vec![text("y", "inner")])],
                )],
            )
        };
        let css = |a: &str, b: &str| {
            format!(
                "#a, #b {{ display:inline; }} #a {{ background-color:{a}; }}
                 #b {{ background-color:{b}; }}
                 #a:before {{ content:\"own\"; background-color:#ffffff; }}"
            )
        };
        // `#b` keeps its own white, the color `#a` goes to: it must not move with `#a`, and
        // neither must `#a`'s generated text with its own white.
        let from = BoxTree::new(source(), &css("#000000", "#ffffff")).unwrap();
        let to = from.restyled(&[&css("#ffffff", "#ffffff")]).unwrap();
        let mut animations = BoxTreeAnimations::new();
        let start = Instant::now();
        animations
            .request(&from, &to, "a", &["background-color"], second(), start)
            .unwrap();
        animations.advance(start + Duration::from_millis(500), &to);
        let shown = animations.presented(&to);
        assert!(near(
            shown_under(&shown, "outer")[0].unwrap(),
            [0.5, 0.5, 0.5]
        ));
        assert_eq!(shown_under(&shown, "inner"), vec![Some([1.0, 1.0, 1.0])]);
        assert_eq!(shown_under(&shown, "own"), vec![Some([1.0, 1.0, 1.0])]);
        // Both animating to the same color, in either order of request: each shows its own.
        for order in [["a", "b"], ["b", "a"]] {
            let from = BoxTree::new(source(), &css("#000000", "#ff0000")).unwrap();
            let to = from.restyled(&[&css("#ffffff", "#ffffff")]).unwrap();
            let mut animations = BoxTreeAnimations::new();
            for id in order {
                animations
                    .request(&from, &to, id, &["background-color"], second(), start)
                    .unwrap();
            }
            animations.advance(start + Duration::from_millis(500), &to);
            let shown = animations.presented(&to);
            assert!(
                near(shown_under(&shown, "outer")[0].unwrap(), [0.5, 0.5, 0.5]),
                "{order:?}"
            );
            assert!(
                near(shown_under(&shown, "inner")[0].unwrap(), [1.0, 0.5, 0.5]),
                "{order:?}"
            );
        }
    }

    #[test]
    fn generated_text_follows_and_a_nested_inlines_sides_stay() {
        let source = || {
            node(
                "page",
                vec![node(
                    "a",
                    vec![text("x", "outer"), node("b", vec![text("y", "inner")])],
                )],
            )
        };
        let css = |a: &str| {
            format!(
                "#a, #b {{ display:inline; }} #a {{ background-color:{a}; }}
                 #b {{ padding:3px; background-color:#ffffff; }}
                 #a:before {{ content:\"gen\"; }}"
            )
        };
        let from = BoxTree::new(source(), &css("#000000")).unwrap();
        let to = from.restyled(&[&css("#ffffff")]).unwrap();
        let mut animations = BoxTreeAnimations::new();
        let start = Instant::now();
        animations
            .request(&from, &to, "a", &["background-color"], second(), start)
            .unwrap();
        animations.advance(start + Duration::from_millis(500), &to);
        let shown = animations.presented(&to);
        // Generated text with no background of its own shows `#a`'s, and moves with it.
        assert!(near(
            shown_under(&shown, "gen")[0].unwrap(),
            [0.5, 0.5, 0.5]
        ));
        // `#b`'s padded box keeps its own white sides.
        let sides: Vec<Option<[f32; 3]>> = shown
            .text_leaves()
            .iter()
            .flat_map(|(_, leaf)| leaf.runs.iter())
            .flat_map(|run| run.inline_boxes.iter())
            .map(|sides| sides.background)
            .collect();
        assert!(!sides.is_empty());
        assert!(
            sides.iter().all(|color| *color == Some([1.0, 1.0, 1.0])),
            "{sides:?}"
        );
    }

    #[test]
    fn a_border_color_moves_only_where_its_side_has_a_width() {
        let start = Instant::now();
        for display in ["inline", "block"] {
            let source = || {
                node(
                    "page",
                    vec![node("p", vec![node("s", vec![text("x", "words")])])],
                )
            };
            let run = |from: &str, to: &str, properties: &[&str]| {
                let one =
                    BoxTree::new(source(), &format!("#s {{ display:{display}; {from} }}")).unwrap();
                let two = one
                    .restyled(&[&format!("#s {{ display:{display}; {to} }}")])
                    .unwrap();
                let mut animations = BoxTreeAnimations::new();
                animations
                    .request(&one, &two, "s", properties, second(), start)
                    .unwrap()
            };
            // No width at either end, by width or by style: nothing draws, so nothing runs.
            assert_eq!(
                run(
                    "border:0 solid transparent;",
                    "border:0 solid #ff0000;",
                    &["border-color"]
                ),
                0,
                "{display}"
            );
            assert_eq!(
                run(
                    "border:2px none transparent;",
                    "border:2px none #ff0000;",
                    &["border-color"]
                ),
                0,
                "{display}"
            );
            // A side growing from none fades its color in as it grows.
            assert_eq!(
                run(
                    "border:0 solid transparent;",
                    "border:2px solid #ff0000;",
                    &["border-color", "border-width"]
                ),
                8,
                "{display}"
            );
            // And fades it out as it shrinks to none.
            assert_eq!(
                run(
                    "border:2px solid #ff0000;",
                    "border:0 solid transparent;",
                    &["border-color", "border-width"]
                ),
                8,
                "{display}"
            );
            // A width still shrinking to none draws a color: a new one moves there, on that
            // side only.
            let thin = |color: &str| format!("#s {{ display:{display}; border:0 solid {color}; }}");
            let one = BoxTree::new(
                source(),
                &format!(
                    "#s {{ display:{display}; border:0 solid #ff0000; border-left-width:2px; }}"
                ),
            )
            .unwrap();
            let two = one.restyled(&[&thin("#ff0000")]).unwrap();
            let mut animations = BoxTreeAnimations::new();
            animations
                .request(&one, &two, "s", &["border-width"], second(), start)
                .unwrap();
            let later = start + Duration::from_millis(500);
            animations.advance(later, &two);
            let three = two.restyled(&[&thin("#0000ff")]).unwrap();
            assert_eq!(
                animations
                    .request(&two, &three, "s", &["border-color"], second(), later)
                    .unwrap(),
                1,
                "{display}"
            );
            // A width animating on another capture of the same id is not this one's.
            let other = BoxTree::new(source(), &thin("#ff0000")).unwrap();
            let blue = other.restyled(&[&thin("#0000ff")]).unwrap();
            let mut shared = BoxTreeAnimations::new();
            shared
                .request(&one, &two, "s", &["border-width"], second(), start)
                .unwrap();
            shared.advance(later, &two);
            assert_eq!(
                shared
                    .request(&other, &blue, "s", &["border-color"], second(), later)
                    .unwrap(),
                0,
                "{display}"
            );
            // Once it has none, at once again.
            animations.advance(start + Duration::from_millis(1500), &three);
            let four = three.restyled(&[&thin("#00ff00")]).unwrap();
            assert_eq!(
                animations
                    .request(
                        &three,
                        &four,
                        "s",
                        &["border-color"],
                        second(),
                        start + Duration::from_millis(1500)
                    )
                    .unwrap(),
                0,
                "{display}"
            );
            // One side with a width, the others none: that one.
            assert_eq!(
                run(
                    "border:0 solid transparent; border-left-width:2px;",
                    "border:0 solid #ff0000; border-left-width:2px;",
                    &["border-color"]
                ),
                1,
                "{display}"
            );
        }
    }

    #[test]
    fn an_interaction_styling_that_could_give_a_side_width_keeps_its_color_moving() {
        let start = Instant::now();
        let source = || {
            node(
                "page",
                vec![
                    node("before", vec![node("y", vec![])]),
                    node(
                        "p",
                        vec![node("x", vec![]), node("s", vec![text("t", "words")])],
                    ),
                    node("after", vec![]),
                ],
            )
        };
        let run = |rule: &str| {
            let css = |color: &str| format!("#s {{ border:0 solid {color}; }} {rule}");
            let one = BoxTree::new(source(), &css("transparent")).unwrap();
            let two = one.restyled(&[&css("#ff0000")]).unwrap();
            let mut animations = BoxTreeAnimations::new();
            animations
                .request(&one, &two, "s", &["border-color"], second(), start)
                .unwrap()
        };
        // On it, an ancestor, an earlier sibling, an ancestor's earlier sibling.
        for rule in [
            "#s:hover { border-width:2px; }",
            "#p:hover #s { border-width:2px; }",
            "#x:focus + #s { border-width:2px; }",
            "#before:active ~ #p #s { border-width:2px; }",
        ] {
            assert_eq!(run(rule), 4, "{rule}");
        }
        // One that can reach only elsewhere, or none: at once.
        for rule in [
            "#after:hover { border-width:2px; }",
            "#y:hover { border-width:2px; }",
            "",
        ] {
            assert_eq!(run(rule), 0, "{rule}");
        }
    }

    #[test]
    fn an_inlines_percentage_padding_above_moves_as_a_share() {
        let source = || node("p", vec![node("s", vec![text("t", "words")])]);
        let css = |padding: &str| format!("#s {{ display:inline; padding-top:{padding}; }}");
        let from = BoxTree::new(source(), &css("10%")).unwrap();
        let to = from.restyled(&[&css("30%")]).unwrap();
        let start = Instant::now();
        let mut animations = BoxTreeAnimations::new();
        let count = animations
            .request(&from, &to, "s", &["padding-top"], second(), start)
            .unwrap();
        assert_eq!(count, 1);
        animations.advance(start + Duration::from_millis(500), &to);
        let half = animations.presented(&to);
        // Halfway, a fifth of the block's width, sized once it is known.
        let share = half
            .text_leaves()
            .iter()
            .flat_map(|(_, leaf)| &leaf.runs)
            .flat_map(|run| &run.inline_boxes)
            .map(|decoration| decoration.padding_share[0])
            .next();
        assert_eq!(share, Some((0.0, 20.0)));
    }

    #[test]
    fn a_collapsed_tables_border_color_moves_on_its_outer_cells() {
        let source = || {
            node(
                "t",
                vec![node("r", vec![node("a", vec![]), node("b", vec![])])],
            )
        };
        let css = |color: &str| {
            format!(
                "#t {{ display:table; border-collapse:collapse; border:4px solid {color}; }}
                 #r {{ display:table-row; }}
                 #a, #b {{ display:table-cell; width:20px; height:20px;
                           border:1px solid #000000; }}"
            )
        };
        let from = BoxTree::new(source(), &css("#ff0000")).unwrap();
        let to = from.restyled(&[&css("#0000ff")]).unwrap();
        let start = Instant::now();
        let mut animations = BoxTreeAnimations::new();
        let count = animations
            .request(&from, &to, "t", &["border-color"], second(), start)
            .unwrap();
        assert_eq!(count, 4);
        animations.advance(start + Duration::from_millis(500), &to);
        let half = animations.presented(&to).paints()[2];
        // Its left side, the table's, halfway from red to blue; its right, its own, black.
        let left = half.border[3].unwrap();
        assert!(left[0] > 0.1 && left[2] > 0.1 && left[1] < 0.01, "{left:?}");
        assert_eq!(half.border[1], Some([0.0, 0.0, 0.0]));
        animations.advance(start + Duration::from_millis(1500), &to);
        let end = animations.presented(&to).paints()[2];
        assert_eq!(end.border[3], Some([0.0, 0.0, 1.0]));
    }

    #[test]
    fn a_boxs_border_fades_to_and_from_transparent() {
        let source = || node("page", vec![node("b", vec![text("x", "words")])]);
        let start = Instant::now();
        let run = |css: &dyn Fn(&str) -> String, ends: [&str; 2]| {
            let from = BoxTree::new(source(), &css(ends[0])).unwrap();
            let to = from.restyled(&[&css(ends[1])]).unwrap();
            let mut animations = BoxTreeAnimations::new();
            let count = animations
                .request(&from, &to, "b", &["border-color"], second(), start)
                .unwrap();
            animations.advance(start + Duration::from_millis(500), &to);
            let half = animations.presented(&to).paints()[1];
            animations.advance(start + Duration::from_millis(1500), &to);
            let end = animations.presented(&to).paints()[1];
            (count, half, end)
        };
        let css = |color: &str| format!("#b {{ border:4px solid {color}; }}");
        let red = [1.0, 0.0, 0.0];
        for ends in [["transparent", "#ff0000"], ["#ff0000", "transparent"]] {
            let (count, half, end) = run(&css, ends);
            assert_eq!(count, 4, "{ends:?}");
            // Halfway: the red at half, whichever way.
            assert_eq!(
                (half.border, half.border_fade),
                ([Some(red); 4], [Some(0.5); 4])
            );
            let last = if ends[1] == "transparent" {
                None
            } else {
                Some(red)
            };
            assert_eq!(
                (end.border, end.border_fade),
                ([last; 4], [None; 4]),
                "{ends:?}"
            );
        }
        // Turned back halfway: from the half, back to none.
        let clear = BoxTree::new(source(), &css("transparent")).unwrap();
        let full = clear.restyled(&[&css("#ff0000")]).unwrap();
        let mut animations = BoxTreeAnimations::new();
        animations
            .request(&clear, &full, "b", &["border-color"], second(), start)
            .unwrap();
        let later = start + Duration::from_millis(500);
        animations.advance(later, &full);
        animations
            .request(&full, &clear, "b", &["border-color"], second(), later)
            .unwrap();
        animations.advance(later + Duration::from_millis(500), &clear);
        let back = animations.presented(&clear).paints()[1];
        assert_eq!(
            (back.border[0], back.border_fade[0]),
            (Some(red), Some(0.25))
        );
        // One side alone.
        let one =
            |color: &str| format!("#b {{ border:4px solid red; border-left-color:{color}; }}");
        let (count, half, _) = run(&one, ["transparent", "#0000ff"]);
        assert_eq!(count, 1);
        assert_eq!(half.border_fade, [None, None, None, Some(0.5)]);
        // A side that draws nothing, by style or width, takes its color at once.
        let none = |color: &str| format!("#b {{ border:4px none {color}; }}");
        assert_eq!(run(&none, ["transparent", "#ff0000"]).0, 0);
        let thin = |color: &str| format!("#b {{ border:0 solid {color}; }}");
        assert_eq!(run(&thin, ["transparent", "#ff0000"]).0, 0);
        // A collapsed table's cells fade where their sides draw: not a side that gives way
        // along all of its edge to a wider neighbour.
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
        let table = |wide: &str, narrow: &str| {
            format!(
                "#t {{ display:table; border-collapse:collapse; }} #r {{ display:table-row; }}
                 #a {{ display:table-cell; border:4px solid {wide}; }}
                 #b {{ display:table-cell; border:2px solid {narrow}; }}"
            )
        };
        let collapsed = |id: &str, ends: [(&str, &str); 2]| {
            let one = BoxTree::new(source(), &table(ends[0].0, ends[0].1)).unwrap();
            let two = one.restyled(&[&table(ends[1].0, ends[1].1)]).unwrap();
            let mut animations = BoxTreeAnimations::new();
            let count = animations
                .request(&one, &two, id, &["border-color"], second(), start)
                .unwrap();
            animations.advance(start + Duration::from_millis(500), &two);
            (count, animations.presented(&two))
        };
        // The wide one wins the edge they share: all four of its sides fade.
        let (count, shown) = collapsed("a", [("transparent", "#000000"), ("#ff0000", "#000000")]);
        assert_eq!(count, 4);
        // Page, table, row, then the wide cell.
        assert_eq!(shown.paints()[3].border_fade, [Some(0.5); 4]);
        // The narrow one gives that edge way: three.
        let (count, shown) = collapsed("b", [("#000000", "transparent"), ("#000000", "#0000ff")]);
        assert_eq!(count, 3);
        let faded = shown
            .paints()
            .iter()
            .filter(|paint| paint.border_fade.iter().flatten().count() == 3)
            .count();
        assert_eq!(faded, 1);
        // Where the edge's winner changes, its color comes from the other cell: at once.
        let flip = |wide: &str, narrow: &str| {
            format!(
                "#t {{ display:table; border-collapse:collapse; }} #r {{ display:table-row; }}
                 #a {{ display:table-cell; border:{wide} solid #000000; }}
                 #b {{ display:table-cell; border:2px solid {narrow}; }}"
            )
        };
        let one = BoxTree::new(source(), &flip("4px", "transparent")).unwrap();
        let two = one.restyled(&[&flip("1px", "#0000ff")]).unwrap();
        let mut animations = BoxTreeAnimations::new();
        assert_eq!(
            animations
                .request(&one, &two, "b", &["border-color"], second(), start)
                .unwrap(),
            3
        );
    }

    #[test]
    fn an_inline_elements_border_colors_animate_on_its_line() {
        let source = || node("page", vec![node("s", vec![text("x", "words")])]);
        let css = |color: &str| format!("#s {{ display:inline; border:2px solid {color}; }}");
        let from = BoxTree::new(source(), &css("#000000")).unwrap();
        let to = from.restyled(&[&css("#ffffff")]).unwrap();
        let mut animations = BoxTreeAnimations::new();
        let start = Instant::now();
        let count = animations
            .request(&from, &to, "s", &["border-color"], second(), start)
            .unwrap();
        assert_eq!(count, 4);
        animations.advance(start + Duration::from_millis(500), &to);
        let shown = animations.presented(&to);
        let runs: Vec<&iced_css::box_tree::TextRun> = shown
            .text_leaves()
            .into_iter()
            .flat_map(|(_, leaf)| leaf.runs.iter())
            .collect();
        let mid = [0.5, 0.5, 0.5];
        // Top and bottom on its inline box's bands, left and right on its runs' bars.
        let bands: Vec<[f32; 3]> = runs
            .iter()
            .flat_map(|run| run.inline_boxes.iter())
            .flat_map(|sides| sides.border.iter().filter_map(|band| band.1))
            .collect();
        assert!(
            !bands.is_empty() && bands.iter().all(|color| near(*color, mid)),
            "{bands:?}"
        );
        let bars: Vec<(bool, [f32; 3])> = runs
            .iter()
            .filter_map(|run| run.edge.map(|(left, _, color)| (left, color)))
            .collect();
        assert_eq!(bars.len(), 2);
        assert!(bars.iter().all(|(_, color)| near(*color, mid)), "{bars:?}");
        // Its paint carries the presented colors too.
        let paint = shown.paints()[1];
        assert!(near(
            paint.under_text_border[0].or(paint.border[0]).unwrap(),
            mid
        ));
    }

    #[test]
    fn an_inline_elements_outline_animates_around_its_line() {
        let source = || {
            node(
                "page",
                vec![node("p", vec![node("s", vec![text("x", "words")])])],
            )
        };
        let css = |color: &str, width: &str| {
            format!("#s {{ display:inline; outline:{width} solid {color}; }}")
        };
        let from = BoxTree::new(source(), &css("#ff0000", "2px")).unwrap();
        let to = from.restyled(&[&css("#0000ff", "6px")]).unwrap();
        let mut animations = BoxTreeAnimations::new();
        let start = Instant::now();
        let count = animations
            .request(
                &from,
                &to,
                "s",
                &["outline-color", "outline-width"],
                second(),
                start,
            )
            .unwrap();
        assert_eq!(count, 2);
        animations.advance(start + Duration::from_millis(500), &to);
        let shown = animations.presented(&to);
        // Page, paragraph, then the span.
        let at = 2;
        assert_eq!(to.paints()[at].outline, Some((6.0, [0.0, 0.0, 1.0])));
        let (width, color) = shown.paints()[at].outline.unwrap();
        assert!((width - 4.0).abs() < 1e-3, "{width}");
        assert!(near(color, [0.5, 0.0, 0.5]), "{color:?}");
        // At the end, the end.
        animations.advance(start + Duration::from_millis(1500), &to);
        let shown = animations.presented(&to);
        assert_eq!(shown.paints()[at].outline, Some((6.0, [0.0, 0.0, 1.0])));
    }

    #[test]
    fn one_inline_border_side_animates_alone() {
        let source = || node("page", vec![node("s", vec![text("x", "words")])]);
        let css = |left: &str, top: &str| {
            format!(
                "#s {{ display:inline; border:2px solid #ff0000; border-left-color:{left};
                       border-top-color:{top}; border-bottom-color:#0000ff; }}"
            )
        };
        let shown = |from: &str, to: &str, property: &str| {
            let first = BoxTree::new(source(), from).unwrap();
            let later = first.restyled(&[to]).unwrap();
            let mut animations = BoxTreeAnimations::new();
            let start = Instant::now();
            assert_eq!(
                animations
                    .request(&first, &later, "s", &[property], second(), start)
                    .unwrap(),
                1
            );
            animations.advance(start + Duration::from_millis(500), &later);
            animations.presented(&later)
        };
        let runs = |tree: &BoxTree| -> Vec<iced_css::box_tree::TextRun> {
            tree.text_leaves()
                .into_iter()
                .flat_map(|(_, leaf)| leaf.runs.clone())
                .collect()
        };
        let (mid, red, blue, black) = ([0.5; 3], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0; 3]);
        // The left bar moves; the right bar and both bands stay.
        let tree = shown(
            &css("#000000", "#000000"),
            &css("#ffffff", "#000000"),
            "border-left-color",
        );
        let runs_now = runs(&tree);
        let bar = |left: bool| {
            runs_now
                .iter()
                .find_map(|run| run.edge.filter(|edge| edge.0 == left).map(|edge| edge.2))
                .unwrap()
        };
        assert!(near(bar(true), mid));
        assert!(near(bar(false), red));
        let bands: Vec<[f32; 3]> = runs_now
            .iter()
            .flat_map(|run| run.inline_boxes.iter())
            .flat_map(|sides| [sides.border[0].1.unwrap(), sides.border[1].1.unwrap()])
            .collect();
        assert!(
            bands
                .chunks(2)
                .all(|pair| near(pair[0], black) && near(pair[1], blue))
        );
        // The top band moves; the bottom band and both bars stay.
        let tree = shown(
            &css("#000000", "#000000"),
            &css("#000000", "#ffffff"),
            "border-top-color",
        );
        let runs_now = runs(&tree);
        for sides in runs_now.iter().flat_map(|run| run.inline_boxes.iter()) {
            assert!(near(sides.border[0].1.unwrap(), mid));
            assert!(near(sides.border[1].1.unwrap(), blue));
        }
        assert!(
            runs_now
                .iter()
                .filter_map(|run| run.edge)
                .all(|edge| near(edge.2, if edge.0 { black } else { red }))
        );
    }

    #[test]
    fn a_nested_inlines_own_borders_stay_while_its_parents_move() {
        let source = || {
            node(
                "page",
                vec![node(
                    "s",
                    vec![text("x", "outer"), node("t", vec![text("y", "inner")])],
                )],
            )
        };
        let css = |color: &str| {
            format!(
                "#s, #t {{ display:inline; }} #s {{ border:2px solid {color}; }}
                 #t {{ border:2px solid #ff0000; }}"
            )
        };
        let from = BoxTree::new(source(), &css("#000000")).unwrap();
        let to = from.restyled(&[&css("#ffffff")]).unwrap();
        let mut animations = BoxTreeAnimations::new();
        let start = Instant::now();
        animations
            .request(&from, &to, "s", &["border-color"], second(), start)
            .unwrap();
        animations.advance(start + Duration::from_millis(500), &to);
        let shown = animations.presented(&to);
        let red = [1.0, 0.0, 0.0];
        let mut reds = 0;
        for run in shown
            .text_leaves()
            .iter()
            .flat_map(|(_, leaf)| leaf.runs.iter())
        {
            for sides in &run.inline_boxes {
                for band in sides.border.iter().filter_map(|band| band.1) {
                    assert!(near(band, red) || near(band, [0.5; 3]), "{band:?}");
                    reds += usize::from(near(band, red));
                }
            }
            if let Some(edge) = run.edge {
                assert!(near(edge.2, red) || near(edge.2, [0.5; 3]), "{edge:?}");
                reds += usize::from(near(edge.2, red));
            }
        }
        // `#t`'s own bands kept their red.
        assert!(reds >= 2, "{reds}");
        // Each has two bars: `#t`'s stay red, `#s`'s move.
        let bars: Vec<[f32; 3]> = shown
            .text_leaves()
            .iter()
            .flat_map(|(_, leaf)| leaf.runs.iter())
            .filter_map(|run| run.edge.map(|edge| edge.2))
            .collect();
        assert_eq!(bars.len(), 4);
        assert_eq!(
            bars.iter().filter(|color| near(**color, red)).count(),
            2,
            "{bars:?}"
        );
    }

    #[test]
    fn a_fade_moves_between_a_box_and_an_inline() {
        let source = || node("page", vec![node("s", vec![text("x", "words")])]);
        let start = Instant::now();
        let layer = |tree: &BoxTree| {
            tree.text_leaves()
                .iter()
                .flat_map(|(_, leaf)| leaf.runs.iter())
                .find(|run| run.text.contains("words"))
                .and_then(|run| run.fades.last().map(|layer| (layer.1, layer.2)))
        };
        let halfway = |from: &str, to: &str, property: &str| {
            let from = BoxTree::new(source(), from).unwrap();
            let to = from.restyled(&[to]).unwrap();
            let mut animations = BoxTreeAnimations::new();
            let count = animations
                .request(&from, &to, "s", &[property], second(), start)
                .unwrap();
            animations.advance(start + Duration::from_millis(500), &to);
            (count, animations.presented(&to))
        };
        let white = [1.0, 1.0, 1.0];
        // `display` changes at once; the color moves as the new one paints it. From a
        // transparent box to an inline: a layer on its line.
        let (count, shown) = halfway(
            "#s { display:block; }",
            "#s { display:inline; background-color:#ffffff; }",
            "background-color",
        );
        assert_eq!(count, 1);
        assert_eq!(layer(&shown), Some((white, 0.5)));
        // From a transparent inline to a box: the box's background at half.
        let (count, shown) = halfway(
            "#s { display:inline; }",
            "#s { display:block; background-color:#ffffff; }",
            "background-color",
        );
        assert_eq!(count, 1);
        let paint = shown.paints()[1];
        assert_eq!(
            (paint.background, paint.background_fade),
            (Some(white), Some(0.5))
        );
        // From an opaque one, a color to a color, as before.
        let (count, _) = halfway(
            "#s { display:block; background-color:#000000; }",
            "#s { display:inline; background-color:#ffffff; }",
            "background-color",
        );
        assert_eq!(count, 1);
        // Borders too, either way.
        let (count, shown) = halfway(
            "#s { border:2px solid transparent; }",
            "#s { display:inline; border:2px solid #ffffff; }",
            "border-color",
        );
        assert_eq!(count, 4);
        let bars: Vec<f32> = shown
            .text_leaves()
            .iter()
            .flat_map(|(_, leaf)| leaf.runs.iter())
            .filter(|run| run.edge.is_some())
            .map(|run| run.edge_alpha)
            .collect();
        assert_eq!(bars, vec![0.5, 0.5]);
        let (count, shown) = halfway(
            "#s { display:inline; border:2px solid transparent; }",
            "#s { border:2px solid #ffffff; }",
            "border-color",
        );
        assert_eq!(count, 4);
        assert_eq!(shown.paints()[1].border_fade, [Some(0.5); 4]);
        // A box mid-fade that becomes an inline goes on from where it is.
        let bare = BoxTree::new(source(), "#s { display:block; }").unwrap();
        let shown_white = bare
            .restyled(&["#s { display:block; background-color:#ffffff; }"])
            .unwrap();
        let mut animations = BoxTreeAnimations::new();
        animations
            .request(
                &bare,
                &shown_white,
                "s",
                &["background-color"],
                second(),
                start,
            )
            .unwrap();
        let later = start + Duration::from_millis(500);
        animations.advance(later, &shown_white);
        let inline = shown_white
            .restyled(&["#s { display:inline; background-color:#000000; }"])
            .unwrap();
        let count = animations
            .request(
                &shown_white,
                &inline,
                "s",
                &["background-color"],
                second(),
                later,
            )
            .unwrap();
        assert_eq!(count, 1);
        animations.advance(later + Duration::from_millis(500), &inline);
        // Halfway from white at half to black: at three quarters, a third of the way gray.
        let (color, alpha) = layer(&animations.presented(&inline)).unwrap();
        assert!((alpha - 0.75).abs() < 1e-4, "{alpha}");
        assert!(near(color, [1.0 / 3.0; 3]), "{color:?}");
    }

    #[test]
    fn a_color_reaches_text_when_only_one_end_has_inline_sides() {
        let source = || node("page", vec![node("s", vec![text("x", "x")])]);
        let from = BoxTree::new(source(), "#s { display:inline; color:#000000; }").unwrap();
        let to = from
            .restyled(&["#s { display:inline; color:#ffffff; padding:0 8px; }"])
            .unwrap();
        let mut animations = BoxTreeAnimations::new();
        let start = Instant::now();
        animations
            .request(&from, &to, "s", &["color"], second(), start)
            .unwrap();
        animations.advance(start + Duration::from_millis(500), &to);
        let shown = animations.presented(&to);
        let leaves = shown.text_leaves();
        let text = leaves[0].1.runs.iter().find(|run| run.text == "x").unwrap();
        assert!(near(text.color.unwrap(), [0.5; 3]), "{:?}", text.color);
    }

    type Rgb = Option<[f32; 3]>;

    fn inline_sides(shown: &BoxTree) -> Vec<(Rgb, Rgb)> {
        let leaves = shown.text_leaves();
        let mut found = Vec::new();
        for run in &leaves[0].1.runs {
            for sides in &run.inline_boxes {
                found.push((sides.border[0].1, run.edge.map(|edge| edge.2)));
            }
        }
        found
    }

    #[test]
    fn inline_borders_follow_their_box_and_only_it() {
        // The text inside has its own color, and the border takes the box's.
        let source = || {
            node(
                "page",
                vec![node("s", vec![node("b", vec![text("x", "x")])])],
            )
        };
        let css = |color: &str| {
            format!(
                "#s {{ display:inline; color:{color}; border:2px solid; }}
                 #b {{ display:inline; color:#00ff00; }}"
            )
        };
        let from = BoxTree::new(source(), &css("#000000")).unwrap();
        let to = from.restyled(&[&css("#ffffff")]).unwrap();
        let mut animations = BoxTreeAnimations::new();
        let start = Instant::now();
        animations
            .request(&from, &to, "s", &["color"], second(), start)
            .unwrap();
        animations.advance(start + Duration::from_millis(500), &to);
        for (top, edge) in inline_sides(&animations.presented(&to)) {
            assert!(near(top.unwrap(), [0.5; 3]), "{top:?}");
            if let Some(edge) = edge {
                assert!(near(edge, [0.5; 3]), "{edge:?}");
            }
        }
        // A border with a color of its own stays put.
        let css = |color: &str| {
            format!("#s {{ display:inline; color:{color}; border:2px solid #ffffff; }}")
        };
        let source = || node("page", vec![node("s", vec![text("x", "x")])]);
        let from = BoxTree::new(source(), &css("#000000")).unwrap();
        let to = from.restyled(&[&css("#ffffff")]).unwrap();
        let mut animations = BoxTreeAnimations::new();
        animations
            .request(&from, &to, "s", &["color"], second(), start)
            .unwrap();
        animations.advance(start + Duration::from_millis(500), &to);
        for (top, edge) in inline_sides(&animations.presented(&to)) {
            assert_eq!(top, Some([1.0; 3]));
            if let Some(edge) = edge {
                assert_eq!(edge, [1.0; 3]);
            }
        }
    }

    #[test]
    fn a_generated_box_keeps_its_own_sides_while_its_element_animates() {
        let source = || node("page", vec![node("s", vec![text("x", "x")])]);
        let blue = [0.0, 0.0, 1.0];
        // The element's border follows its color, or has one of its own that moves.
        for (own, properties) in [(false, "color"), (true, "border-color")] {
            let css = |color: &str| {
                let border = if own { color } else { "" };
                format!(
                    "#s {{ display:inline; color:{color}; border:2px solid {border}; }}
                     #s:before {{ content:\"a\"; border:2px solid #0000ff; }}"
                )
            };
            let from = BoxTree::new(source(), &css("#000000")).unwrap();
            let to = from.restyled(&[&css("#ffffff")]).unwrap();
            let mut animations = BoxTreeAnimations::new();
            let start = Instant::now();
            animations
                .request(&from, &to, "s", &[properties], second(), start)
                .unwrap();
            animations.advance(start + Duration::from_millis(500), &to);
            let shown = animations.presented(&to);
            let leaves = shown.text_leaves();
            let (mut moved, mut kept) = (0, 0);
            for run in &leaves[0].1.runs {
                for sides in &run.inline_boxes {
                    let band = sides.border[0].1.unwrap();
                    if sides.owner > usize::MAX / 2 {
                        assert_eq!(band, blue, "{properties}");
                        kept += 1;
                    } else {
                        assert!(near(band, [0.5; 3]), "{properties} {band:?}");
                        moved += 1;
                    }
                }
                if let Some((_, _, edge)) = run.edge {
                    if run.inline_boxes.last().unwrap().owner > usize::MAX / 2 {
                        assert_eq!(edge, blue, "{properties}");
                        kept += 1;
                    } else {
                        assert!(near(edge, [0.5; 3]), "{properties} {edge:?}");
                        moved += 1;
                    }
                }
            }
            // Both bars of each box, and each box's band on its runs.
            assert!(moved > 2 && kept > 2, "{properties} {moved} {kept}");
        }
    }

    #[test]
    fn visibility_shows_all_the_way_while_either_end_shows() {
        // An inline with sides inside, text, and a child hidden in both stylings.
        let source = || {
            node(
                "page",
                vec![node(
                    "box",
                    vec![
                        text("t", "words"),
                        node(
                            "in",
                            vec![text("i", "inline"), node("keep", vec![text("k", "kept")])],
                        ),
                        node("off", vec![text("o", "never")]),
                    ],
                )],
            )
        };
        let css = |shown: &str| {
            format!(
                "#box {{ visibility:{shown}; }}
                 #box:before {{ content:\"\u{203A}\"; visibility:visible; }}
                 #box:after {{ content:\".\"; visibility:hidden; }}
                 #in {{ display:inline; padding:0 4px; background-color:#ff0000;
                        border-left:2px solid #000000; }}
                 #keep {{ display:inline; visibility:visible; }}
                 #in:after {{ content:\"\u{BB}\"; padding:0 4px; background-color:#00ff00;
                              border-left:2px solid #000000; }}
                 #off {{ visibility:hidden; }}"
            )
        };
        let shown = BoxTree::new(source(), &css("visible")).unwrap();
        let hidden = shown.restyled(&[&css("hidden")]).unwrap();
        // What shows: the box, its text and inline, and the inline's box; never `#off`.
        let state = |tree: &BoxTree| {
            let runs: Vec<(String, bool, bool)> = tree
                .text_leaves()
                .iter()
                .flat_map(|(_, leaf)| leaf.runs.iter())
                .filter(|run| !run.text.trim().is_empty())
                .map(|run| {
                    (
                        run.text.clone(),
                        run.visible,
                        run.inline_boxes.iter().any(|sides| sides.visible),
                    )
                })
                .collect();
            (tree.paints()[1].visible, tree.paints()[7].visible, runs)
        };
        let words = |shown: bool| ("words".to_owned(), shown, false);
        let never = ("never".to_owned(), false, false);
        // Fading out: still shown halfway, hidden at the end.
        let mut animations = BoxTreeAnimations::new();
        let start = Instant::now();
        let count = animations
            .request(&shown, &hidden, "box", &["visibility"], second(), start)
            .unwrap();
        assert_eq!(count, 1);
        animations.advance(start + Duration::from_millis(500), &hidden);
        let (own, off, runs) = state(&animations.presented(&hidden));
        assert!(own && !off);
        assert!(
            runs.contains(&words(true)) && runs.contains(&never),
            "{runs:?}"
        );
        assert!(
            runs.iter().any(|run| run.0 == "inline" && run.1),
            "{runs:?}"
        );
        // Generated content with a visibility of its own keeps it: the dot stays hidden.
        assert!(runs.contains(&(".".to_owned(), false, false)), "{runs:?}");
        animations.advance(start + Duration::from_millis(1500), &hidden);
        assert!(!animations.is_animating());
        let (own, _, runs) = state(&animations.presented(&hidden));
        assert!(!own && runs.contains(&words(false)));
        // Fading in after a delay: hidden through the delay, its inline's box too.
        let delayed = AnimationTiming::new(Duration::from_secs(1), Duration::from_secs(1));
        let mut animations = BoxTreeAnimations::new();
        animations
            .request(&hidden, &shown, "box", &["visibility"], delayed, start)
            .unwrap();
        animations.advance(start + Duration::from_millis(500), &shown);
        let waiting = animations.presented(&shown);
        let (own, off, _) = state(&waiting);
        assert!(!own && !off);
        // Every run, side spaces too: no text, sides, bars or background; but the
        // `:before` shown at both ends stays shown.
        for (_, leaf) in waiting.text_leaves() {
            for run in &leaf.runs {
                if run.text.contains('\u{203A}') {
                    assert!(run.visible);
                    continue;
                }
                // Shown at both ends, inside the hidden inline: no box or background of it.
                if run.text.contains("kept") {
                    assert!(run.visible);
                    assert!(run.inline_boxes.iter().all(|sides| !sides.visible));
                    assert!(run.background.is_none());
                    continue;
                }
                assert!(!run.visible, "{:?}", run.text);
                assert!(
                    run.inline_boxes.iter().all(|sides| !sides.visible),
                    "{:?}",
                    run.text
                );
                assert_eq!(run.background, None, "{:?}", run.text);
            }
        }
        // Then shown as soon as it moves.
        animations.advance(start + Duration::from_millis(1100), &shown);
        let (own, _, runs) = state(&animations.presented(&shown));
        assert!(own && runs.contains(&words(true)) && runs.contains(&never));
        assert!(
            runs.iter().any(|run| run.0 == "inline" && run.1 && run.2),
            "{runs:?}"
        );
        // A descendant that changes on its own applies at once.
        let css = |outer: &str, inner: &str| {
            format!("#box {{ visibility:{outer}; }} #off {{ visibility:{inner}; }}")
        };
        let from = BoxTree::new(source(), &css("hidden", "visible")).unwrap();
        let to = from.restyled(&[&css("visible", "hidden")]).unwrap();
        let mut animations = BoxTreeAnimations::new();
        animations
            .request(&from, &to, "box", &["visibility"], delayed, start)
            .unwrap();
        animations.advance(start + Duration::from_millis(500), &to);
        let waiting = animations.presented(&to);
        assert!(!waiting.paints()[1].visible && !waiting.paints()[7].visible);
        animations.advance(start + Duration::from_millis(1100), &to);
        let moving = animations.presented(&to);
        assert!(moving.paints()[1].visible && !moving.paints()[7].visible);
        // Nothing to do when it does not change.
        let mut animations = BoxTreeAnimations::new();
        assert_eq!(
            animations
                .request(&shown, &shown, "box", &["visibility"], second(), start)
                .unwrap(),
            0
        );
    }

    #[test]
    fn font_size_scales_what_keeps_its_share_of_it() {
        let source = || {
            node(
                "page",
                vec![node(
                    "box",
                    vec![
                        text("t", "words"),
                        node("half", vec![text("h", "half")]),
                        node("fixed", vec![text("f", "fixed")]),
                    ],
                )],
            )
        };
        let css = |size: &str| {
            format!(
                "#box {{ font-size:{size}; }}
                 #half {{ display:inline; font-size:0.5em; }}
                 #fixed {{ display:inline; font-size:10px; }}"
            )
        };
        let from = BoxTree::new(source(), &css("16px")).unwrap();
        let to = from.restyled(&[&css("32px")]).unwrap();
        let mut animations = BoxTreeAnimations::new();
        let start = Instant::now();
        let count = animations
            .request(&from, &to, "box", &["font-size"], second(), start)
            .unwrap();
        assert_eq!(count, 1);
        animations.advance(start + Duration::from_millis(500), &to);
        let shown = animations.presented(&to);
        let size_of = |text: &str| {
            shown
                .text_leaves()
                .iter()
                .flat_map(|(_, leaf)| leaf.runs.iter())
                .find(|run| run.text.trim() == text)
                .map(|run| run.size)
        };
        // Halfway from 16 to 32: 24, its half 12, and a size of its own stays.
        assert_eq!(size_of("words"), Some(24.0));
        assert_eq!(size_of("half"), Some(12.0));
        assert_eq!(size_of("fixed"), Some(10.0));
        animations.advance(start + Duration::from_millis(1500), &to);
        assert!(!animations.is_animating());
        let done = animations.presented(&to);
        let leaves = done.text_leaves();
        let words = leaves
            .iter()
            .flat_map(|(_, leaf)| leaf.runs.iter())
            .find(|run| run.text.trim() == "words")
            .unwrap();
        assert_eq!(words.size, 32.0);
        // A follower that moves on without a request keeps its new size.
        let changed = to
            .restyled(&[&css("32px"), "#half { font-size:20px; }"])
            .unwrap();
        let mut animations = BoxTreeAnimations::new();
        animations
            .request(&from, &to, "box", &["font-size"], second(), start)
            .unwrap();
        animations.advance(start + Duration::from_millis(500), &changed);
        let shown = animations.presented(&changed);
        let half = shown
            .text_leaves()
            .iter()
            .flat_map(|(_, leaf)| leaf.runs.iter())
            .find(|run| run.text.trim() == "half")
            .map(|run| run.size);
        assert_eq!(half, Some(20.0));
    }

    #[test]
    fn a_font_size_to_zero_keeps_sizes_finite() {
        let source = || node("page", vec![node("box", vec![text("t", "words")])]);
        let from = BoxTree::new(source(), "#box { font-size:16px; }").unwrap();
        let to = from.restyled(&["#box { font-size:0px; }"]).unwrap();
        let mut animations = BoxTreeAnimations::new();
        let start = Instant::now();
        // Nothing would move in between: it applies at once, with no track left running.
        let count = animations
            .request(&from, &to, "box", &["font-size"], second(), start)
            .unwrap();
        assert_eq!(count, 0);
        assert!(!animations.is_animating());
        animations.advance(start + Duration::from_millis(500), &to);
        let shown = animations.presented(&to);
        for (_, leaf) in shown.text_leaves() {
            for run in &leaf.runs {
                assert!(run.size.is_finite(), "{:?}", run.size);
            }
        }
        // And from 0 as well.
        let count = animations
            .request(&to, &from, "box", &["font-size"], second(), start)
            .unwrap();
        assert_eq!(count, 0);
        assert!(!animations.is_animating());
    }

    #[test]
    fn a_refused_timing_changes_nothing() {
        let from = BoxTree::new(
            node("page", vec![node("box", vec![])]),
            "#box { width:10px; }",
        )
        .unwrap();
        let to = from.restyled(&["#box { width:30px; }"]).unwrap();
        let mut animations = BoxTreeAnimations::new();
        let start = Instant::now();
        let refused = AnimationTiming::new(Duration::MAX, Duration::ZERO);
        assert!(
            animations
                .request(&from, &to, "box", &["width"], refused, start)
                .is_err()
        );
        assert!(!animations.is_animating());
        // A later request on the same element still starts from scratch.
        let count = animations
            .request(&from, &to, "box", &["width"], second(), start)
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn nested_font_size_requests_scale_once() {
        let source = || {
            node(
                "page",
                vec![node(
                    "parent",
                    vec![node("child", vec![text("c", "child")])],
                )],
            )
        };
        let css = |size: &str| {
            format!(
                "#parent {{ font-size:{size}; }}
                 #child {{ font-size:0.5em; padding-top:1em; }}
                 #parent:before {{ content:\"*\"; font-size:12px; }}
                 #parent:after {{ content:\"*\"; }}"
            )
        };
        let from = BoxTree::new(source(), &css("16px")).unwrap();
        let to = from.restyled(&[&css("32px")]).unwrap();
        let start = Instant::now();
        let sizes = |order: [&str; 2]| {
            let mut animations = BoxTreeAnimations::new();
            for id in order {
                animations
                    .request(&from, &to, id, &["font-size"], second(), start)
                    .unwrap();
            }
            animations.advance(start + Duration::from_millis(500), &to);
            let shown = animations.presented(&to);
            let geometry = shown.layout(800.0, None).unwrap();
            let child = geometry[2].as_ref().unwrap();
            // Its padding of 1em, 8 to 16: 12 halfway, scaled once.
            assert_eq!(child.content.top - child.padding.top, 12.0, "{order:?}");
            let leaves = shown.text_leaves();
            let runs: Vec<(String, f32)> = leaves
                .iter()
                .flat_map(|(_, leaf)| leaf.runs.iter())
                .filter(|run| !run.spacer)
                .map(|run| (run.text.clone(), run.size))
                .collect();
            runs
        };
        // The child halfway from 8 to 16 is 12, whichever request came first; the `:before`
        // keeps its 12 pixels, the `:after` with the same text follows to 24.
        for order in [["child", "parent"], ["parent", "child"]] {
            let runs = sizes(order);
            assert!(
                runs.contains(&("child".to_owned(), 12.0)),
                "{order:?} {runs:?}"
            );
            assert!(runs.contains(&("*".to_owned(), 12.0)), "{order:?} {runs:?}");
            assert!(runs.contains(&("*".to_owned(), 24.0)), "{order:?} {runs:?}");
        }
    }

    #[test]
    fn an_em_length_animated_of_its_own_keeps_its_animation() {
        let css = |size: &str| format!("#box {{ font-size:{size}; width:10em; }}");
        let from = tree(&css("16px"));
        let to = from.restyled(&[&css("32px")]).unwrap();
        let mut animations = BoxTreeAnimations::new();
        let start = Instant::now();
        animations
            .request(&from, &to, "box", &["font-size", "width"], second(), start)
            .unwrap();
        animations.advance(start + Duration::from_millis(500), &to);
        let geometry = animations.presented(&to).layout(800.0, None).unwrap();
        let moved = geometry[1].as_ref().unwrap();
        // 160 to 320 by its own animation: 240, not scaled again with the size.
        assert_eq!(moved.border.right - moved.border.left, 240.0);
    }

    #[test]
    fn em_lengths_another_styling_moves_keep_its_value() {
        let css = |size: &str| {
            format!(
                "#box {{ font-size:{size}; width:10em; }}
                 #kid {{ line-height:1.5em; text-indent:1em; }}"
            )
        };
        let source = || {
            node(
                "page",
                vec![node("box", vec![node("kid", vec![text("t", "words")])])],
            )
        };
        let from = BoxTree::new(source(), &css("16px")).unwrap();
        let to = from.restyled(&[&css("32px")]).unwrap();
        let mut animations = BoxTreeAnimations::new();
        let start = Instant::now();
        animations
            .request(&from, &to, "box", &["font-size"], second(), start)
            .unwrap();
        animations.advance(start + Duration::from_millis(500), &to);
        // Moved on to pixels of its own, as a hover styling can: they stay.
        let moved = to
            .restyled(&[
                &css("32px"),
                "#box { width:200px; } #kid { line-height:30px; text-indent:10px; }",
            ])
            .unwrap();
        let shown = animations.presented(&moved);
        let geometry = shown.layout(800.0, None).unwrap();
        let wide = geometry[1].as_ref().unwrap();
        assert_eq!(wide.border.right - wide.border.left, 200.0);
        let leaves = shown.text_leaves();
        assert_eq!(
            leaves[0].1.line_height,
            iced_css::evaluation::tree_layout::TextLineHeight::Px(30.0)
        );
        let indent = |tree: &BoxTree| {
            tree.text_leaves()[0]
                .1
                .runs
                .iter()
                .find(|run| run.spacer && run.side_of.is_none())
                .and_then(|run| run.percent_room)
                .map(|room| room.0)
        };
        assert_eq!(indent(&shown), Some(10.0));
        // Unmoved, they follow the size: 240 wide, 36 high lines.
        let shown = animations.presented(&to);
        let geometry = shown.layout(800.0, None).unwrap();
        let wide = geometry[1].as_ref().unwrap();
        assert_eq!(wide.border.right - wide.border.left, 240.0);
        assert_eq!(
            shown.text_leaves()[0].1.line_height,
            iced_css::evaluation::tree_layout::TextLineHeight::Px(36.0)
        );
        // Its indent of 1em, 16 to 32: 24 halfway.
        assert_eq!(indent(&shown), Some(24.0));
    }

    #[test]
    fn font_weight_animates_in_steps_through_what_inherits_it() {
        let source = || {
            node(
                "page",
                vec![
                    node(
                        "li",
                        vec![
                            text("a", "own "),
                            node("b", vec![text("c", "inherits ")]),
                            node("d", vec![text("e", "bolder")]),
                            node("g", vec![text("h", "fixed")]),
                        ],
                    ),
                    node("other", vec![text("f", "sibling")]),
                ],
            )
        };
        let css = |weight: &str| {
            format!(
                "#li {{ display:list-item; font-weight:{weight}; }}
                 #b {{ display:inline; }}
                 #d {{ display:inline; font-weight:bolder; }}
                 #g {{ display:inline; font-weight:800; }}
                 #other {{ font-weight:{weight}; }}"
            )
        };
        let from = BoxTree::new(source(), &css("400")).unwrap();
        let to = from.restyled(&[&css("800")]).unwrap();
        let mut animations = BoxTreeAnimations::new();
        let start = Instant::now();
        let count = animations
            .request(&from, &to, "li", &["font-weight"], second(), start)
            .unwrap();
        assert_eq!(count, 1);
        let weights = |tree: &BoxTree| {
            let leaves = tree.text_leaves();
            let of = |text: &str| {
                leaves
                    .iter()
                    .flat_map(|(_, leaf)| leaf.runs.iter())
                    .find(|run| run.text.contains(text))
                    .unwrap()
                    .weight
            };
            let marker = tree.markers().values().next().unwrap().runs[0].weight;
            (
                of("own"),
                of("inherits"),
                of("bolder"),
                of("fixed"),
                of("sibling"),
                marker,
            )
        };
        // The fixed one is 800 at both ends, and the sibling moves with no request.
        assert_eq!(weights(&from), (400, 400, 700, 800, 400, 400));
        assert_eq!(weights(&to), (800, 800, 900, 800, 800, 800));
        // A quarter of the way: 500. Halfway: 600. The bolder one has its own two ends.
        animations.advance(start + Duration::from_millis(250), &to);
        assert_eq!(
            weights(&animations.presented(&to)),
            (500, 500, 900, 800, 800, 500)
        );
        // In steps of 100, to the nearest: a third of the way is 500, not 533, and
        // two fifths 600, not 560.
        animations.advance(start + Duration::from_millis(333), &to);
        assert_eq!(weights(&animations.presented(&to)).0, 500);
        animations.advance(start + Duration::from_millis(400), &to);
        assert_eq!(weights(&animations.presented(&to)).0, 600);
        animations.advance(start + Duration::from_millis(500), &to);
        assert_eq!(
            weights(&animations.presented(&to)),
            (600, 600, 900, 800, 800, 600)
        );
        // A descendant moved on to a weight of its own, as a hover styling can: it stays.
        let moved = to
            .restyled(&[&css("800"), "#b { display:inline; font-weight:300; }"])
            .unwrap();
        assert_eq!(weights(&animations.presented(&moved)).1, 300);
        assert_eq!(weights(&animations.presented(&moved)).0, 600);
        animations.advance(start + Duration::from_millis(1500), &to);
        assert_eq!(weights(&animations.presented(&to)), weights(&to));
    }

    #[test]
    fn a_weight_animates_only_runs_that_show_it_at_both_ends() {
        let start = Instant::now();
        let weight = |tree: &BoxTree, text: &str| {
            tree.text_leaves()
                .iter()
                .flat_map(|(_, leaf)| leaf.runs.iter())
                .find(|run| run.text.contains(text))
                .unwrap_or_else(|| panic!("{text}"))
                .weight
        };
        let halfway =
            |source: &dyn Fn() -> Element, css: &dyn Fn(&str) -> String, ends: [&str; 2]| {
                let from = BoxTree::new(source(), &css(ends[0])).unwrap();
                let to = from.restyled(&[&css(ends[1])]).unwrap();
                let mut animations = BoxTreeAnimations::new();
                animations
                    .request(&from, &to, "p", &["font-weight"], second(), start)
                    .unwrap();
                animations.advance(start + Duration::from_millis(500), &to);
                (from, animations.presented(&to), to)
            };
        let source = || node("page", vec![node("p", vec![text("t", "words")])]);
        // A `:before` with a weight of its own keeps it; one `lighter` than its element,
        // 700 then 100, takes its new weight at once.
        let css = |weight: &str| {
            format!(
                "#p {{ font-weight:{weight}; }}
                 #p:before {{ content:\"FIX\"; font-weight:800; }}
                 #p:after {{ content:\"LIGHT\"; font-weight:lighter; }}"
            )
        };
        let (from, shown, to) = halfway(&source, &css, ["900", "100"]);
        assert_eq!((weight(&from, "LIGHT"), weight(&to, "LIGHT")), (700, 100));
        assert_eq!(weight(&shown, "words"), 500);
        assert_eq!(weight(&shown, "FIX"), 800);
        assert_eq!(weight(&shown, "LIGHT"), 100);
        // Runs with the same text: by owner, and by order among the same owner's.
        let source = || {
            node(
                "page",
                vec![
                    node(
                        "p",
                        vec![node("b", vec![text("c", "same ")]), text("a", "same ")],
                    ),
                    node("other", vec![text("d", "sibling")]),
                ],
            )
        };
        let css = |weight: &str| {
            format!(
                "#p {{ font-weight:{weight}; }}
                 #b {{ display:inline; font-weight:800; }}
                 #p:before {{ content:\"X\"; font-weight:800; }}
                 #p:after {{ content:\"X\"; }}
                 #other {{ display:list-item; font-weight:{weight}; }}"
            )
        };
        let (_, shown, _) = halfway(&source, &css, ["400", "800"]);
        let runs: Vec<(String, u16)> = shown
            .text_leaves()
            .iter()
            .flat_map(|(_, leaf)| leaf.runs.iter())
            .filter(|run| run.text.contains("same") || run.text.contains('X'))
            .map(|run| (run.text.trim().to_owned(), run.weight))
            .collect();
        let pair = |text: &str, weight: u16| (text.to_owned(), weight);
        assert_eq!(
            runs,
            vec![
                pair("X", 800),
                pair("same", 800),
                pair("same", 600),
                pair("X", 600)
            ]
        );
        // A list item outside it, moving with no request, has its marker at the new weight.
        let markers: Vec<u16> = shown
            .markers()
            .values()
            .map(|marker| marker.runs[0].weight)
            .collect();
        assert_eq!(markers, vec![800]);
        // A side's spaces go with the box they are a side of, not with their owner's.
        let source = || {
            node(
                "page",
                vec![
                    node(
                        "p",
                        vec![text("a", "moves "), node("g", vec![text("b", "fixed")])],
                    ),
                    node("q", vec![node("m", vec![text("c", "beside")])]),
                ],
            )
        };
        let css = |weight: &str| {
            format!(
                "#p {{ font-weight:{weight}; }}
                 #g {{ display:inline; font-weight:800; margin-left:20px; }}
                 #m {{ display:inline; margin-left:20px; }}"
            )
        };
        let (_, shown, _) = halfway(&source, &css, ["400", "800"]);
        let sides: Vec<(usize, u16)> = shown
            .text_leaves()
            .iter()
            .flat_map(|(_, leaf)| leaf.runs.iter())
            .filter_map(|run| run.side_of.map(|side| (side.0, run.weight)))
            .collect();
        assert_eq!(weight(&shown, "moves"), 600);
        assert_eq!(weight(&shown, "fixed"), 800);
        // By source node: #g is 2, its spaces 800 with it; #m is 5, outside, 400.
        assert_eq!(sides, vec![(2, 800), (5, 400)]);
        // The other way: an animated inline's sides, inside a block that keeps its weight.
        let css = |weight: &str| {
            format!(
                "#m {{ display:inline; margin-left:20px; font-weight:{weight}; }}
                 #g {{ display:inline; margin-left:20px; }}"
            )
        };
        let from = BoxTree::new(source(), &css("400")).unwrap();
        let to = from.restyled(&[&css("800")]).unwrap();
        let mut animations = BoxTreeAnimations::new();
        animations
            .request(&from, &to, "m", &["font-weight"], second(), start)
            .unwrap();
        animations.advance(start + Duration::from_millis(500), &to);
        let shown = animations.presented(&to);
        let mine: Vec<u16> = shown
            .text_leaves()
            .iter()
            .flat_map(|(_, leaf)| leaf.runs.iter())
            .filter(|run| run.side_of.is_some_and(|side| side.0 == 5))
            .map(|run| run.weight)
            .collect();
        assert_eq!(weight(&shown, "beside"), 600);
        assert!(
            !mine.is_empty() && mine.iter().all(|weight| *weight == 600),
            "{mine:?}"
        );
        // A descendant animating its weight itself keeps its own timing, whichever asks
        // first: its text, its sides' spaces and its marker. One animating only its color
        // follows; a list item with a weight of its own keeps its marker's.
        let source = || {
            node(
                "page",
                vec![node(
                    "p",
                    vec![
                        text("a", "outer "),
                        node("b", vec![text("c", "inner")]),
                        node("e", vec![text("f", "colored")]),
                        node("l", vec![text("g", "item")]),
                        node("k", vec![text("h", "kept")]),
                    ],
                )],
            )
        };
        let css = |weight: &str, color: &str| {
            format!(
                "#p {{ font-weight:{weight}; }}
                 #b {{ display:inline; font-weight:{weight}; margin-left:20px; }}
                 #e {{ display:inline; color:{color}; }}
                 #l {{ display:list-item; font-weight:{weight}; }}
                 #k {{ display:list-item; font-weight:800; }}"
            )
        };
        let from = BoxTree::new(source(), &css("400", "#ff0000")).unwrap();
        let to = from.restyled(&[&css("800", "#0000ff")]).unwrap();
        for order in [["p", "b", "l"], ["b", "l", "p"]] {
            let mut animations = BoxTreeAnimations::new();
            for id in order {
                let timing = if id == "p" {
                    second()
                } else {
                    AnimationTiming::new(Duration::from_secs(4), Duration::ZERO)
                };
                animations
                    .request(&from, &to, id, &["font-weight"], timing, start)
                    .unwrap();
            }
            animations
                .request(&from, &to, "e", &["color"], second(), start)
                .unwrap();
            animations.advance(start + Duration::from_millis(500), &to);
            let shown = animations.presented(&to);
            let sides: Vec<u16> = shown
                .text_leaves()
                .iter()
                .flat_map(|(_, leaf)| leaf.runs.iter())
                .filter(|run| run.side_of.is_some())
                .map(|run| run.weight)
                .collect();
            let markers: Vec<u16> = shown
                .markers()
                .values()
                .map(|marker| marker.runs[0].weight)
                .collect();
            // Its own eighth of the way: 450, to 500.
            assert_eq!(
                (
                    weight(&shown, "outer"),
                    weight(&shown, "inner"),
                    weight(&shown, "colored"),
                    weight(&shown, "item"),
                    weight(&shown, "kept"),
                ),
                (600, 500, 600, 500, 800),
                "{order:?}"
            );
            assert!(
                !sides.is_empty() && sides.iter().all(|w| *w == 500),
                "{sides:?}"
            );
            assert_eq!(markers, vec![500, 800], "{order:?}");
        }
        // Runs shifted by a styling that shows another element: that one keeps its own.
        let source = || {
            node(
                "page",
                vec![node(
                    "p",
                    vec![node("z", vec![text("y", "zed ")]), text("a", "words")],
                )],
            )
        };
        let css = |weight: &str| format!("#p {{ font-weight:{weight}; }} #z {{ display:none; }}");
        let from = BoxTree::new(source(), &css("400")).unwrap();
        let to = from.restyled(&[&css("800")]).unwrap();
        let mut animations = BoxTreeAnimations::new();
        animations
            .request(&from, &to, "p", &["font-weight"], second(), start)
            .unwrap();
        animations.advance(start + Duration::from_millis(500), &to);
        let moved = to
            .restyled(&[&css("800"), "#z { display:inline; font-weight:800; }"])
            .unwrap();
        let shown = animations.presented(&moved);
        assert_eq!((weight(&shown, "zed"), weight(&shown, "words")), (800, 600));
    }

    #[test]
    fn text_beside_a_negative_margin_follows_whatever_its_space() {
        let source = || {
            node(
                "page",
                vec![node(
                    "p",
                    vec![text("a", "before "), node("s", vec![text("t", "in")])],
                )],
            )
        };
        let start = Instant::now();
        let before = |tree: &BoxTree| {
            tree.text_leaves()
                .iter()
                .flat_map(|(_, leaf)| leaf.runs.iter())
                .find(|run| run.text.starts_with("before"))
                .cloned()
                .unwrap()
        };
        // Its space narrows to nothing at 16 pixels, to a sixth of an em at 32: the size
        // still moves it.
        let css = |size: &str| {
            format!("#p {{ font-size:{size}; }} #s {{ display:inline; margin-left:-2px; }}")
        };
        let from = BoxTree::new(source(), &css("16px")).unwrap();
        let to = from.restyled(&[&css("32px")]).unwrap();
        assert_ne!(before(&from).text, before(&to).text);
        let mut animations = BoxTreeAnimations::new();
        animations
            .request(&from, &to, "p", &["font-size"], second(), start)
            .unwrap();
        animations.advance(start + Duration::from_millis(500), &to);
        assert_eq!(before(&animations.presented(&to)).size, 24.0);
        // Its space there at one end, gone at the other: the color still moves it.
        let css = |color: &str, margin: &str| {
            format!("#p {{ color:{color}; }} #s {{ display:inline; margin-left:{margin}; }}")
        };
        let from = BoxTree::new(source(), &css("#000000", "-8px")).unwrap();
        let to = from.restyled(&[&css("#ffffff", "0")]).unwrap();
        assert_ne!(before(&from).text, before(&to).text);
        let mut animations = BoxTreeAnimations::new();
        animations
            .request(&from, &to, "p", &["color"], second(), start)
            .unwrap();
        animations.advance(start + Duration::from_millis(500), &to);
        let color = before(&animations.presented(&to)).color.unwrap();
        assert!(near(color, [0.5; 3]), "{color:?}");
        // And the weight.
        let css = |weight: &str, margin: &str| {
            format!("#p {{ font-weight:{weight}; }} #s {{ display:inline; margin-left:{margin}; }}")
        };
        let from = BoxTree::new(source(), &css("400", "-8px")).unwrap();
        let to = from.restyled(&[&css("800", "0")]).unwrap();
        let mut animations = BoxTreeAnimations::new();
        animations
            .request(&from, &to, "p", &["font-weight"], second(), start)
            .unwrap();
        animations.advance(start + Duration::from_millis(500), &to);
        assert_eq!(before(&animations.presented(&to)).weight, 600);
        // An empty `:after` is not counted among side spaces that one end has and the other
        // does not: it still follows the size.
        let source = || {
            node(
                "page",
                vec![node(
                    "p",
                    vec![node(
                        "e",
                        vec![text("a", "words "), node("c", vec![text("b", "child")])],
                    )],
                )],
            )
        };
        let css = |size: &str, margin: &str| {
            format!(
                "#p {{ font-size:{size}; }} #e {{ display:inline; }} #e:after {{ content:\"\"; }}
                 #c {{ display:inline; margin-left:{margin}; }}"
            )
        };
        let from = BoxTree::new(source(), &css("16px", "0")).unwrap();
        let to = from.restyled(&[&css("32px", "8px")]).unwrap();
        let mut animations = BoxTreeAnimations::new();
        animations
            .request(&from, &to, "p", &["font-size"], second(), start)
            .unwrap();
        animations.advance(start + Duration::from_millis(500), &to);
        let shown = animations.presented(&to);
        let sizes: Vec<(String, f32)> = shown
            .text_leaves()
            .iter()
            .flat_map(|(_, leaf)| leaf.runs.iter())
            .filter(|run| !run.spacer)
            .map(|run| (run.text.clone(), run.size))
            .collect();
        assert!(sizes.iter().all(|(_, size)| *size == 24.0), "{sizes:?}");
    }

    #[test]
    fn a_before_or_after_animates_its_own_colors_and_visibility() {
        let source = || {
            node(
                "page",
                vec![node("p", vec![node("s", vec![text("t", "words")])])],
            )
        };
        let start = Instant::now();
        let halfway = |css: &dyn Fn(&str) -> String, ends: [&str; 2], id: &str, property: &str| {
            let from = BoxTree::new(source(), &css(ends[0])).unwrap();
            let to = from.restyled(&[&css(ends[1])]).unwrap();
            let mut animations = BoxTreeAnimations::new();
            let count = animations
                .request(&from, &to, id, &[property], second(), start)
                .unwrap();
            animations.advance(start + Duration::from_millis(500), &to);
            (count, animations.presented(&to))
        };
        let run = |tree: &BoxTree, text: &str| {
            tree.text_leaves()
                .iter()
                .flat_map(|(_, leaf)| leaf.runs.iter())
                .find(|run| run.text.contains(text))
                .cloned()
                .unwrap()
        };
        let mid = [0.5, 0.0, 0.5];
        // Its color, named as `s::before`; the element's own text keeps its color.
        let css = |color: &str| {
            format!(
                "#s {{ display:inline; color:#000000; }} #s:before {{ content:\"GEN\"; color:{color}; }}"
            )
        };
        let (count, shown) = halfway(&css, ["#ff0000", "#0000ff"], "s::before", "color");
        assert_eq!(count, 1);
        assert!(near(run(&shown, "GEN").color.unwrap(), mid));
        assert_eq!(run(&shown, "words").color, Some([0.0; 3]));
        // The element's own request leaves it be.
        let (count, _) = halfway(&css, ["#ff0000", "#0000ff"], "s", "color");
        assert_eq!(count, 0);
        // `:after`, with one colon too.
        let after = |color: &str| {
            format!("#s {{ display:inline; }} #s:after {{ content:\"END\"; color:{color}; }}")
        };
        let (count, shown) = halfway(&after, ["#ff0000", "#0000ff"], "s:after", "color");
        assert_eq!(count, 1);
        assert!(near(run(&shown, "END").color.unwrap(), mid));
        // Moved on to a color of its own, as a hover styling can: it stays.
        let from = BoxTree::new(source(), &css("#ff0000")).unwrap();
        let to = from.restyled(&[&css("#0000ff")]).unwrap();
        let mut animations = BoxTreeAnimations::new();
        animations
            .request(&from, &to, "s::before", &["color"], second(), start)
            .unwrap();
        animations.advance(start + Duration::from_millis(500), &to);
        let moved = to
            .restyled(&[&css("#0000ff"), "#s:before { color:#00ff00; }"])
            .unwrap();
        assert_eq!(
            run(&animations.presented(&moved), "GEN").color,
            Some([0.0, 1.0, 0.0])
        );
        // Its element's `!important` is not its own.
        let important = |color: &str| {
            format!(
                "#s {{ display:inline; color:#000000 !important; }} #s:before {{ content:\"GEN\"; color:{color}; }}"
            )
        };
        let (count, _) = halfway(&important, ["#ff0000", "#0000ff"], "s::before", "color");
        assert_eq!(count, 1);
        // Its visibility: shown all the way to the end when one end shows.
        let hidden = |visibility: &str| {
            format!(
                "#s {{ display:inline; }} #s:before {{ content:\"GEN\"; visibility:{visibility}; }}"
            )
        };
        let (count, shown) = halfway(&hidden, ["visible", "hidden"], "s::before", "visibility");
        assert_eq!(count, 1);
        assert!(run(&shown, "GEN").visible);
        assert!(run(&shown, "words").visible);
        // Its box's side spaces show with it.
        let sided = |visibility: &str| {
            format!(
                "#s {{ display:inline; }}
                 #s:before {{ content:\"GEN\"; padding-left:8px; background-color:#ff0000; visibility:{visibility}; }}"
            )
        };
        let (_, shown) = halfway(&sided, ["visible", "hidden"], "s::before", "visibility");
        let spaces: Vec<bool> = shown
            .text_leaves()
            .iter()
            .flat_map(|(_, leaf)| leaf.runs.iter())
            .filter(|run| run.side_of.is_some())
            .map(|run| run.visible)
            .collect();
        assert!(
            !spaces.is_empty() && spaces.iter().all(|shows| *shows),
            "{spaces:?}"
        );
        // Its background and border colors on its line, from a color to a color.
        let boxed = |color: &str| {
            format!(
                "#s {{ display:inline; }}
                 #s:before {{ content:\"GEN\"; background-color:{color}; border:2px solid {color}; }}"
            )
        };
        let (count, shown) = halfway(
            &boxed,
            ["#ff0000", "#0000ff"],
            "s::before",
            "background-color",
        );
        assert_eq!(count, 1);
        assert!(near(run(&shown, "GEN").background.unwrap(), mid));
        let (count, shown) = halfway(&boxed, ["#ff0000", "#0000ff"], "s::before", "border-color");
        assert_eq!(count, 4);
        let content = run(&shown, "GEN");
        let band = content
            .inline_boxes
            .iter()
            .rev()
            .find_map(|sides| sides.border[0].1)
            .unwrap();
        assert!(near(band, mid), "{band:?}");
        let bars: Vec<[f32; 3]> = shown
            .text_leaves()
            .iter()
            .flat_map(|(_, leaf)| leaf.runs.iter())
            .filter_map(|run| run.edge.map(|edge| edge.2))
            .collect();
        assert!(
            bars.len() == 2 && bars.iter().all(|bar| near(*bar, mid)),
            "{bars:?}"
        );
        // Hidden, its own background shows nowhere: its text keeps the one around it.
        let covered = |color: &str| {
            format!(
                "#s {{ display:inline; background-color:#00ff00; }}
                 #s:before {{ content:\"GEN\"; visibility:hidden; background-color:{color}; }}"
            )
        };
        let (count, shown) = halfway(
            &covered,
            ["#ff0000", "#0000ff"],
            "s::before",
            "background-color",
        );
        assert_eq!(count, 1);
        assert_eq!(run(&shown, "GEN").background, Some([0.0, 1.0, 0.0]));
        // A block one's text color.
        let block =
            |color: &str| format!("#s:before {{ content:\"GEN\"; display:block; color:{color}; }}");
        let (count, shown) = halfway(&block, ["#ff0000", "#0000ff"], "s::before", "color");
        assert_eq!(count, 1);
        assert!(near(run(&shown, "GEN").color.unwrap(), mid));
        // Its own background under its text, with no sides, shows while it does.
        let plain = |visibility: &str| {
            format!(
                "#s {{ display:inline; }}
                 #s:before {{ content:\"GEN\"; background-color:#ff0000; visibility:{visibility}; }}"
            )
        };
        let (_, shown) = halfway(&plain, ["visible", "hidden"], "s::before", "visibility");
        assert_eq!(run(&shown, "GEN").background, Some([1.0, 0.0, 0.0]));
        // So while its element's own background moves too.
        let under = |visibility: &str, color: &str| {
            format!(
                "#s {{ display:inline; background-color:{color}; }}
                 #s:before {{ content:\"GEN\"; background-color:#ff0000; visibility:{visibility}; }}"
            )
        };
        let from = BoxTree::new(source(), &under("visible", "#00ff00")).unwrap();
        let to = from.restyled(&[&under("hidden", "#ffff00")]).unwrap();
        let mut animations = BoxTreeAnimations::new();
        animations
            .request(&from, &to, "s::before", &["visibility"], second(), start)
            .unwrap();
        animations
            .request(&from, &to, "s", &["background-color"], second(), start)
            .unwrap();
        animations.advance(start + Duration::from_millis(500), &to);
        let shown = animations.presented(&to);
        assert_eq!(run(&shown, "GEN").background, Some([1.0, 0.0, 0.0]));
        assert!(near(
            run(&shown, "words").background.unwrap(),
            [0.5, 1.0, 0.0]
        ));
        // After an inside list marker: its background still found.
        let item = || node("page", vec![node("li", vec![text("t", "item")])]);
        let marked = |color: &str| {
            format!(
                "#li {{ display:list-item; list-style-position:inside; }}
                 #li:before {{ content:\"GEN\"; padding-left:4px; background-color:{color}; }}"
            )
        };
        let from = BoxTree::new(item(), &marked("#ff0000")).unwrap();
        let to = from.restyled(&[&marked("#0000ff")]).unwrap();
        let mut animations = BoxTreeAnimations::new();
        assert_eq!(
            animations
                .request(
                    &from,
                    &to,
                    "li::before",
                    &["background-color"],
                    second(),
                    start
                )
                .unwrap(),
            1
        );
        // Either spelling continues the other's animation: back from the half.
        let from = BoxTree::new(source(), &css("#ff0000")).unwrap();
        let to = from.restyled(&[&css("#0000ff")]).unwrap();
        let mut animations = BoxTreeAnimations::new();
        animations
            .request(&from, &to, "s:before", &["color"], second(), start)
            .unwrap();
        let later = start + Duration::from_millis(500);
        animations.advance(later, &to);
        animations
            .request(&to, &from, "s::before", &["color"], second(), later)
            .unwrap();
        animations.advance(later + Duration::from_millis(500), &from);
        let back = run(&animations.presented(&from), "GEN").color.unwrap();
        assert!(near(back, [0.75, 0.0, 0.25]), "{back:?}");
        // With its element's color moving too, its own timing, whichever was asked first.
        let both = |color: &str| {
            format!("#s {{ display:inline; color:{color}; }} #s:before {{ content:\"GEN\"; }}")
        };
        let from = BoxTree::new(source(), &both("#000000")).unwrap();
        let to = from.restyled(&[&both("#ffffff")]).unwrap();
        let mut animations = BoxTreeAnimations::new();
        animations
            .request(&from, &to, "s", &["color"], second(), start)
            .unwrap();
        animations
            .request(
                &from,
                &to,
                "s::before",
                &["color"],
                AnimationTiming::new(Duration::from_secs(4), Duration::ZERO),
                start,
            )
            .unwrap();
        animations.advance(start + Duration::from_millis(500), &to);
        let shown = animations.presented(&to);
        assert!(near(run(&shown, "GEN").color.unwrap(), [0.125; 3]));
        assert!(near(run(&shown, "words").color.unwrap(), [0.5; 3]));
        // To or from transparent on its line: a layer at its opacity, its box's and its
        // bands above and below.
        for ends in [["transparent", "#0000ff"], ["#0000ff", "transparent"]] {
            let (count, shown) = halfway(&boxed, ends, "s::before", "background-color");
            assert_eq!(count, 1);
            let own = run(&shown, "GEN")
                .inline_boxes
                .iter()
                .rev()
                .find(|sides| sides.background.is_some())
                .copied()
                .unwrap();
            assert_eq!((own.background, own.alpha), (Some([0.0, 0.0, 1.0]), 0.5));
            let (count, shown) = halfway(&boxed, ends, "s::before", "border-color");
            // Above and below; across a transparent bar keeps no width: at once.
            assert_eq!(count, 2);
            let own = *run(&shown, "GEN").inline_boxes.last().unwrap();
            assert_eq!(own.border[0].1, Some([0.0, 0.0, 1.0]));
            assert_eq!(own.border_alpha, [0.5; 2]);
        }
        // Over its element's own opaque background: a layer on its text and its spaces.
        let over = |color: &str| {
            format!(
                "#s {{ display:inline; background-color:#00ff00; }}
                 #s:before {{ content:\"GEN\"; padding:0 4px; background-color:{color}; }}"
            )
        };
        let (_, shown) = halfway(
            &over,
            ["transparent", "#ff0000"],
            "s::before",
            "background-color",
        );
        // Each run's background and its layers.
        type Layered = (Option<[f32; 3]>, Vec<([f32; 3], f32)>);
        let layered: Vec<Layered> = shown
            .text_leaves()
            .iter()
            .flat_map(|(_, leaf)| leaf.runs.iter())
            .filter(|run| {
                run.generated.is_some() || run.side_of.is_some_and(|at| at.0 > usize::MAX / 2)
            })
            .map(|run| {
                (
                    run.background,
                    run.fades.iter().map(|layer| (layer.1, layer.2)).collect(),
                )
            })
            .collect();
        assert!(
            layered.len() == 3
                && layered
                    .iter()
                    .all(|entry| *entry == (Some([0.0, 1.0, 0.0]), vec![([1.0, 0.0, 0.0], 0.5)])),
            "{layered:?}"
        );
        // Its margin's spaces, outside its box, take none.
        let spaced = |color: &str| {
            format!(
                "#s {{ display:inline; }}
                 #s:before {{ content:\"GEN\"; padding:0 4px; margin:0 8px; background-color:{color}; }}"
            )
        };
        let (_, shown) = halfway(
            &spaced,
            ["transparent", "#ff0000"],
            "s::before",
            "background-color",
        );
        let margins: Vec<usize> = shown
            .text_leaves()
            .iter()
            .flat_map(|(_, leaf)| leaf.runs.iter())
            .filter(|run| run.side_of.is_some() && run.percent_room.is_some_and(|room| !room.2))
            .map(|run| run.fades.len())
            .collect();
        assert_eq!(margins, vec![0, 0]);
        // Nor does an inside list marker before its box.
        let item = || node("page", vec![node("li", vec![text("t", "item")])]);
        let marked = |color: &str| {
            format!(
                "#li {{ display:list-item; list-style-position:inside; }}
                 #li:before {{ content:\"GEN\"; padding:0 4px; background-color:{color}; }}"
            )
        };
        let from = BoxTree::new(item(), &marked("transparent")).unwrap();
        let to = from.restyled(&[&marked("#ff0000")]).unwrap();
        let mut animations = BoxTreeAnimations::new();
        animations
            .request(
                &from,
                &to,
                "li::before",
                &["background-color"],
                second(),
                start,
            )
            .unwrap();
        animations.advance(start + Duration::from_millis(500), &to);
        let shown = animations.presented(&to);
        let marker = shown.text_leaves()[0].1.runs[0].clone();
        assert!(
            marker.generated.is_some() && marker.fades.is_empty(),
            "{marker:?}"
        );
        // Hidden, none.
        let hidden = |color: &str| {
            format!(
                "#s {{ display:inline; }} #s:before {{ content:\"GEN\"; visibility:hidden; background-color:{color}; }}"
            )
        };
        let (count, shown) = halfway(
            &hidden,
            ["transparent", "#ff0000"],
            "s::before",
            "background-color",
        );
        assert_eq!(count, 1);
        assert!(run(&shown, "GEN").fades.is_empty());
        // With its element fading too: its element's layer beneath its own.
        let both = |color: &str| {
            format!(
                "#s {{ display:inline; background-color:{color}; }}
                 #s:before {{ content:\"GEN\"; background-color:{color}; }}"
            )
        };
        let from = BoxTree::new(source(), &both("transparent")).unwrap();
        let to = from.restyled(&[&both("#0000ff")]).unwrap();
        let mut animations = BoxTreeAnimations::new();
        for id in ["s::before", "s"] {
            animations
                .request(&from, &to, id, &["background-color"], second(), start)
                .unwrap();
        }
        animations.advance(start + Duration::from_millis(500), &to);
        let layers: Vec<(usize, f32)> = run(&animations.presented(&to), "GEN")
            .fades
            .iter()
            .map(|layer| (usize::from(layer.0 > usize::MAX / 2), layer.2))
            .collect();
        assert_eq!(layers, vec![(0, 0.5), (1, 0.5)]);
        // Its currentColor borders move with its color; one given its own color stays.
        let current = |color: &str| {
            format!(
                "#s {{ display:inline; }}
                 #s:before {{ content:\"GEN\"; color:{color}; border:2px solid;
                              border-top-color:#ff0000; border-bottom-color:#0000ff; }}"
            )
        };
        let (count, shown) = halfway(&current, ["#ff0000", "#0000ff"], "s::before", "color");
        assert_eq!(count, 1);
        let own = *run(&shown, "GEN").inline_boxes.last().unwrap();
        // Red, then blue of their own: neither shows its color at both ends.
        assert_eq!(own.border[0].1, Some([1.0, 0.0, 0.0]));
        assert_eq!(own.border[1].1, Some([0.0, 0.0, 1.0]));
        // Given a color of its own by another styling, as a hover can: it stays.
        let from = BoxTree::new(source(), &current("#ff0000")).unwrap();
        let to = from.restyled(&[&current("#0000ff")]).unwrap();
        let mut animations = BoxTreeAnimations::new();
        animations
            .request(&from, &to, "s::before", &["color"], second(), start)
            .unwrap();
        animations.advance(start + Duration::from_millis(500), &to);
        let moved = to
            .restyled(&[
                &current("#0000ff"),
                "#s:before { border-left-color:#00ff00; }",
            ])
            .unwrap();
        let left = animations
            .presented(&moved)
            .text_leaves()
            .iter()
            .flat_map(|(_, leaf)| leaf.runs.iter())
            .find_map(|run| run.edge.filter(|edge| edge.0).map(|edge| edge.2));
        assert_eq!(left, Some([0.0, 1.0, 0.0]));
        let bars: Vec<[f32; 3]> = shown
            .text_leaves()
            .iter()
            .flat_map(|(_, leaf)| leaf.runs.iter())
            .filter_map(|run| run.edge.map(|edge| edge.2))
            .collect();
        assert!(
            bars.len() == 2 && bars.iter().all(|bar| near(*bar, mid)),
            "{bars:?}"
        );
        // A `:before` with no content, or an element with no such id: nothing.
        let (count, _) = halfway(&after, ["#ff0000", "#0000ff"], "s::before", "color");
        assert_eq!(count, 0);
        let (count, _) = halfway(&css, ["#ff0000", "#0000ff"], "x::before", "color");
        assert_eq!(count, 0);
        // Its text's size.
        let sized = |size: &str| {
            format!("#s {{ display:inline; }} #s:before {{ content:\"GEN\"; font-size:{size}; }}")
        };
        let (count, shown) = halfway(&sized, ["16px", "32px"], "s::before", "font-size");
        assert_eq!(count, 1);
        assert_eq!(run(&shown, "GEN").size, 24.0);
        assert_eq!(run(&shown, "words").size, 16.0);
        // Its side spaces at its size, as wide as before: 5 pixels round up to 2 sixths of
        // 24, a third of an em.
        let sided = |size: &str| {
            format!(
                "#s {{ display:inline; }} #s:before {{ content:\"GEN\"; padding-left:5px; font-size:{size}; }}"
            )
        };
        let (_, shown) = halfway(&sided, ["16px", "32px"], "s::before", "font-size");
        let spaces: Vec<(String, f32)> = shown
            .text_leaves()
            .iter()
            .flat_map(|(_, leaf)| leaf.runs.iter())
            .filter(|run| run.side_of.is_some())
            .map(|run| (run.text.clone(), run.size))
            .collect();
        assert_eq!(spaces, vec![("\u{2004}".to_owned(), 24.0)]);
        // Its sides on its line: across only where it has spaces at both ends, not where a
        // negative margin pulls it; above and below, its box's bands.
        let sides = |side: &str| {
            format!("#s {{ display:inline; }} #s:before {{ content:\"GEN\"; {side} }}")
        };
        for (one, two, property, expected) in [
            // With no sides at one end, no box on its line there either: at once.
            ("padding:0;", "padding:4px 10px;", "padding", 0),
            ("padding:2px 4px;", "padding:4px 10px;", "padding", 4),
            (
                "border:0 solid red;",
                "border:2px solid red;",
                "border-width",
                0,
            ),
            (
                "border:1px solid red;",
                "border:3px solid red;",
                "border-width",
                4,
            ),
            // Spaces at both ends but a bar at one only: no bar to grow across, only its
            // bands above and below.
            (
                "padding:0 4px; border:0 solid red;",
                "padding:0 4px; border:2px solid red;",
                "border-width",
                2,
            ),
            // Its padding where a negative margin pulls a side with nothing else: at once.
            ("padding:0 4px;", "padding:0; margin:0 -4px;", "padding", 0),
            // A transparent border at one end: its padding across at once, above and below
            // still moving (here not at all).
            (
                "padding:0 4px; margin:0 2px; border:2px solid transparent;",
                "padding:0 4px; margin:0 2px; border:2px solid red;",
                "padding",
                0,
            ),
            // Its padding where a negative margin pulls it: at once.
            (
                "padding:0 8px; margin:0 -4px;",
                "padding:0 12px; margin:0 -4px;",
                "padding",
                0,
            ),
            ("margin:0;", "margin:0 8px;", "margin", 0),
            ("margin:0 4px;", "margin:0 8px;", "margin", 2),
            (
                "padding:0 8px; margin:0 -4px;",
                "padding:0 8px; margin:0 -2px;",
                "margin",
                0,
            ),
        ] {
            let (count, _) = halfway(&|v: &str| sides(v), [one, two], "s::before", property);
            assert_eq!(count, expected, "{one} {two}");
        }
        let (_, shown) = halfway(
            &|v: &str| sides(v),
            ["padding:2px 4px;", "padding:6px 12px;"],
            "s::before",
            "padding",
        );
        let content = run(&shown, "GEN");
        let own = content.inline_boxes.last().unwrap();
        assert_eq!(own.padding, [4.0, 4.0]);
        let across: Vec<f32> = shown
            .text_leaves()
            .iter()
            .flat_map(|(_, leaf)| leaf.runs.iter())
            .filter(|run| run.side_of.is_some())
            .map(|run| run.percent_room.unwrap().0)
            .collect();
        assert_eq!(across, vec![8.0, 8.0]);
        // A padded inline right after it keeps its own spaces at its own size.
        let sibling = || {
            node(
                "page",
                vec![node(
                    "p",
                    vec![node("s", vec![node("b", vec![text("x", "x")])])],
                )],
            )
        };
        let css = |size: &str| {
            format!(
                "#s {{ display:inline; }} #s:before {{ content:\"GEN\"; font-size:{size}; }}
                 #b {{ display:inline; padding-left:5px; }}"
            )
        };
        let from = BoxTree::new(sibling(), &css("16px")).unwrap();
        let to = from.restyled(&[&css("32px")]).unwrap();
        let mut animations = BoxTreeAnimations::new();
        animations
            .request(&from, &to, "s::before", &["font-size"], second(), start)
            .unwrap();
        animations.advance(start + Duration::from_millis(500), &to);
        let shown = animations.presented(&to);
        let beside: Vec<(String, f32)> = shown
            .text_leaves()
            .iter()
            .flat_map(|(_, leaf)| leaf.runs.iter())
            .filter(|run| run.side_of.is_some())
            .map(|run| (run.text.clone(), run.size))
            .collect();
        let unmoved: Vec<(String, f32)> = to
            .text_leaves()
            .iter()
            .flat_map(|(_, leaf)| leaf.runs.iter())
            .filter(|run| run.side_of.is_some())
            .map(|run| (run.text.clone(), run.size))
            .collect();
        assert_eq!(beside, unmoved);
        assert_eq!(beside.len(), 1);
    }

    #[test]
    fn an_inline_em_outline_follows_its_font_size() {
        let source = || {
            node(
                "page",
                vec![node("p", vec![node("s", vec![text("t", "x")])])],
            )
        };
        let css = |size: &str| {
            format!("#s {{ display:inline; font-size:{size}; outline:0.25em solid #ff0000; }}")
        };
        let from = BoxTree::new(source(), &css("16px")).unwrap();
        let to = from.restyled(&[&css("32px")]).unwrap();
        // A quarter of each font size at the ends.
        assert_eq!(from.paints()[2].outline.unwrap().0, 4.0);
        assert_eq!(to.paints()[2].outline.unwrap().0, 8.0);
        // An ex is taken as half an em.
        let ex = BoxTree::new(
            source(),
            "#s { display:inline; font-size:16px; outline:1ex solid #ff0000; }",
        )
        .unwrap();
        assert_eq!(ex.paints()[2].outline.unwrap().0, 8.0);
        let mut animations = BoxTreeAnimations::new();
        let start = Instant::now();
        animations
            .request(&from, &to, "s", &["font-size"], second(), start)
            .unwrap();
        animations.advance(start + Duration::from_millis(500), &to);
        // Halfway a 24 pixel font: a 6 pixel outline, not the end's 8.
        let width = animations.presented(&to).paints()[2].outline.unwrap().0;
        assert!((width - 6.0).abs() < 1e-3, "{width}");
    }

    #[test]
    fn an_inline_padding_animated_of_its_own_keeps_its_timing() {
        let source = || {
            node(
                "page",
                vec![node("p", vec![node("s", vec![text("t", "x")])])],
            )
        };
        let css = |size: &str| format!("#s {{ display:inline; font-size:{size}; padding:0 1em; }}");
        let from = BoxTree::new(source(), &css("16px")).unwrap();
        let to = from.restyled(&[&css("32px")]).unwrap();
        let mut animations = BoxTreeAnimations::new();
        let start = Instant::now();
        animations
            .request(&from, &to, "s", &["font-size"], second(), start)
            .unwrap();
        animations
            .request(
                &from,
                &to,
                "s",
                &["padding-left"],
                AnimationTiming::new(Duration::from_secs(2), Duration::ZERO),
                start,
            )
            .unwrap();
        animations.advance(start + Duration::from_millis(500), &to);
        let shown = animations.presented(&to);
        // Its own animation, a quarter of 16 to 32: 20 pixels of room, not the size's 24.
        let room = shown
            .text_leaves()
            .iter()
            .flat_map(|(_, leaf)| leaf.runs.iter())
            .find(|run| {
                run.side_of.is_some_and(|at| at.1 == 3) && run.percent_room.is_some_and(|r| r.2)
            })
            .and_then(|run| run.percent_room)
            .map(|room| room.0);
        assert_eq!(room, Some(20.0));
    }

    #[test]
    fn an_inline_fades_to_and_from_transparent_over_what_is_beneath() {
        let source = || {
            node(
                "page",
                vec![node(
                    "o",
                    vec![text("a", "outer "), node("s", vec![text("t", "span")])],
                )],
            )
        };
        let css = |color: &str| {
            format!(
                "#o {{ display:inline; background-color:#0000ff; }}
                 #s {{ display:inline; padding:0 4px; background-color:{color};
                       border-left:2px solid {color}; border-top:2px solid {color}; }}"
            )
        };
        let clear = BoxTree::new(source(), &css("transparent")).unwrap();
        let red = clear.restyled(&[&css("#ff0000")]).unwrap();
        let start = Instant::now();
        for (from, to) in [(&clear, &red), (&red, &clear)] {
            let mut animations = BoxTreeAnimations::new();
            let count = animations
                .request(
                    from,
                    to,
                    "s",
                    &["background-color", "border-left-color", "border-top-color"],
                    second(),
                    start,
                )
                .unwrap();
            assert_eq!(count, 3);
            animations.advance(start + Duration::from_millis(500), to);
            let shown = animations.presented(to);
            let leaves = shown.text_leaves();
            let runs: Vec<&iced_css::box_tree::TextRun> = leaves
                .iter()
                .flat_map(|(_, leaf)| leaf.runs.iter())
                .collect();
            let red = [1.0, 0.0, 0.0];
            // Its text: the outer blue beneath, the red half over it.
            let span = runs.iter().find(|run| run.text.contains("span")).unwrap();
            assert_eq!(span.background, Some([0.0, 0.0, 1.0]));
            assert_eq!(
                span.fades
                    .iter()
                    .map(|layer| (layer.1, layer.2))
                    .collect::<Vec<_>>(),
                vec![(red, 0.5)]
            );
            // Its box: the red at half, padding and top band too.
            let own = span.inline_boxes.last().unwrap();
            assert_eq!((own.background, own.alpha), (Some(red), 0.5));
            assert_eq!((own.border[0].1, own.border_alpha[0]), (Some(red), 0.5));
            // Its left bar.
            let bar = runs
                .iter()
                .find_map(|run| run.edge.map(|edge| (edge, run.edge_alpha)));
            assert_eq!(bar, Some(((true, 2.0, red), 0.5)));
            // The outer text is no part of it.
            let outer = runs.iter().find(|run| run.text.contains("outer")).unwrap();
            assert_eq!(
                outer
                    .fades
                    .iter()
                    .map(|layer| (layer.1, layer.2))
                    .collect::<Vec<_>>(),
                vec![]
            );
        }
    }

    #[test]
    fn an_inline_fade_keeps_one_layer_with_what_animates_beside_it() {
        let source = || {
            node(
                "page",
                vec![node("p", vec![node("s", vec![text("t", "span")])])],
            )
        };
        let red = [1.0, 0.0, 0.0];
        let start = Instant::now();
        let halfway = |from: &str, to: &str, properties: &[&str]| {
            let one = BoxTree::new(source(), from).unwrap();
            let two = one.restyled(&[to]).unwrap();
            let mut animations = BoxTreeAnimations::new();
            animations
                .request(&one, &two, "s", properties, second(), start)
                .unwrap();
            animations.advance(start + Duration::from_millis(500), &two);
            animations.presented(&two)
        };
        let runs = |tree: &BoxTree| -> Vec<iced_css::box_tree::TextRun> {
            tree.text_leaves()
                .iter()
                .flat_map(|(_, leaf)| leaf.runs.iter().cloned())
                .collect()
        };
        let span = |runs: &[iced_css::box_tree::TextRun]| {
            runs.iter()
                .find(|run| run.text.contains("span"))
                .cloned()
                .unwrap()
        };
        // Nothing beneath: its box paints the one layer, the text no second over it.
        let shown = halfway(
            "#s { display:inline; padding:4px; background-color:transparent; }",
            "#s { display:inline; padding:4px; background-color:#ff0000; }",
            &["background-color"],
        );
        let text = span(&runs(&shown));
        assert_eq!(
            (
                text.background,
                text.fades
                    .iter()
                    .map(|layer| (layer.1, layer.2))
                    .collect::<Vec<_>>()
            ),
            (None, vec![])
        );
        let own = text.inline_boxes.last().unwrap();
        assert_eq!((own.background, own.alpha), (Some(red), 0.5));
        // Shown at once with it, either way: still a layer at half, never the end at full.
        for (from, to) in [
            (
                "#s { display:inline; padding:4px; visibility:hidden; background-color:transparent; }",
                "#s { display:inline; padding:4px; visibility:visible; background-color:#ff0000; }",
            ),
            (
                "#s { display:inline; padding:4px; visibility:visible; background-color:#ff0000; }",
                "#s { display:inline; padding:4px; visibility:hidden; background-color:transparent; }",
            ),
        ] {
            // Whichever is asked first.
            for order in [
                ["visibility", "background-color"],
                ["background-color", "visibility"],
            ] {
                let shown = halfway(from, to, &order);
                let text = span(&runs(&shown));
                assert!(text.visible, "{to} {order:?}");
                assert_eq!(text.background, None, "{to} {order:?}");
                let own = text.inline_boxes.last().unwrap();
                assert_eq!(
                    (own.background, own.alpha, own.visible),
                    (Some(red), 0.5, true),
                    "{to} {order:?}"
                );
            }
        }
        // Its padding shrinking to none: the box made for it fades too.
        let shown = halfway(
            "#s { display:inline; padding:4px; background-color:#ff0000; }",
            "#s { display:inline; background-color:transparent; }",
            &["padding", "background-color"],
        );
        let own = span(&runs(&shown)).inline_boxes.last().copied().unwrap();
        assert_eq!((own.background, own.alpha), (Some(red), 0.5));
        // Its border widening while its color fades: the bar at half.
        let shown = halfway(
            "#s { display:inline; border-left:2px solid #ff0000; }",
            "#s { display:inline; border-left:4px solid transparent; }",
            &["border-left-width", "border-left-color"],
        );
        let bar = runs(&shown)
            .iter()
            .find_map(|run| run.edge.map(|edge| (edge, run.edge_alpha)));
        assert_eq!(bar, Some(((true, 3.0, red), 0.5)));
        // A margin made beside it, which the target has none of, takes no layer of its
        // background.
        let shown = halfway(
            "#s { display:inline; margin-left:10px; background-color:#ff0000; }",
            "#s { display:inline; background-color:transparent; }",
            &["margin-left", "background-color"],
        );
        let margin = runs(&shown)
            .into_iter()
            .find(|run| run.side_of.is_some() && run.percent_room.is_some_and(|r| !r.2))
            .unwrap();
        assert_eq!(
            margin
                .fades
                .iter()
                .map(|layer| (layer.1, layer.2))
                .collect::<Vec<_>>(),
            vec![]
        );
        // Fading in inside an inline whose color moves: that color, as it is, beneath it.
        let nested = || {
            node(
                "page",
                vec![node(
                    "p",
                    vec![node("o", vec![node("s", vec![super::text("t", "span")])])],
                )],
            )
        };
        let css = |outer: &str, own: &str| {
            format!(
                "#o {{ display:inline; background-color:{outer}; }}
                 #s {{ display:inline; background-color:{own}; }}"
            )
        };
        let one = BoxTree::new(nested(), &css("#0000ff", "transparent")).unwrap();
        let two = one.restyled(&[&css("#00ff00", "#ff0000")]).unwrap();
        let mut animations = BoxTreeAnimations::new();
        for id in ["o", "s"] {
            animations
                .request(&one, &two, id, &["background-color"], second(), start)
                .unwrap();
        }
        animations.advance(start + Duration::from_millis(500), &two);
        let shown = animations.presented(&two);
        let text = span(&runs(&shown));
        assert!(
            near(text.background.unwrap(), [0.0, 0.5, 0.5]),
            "{:?}",
            text.background
        );
        assert_eq!(
            text.fades
                .iter()
                .map(|layer| (layer.1, layer.2))
                .collect::<Vec<_>>(),
            vec![(red, 0.5)]
        );
        // Hidden at both ends: nothing of it paints.
        let shown = halfway(
            "#s { display:inline; visibility:hidden; background-color:transparent; }",
            "#s { display:inline; visibility:hidden; background-color:#ff0000; }",
            &["background-color"],
        );
        assert_eq!(
            span(&runs(&shown))
                .fades
                .last()
                .map(|layer| (layer.1, layer.2)),
            None
        );
        // A nearer background of its own inside it covers it there.
        let inner = || {
            node(
                "page",
                vec![node(
                    "p",
                    vec![node("s", vec![node("n", vec![super::text("t", "span")])])],
                )],
            )
        };
        let css = |own: &str| {
            format!(
                "#s {{ display:inline; background-color:{own}; }}
                 #n {{ display:inline; background-color:#00ff00; }}"
            )
        };
        let one = BoxTree::new(inner(), &css("transparent")).unwrap();
        let two = one.restyled(&[&css("#ff0000")]).unwrap();
        let mut animations = BoxTreeAnimations::new();
        animations
            .request(&one, &two, "s", &["background-color"], second(), start)
            .unwrap();
        animations.advance(start + Duration::from_millis(500), &two);
        let shown = animations.presented(&two);
        let text = span(&runs(&shown));
        assert_eq!(
            (
                text.background,
                text.fades
                    .iter()
                    .map(|layer| (layer.1, layer.2))
                    .collect::<Vec<_>>()
            ),
            (Some([0.0, 1.0, 0.0]), vec![])
        );
    }

    #[test]
    fn nested_inline_fades_stack_their_layers() {
        let start = Instant::now();
        let shown = |source: &dyn Fn() -> Element, css: &dyn Fn(&str, &str) -> String| {
            let one = BoxTree::new(source(), &css("transparent", "transparent")).unwrap();
            let two = one.restyled(&[&css("#0000ff", "#ff0000")]).unwrap();
            let mut animations = BoxTreeAnimations::new();
            // The inner one asked first: they still stack outermost first.
            for id in ["s", "o"] {
                animations
                    .request(&one, &two, id, &["background-color"], second(), start)
                    .unwrap();
            }
            animations.advance(start + Duration::from_millis(500), &two);
            animations.presented(&two)
        };
        let layers = |tree: &BoxTree, text: &str| {
            let leaves = tree.text_leaves();
            let run = leaves
                .iter()
                .flat_map(|(_, leaf)| leaf.runs.iter())
                .find(|run| run.text.contains(text))
                .cloned()
                .unwrap();
            let colors: Vec<([f32; 3], f32)> =
                run.fades.iter().map(|layer| (layer.1, layer.2)).collect();
            (run.background, colors)
        };
        let (blue, red, white) = ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
        // Over the block's background: the outer layer, then the inner.
        let source = || {
            node(
                "page",
                vec![node(
                    "o",
                    vec![
                        super::text("a", "outer "),
                        node("s", vec![super::text("t", "span")]),
                    ],
                )],
            )
        };
        let css = |outer: &str, own: &str| {
            format!(
                "#page {{ background-color:#ffffff; }}
                 #o {{ display:inline; background-color:{outer}; }}
                 #s {{ display:inline; padding:4px; background-color:{own}; }}"
            )
        };
        let tree = shown(&source, &css);
        assert_eq!(
            layers(&tree, "span"),
            (Some(white), vec![(blue, 0.5), (red, 0.5)])
        );
        // The outer one without sides: its layer alone, over the block as it paints.
        assert_eq!(layers(&tree, "outer"), (None, vec![(blue, 0.5)]));
        // No block color to take as the base, an image there, or the block fading too:
        // no base, the layers as they are. A hidden block's color is no base either,
        // but the one beyond it is.
        let source = || {
            node(
                "page",
                vec![node(
                    "p",
                    vec![node("o", vec![node("s", vec![super::text("t", "span")])])],
                )],
            )
        };
        for (page, block, base) in [
            ("", "", None),
            (
                "background-color:#ffffff; background-image:url(x.png);",
                "",
                None,
            ),
            (
                "background-color:#ffffff;",
                "visibility:hidden; background-color:#00ff00;",
                Some(white),
            ),
            (
                "background-color:#ffffff;",
                "visibility:hidden; background-image:url(x.png);",
                Some(white),
            ),
        ] {
            let css = |outer: &str, own: &str| {
                format!(
                    "#page {{ {page} }}
                     #p {{ {block} }}
                     #o {{ display:inline; visibility:visible; background-color:{outer}; }}
                     #s {{ display:inline; padding:4px; background-color:{own}; }}"
                )
            };
            let tree = shown(&source, &css);
            assert_eq!(
                layers(&tree, "span"),
                (base, vec![(blue, 0.5), (red, 0.5)]),
                "{page} {block}"
            );
        }
        let css = |outer: &str, own: &str| {
            format!(
                "#page {{ background-color:{own}; }}
                 #o {{ display:inline; background-color:{outer}; }}
                 #s {{ display:inline; padding:4px; background-color:{own}; }}"
            )
        };
        let one = BoxTree::new(source(), &css("transparent", "transparent")).unwrap();
        let two = one.restyled(&[&css("#0000ff", "#ff0000")]).unwrap();
        let mut animations = BoxTreeAnimations::new();
        for id in ["page", "s", "o"] {
            animations
                .request(&one, &two, id, &["background-color"], second(), start)
                .unwrap();
        }
        animations.advance(start + Duration::from_millis(500), &two);
        assert_eq!(
            layers(&animations.presented(&two), "span"),
            (None, vec![(blue, 0.5), (red, 0.5)])
        );
        // An opaque one between them covers the outer layer there.
        let source = || {
            node(
                "page",
                vec![node(
                    "o",
                    vec![node("m", vec![node("s", vec![super::text("t", "span")])])],
                )],
            )
        };
        let css = |outer: &str, own: &str| {
            format!(
                "#page {{ background-color:#ffffff; }}
                 #o {{ display:inline; background-color:{outer}; }}
                 #m {{ display:inline; background-color:#00ff00; }}
                 #s {{ display:inline; background-color:{own}; }}"
            )
        };
        let tree = shown(&source, &css);
        assert_eq!(
            layers(&tree, "span"),
            (Some([0.0, 1.0, 0.0]), vec![(red, 0.5)])
        );
    }

    #[test]
    fn generated_runs_with_the_same_text_keep_their_own_ends() {
        let source = || node("page", vec![text("line", "words")]);
        let css = |color: &str| {
            format!(
                "#page {{ color:{color}; }}
                 #page:before {{ content:\"x\"; color:#ffffff; }}
                 #page:after {{ content:\"x\"; }}"
            )
        };
        let from = BoxTree::new(source(), &css("#000000")).unwrap();
        let to = from.restyled(&[&css("#ffffff")]).unwrap();
        let mut animations = BoxTreeAnimations::new();
        let start = Instant::now();
        animations
            .request(&from, &to, "page", &["color"], second(), start)
            .unwrap();
        animations.advance(start + Duration::from_millis(500), &to);
        let shown = animations.presented(&to);
        let leaves = shown.text_leaves();
        let runs = &leaves[0].1.runs;
        assert_eq!(runs[0].color, Some([1.0; 3]), "{runs:?}");
        assert!(near(runs[2].color.unwrap(), [0.5; 3]), "{runs:?}");
    }

    #[test]
    fn repeats_and_hidden_ends_start_nothing_extra() {
        let from = tree("#box { width:100px; height:10px; background-color:#000000; }");
        let to = from
            .restyled(&["#box { width:200px; height:20px; background-color:#ffffff; }"])
            .unwrap();
        let mut animations = BoxTreeAnimations::new();
        let start = Instant::now();
        let count = animations
            .request(
                &from,
                &to,
                "box",
                &["width", "height", "width"],
                second(),
                start,
            )
            .unwrap();
        assert_eq!(count, 2);
        let gone = from
            .restyled(&["#box { display:none; background-color:#ffffff; }"])
            .unwrap();
        let count = animations
            .request(&from, &gone, "box", &["background-color"], second(), start)
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn frames_builds_with_a_function_item() {
        fn tick(_: Instant) -> u8 {
            0
        }
        let _idle: iced::Subscription<u8> = BoxTreeAnimations::new().frames(tick);
    }

    #[test]
    fn generated_text_of_a_constant_color_stays_put() {
        let source = || node("page", vec![text("line", "words")]);
        let from = BoxTree::new(
            source(),
            "#page { color:#000000; } #page:before { content:\"x\"; color:#ffffff; }",
        )
        .unwrap();
        let to = from
            .restyled(&["#page { color:#ffffff; } #page:before { content:\"x\"; color:#ffffff; }"])
            .unwrap();
        let mut animations = BoxTreeAnimations::new();
        let start = Instant::now();
        animations
            .request(&from, &to, "page", &["color"], second(), start)
            .unwrap();
        animations.advance(start + Duration::from_millis(500), &to);
        let shown = animations.presented(&to);
        let leaves = shown.text_leaves();
        let runs = &leaves[0].1.runs;
        assert_eq!(runs[0].color, Some([1.0; 3]), "{runs:?}");
        assert!(near(runs[1].color.unwrap(), [0.5; 3]), "{runs:?}");
    }

    #[test]
    fn a_presented_color_reaches_what_inherits_it_and_nothing_else() {
        let from = tree(
            "#box { color:#000000; border:1px solid; }
             #kid { border:1px solid; }
             #own { color:#ffffff; border:1px solid #ffffff; }",
        );
        let to = from
            .restyled(&["#box { color:#ffffff; border:1px solid; }
                 #kid { border:1px solid; }
                 #own { color:#ffffff; border:1px solid #ffffff; }"])
            .unwrap();
        let mut animations = BoxTreeAnimations::new();
        let start = Instant::now();
        animations
            .request(&from, &to, "box", &["color"], second(), start)
            .unwrap();
        animations.advance(start + Duration::from_millis(500), &to);
        let shown = animations.presented(&to);
        let gray = [0.5; 3];
        let paints = shown.paints();
        assert!(near(paints[1].color.unwrap(), gray), "{:?}", paints[1]);
        assert!(near(paints[1].border[0].unwrap(), gray));
        assert!(near(paints[2].color.unwrap(), gray));
        assert!(near(paints[2].border[3].unwrap(), gray));
        // Always white, so it does not move from black.
        assert_eq!(paints[3].color, Some([1.0; 3]));
        assert_eq!(paints[3].border[0], Some([1.0; 3]));
        assert_eq!(paints[0].color, to.paints()[0].color);
    }

    #[test]
    fn presented_text_color_and_delay() {
        let source = || node("page", vec![text("line", "words")]);
        let from = BoxTree::new(source(), "#page { color:#000000; }").unwrap();
        let to = from.restyled(&["#page { color:#ffffff; }"]).unwrap();
        let mut animations = BoxTreeAnimations::new();
        let start = Instant::now();
        let timing = AnimationTiming::new(Duration::from_secs(1), Duration::from_secs(1));
        animations
            .request(&from, &to, "page", &["color"], timing, start)
            .unwrap();
        animations.advance(start + Duration::from_millis(500), &to);
        let black = animations.presented(&to);
        assert!(near(
            black.text_leaves()[0].1.runs[0].color.unwrap(),
            [0.0; 3]
        ));
        animations.advance(start + Duration::from_millis(1500), &to);
        let gray = animations.presented(&to);
        assert!(near(
            gray.text_leaves()[0].1.runs[0].color.unwrap(),
            [0.5; 3]
        ));
    }
}

#[test]
fn text_pieces_in_a_line_keep_words_their_container_and_their_owner() {
    let span = node;
    let root = node(
        "wrap",
        vec![
            node("chip", vec![]),
            span("s1", vec![text("hello", "Hello")]),
            text("gap", " "),
            span("s2", vec![text("world", "world")]),
        ],
    );
    let css = "#wrap { text-align:right; }
        #chip { display:inline-block; text-align:left; width:10px; }
        #s1, #s2 { display:inline; }";
    let tree = BoxTree::new(root, css).unwrap();
    let leaves = tree.text_leaves();
    assert_eq!(leaves[0].1.text(), "Hello world");
    assert_eq!(leaves[0].1.align, "right");

    let root = node("page", vec![text("one", "one"), node("a", vec![])]);
    let tree = BoxTree::new(root, "#a { display:inline-block; }").unwrap();
    assert_eq!(tree.text_leaves()[0].0, 1, "first run's index, not #a's");
}

#[test]
fn fixed_tables_take_columns_from_the_first_row_only() {
    let cell = |id: &str| node(id, vec![]);
    let root = node(
        "table",
        vec![
            node("r1", vec![cell("a"), cell("b"), cell("c")]),
            node("r2", vec![cell("d"), cell("e"), cell("f")]),
        ],
    );
    let css = "#table { display:table; table-layout:fixed; width:300px; }
        #r1, #r2 { display:table-row; }
        #a, #b, #c, #d, #e, #f { display:table-cell; height:5px; }
        #a { width:50px; padding:0 5px; }
        #e { width:200px; }";
    let geometry = BoxTree::new(root, css)
        .unwrap()
        .layout(800.0, None)
        .unwrap();
    let across = |at: usize| {
        let border = geometry[at].as_ref().unwrap().border;
        (border.left, border.right)
    };
    // Row one: #a is 60 with padding; the other two share 240. #e's width counts for nothing.
    assert_eq!(across(2), (0.0, 60.0));
    assert_eq!(across(3), (60.0, 180.0));
    assert_eq!(across(4), (180.0, 300.0));
    assert_eq!(across(7), (60.0, 180.0));
}

#[test]
fn table_layout_does_not_fix_an_auto_width_table() {
    let root = node(
        "table",
        vec![node("r", vec![node("a", vec![]), node("b", vec![])])],
    );
    let css = "#table { display:table; table-layout:fixed; }
        #r { display:table-row; }
        #a { display:table-cell; width:70px; height:5px; }
        #b { display:table-cell; width:30px; height:5px; }";
    let geometry = BoxTree::new(root, css)
        .unwrap()
        .layout(800.0, None)
        .unwrap();
    let border = geometry[3].as_ref().unwrap().border;
    // Automatic layout: the second cell after the first's preferred width, nothing shared out.
    assert_eq!((border.left, border.right), (70.0, 100.0));
}

#[test]
fn empty_cells_hide_only_without_content_and_without_collapse() {
    let root = || {
        node(
            "table",
            vec![node(
                "r",
                vec![
                    node("a", vec![]),
                    node("b", vec![text("t", "x")]),
                    node("c", vec![text("gap", "  ")]),
                    node("d", vec![node("out", vec![])]),
                ],
            )],
        )
    };
    let css = |collapse: &str, cell: &str| {
        format!(
            "#table {{ display:table; empty-cells:hide; border-collapse:{collapse}; }}
             #r {{ display:table-row; }}
             #a, #b, #c, #d {{ display:table-cell; background-color:#ff0000; {cell} }}
             #out {{ position:absolute; }}"
        )
    };
    let tree = BoxTree::new(root(), &css("separate", "")).unwrap();
    let hidden = |tree: &BoxTree, id: usize| tree.paints()[id].hidden_empty;
    let shown = BoxTree::new(
        root(),
        "#table { display:table; } #r { display:table-row; } #a { display:table-cell; }",
    )
    .unwrap();
    assert!(!hidden(&shown, 2), "`show` is the initial value");
    assert!(hidden(&tree, 2));
    assert!(!hidden(&tree, 3));
    assert!(hidden(&tree, 5), "white space collapses away");
    assert!(hidden(&tree, 7), "out-of-flow content does not count");
    // The table decides collapse, not the cell.
    let tree = BoxTree::new(root(), &css("collapse", "border-collapse:separate;")).unwrap();
    assert!(!hidden(&tree, 2));
    let tree = BoxTree::new(root(), &css("separate", "border-collapse:collapse;")).unwrap();
    assert!(hidden(&tree, 2));
}

#[test]
fn fixed_tables_ignore_content_and_grow_only_for_their_columns() {
    let across = |css: &str, cells: Vec<Element>, at: usize| {
        let root = node(
            "table",
            vec![node("r1", cells), node("r2", vec![node("e", vec![])])],
        );
        let css = format!(
            "#table {{ display:table; table-layout:fixed; }}
             #r1, #r2 {{ display:table-row; }}
             #a, #b, #e {{ display:table-cell; height:5px; }}
             {css}"
        );
        let geometry = BoxTree::new(root, &css)
            .unwrap()
            .layout(800.0, None)
            .unwrap();
        let border = geometry[at].as_ref().unwrap().border;
        (border.left, border.right)
    };
    let cells = || vec![node("a", vec![]), node("b", vec![])];
    // A wide cell in a later row changes nothing.
    assert_eq!(
        across("#table { width:300px; } #e { width:400px; }", cells(), 0),
        (0.0, 300.0)
    );
    assert_eq!(
        across("#table { width:300px; } #e { width:400px; }", cells(), 3),
        (150.0, 300.0)
    );
    // First-row widths past the table's own widen it.
    assert_eq!(
        across("#table { width:100px; } #a, #b { width:80%; }", cells(), 0),
        (0.0, 160.0)
    );
    // A percent table's own width is recovered from the grown one.
    let grown = |at: usize| {
        let root = node(
            "page",
            vec![node(
                "table",
                vec![node("r1", vec![node("a", vec![]), node("b", vec![])])],
            )],
        );
        let css = "#page { width:200px; }
             #table { display:table; table-layout:fixed; width:50%; }
             #r1 { display:table-row; }
             #a, #b { display:table-cell; height:5px; }
             #a { width:60%; }
             #b { width:100%; }";
        let geometry = BoxTree::new(root, css)
            .unwrap()
            .layout(800.0, None)
            .unwrap();
        let border = geometry[at].as_ref().unwrap().border;
        (border.left, border.right)
    };
    // 60% and 100% of the table's own 100px.
    assert_eq!(grown(1), (0.0, 160.0));
    let near = |(left, right): (f32, f32), (l, r): (f32, f32)| {
        (left - l).abs() < 0.01 && (right - r).abs() < 0.01
    };
    assert!(near(grown(3), (0.0, 60.0)), "{:?}", grown(3));
    assert!(near(grown(4), (60.0, 160.0)), "{:?}", grown(4));
    // Against the table's own width, so the grown table still holds them.
    assert_eq!(
        across("#table { width:100px; } #a, #b { width:80%; }", cells(), 3),
        (80.0, 160.0)
    );
}

#[test]
fn an_absolute_auto_table_stays_automatic_and_an_empty_element_is_content() {
    let root = node(
        "page",
        vec![node(
            "t",
            vec![node(
                "r",
                vec![node("a", vec![text("x", "x")]), node("b", vec![])],
            )],
        )],
    );
    let css = "#t { position:absolute; display:table; table-layout:fixed; }
        #r { display:table-row; }
        #a { display:table-cell; width:70px; }
        #b { display:table-cell; width:30px; height:5px; }";
    let geometry = BoxTree::new(root, css)
        .unwrap()
        .layout_text(800.0, Some(600.0), &mut |_, _| 10.0)
        .unwrap();
    let border = geometry[3].as_ref().unwrap().border;
    assert_eq!(border.right - border.left, 70.0, "not a shared-out column");

    let root = node(
        "t",
        vec![node("r", vec![node("c", vec![node("s", vec![])])])],
    );
    let css = "#t { display:table; empty-cells:hide; }
        #r { display:table-row; }
        #c { display:table-cell; }
        #s { display:inline; }";
    assert!(!BoxTree::new(root, css).unwrap().paints()[2].hidden_empty);
}

#[test]
fn a_percent_fixed_table_finds_its_width_past_a_spanning_cell() {
    let attrs = AttributeLimits {
        entries: NonZeroUsize::new(1000).unwrap(),
        retained_bytes: NonZeroUsize::new(1000).unwrap(),
        work: NonZeroUsize::new(1000).unwrap(),
    };
    let wide = adapters::column_from_vec(vec![], capture_limits())
        .unwrap()
        .style_id("a", attrs)
        .unwrap()
        .style_attribute("colspan", "2", attrs)
        .unwrap()
        .erase();
    let root = node(
        "page",
        vec![node("t", vec![node("r", vec![wide, node("b", vec![])])])],
    );
    let css = "#page { width:200px; }
        #t { display:table; table-layout:fixed; width:50%; border-spacing:40px; }
        #r { display:table-row; }
        #a, #b { display:table-cell; width:100%; height:5px; }";
    let geometry = BoxTree::new(root, css)
        .unwrap()
        .layout(800.0, None)
        .unwrap();
    for at in [3, 4] {
        let border = geometry[at].as_ref().unwrap().border;
        assert!(
            (border.right - border.left - 100.0).abs() < 0.1,
            "{border:?}"
        );
    }
}

#[test]
fn fixed_tables_read_column_elements_first() {
    let root = node(
        "table",
        vec![
            node("c1", vec![]),
            node("r", vec![node("a", vec![]), node("b", vec![])]),
        ],
    );
    let css = "#table { display:table; table-layout:fixed; width:300px; }
        #c1 { display:table-column; width:200px; }
        #r { display:table-row; }
        #a { display:table-cell; width:10px; height:5px; }
        #b { display:table-cell; height:5px; }";
    let geometry = BoxTree::new(root, css)
        .unwrap()
        .layout(800.0, None)
        .unwrap();
    let border = geometry[3].as_ref().unwrap().border;
    assert_eq!((border.left, border.right), (0.0, 200.0));
}

#[test]
fn clip_takes_negative_and_inherited_sides() {
    let root = node("page", vec![node("abs", vec![node("kid", vec![])])]);
    let tree = BoxTree::new(
        root,
        "#abs { position:absolute; clip:rect(-5px, 50px, auto, -10px); }
         #kid { clip:inherit; }",
    )
    .unwrap();
    let sides = Some([Some(-5.0), Some(50.0), None, Some(-10.0)]);
    assert_eq!(tree.paints()[1].clip, sides);
    assert_eq!(tree.paints()[2].clip, sides);
}

#[test]
fn clip_cuts_absolute_boxes_and_what_they_hold() {
    let root = node("page", vec![node("abs", vec![node("kid", vec![])])]);
    let tree = BoxTree::new(
        root,
        "#abs { position:absolute; top:10px; left:20px; width:100px; height:100px;
                clip:rect(5px, 50px, auto, 10px); }
         #kid { height:200px; }",
    )
    .unwrap();
    assert_eq!(
        tree.paints()[1].clip,
        Some([Some(5.0), Some(50.0), None, Some(10.0)])
    );
    // A static box ignores it.
    let root = node("page", vec![node("abs", vec![])]);
    let tree = BoxTree::new(
        root,
        "#abs { clip:rect(5px, 50px, auto, 10px); height:9px; }",
    )
    .unwrap();
    assert_eq!(tree.paints()[1].clip.map(|sides| sides[0]), Some(Some(5.0)));
}

#[test]
fn a_huge_text_indent_is_bounded_and_a_row_indents_its_first_line() {
    let root = node("page", vec![text("a", "words")]);
    let tree = BoxTree::new(root, "#page { text-indent:1000000000px; font-size:16px; }").unwrap();
    assert_eq!(tree.text_leaves()[0].1.runs[0].text.chars().count(), 512);

    let root = node(
        "page",
        vec![text("a", "first"), node("chip", vec![]), text("b", "later")],
    );
    let tree = BoxTree::new(
        root,
        "#page { text-indent:10px; font-size:10px; }
         #chip { display:inline-block; width:5px; text-indent:0; }",
    )
    .unwrap();
    // 16.1: the line of inline-blocks starts 10px in; its text holds no spaces for it.
    let leaves = tree.text_leaves();
    assert_eq!(leaves[0].1.text(), "first");
    assert_eq!(leaves[1].1.text(), "later");
    let g = tree.layout(400.0, None).unwrap();
    assert_eq!(g[1].unwrap().border.left, 10.0);
}

#[test]
fn small_letters_keep_the_line_height_of_their_computed_size() {
    let root = node("page", vec![text("t", "Ab")]);
    let tree = BoxTree::new(root, "#page { font-variant:small-caps; font-size:20px; }").unwrap();
    let leaves = tree.text_leaves();
    assert_eq!(leaves[0].1.runs[0].line_size, None);
    assert_eq!(leaves[0].1.runs[1].line_size, Some(20.0));
}

#[test]
fn small_caps_scale_what_was_lowercase() {
    let root = node("page", vec![text("t", "Ab c")]);
    let tree = BoxTree::new(root, "#page { font-variant:small-caps; font-size:10px; }").unwrap();
    let leaves = tree.text_leaves();
    let runs = &leaves[0].1.runs;
    let parts: Vec<(&str, f32)> = runs
        .iter()
        .map(|run| (run.text.as_str(), run.size))
        .collect();
    assert_eq!(
        parts,
        vec![("A", 10.0), ("B", 8.0), (" ", 10.0), ("C", 8.0)]
    );
    assert_eq!(leaves[0].1.text(), "AB C");
}

#[test]
fn text_indent_leads_the_first_line_of_a_block_only() {
    let root = node(
        "page",
        vec![
            text("a", "first"),
            node("box", vec![]),
            text("b", "after a block"),
            node("own", vec![text("c", "own")]),
        ],
    );
    let tree = BoxTree::new(
        root,
        "#page { text-indent:10px; font-size:10px; }
         #own { text-indent:0; }
         #box { height:1px; }",
    )
    .unwrap();
    let leaves = tree.text_leaves();
    let of = |index: usize| leaves[index].1.text();
    assert_eq!(of(0), "\u{2003}first");
    // Later anonymous blocks are not the first line of their container.
    assert_eq!(of(1), "after a block");
    assert_eq!(of(2), "own");
    assert_eq!(leaves[0].1.runs.len(), 2);
    assert!(!leaves[0].1.runs[0].underline);
    // Blank content gets no indent, and a negative one is ignored.
    let root = node("page", vec![text("a", "words")]);
    let tree = BoxTree::new(root, "#page { text-indent:-10px; }").unwrap();
    assert_eq!(tree.text_leaves()[0].1.text(), "words");
    let root = node("page", vec![node("gap", vec![])]);
    let tree = BoxTree::new(root, "#page { text-indent:10px; }").unwrap();
    assert!(tree.text_leaves().is_empty());
}

#[test]
fn an_inline_box_opens_and_closes_around_its_runs_once() {
    let root = node(
        "page",
        vec![
            text("a", "a"),
            node("span", vec![text("b", "b"), text("c", "c")]),
            text("d", "d"),
        ],
    );
    let tree = BoxTree::new(
        root,
        "#page { font-size:16px; }
         #span { display:inline; padding:2px 8px; margin:0 16px; border-left:4px solid #ff0000;
                 background-color:#00ff00; }",
    )
    .unwrap();
    let leaves = tree.text_leaves();
    let runs = &leaves[0].1.runs;
    let texts: Vec<&str> = runs.iter().map(|run| run.text.as_str()).collect();
    assert_eq!(
        texts,
        vec![
            "a",
            "\u{2003}",
            "\u{2002}\u{2004}",
            "b",
            "c",
            "\u{2002}",
            "\u{2003}",
            "d",
        ]
    );
    assert_eq!(runs[2].edge, Some((true, 4.0, [1.0, 0.0, 0.0])));
    assert_eq!(runs[5].edge, None);
    assert_eq!(runs[3].inline_boxes[0].padding, [2.0, 2.0]);
    assert_eq!(runs[3].inline_boxes[0].background, Some([0.0, 1.0, 0.0]));
    assert!(runs[0].inline_boxes.is_empty());
    assert!(
        runs[1].inline_boxes.is_empty(),
        "a margin is outside the box"
    );
    // Blank content collapses away, but a padded box stays (10.8): a zero-width place.
    let root = node("page", vec![node("span", vec![text("b", " ")])]);
    let tree = BoxTree::new(root, "#span { display:inline; padding:8px; }").unwrap();
    let leaves = tree.text_leaves();
    assert!(
        leaves
            .iter()
            .any(|(_, leaf)| leaf.text().contains('\u{200B}'))
    );
    assert!(
        leaves
            .iter()
            .flat_map(|(_, leaf)| leaf.runs.iter())
            .any(|run| !run.inline_boxes.is_empty())
    );
    // Between words, its blank collapses with theirs: one space in all.
    let root = node(
        "page",
        vec![
            text("a", "before "),
            node("span", vec![text("b", " ")]),
            text("c", " after"),
        ],
    );
    let tree = BoxTree::new(root, "#span { display:inline; padding:0 8px; }").unwrap();
    let joined: String = tree
        .text_leaves()
        .iter()
        .map(|(_, leaf)| leaf.text())
        .collect();
    assert_eq!(joined.matches(' ').count(), 1, "{joined:?}");
    assert!(joined.contains('\u{200B}'));
    let placed = |children: Vec<Element>| {
        let tree = BoxTree::new(
            node("page", vec![node("span", children)]),
            "#span { display:inline; padding:8px; } #e { display:inline; }",
        )
        .unwrap();
        tree.text_leaves()
            .iter()
            .any(|(_, leaf)| leaf.text().contains('\u{200B}'))
    };
    // Two blanks are still blank, and so is an empty inline; a no-break space is content.
    assert!(placed(vec![text("b", " "), text("c", "\n\t")]));
    assert!(!placed(vec![text("b", "\u{A0}")]));
    assert!(placed(vec![text("b", " "), node("e", vec![])]));
    // A block inside is content.
    assert!(!placed(vec![node("block", vec![])]));
}

#[test]
fn an_inline_holding_only_empty_inlines_holds_one_place() {
    // How many zero-width places the leaves hold, and whether each wears `#span`'s box.
    let places = |children: Vec<Element>, css: &str| {
        let tree = BoxTree::new(
            node("page", vec![node("span", children)]),
            &format!(
                "#span {{ display:inline; padding:8px; background-color:#00ff00; }}
                 #e, #f, #g {{ display:inline; }} {css}"
            ),
        )
        .unwrap();
        let leaves = tree.text_leaves();
        let runs = leaves.iter().flat_map(|(_, leaf)| leaf.runs.iter());
        (
            leaves
                .iter()
                .map(|(_, leaf)| leaf.text().matches('\u{200B}').count())
                .sum::<usize>(),
            runs.filter(|run| run.text.contains('\u{200B}'))
                .all(|run| !run.inline_boxes.is_empty()),
        )
    };
    let nested = |inner: Vec<Element>| vec![node("e", vec![node("f", inner)])];
    // Empty all the way down, through white space and hidden children: one place, boxed.
    assert_eq!(places(nested(vec![]), ""), (1, true));
    assert_eq!(places(nested(vec![text("w", " \n")]), ""), (1, true));
    assert_eq!(
        places(
            nested(vec![node("g", vec![text("x", "x")])]),
            "#g { display:none; }"
        ),
        (1, true)
    );
    // Content anywhere inside makes none.
    assert_eq!(places(nested(vec![text("x", "x")]), ""), (0, true));
    // Every child counts, not just the first.
    assert_eq!(
        places(nested(vec![node("g", vec![]), text("x", "x")]), ""),
        (0, true)
    );
    assert_eq!(
        places(nested(vec![]), "#f:before { content:\"x\"; }"),
        (0, true)
    );
    assert_eq!(
        places(nested(vec![]), "#f:after { content:\"x\"; }"),
        (0, true)
    );
    // An inner box with sides holds the place itself, and the outer box wraps it.
    assert_eq!(places(nested(vec![]), "#f { padding:2px; }"), (1, true));
    // An inline-block inside is content, of a line of its own.
    let (count, _) = places(nested(vec![]), "#f { display:inline-block; }");
    assert_eq!(count, 0);
}

#[test]
fn an_inline_generated_box_takes_its_sides_and_answers_to_no_element() {
    let runs_of = |css: &str| {
        let tree = BoxTree::new(
            node("page", vec![node("s", vec![text("x", "text")])]),
            &format!("#s {{ display:inline; }} #s:before {{ {css} }}"),
        )
        .unwrap();
        tree.text_leaves()[0].1.runs.clone()
    };
    // `content:""` with sides: a zero-width place between its side spaces, boxed.
    let runs = runs_of("content:\"\"; padding:0 8px; background-color:#ff0000;");
    let place = runs.iter().position(|run| run.text == "\u{200B}").unwrap();
    assert!(runs[..place].iter().any(|run| run.spacer));
    assert!(runs[place + 1..].iter().any(|run| run.spacer));
    let own = &runs[place].inline_boxes;
    assert_eq!(own.len(), 1);
    assert_eq!(own[0].background, Some([1.0, 0.0, 0.0]));
    assert!(own[0].owner > usize::MAX / 2);
    // Its text after it has no box of its.
    assert!(runs.last().unwrap().inline_boxes.is_empty());
    // Text content gets the same, and a border its bars.
    let runs = runs_of("content:\"ab\"; border:2px solid #0000ff;");
    assert_eq!(runs.iter().filter(|run| run.edge.is_some()).count(), 2);
    let ab = runs.iter().find(|run| run.text == "ab").unwrap();
    assert_eq!(ab.inline_boxes[0].border[0].0, 2.0);
    // Without sides, nothing changes: no place, no box.
    let runs = runs_of("content:\"\";");
    assert!(runs.iter().all(|run| !run.text.contains('\u{200B}')));
    assert!(runs.iter().all(|run| run.inline_boxes.is_empty()));
    // Inside a padded inline, it sits within that box too, innermost.
    let tree = BoxTree::new(
        node("page", vec![node("s", vec![text("x", "text")])]),
        "#s { display:inline; padding:0 4px; }
         #s:after { content:\"\"; padding:0 8px; background-color:#ff0000; }",
    )
    .unwrap();
    let runs = &tree.text_leaves()[0].1.runs;
    let place = runs.iter().find(|run| run.text == "\u{200B}").unwrap();
    assert_eq!(place.inline_boxes.len(), 2);
    assert!(place.inline_boxes[1].owner > usize::MAX / 2);
    assert!(place.inline_boxes[0].owner < usize::MAX / 2);
}

#[test]
fn an_inside_marker_stays_outside_its_before_box() {
    let runs_of = |css: &str| {
        let tree = BoxTree::new(
            node("page", vec![node("li", vec![text("x", "item")])]),
            &format!(
                "#li {{ display:list-item; list-style:inside decimal; }}
                 #li:before {{ padding:0 8px; background-color:#ff0000; {css} }}"
            ),
        )
        .unwrap();
        tree.text_leaves()[0]
            .1
            .runs
            .iter()
            .map(|run| {
                // A run outside the box shows none of its background either.
                if run.inline_boxes.is_empty() {
                    assert_eq!(run.background, None, "{:?}", run.text);
                }
                (run.text.clone(), !run.inline_boxes.is_empty())
            })
            .collect::<Vec<_>>()
    };
    // The marker comes first, outside the box its `:before` content makes.
    let runs = runs_of("content:\"x\";");
    assert_eq!(runs[0], ("1.\u{a0}".to_owned(), false), "{runs:?}");
    assert!(runs.contains(&("x".to_owned(), true)), "{runs:?}");
    assert_eq!(runs.last().unwrap(), &("item".to_owned(), false));
    // An empty one still holds its place after the marker.
    let runs = runs_of("content:\"\";");
    assert_eq!(runs[0], ("1.\u{a0}".to_owned(), false), "{runs:?}");
    assert!(runs.contains(&("\u{200B}".to_owned(), true)), "{runs:?}");
    // With no content there is no box: the marker alone, as before.
    let runs = runs_of("");
    assert!(runs.iter().all(|(_, boxed)| !boxed), "{runs:?}");
    assert_eq!(
        runs.iter()
            .map(|(text, _)| text.as_str())
            .collect::<String>(),
        "1.\u{a0}item"
    );
}

#[test]
fn every_rule_that_can_match_an_element_reaches_it() {
    // Rules are tried by what their last compound needs; none that can match is left out.
    let bound = NonZeroUsize::new(1000).unwrap();
    let limits = AttributeLimits {
        entries: bound,
        retained_bytes: bound,
        work: bound,
    };
    let classed = |id: &str, classes: &str| -> Element {
        let column = adapters::column_from_vec(Vec::<Element>::new(), capture_limits())
            .unwrap()
            .style_id(id, limits)
            .unwrap();
        if classes.is_empty() {
            column.erase()
        } else {
            column
                .style_attribute("class", classes, limits)
                .unwrap()
                .erase()
        }
    };
    let root = node(
        "page",
        vec![
            classed("one", "a\tb"),
            classed("two", "b"),
            classed("three", ""),
        ],
    );
    let tree = BoxTree::new(
        root,
        ".b { color:#00ff00; }
         #three, .a { border-top:1px solid #ff0000; }
         column.b { outline:2px solid #0000ff; }
         [class~=\"b\"] { background-color:#ffff00; }
         #page > .a { border-bottom:1px solid #00ffff; }
         * { border-left:1px solid #ff00ff; }
         #two:first-child { border-right:1px solid #000000; }
         column { padding-left:1px; }
         nosuchtype { padding-left:9px; }
         .B, #ONE { background-color:#000000; }
         [id=\"three\"] { outline:3px dotted #00ff00; }
         .a:before { content:\"x\"; }",
    )
    .unwrap();
    let paints = tree.paints();
    let (one, two, three) = (paints[1], paints[2], paints[3]);
    let green = Some([0.0, 1.0, 0.0]);
    assert_eq!((one.color, two.color), (green, green));
    assert_ne!(three.color, green);
    assert_eq!(one.border[0], Some([1.0, 0.0, 0.0]));
    assert_eq!(three.border[0], Some([1.0, 0.0, 0.0]));
    // A border's color defaults to the text's: whether it shows is its style.
    assert_eq!(two.border_style[0], "none");
    let width = |paint: iced_css::evaluation::tree_layout::BoxPaint| paint.outline.map(|o| o.0);
    assert_eq!((width(one), width(two)), (Some(2.0), Some(2.0)));
    let yellow = Some([1.0, 1.0, 0.0]);
    assert_eq!(
        (one.background, two.background, three.background),
        (yellow, yellow, None)
    );
    assert_eq!(one.border[2], Some([0.0, 1.0, 1.0]));
    assert_eq!(two.border_style[2], "none");
    for paint in [one, two, three] {
        assert_eq!(paint.border[3], Some([1.0, 0.0, 1.0]));
    }
    // Not its first child: the rule needs both.
    assert_eq!(two.border_style[1], "none");
    // A type alone, and a type no box here has.
    let geometry = tree.layout(800.0, None).unwrap();
    for (at, boxed) in geometry[1..=3].iter().enumerate() {
        let boxed = boxed.as_ref().unwrap();
        assert_eq!(boxed.content.left - boxed.padding.left, 1.0, "{at}");
    }
    // Names match as written: `.B` and `#ONE` are not `b` and `one`.
    assert_eq!(one.background, yellow);
    // An attribute test on the id is no id rule: it still matches.
    assert_eq!(three.outline.map(|outline| outline.0), Some(3.0));
    // A class's `:before` reaches its element.
    let texts: Vec<String> = tree
        .text_leaves()
        .iter()
        .map(|(_, leaf)| leaf.text())
        .collect();
    assert_eq!(texts, vec!["x".to_owned()]);
}

#[test]
fn nested_inline_boxes_paint_every_level_and_blocks_do_not_repeat_sides() {
    let root = node(
        "page",
        vec![node(
            "o",
            vec![text("a", "a"), node("i", vec![text("b", "b")])],
        )],
    );
    let tree = BoxTree::new(
        root,
        "#page { font-size:16px; }
         #o { display:inline; padding:4px; background-color:#0000ff; }
         #i { display:inline; margin:0 16px; }",
    )
    .unwrap();
    let leaves = tree.text_leaves();
    let run = |text: &str| {
        leaves[0]
            .1
            .runs
            .iter()
            .find(|run| run.text == text)
            .unwrap()
            .clone()
    };
    assert_eq!(run("b").inline_boxes.len(), 2);
    // The inner margin still lies inside the outer box.
    assert_eq!(run("\u{2003}").inline_boxes.len(), 1);

    let root = node(
        "span",
        vec![text("a", "one"), node("block", vec![]), text("b", "two")],
    );
    let root = node("page", vec![root]);
    let tree = BoxTree::new(
        root,
        "#page { font-size:12px; }
         #span { display:inline; padding:0 12px; }
         #block { display:block; height:1px; }",
    )
    .unwrap();
    let leaves = tree.text_leaves();
    assert_eq!(leaves[0].1.text(), "\u{2003}one");
    assert_eq!(leaves[1].1.text(), "two\u{2003}");
}

#[test]
fn a_box_closes_before_trailing_white_space_and_spaces_show_backgrounds_around_them() {
    let root = node(
        "page",
        vec![node(
            "span",
            vec![text("a", "one"), node("block", vec![]), text("gap", "\n")],
        )],
    );
    let tree = BoxTree::new(
        root,
        "#page { font-size:12px; }
         #span { display:inline; padding:0 12px; }
         #block { display:block; height:1px; }",
    )
    .unwrap();
    assert_eq!(tree.text_leaves()[0].1.text(), "\u{2003}one\u{2003}");

    let root = node(
        "page",
        vec![node("o", vec![node("i", vec![text("x", "x")])])],
    );
    let tree = BoxTree::new(
        root,
        "#page { font-size:12px; }
         #o { display:inline; background-color:#ff0000; }
         #i { display:inline; padding:0 12px; margin:0 12px; }",
    )
    .unwrap();
    let leaves = tree.text_leaves();
    let red = Some([1.0, 0.0, 0.0]);
    assert!(
        leaves[0].1.runs.iter().all(|run| run.background == red),
        "{:?}",
        leaves[0].1.runs
    );
}

#[test]
fn a_hidden_generated_run_shows_the_background_around_it_not_its_own() {
    let root = node("page", vec![node("o", vec![text("x", "x")])]);
    let tree = BoxTree::new(
        root,
        "#o { display:inline; background-color:#0000ff; }
         #o:before { content:\"y\"; visibility:hidden; background-color:#ff0000; }",
    )
    .unwrap();
    let leaves = tree.text_leaves();
    let run = &leaves[0].1.runs[0];
    assert!(!run.visible);
    assert_eq!(run.background, Some([0.0, 0.0, 1.0]));
}

#[test]
fn a_hidden_box_lends_no_background() {
    let root = node("page", vec![node("s", vec![text("x", "x")])]);
    let tree = BoxTree::new(
        root,
        "#s { display:inline; visibility:hidden; background-color:#0000ff; }
         #x { visibility:visible; }",
    )
    .unwrap();
    let leaves = tree.text_leaves();
    let run = &leaves[0].1.runs[0];
    assert!(run.visible);
    assert_eq!(run.background, None);
}

#[test]
fn hidden_boxes_keep_their_room_and_thin_side_borders_keep_their_bars() {
    let root = node("page", vec![node("s", vec![text("x", "x")])]);
    let tree = BoxTree::new(
        root,
        "#page { font-size:16px; }
         #s { display:inline; border-left:1px solid #ff0000; }",
    )
    .unwrap();
    let leaves = tree.text_leaves();
    assert_eq!(leaves[0].1.runs[0].edge, Some((true, 1.0, [1.0, 0.0, 0.0])));

    let root = node("page", vec![node("s", vec![text("x", "x")])]);
    let tree = BoxTree::new(
        root,
        "#page { font-size:16px; }
         #s { display:inline; visibility:hidden; padding:0 8px; border:2px solid #ff0000;
              background-color:#0000ff; }
         #x { visibility:visible; }",
    )
    .unwrap();
    let leaves = tree.text_leaves();
    assert!(
        leaves[0]
            .1
            .runs
            .iter()
            .all(|run| run.inline_boxes.iter().all(|sides| !sides.visible))
    );
    // Its sides are kept, marked hidden, for an animated `visibility` to show.
    assert!(
        leaves[0]
            .1
            .runs
            .iter()
            .any(|run| !run.inline_boxes.is_empty())
    );
    assert_eq!(leaves[0].1.runs.len(), 3, "the room stays");
    assert!(!leaves[0].1.runs[0].visible);
}

#[test]
fn an_indent_takes_no_sides_from_the_box_it_precedes() {
    let root = node("page", vec![node("s", vec![text("x", "x")])]);
    let tree = BoxTree::new(
        root,
        "#page { font-size:16px; text-indent:16px; }
         #s { display:inline; border:2px solid #ff0000; }",
    )
    .unwrap();
    let leaves = tree.text_leaves();
    let indent = &leaves[0].1.runs[0];
    assert_eq!(indent.text, "\u{2003}");
    assert!(indent.inline_boxes.is_empty() && indent.edge.is_none());
}

#[test]
fn vertical_align_places_inline_blocks_in_their_line() {
    let top_of = |align: &str| {
        let root = node("page", vec![node("tall", vec![]), node("chip", vec![])]);
        let css = format!(
            "#tall {{ display:inline-block; width:10px; height:40px; }}
             #chip {{ display:inline-block; width:10px; height:10px; vertical-align:{align}; }}"
        );
        let geometry = BoxTree::new(root, &css)
            .unwrap()
            .layout(800.0, None)
            .unwrap();
        geometry[2].as_ref().unwrap().border.top
    };
    assert_eq!(top_of("baseline"), 30.0);
    assert_eq!(top_of("bottom"), 30.0);
    assert_eq!(top_of("middle"), 15.0);
    assert_eq!(top_of("top"), 0.0);
    assert_eq!(top_of("text-top"), 0.0);
    assert_eq!(top_of("text-bottom"), 30.0);
}

#[test]
fn bidi_controls_cover_blocks_and_breaks_and_leave_blank_text_blank() {
    let root = node("page", vec![node("s", vec![text("w", "abc")])]);
    let tree = BoxTree::new(
        root,
        "#s { display:block; direction:rtl; unicode-bidi:bidi-override; } #w { display:inline; }",
    )
    .unwrap();
    assert!(
        tree.text_leaves()[0].1.text().contains('\u{202E}'),
        "a block's override reaches its text"
    );

    let root = node("page", vec![node("s", vec![text("w", "one\ntwo")])]);
    let tree = BoxTree::new(
        root,
        "#page { white-space:pre; } #s { display:block; direction:rtl; } #w { display:inline; }",
    )
    .unwrap();
    assert_eq!(tree.text_leaves()[0].1.text(), "\u{200F}one\n\u{200F}two");

    let root = node("page", vec![node("s", vec![text("w", "one\ntwo")])]);
    let tree = BoxTree::new(
        root,
        "#page { white-space:pre; } #s { display:inline; direction:rtl; unicode-bidi:embed; }
         #w { display:inline; }",
    )
    .unwrap();
    assert_eq!(
        tree.text_leaves()[0].1.text().matches('\u{202B}').count(),
        2,
        "an embedding reopens after a forced break"
    );

    let root = node("page", vec![node("s", vec![text("w", " ")])]);
    let tree = BoxTree::new(root, "#s { display:inline; unicode-bidi:embed; }").unwrap();
    assert!(
        tree.text_leaves()
            .iter()
            .all(|(_, leaf)| !leaf.text().contains('\u{202A}'))
    );
}

#[test]
fn word_spacing_reaches_no_break_spaces() {
    let root = node("page", vec![text("t", "a\u{a0}b c")]);
    let tree = BoxTree::new(root, "#page { word-spacing:16px; font-size:16px; }").unwrap();
    // The stand-ins joined so no line breaks among them: before a space, and after a
    // no-break space with no break after them either.
    assert_eq!(
        tree.text_leaves()[0].1.text(),
        "a\u{a0}\u{2060}\u{2003}\u{2060}b\u{2003}\u{2060} c"
    );
    // Several stand-ins, joined among themselves too.
    let root = node("page", vec![text("t", "a b")]);
    let tree = BoxTree::new(root, "#page { word-spacing:24px; font-size:16px; }").unwrap();
    assert_eq!(
        tree.text_leaves()[0].1.text(),
        "a\u{2003}\u{2060}\u{2002}\u{2060} b"
    );
}

#[test]
fn line_ends_normalize_and_a_final_break_opens_no_line() {
    let leaf = |words: &'static str| {
        let root = node("page", vec![node("s", vec![text("w", words)])]);
        let tree = BoxTree::new(
            root,
            "#page { white-space:pre; } #s { display:block; direction:rtl; } #w { display:inline; }",
        )
        .unwrap();
        tree.text_leaves()[0].1.text()
    };
    assert_eq!(leaf("one\n"), "\u{200F}one\n");
    assert_eq!(leaf("one\r\ntwo"), "\u{200F}one\n\u{200F}two");
    assert_eq!(leaf("one\rtwo"), "\u{200F}one\n\u{200F}two");
}

#[test]
fn ltr_text_holding_rtl_characters_keeps_its_base_direction() {
    let root = node("page", vec![text("t", "\u{5d0} abc")]);
    let tree = BoxTree::new(root, "").unwrap();
    assert!(tree.text_leaves()[0].1.text().starts_with('\u{200E}'));
}

#[test]
fn an_embedding_closes_where_its_element_ends() {
    let root = node(
        "page",
        vec![node("s", vec![text("w", "a")]), text("after", "b")],
    );
    let tree = BoxTree::new(
        root,
        "#s { display:inline; direction:rtl; unicode-bidi:embed; } #w, #after { display:inline; }",
    )
    .unwrap();
    let leaves = tree.text_leaves();
    assert_eq!(leaves[0].1.text(), "\u{202B}a\u{202C}b");
}

#[test]
fn cells_in_different_parents_get_different_anonymous_tables() {
    let tree = BoxTree::new(
        node(
            "d",
            vec![node("s", vec![node("a", vec![])]), node("b", vec![])],
        ),
        "#s { display:inline; } #a, #b { display:table-cell; width:40px; height:30px; }",
    )
    .unwrap();
    let geometry = tree.layout(400.0, None).unwrap();
    let (a, b) = (geometry[2].unwrap().border, geometry[3].unwrap().border);
    // `#a` is wrapped inside the span, `#b` in the block: two tables, one under the other.
    assert_eq!((a.left, a.top), (0.0, 0.0));
    assert_eq!((b.left, b.top), (0.0, 30.0));
    // Nested inline elements, and blank text after one, change nothing of that.
    let tree = BoxTree::new(
        node(
            "d",
            vec![
                node("s", vec![node("t", vec![node("a", vec![])])]),
                text("gap", " "),
                node("b", vec![]),
            ],
        ),
        "#s, #t { display:inline; } #a, #b { display:table-cell; width:40px; height:30px; }",
    )
    .unwrap();
    let geometry = tree.layout(400.0, None).unwrap();
    let (a, b) = (geometry[3].unwrap().border, geometry[5].unwrap().border);
    assert_eq!((a.left, a.top, b.left, b.top), (0.0, 0.0, 0.0, 30.0));
    // Siblings still share one.
    let tree = BoxTree::new(
        node("d", vec![node("a", vec![]), node("b", vec![])]),
        "#a, #b { display:table-cell; width:40px; height:30px; }",
    )
    .unwrap();
    let geometry = tree.layout(400.0, None).unwrap();
    assert_eq!(geometry[2].unwrap().border.left, 40.0);
}

#[test]
fn a_rows_generated_content_shares_its_anonymous_cell_with_its_text() {
    for display in ["table-row", "table-row-group", "table"] {
        let tree = BoxTree::new(
            node("d", vec![node("r", vec![text("y", "y")])]),
            &format!("#r {{ display:{display}; }} #r:before {{ content:\"x\"; }}"),
        )
        .unwrap();
        let leaves: Vec<String> = tree
            .text_leaves()
            .iter()
            .map(|(_, leaf)| leaf.text())
            .collect();
        assert_eq!(leaves, vec!["xy"], "{display}");
    }
}

#[test]
fn each_collapsed_grid_line_overlaps_by_its_own_widest_border() {
    let layout = |direction: &str| {
        let tree = BoxTree::new(
            node(
                "t",
                vec![node(
                    "r",
                    vec![node("a", vec![]), node("b", vec![]), node("c", vec![])],
                )],
            ),
            &format!(
                "#t {{ display:table; border-collapse:collapse; direction:{direction}; }}
                 #r {{ display:table-row; }}
                 #a, #b, #c {{ display:table-cell; width:40px; height:20px;
                               border:1px solid black; }}
                 #b {{ border-right:8px solid black; }} #c {{ border-left:8px solid black; }}"
            ),
        )
        .unwrap();
        let geometry = tree.layout(400.0, None).unwrap();
        [2, 3, 4].map(|at| geometry[at].unwrap().border)
    };
    // The 1px line overlaps 1px, the 8px line 8px, not the widest border everywhere.
    let [a, b, c] = layout("ltr");
    assert_eq!(a.right - b.left, 1.0);
    assert_eq!(b.right - c.left, 8.0);
    // In rtl the first column is on the right, so `#b`'s 8px right side is on the line it
    // shares with `#a`, and the line to `#c` is 1px.
    let [a, b, c] = layout("rtl");
    assert_eq!(b.right - a.left, 8.0, "rtl a/b");
    assert_eq!(c.right - b.left, 1.0, "rtl b/c");
}

#[test]
fn collapsed_rows_spans_and_table_width_follow_each_lines_own_width() {
    let attrs = AttributeLimits {
        entries: NonZeroUsize::new(100).unwrap(),
        retained_bytes: NonZeroUsize::new(100_000).unwrap(),
        work: NonZeroUsize::new(100_000).unwrap(),
    };
    let wide = node("e", vec![])
        .style_attribute("colspan", "2", attrs)
        .unwrap();
    let tree = BoxTree::new(
        node(
            "t",
            vec![
                node(
                    "r1",
                    vec![node("a", vec![]), node("b", vec![]), node("x", vec![])],
                ),
                node(
                    "r2",
                    vec![node("c", vec![]), node("d", vec![]), node("y", vec![])],
                ),
                node("r3", vec![wide, node("z", vec![])]),
            ],
        ),
        "#t { display:table; border-collapse:collapse; }
         #r1, #r2, #r3 { display:table-row; }
         #a, #b, #x, #c, #d, #y, #e, #z { display:table-cell; width:40px; height:20px;
                                          border:1px solid black; }
         #b { border-right:8px solid black; }
         #a, #b, #x { border-bottom:8px solid black; }",
    )
    .unwrap();
    let geometry = tree.layout(400.0, None).unwrap();
    let at = |index: usize| geometry[index].unwrap().border;
    let (a, b, c, e) = (at(2), at(3), at(6), at(10));
    // The table is measured with its own lines (1px and 8px across), so no slack reaches
    // the columns: `#a` stays its 40px plus two 1px borders.
    assert_eq!(a.right - a.left, 42.0);
    // And the table box ends where its last column does.
    assert_eq!(at(0).right, at(4).right);
    assert_eq!(a.right - b.left, 1.0);
    // Down: the 8px line under the first row, the 1px line under the second.
    assert_eq!(a.bottom - c.top, 8.0);
    assert_eq!(c.bottom - e.top, 1.0);
    // The spanning cell covers both columns and the 1px line inside it.
    assert_eq!((e.left, e.right), (a.left, b.right));
}

#[test]
fn a_collapsed_span_measures_the_lines_inside_it_and_its_own_top_sets_its_line() {
    let attrs = AttributeLimits {
        entries: NonZeroUsize::new(100).unwrap(),
        retained_bytes: NonZeroUsize::new(100_000).unwrap(),
        work: NonZeroUsize::new(100_000).unwrap(),
    };
    let wide = node("e", vec![])
        .style_attribute("colspan", "2", attrs)
        .unwrap();
    let tree = BoxTree::new(
        node(
            "t",
            vec![
                node("r1", vec![node("a", vec![]), node("b", vec![])]),
                node("r2", vec![wide]),
            ],
        ),
        "#t { display:table; border-collapse:collapse; }
         #r1, #r2 { display:table-row; }
         #a, #b, #e { display:table-cell; height:20px; border:1px solid black; }
         #a, #b { width:40px; }
         #e { width:150px; border-left:8px solid black; border-top:8px solid black; }",
    )
    .unwrap();
    let geometry = tree.layout(400.0, None).unwrap();
    let at = |index: usize| geometry[index].unwrap().border;
    let (a, b, e) = (at(2), at(3), at(5));
    // The columns grow just enough for `#e`: its 150px and borders, over the 1px line
    // inside it, not the table's widest vertical border.
    assert_eq!(e.right - e.left, 159.0);
    assert_eq!((e.left, e.right), (a.left, b.right));
    // `#e`'s own 8px top makes the line above its row 8px.
    assert_eq!(a.bottom - e.top, 8.0);
}

#[test]
fn an_empty_inline_with_sides_holds_a_zero_width_place() {
    let leaves = |root: Element, css: &str| -> Vec<String> {
        BoxTree::new(root, css)
            .unwrap()
            .text_leaves()
            .iter()
            .map(|(_, leaf)| leaf.text())
            .collect()
    };
    let spaced = || {
        node(
            "p",
            vec![text("a", "before "), node("s", vec![]), text("c", " after")],
        )
    };
    // Spaces collapse across it, as across any inline (16.6.1): one space before it, its
    // border's stand-in spaces around it, none after.
    assert_eq!(
        leaves(spaced(), "#s { display:inline; border:2px solid black; }"),
        vec!["before \u{2006}\u{200B}\u{2006}after".to_owned()]
    );
    // Without sides it is nothing at all.
    assert_eq!(
        leaves(spaced(), "#s { display:inline; }"),
        vec!["before after".to_owned()]
    );
    // Its side bars wrap that place.
    let tree = BoxTree::new(spaced(), "#s { display:inline; border:2px solid black; }").unwrap();
    let bars = tree
        .text_leaves()
        .iter()
        .flat_map(|(_, leaf)| leaf.runs.iter())
        .filter(|run| run.edge.is_some())
        .count();
    assert_eq!(bars, 2);
    // Before an inline-block it stays, in the line the inline-block opens.
    let row = node(
        "p",
        vec![node("s", vec![]), node("ib", vec![text("x", "X")])],
    );
    let texts = leaves(
        row,
        "#s { display:inline; border:2px solid black; } #ib { display:inline-block; }",
    );
    assert!(
        texts.iter().any(|text| text.contains('\u{200B}')),
        "{texts:?}"
    );
}

#[test]
fn table_parts_inside_an_inline_get_an_inline_table() {
    let tree = |inline: &str| {
        BoxTree::new(
            node(
                "p",
                vec![
                    text("x", "before "),
                    node("s", vec![node("a", vec![]), node("b", vec![])]),
                    text("y", " after"),
                ],
            ),
            &format!(
                "#p {{ width:400px; }} #s {{ display:{inline}; }}
                 #a, #b {{ display:table-cell; width:20px; height:10px; }}"
            ),
        )
        .unwrap()
    };
    // 17.2.1: in an inline element, the cells' anonymous table is an inline-table: on the
    // line, the text after it beside it, both cells sharing it. (Text here has no fonts,
    // so no width.)
    let g = tree("inline").layout(400.0, None).unwrap();
    let (a, b, after) = (
        g[3].unwrap().border,
        g[4].unwrap().border,
        g[5].unwrap().border,
    );
    assert_eq!((a.top, a.right), (b.top, b.left));
    assert_eq!(after.left, b.right);
    // In a block, a block-level table: the text after it starts a line of its own.
    let g = tree("block").layout(400.0, None).unwrap();
    assert_eq!(g[5].unwrap().border.left, 0.0);
}

#[test]
fn cells_in_different_inlines_get_inline_tables_of_their_own() {
    let tree = BoxTree::new(
        node(
            "p",
            vec![
                node("s", vec![node("a", vec![])]),
                node("t", vec![node("b", vec![])]),
            ],
        ),
        "#s, #t { display:inline; }
         #a, #b { display:table-cell; width:20px; }
         #a { height:10px; } #b { height:30px; }",
    )
    .unwrap();
    let g = tree.layout(400.0, None).unwrap();
    // One table each: the shorter cell keeps its height rather than its neighbour's row's.
    let (a, b) = (g[2].unwrap().border, g[4].unwrap().border);
    assert_eq!(a.bottom - a.top, 10.0, "{a:?}");
    assert_eq!(b.bottom - b.top, 30.0, "{b:?}");
}

#[test]
fn a_table_inside_an_inline_stays_block_level() {
    let tree = BoxTree::new(
        node(
            "p",
            vec![
                node("s", vec![node("t", vec![node("a", vec![])])]),
                text("y", " after"),
            ],
        ),
        "#s { display:inline; } #t { display:table; }
         #a { display:table-cell; width:20px; height:10px; }",
    )
    .unwrap();
    let g = tree.layout(400.0, None).unwrap();
    // A `table` element is block-level wherever it is: the text after it starts a line.
    assert_eq!(g[4].unwrap().border.left, 0.0);
}

#[test]
fn an_inline_block_opening_a_line_takes_its_indent() {
    let placed = |css: &str| {
        let tree = BoxTree::new(
            node("p", vec![node("b", vec![]), text("t", "after")]),
            &format!(
                "#p {{ width:200px; {css} }}
                 #b {{ display:inline-block; width:20px; height:10px; text-indent:0; }}"
            ),
        )
        .unwrap();
        tree.layout(400.0, None).unwrap()[1].unwrap().border
    };
    // 16.1: the first line of the line of inline-blocks starts its indent in.
    assert_eq!(placed("text-indent:30px;").left, 30.0);
    // From the right in a right-to-left block (which sits at the right of the 400px),
    // and out past the edge when negative.
    assert_eq!(placed("text-indent:30px; direction:rtl;").right, 370.0);
    assert_eq!(placed("text-indent:-10px;").left, -10.0);
    assert_eq!(placed("").left, 0.0);
}

#[test]
fn only_a_lines_first_row_of_inline_blocks_takes_its_indent() {
    // Its first line only: a box that wraps starts the next one at the left.
    let tree = BoxTree::new(
        node(
            "p",
            vec![node("a", vec![]), node("b", vec![]), node("c", vec![])],
        ),
        "#p { width:200px; text-indent:30px; }
         #a, #b, #c { display:inline-block; width:80px; height:10px; text-indent:0; }",
    )
    .unwrap();
    let g = tree.layout(400.0, None).unwrap();
    assert_eq!(g[1].unwrap().border.left, 30.0);
    assert_eq!(g[3].unwrap().border.left, 0.0);
    // A row after a block in it does not open the paragraph's first line.
    let tree = BoxTree::new(
        node("p", vec![node("d", vec![]), node("b", vec![])]),
        "#p { width:200px; text-indent:30px; } #d { height:10px; }
         #b { display:inline-block; width:20px; height:10px; text-indent:0; }",
    )
    .unwrap();
    let g = tree.layout(400.0, None).unwrap();
    assert_eq!(g[2].unwrap().border.left, 0.0);
}

#[test]
fn capitalize_finds_its_words_across_the_runs_of_a_leaf() {
    let leaf_text = |root, css: &str| {
        let tree = BoxTree::new(root, css).unwrap();
        tree.text_leaves()[0].1.text()
    };
    let css = "#page { text-transform:capitalize; } #b { display:inline; font-weight:bold; }";
    // An apostrophe inside a word starts no word.
    let root = node("page", vec![text("copy", "don't stop")]);
    assert_eq!(leaf_text(root, css), "Don't Stop");
    // Neither does an inline element's edge.
    let root = node(
        "page",
        vec![
            text("head", "w"),
            node("b", vec![text("mid", "or")]),
            text("tail", "d two"),
        ],
    );
    assert_eq!(leaf_text(root, css), "Word Two");
    // Nor a combining mark.
    let root = node("page", vec![text("copy", "cafe\u{301}s open")]);
    assert_eq!(leaf_text(root, css), "Cafe\u{301}s Open");
    // Punctuation before a word is passed over.
    let root = node("page", vec![text("copy", "\"quoted\" well-known")]);
    assert_eq!(leaf_text(root, css), "\"Quoted\" Well-Known");
}

#[test]
fn a_paragraphs_line_height_and_wrapping_are_its_blocks_not_its_first_inlines() {
    use iced_css::evaluation::tree_layout::TextLineHeight;
    let leaf = |css: &str| {
        let root = node(
            "page",
            vec![node(
                "p",
                vec![node("s", vec![text("a", "Tel:")]), text("b", " long text")],
            )],
        );
        let tree = BoxTree::new(root, css).unwrap();
        let leaves = tree.text_leaves();
        assert_eq!(leaves.len(), 1);
        (leaves[0].1.line_height, leaves[0].1.wraps)
    };
    // 10.8.1: the block's strut, not a leading inline's line height.
    assert_eq!(
        leaf("#p { line-height:40px; } #s { display:inline; line-height:10px; }"),
        (TextLineHeight::Px(40.0), true)
    );
    // 16.6: a leading inline's `nowrap` does not stop the text after it wrapping.
    assert!(leaf("#s { display:inline; white-space:nowrap; }").1);
    // Text that is `nowrap` throughout stays on its line, whichever box says so.
    assert!(!leaf("#p { white-space:nowrap; } #s { display:inline; }").1);
    assert!(!leaf("#s { display:inline; } #a, #b { white-space:nowrap; }").1);
}

#[test]
fn restyled_accepts_many_author_sheets() {
    let tree = BoxTree::new(page(), CSS).unwrap();
    let sheets: Vec<String> = (0..60)
        .map(|index| format!("#card {{ height:{index}px; }}"))
        .collect();
    let refs: Vec<&str> = sheets.iter().map(String::as_str).collect();
    assert!(tree.restyled(&refs).is_ok());
}

#[test]
fn a_captured_radio_keeps_only_its_label_as_text() {
    let radio = adapters::radio("Pick", 1u8, None, |_| (), capture_limits())
        .unwrap()
        .erase();
    let tree = BoxTree::new(node("page", vec![radio]), "").unwrap();
    let texts: Vec<String> = tree
        .text_leaves()
        .iter()
        .flat_map(|(_, leaf)| leaf.runs.iter().map(|run| run.text.clone()))
        .collect();
    assert_eq!(texts.concat(), "Pick");
}

#[test]
fn an_offset_inherits_through_a_static_parent_at_any_tree_position() {
    // `b` is the third row in preorder, past the two sibling-projection parents.
    let tree = BoxTree::new(
        node(
            "page",
            vec![node("a", vec![]), node("b", vec![node("c", vec![])])],
        ),
        "#b { top:7px; } #c { position:relative; top:inherit; height:10px; }",
    )
    .unwrap();
    let geometry = tree.layout(200.0, None).unwrap();
    assert_eq!(geometry[3].unwrap().border.top, 7.0);
}

#[test]
fn decoration_skips_atomic_inlines_and_inherit_takes_the_parents_own() {
    let root = node(
        "page",
        vec![
            text("copy", "x"),
            node("chip", vec![text("in", "y")]),
            node("side", vec![text("out", "z")]),
        ],
    );
    let tree = BoxTree::new(
        root,
        "#page { text-decoration:underline overline; }
         #chip { display:inline-block; }
         #side { float:left; text-decoration:inherit; }",
    )
    .unwrap();
    let leaves = tree.text_leaves();
    let run = |wanted: &str| {
        let run = &leaves
            .iter()
            .find(|(_, leaf)| leaf.text() == wanted)
            .unwrap()
            .1
            .runs[0];
        (run.underline, run.overline)
    };
    assert_eq!(run("x"), (true, true));
    // 16.3.1: not into the contents of an inline-block.
    assert_eq!(run("y"), (false, false));
    // A float carries none, but an explicit `inherit` is the parent's own value.
    assert_eq!(run("z"), (true, true));
}

#[test]
fn pseudo_display_and_clear_inherit_take_the_elements() {
    let root = node(
        "page",
        vec![
            node("wrap", vec![node("side", vec![])]),
            node("after", vec![]),
        ],
    );
    let tree = BoxTree::new(
        root,
        "#wrap { display:block; clear:both; }
         #wrap:after { content:\"\"; display:inherit; clear:inherit; }
         #side { float:left; width:10px; height:30px; }
         #after { height:1px; }",
    )
    .unwrap();
    let g = tree.layout_text(100.0, None, &mut |_, _| 0.0).unwrap();
    assert_eq!(g[1].unwrap().border.bottom, 30.0);
    assert_eq!(g[3].unwrap().border.top, 30.0);
}

#[test]
fn an_inside_marker_keeps_its_items_style_and_line() {
    let runs_of = |css: &str| {
        let tree = BoxTree::new(
            node(
                "page",
                vec![node("li", vec![node("x", vec![text("t", "item")])])],
            ),
            &format!(
                "#li {{ display:list-item; list-style:inside decimal; color:#0000ff;
                        font-size:10px; }}
                 #x {{ display:block; }}
                 #li:before {{ color:#ff0000; font-size:30px; margin:10px; {css} }}"
            ),
        )
        .unwrap();
        let g = tree.layout_text(100.0, None, &mut |_, _| 0.0).unwrap();
        let leaves = tree.text_leaves();
        let runs = &leaves[0].1.runs;
        (
            leaves.len(),
            runs[0].text.clone(),
            runs[0].color,
            runs[0].size,
            // The `:before`'s margin would move the item's block down.
            g[3].unwrap().border.top - g[1].unwrap().border.top,
        )
    };
    let plain = runs_of("").4;
    let marker = |leaves: usize| {
        (
            leaves,
            "1.\u{a0}".to_owned(),
            Some([0.0, 0.0, 1.0]),
            10.0,
            plain,
        )
    };
    // No `:before` box: the marker is the item's text, in the item's color and size.
    assert_eq!(runs_of(""), marker(2));
    // A hidden or empty block `:before` does not take the marker into a block of its own.
    assert_eq!(runs_of("display:none; content:\"x\";"), marker(2));
    assert_eq!(runs_of("display:block;"), marker(2));
    // With content, only the content takes the `:before`'s style.
    assert_eq!(runs_of("content:\"x\";"), marker(2));
}
