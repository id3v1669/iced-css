//! A measure, not a conformance suite: every CSS 2.2 longhand, applied in a page where it
//! should matter, must change what the tree computes, except the few known to be unread.
//! It shows a property is read, not that every value is right.
use iced::Theme;
use iced_css::{
    box_tree::{BoxTree, capture_limits},
    capture::{CapturedElement, adapters},
};
use iced_css_core::{source::AttributeLimits, values::registry::PropertyId};
use std::num::NonZeroUsize;

type Node = CapturedElement<'static, u8, Theme, iced::Renderer>;

fn attrs() -> AttributeLimits {
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
        .style_id(id, attrs())
        .unwrap()
        .erase()
}
fn text(id: &str, words: &'static str) -> Node {
    adapters::text(words, capture_limits())
        .unwrap()
        .style_id(id, attrs())
        .unwrap()
        .erase()
}

/// A page with something for every property to act on.
fn page() -> Node {
    node(
        "p",
        vec![
            node("fl", vec![]),
            node(
                "b",
                vec![
                    text("t", "one two three"),
                    node("k", vec![]),
                    text("u", "end"),
                ],
            ),
            node("li", vec![text("lt", "item")]),
            node(
                "tbl",
                vec![
                    node("cap", vec![text("ct", "caption")]),
                    node(
                        "r",
                        vec![
                            node("td1", vec![text("tt", "cell")]),
                            node("td2", vec![]),
                            node("td3", vec![]),
                        ],
                    ),
                ],
            ),
        ],
    )
}

const FIXTURE: &str = "
    #p { width:600px; }
    #fl { float:left; width:10px; height:100px; }
    #k { height:10px; }
    #li { display:list-item; }
    #tbl { display:table; width:300px; }
    #cap { display:table-caption; }
    #r { display:table-row; }
    #td1, #td2, #td3 { display:table-cell; }
    #td2 { height:40px; }
    #t, #u, #lt, #ct, #tt { display:inline; }
";

/// What a sheet computes, through the public surface.
fn computed(css: &str) -> String {
    let sheet = format!("{FIXTURE}{css}");
    let tree = BoxTree::new(page(), &sheet).unwrap();
    let geometry = tree
        .layout_text(800.0, Some(600.0), &mut |_, _| 20.0)
        .unwrap();
    format!(
        "{:?} {:?} {geometry:?} {:?} {:?} {:?}",
        tree.paints(),
        tree.text_leaves(),
        tree.markers(),
        tree.background_images(),
        tree.list_images(),
    )
}

const UNREAD: &[&str] = &[
    // Paged media: nothing here paginates.
    "orphans",
    "page-break-after",
    "page-break-before",
    "page-break-inside",
    "widows",
    // iced has no spacing between letters.
    "letter-spacing",
];

/// Property, what it needs around it, and a declaration that should show.
const CASES: &[(&str, &str, &str)] = &[
    (
        "background-attachment",
        "#b { background-image:url(x.png); }",
        "#b { background-attachment:fixed; }",
    ),
    ("background-color", "", "#b { background-color:#ff0000; }"),
    (
        "background-image",
        "",
        "#b { background-image:url(x.png); }",
    ),
    (
        "background-position",
        "#b { background-image:url(x.png); }",
        "#b { background-position:right; }",
    ),
    (
        "background-repeat",
        "#b { background-image:url(x.png); }",
        "#b { background-repeat:no-repeat; }",
    ),
    (
        "border-bottom-color",
        "#b { border-bottom:3px solid; }",
        "#b { border-bottom-color:#ff0000; }",
    ),
    (
        "border-bottom-style",
        "#b { border-bottom-width:3px; }",
        "#b { border-bottom-style:solid; }",
    ),
    (
        "border-bottom-width",
        "#b { border-bottom-style:solid; }",
        "#b { border-bottom-width:9px; }",
    ),
    (
        "border-collapse",
        "#td1, #td2 { border:2px solid; }",
        "#tbl { border-collapse:collapse; }",
    ),
    (
        "border-left-color",
        "#b { border-left:3px solid; }",
        "#b { border-left-color:#ff0000; }",
    ),
    (
        "border-left-style",
        "#b { border-left-width:3px; }",
        "#b { border-left-style:solid; }",
    ),
    (
        "border-left-width",
        "#b { border-left-style:solid; }",
        "#b { border-left-width:9px; }",
    ),
    (
        "border-right-color",
        "#b { border-right:3px solid; }",
        "#b { border-right-color:#ff0000; }",
    ),
    (
        "border-right-style",
        "#b { border-right-width:3px; }",
        "#b { border-right-style:solid; }",
    ),
    (
        "border-right-width",
        "#b { border-right-style:solid; }",
        "#b { border-right-width:9px; }",
    ),
    ("border-spacing", "", "#tbl { border-spacing:9px; }"),
    (
        "border-top-color",
        "#b { border-top:3px solid; }",
        "#b { border-top-color:#ff0000; }",
    ),
    (
        "border-top-style",
        "#b { border-top-width:3px; }",
        "#b { border-top-style:solid; }",
    ),
    (
        "border-top-width",
        "#b { border-top-style:solid; }",
        "#b { border-top-width:9px; }",
    ),
    ("bottom", "#b { position:relative; }", "#b { bottom:5px; }"),
    ("caption-side", "", "#cap { caption-side:bottom; }"),
    ("clear", "", "#b { clear:left; }"),
    (
        "clip",
        "#b { position:absolute; }",
        "#b { clip:rect(1px, 2px, 3px, 4px); }",
    ),
    ("color", "", "#b { color:#ff0000; }"),
    ("content", "", "#b:before { content:\"x\"; }"),
    (
        "counter-increment",
        "#b:before { content:counter(n); }",
        "#b { counter-increment:n; }",
    ),
    (
        "counter-reset",
        "#b { counter-increment:n; } #b:before { content:counter(n); }",
        "#p { counter-reset:n 5; }",
    ),
    ("cursor", "", "#b { cursor:pointer; }"),
    ("direction", "#k { width:100px; }", "#b { direction:rtl; }"),
    ("display", "", "#b { display:none; }"),
    ("empty-cells", "", "#tbl { empty-cells:hide; }"),
    ("float", "", "#b { float:right; }"),
    ("font-family", "", "#b { font-family:serif; }"),
    ("font-size", "", "#b { font-size:30px; }"),
    ("font-style", "", "#b { font-style:italic; }"),
    ("font-variant", "", "#b { font-variant:small-caps; }"),
    ("font-weight", "", "#b { font-weight:bold; }"),
    ("height", "", "#k { height:50px; }"),
    ("left", "#b { position:relative; }", "#b { left:5px; }"),
    ("letter-spacing", "", "#b { letter-spacing:5px; }"),
    ("line-height", "", "#b { line-height:3; }"),
    (
        "list-style-image",
        "",
        "#li { list-style-image:url(x.png); }",
    ),
    (
        "list-style-position",
        "",
        "#li { list-style-position:inside; }",
    ),
    ("list-style-type", "", "#li { list-style-type:square; }"),
    ("margin-bottom", "", "#b { margin-bottom:5px; }"),
    ("margin-left", "", "#b { margin-left:5px; }"),
    (
        "margin-right",
        "#b { width:auto; float:left; }",
        "#b { margin-right:5px; }",
    ),
    ("margin-top", "", "#b { margin-top:5px; }"),
    (
        "max-height",
        "#k { height:50px; }",
        "#k { max-height:5px; }",
    ),
    ("max-width", "", "#k { max-width:5px; }"),
    ("min-height", "", "#k { min-height:50px; }"),
    ("min-width", "#k { width:5px; }", "#k { min-width:50px; }"),
    ("orphans", "", "#b { orphans:5; }"),
    (
        "outline-color",
        "#b { outline:3px solid; }",
        "#b { outline-color:#ff0000; }",
    ),
    (
        "outline-style",
        "#b { outline-width:3px; }",
        "#b { outline-style:solid; }",
    ),
    (
        "outline-width",
        "#b { outline-style:solid; }",
        "#b { outline-width:9px; }",
    ),
    ("overflow", "", "#b { overflow:hidden; }"),
    ("padding-bottom", "", "#b { padding-bottom:5px; }"),
    ("padding-left", "", "#b { padding-left:5px; }"),
    ("padding-right", "", "#b { padding-right:5px; }"),
    ("padding-top", "", "#b { padding-top:5px; }"),
    ("page-break-after", "", "#b { page-break-after:always; }"),
    ("page-break-before", "", "#b { page-break-before:always; }"),
    ("page-break-inside", "", "#b { page-break-inside:avoid; }"),
    ("position", "#b { top:5px; }", "#b { position:relative; }"),
    (
        "quotes",
        "#b:before { content:open-quote; }",
        "#b { quotes:\"<\" \">\"; }",
    ),
    (
        "right",
        "#b { position:absolute; width:50px; }",
        "#b { right:5px; }",
    ),
    (
        "table-layout",
        "#td1 { width:50px; }",
        "#tbl { table-layout:fixed; }",
    ),
    ("text-align", "", "#b { text-align:right; }"),
    ("text-decoration", "", "#b { text-decoration:underline; }"),
    ("text-indent", "", "#b { text-indent:40px; }"),
    ("text-transform", "", "#b { text-transform:uppercase; }"),
    ("top", "#b { position:relative; }", "#b { top:5px; }"),
    (
        "unicode-bidi",
        "#t { direction:rtl; }",
        "#t { unicode-bidi:bidi-override; }",
    ),
    ("vertical-align", "", "#td1 { vertical-align:middle; }"),
    ("visibility", "", "#b { visibility:hidden; }"),
    ("white-space", "", "#b { white-space:nowrap; }"),
    ("widows", "", "#b { widows:5; }"),
    ("width", "", "#k { width:50px; }"),
    ("word-spacing", "", "#b { word-spacing:16px; }"),
    ("z-index", "#b { position:relative; }", "#b { z-index:5; }"),
    // Later modules.
    (
        "box-sizing",
        "#k { width:50px; padding:5px; }",
        "#k { box-sizing:border-box; }",
    ),
    (
        "border-top-left-radius",
        "",
        "#k { border-top-left-radius:5px; }",
    ),
    (
        "border-top-right-radius",
        "",
        "#k { border-top-right-radius:5px; }",
    ),
    (
        "border-bottom-right-radius",
        "",
        "#k { border-bottom-right-radius:5px; }",
    ),
    (
        "border-bottom-left-radius",
        "",
        "#k { border-bottom-left-radius:5px; }",
    ),
    // Every custom property, filed under one identity.
    ("--*", "#k { height:var(--h, 10px); }", "#k { --h:50px; }"),
];

#[test]
fn every_longhand_changes_the_result_or_is_known_not_to() {
    let longhands: Vec<&str> = PropertyId::all()
        .filter(|property| !property.descriptor().shorthand)
        .map(|property| property.descriptor().name)
        .collect();
    let named: Vec<&str> = CASES.iter().map(|case| case.0).collect();
    let missing: Vec<_> = longhands
        .iter()
        .filter(|name| !named.contains(name))
        .collect();
    assert!(missing.is_empty(), "no case for {missing:?}");
    assert_eq!(
        named.len(),
        longhands.len(),
        "a case is repeated or names no longhand"
    );

    let mut read = 0;
    let mut wrong = Vec::new();
    for (name, context, declaration) in CASES {
        let before = computed(context);
        let after = computed(&format!("{context} {declaration}"));
        let changed = before != after;
        if changed {
            read += 1;
        }
        if changed == UNREAD.contains(name) {
            wrong.push(*name);
        }
    }
    println!("{read} of {} longhands change the result", longhands.len());
    assert!(wrong.is_empty(), "behaves unlike its listing: {wrong:?}");
}
