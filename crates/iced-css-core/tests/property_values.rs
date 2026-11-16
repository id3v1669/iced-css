use iced_css_core::{
    syntax::*,
    values::{LengthTarget, NumericLimits, color::*, property::*},
};
use std::num::NonZeroUsize;
fn limits() -> ColorLimits {
    ColorLimits {
        syntax: StringLimits {
            retained_bytes: NonZeroUsize::new(4096).unwrap(),
            work: NonZeroUsize::new(100000).unwrap(),
        },
        numeric: NumericLimits {
            digits: NonZeroUsize::new(100).unwrap(),
            work: NonZeroUsize::new(100000).unwrap(),
        },
    }
}
fn parse(input: &str) -> Result<PropertyResult, PropertyError> {
    parse_with_limits(input, limits())
}
fn parse_with_limits(input: &str, l: ColorLimits) -> Result<PropertyResult, PropertyError> {
    let DeclarationItem::Declaration(d) = Declarations::new(
        input,
        DeclarationLimits {
            recovery: RecoveryLimits {
                work: l.syntax.work,
                nesting: NonZeroUsize::new(100).unwrap(),
                retained_bytes: l.syntax.retained_bytes,
            },
            name_bytes: l.syntax.retained_bytes,
        },
    )
    .next()
    .unwrap()
    .unwrap() else {
        panic!("{input:?}")
    };
    parse_property(&d, l)
}
#[test]
fn width_accepts_only_its_complete_css22_nonnegative_length_grammar() {
    for (input, value) in [
        ("width:0", "0"),
        ("width:auto", "auto"),
        ("width:2.54cm", "2.54cm"),
        ("width:12.5%", "12.5%"),
        (r"width:1p\78", "1px"),
        ("width:2EX", "2ex"),
    ] {
        let PropertyResult::Parsed(actual) = parse(input).unwrap() else {
            panic!("{input:?}")
        };
        assert_eq!(
            actual.value,
            SpecifiedValue::Length(LengthTarget::parse(value, limits().numeric).unwrap())
        );
    }
    for input in [
        "width:-.000001px",
        "width:-1%",
        "width:1",
        "width:1e2px",
        "width:1e+2px",
        "width:1/**/px",
        "width:f(a;b)",
        "width:1px 2px",
        "width:initial",
        "width:none",
    ] {
        assert_eq!(parse(input).unwrap(), PropertyResult::Invalid, "{input:?}");
    }
}
#[test]
fn inherited_and_color_values_preserve_priority() {
    let PropertyResult::Parsed(actual) = parse(r"color:\69 nherit !important").unwrap() else {
        panic!()
    };
    assert_eq!(actual.value, SpecifiedValue::Inherit);
    assert!(actual.important);
    let PropertyResult::Parsed(actual) = parse("color:green").unwrap() else {
        panic!()
    };
    assert_eq!(
        actual.value,
        SpecifiedValue::Color(Color::Srgb([0, 128, 0]))
    );
}
#[test]
fn unresolved_grammar_is_an_explicit_implementation_error() {
    assert!(matches!(
        parse("font-weight:+100"),
        Err(PropertyError::UnimplementedNumericSpelling(_))
    ));
    assert_eq!(
        parse("future-property:10px").unwrap(),
        PropertyResult::Unknown
    );
}
#[test]
fn nested_invalid_width_does_not_hide_following_color() {
    let l = limits();
    let items = Declarations::new(
        "width:f(a;b);color:green",
        DeclarationLimits {
            recovery: RecoveryLimits {
                work: l.syntax.work,
                nesting: NonZeroUsize::new(100).unwrap(),
                retained_bytes: l.syntax.retained_bytes,
            },
            name_bytes: l.syntax.retained_bytes,
        },
    );
    let results: Vec<_> = items
        .map(|item| {
            let DeclarationItem::Declaration(d) = item.unwrap() else {
                panic!()
            };
            parse_property(&d, l).unwrap()
        })
        .collect();
    assert_eq!(results[0], PropertyResult::Invalid);
    assert!(matches!(results[1], PropertyResult::Parsed(_)));
}

#[test]
fn complete_keyword_property_grammars_are_scoped_to_their_own_literals() {
    for (name, value) in [
        ("background-attachment", "fixed"),
        ("background-repeat", "repeat-x"),
        ("border-collapse", "collapse"),
        ("caption-side", "bottom"),
        ("clear", "both"),
        ("direction", "rtl"),
        ("display", "table-cell"),
        ("empty-cells", "hide"),
        ("float", "right"),
        ("font-style", "oblique"),
        ("font-variant", "small-caps"),
        ("list-style-position", "inside"),
        ("list-style-type", "armenian"),
        ("overflow", "auto"),
        ("page-break-after", "always"),
        ("page-break-before", "right"),
        ("page-break-inside", "avoid"),
        ("position", "fixed"),
        ("table-layout", "fixed"),
        ("text-align", "justify"),
        ("text-transform", "capitalize"),
        ("unicode-bidi", "bidi-override"),
        ("visibility", "collapse"),
        ("white-space", "pre-line"),
    ] {
        let PropertyResult::Parsed(actual) = parse(&format!("{name}:{value}")).unwrap() else {
            panic!("{name}")
        };
        let SpecifiedValue::Keyword(actual) = actual.value else {
            panic!("{name}")
        };
        assert_eq!(actual.as_str(), value);
    }
    for input in [
        "display:contents",
        "display:run-in",
        "white-space:block",
        "display:'block'",
        "float:left right",
        "position:sticky",
    ] {
        assert_eq!(parse(input).unwrap(), PropertyResult::Invalid, "{input}");
    }
    let PropertyResult::Parsed(actual) = parse(r"display:\62 LOCK").unwrap() else {
        panic!()
    };
    let SpecifiedValue::Keyword(actual) = actual.value else {
        panic!()
    };
    assert_eq!(actual.as_str(), "block");
}

#[test]
fn length_property_domains_preserve_signed_geometry_and_reject_invalid_ranges() {
    for input in [
        "height:10px",
        "min-width:0",
        "max-width:none",
        "min-height:2%",
        "max-height:3em",
        "top:-2px",
        "right:auto",
        "bottom:-1%",
        "left:2.54cm",
        "margin-top:-3px",
        "margin-right:auto",
        "margin-bottom:4%",
        "margin-left:0",
        "padding-top:1%",
        "padding-right:2em",
        "padding-bottom:0",
        "padding-left:1px",
        "text-indent:-2%",
        "letter-spacing:-1px",
        "word-spacing:normal",
        "font-size:larger",
        "font-size:1p\\78",
        "border-top-width:thin",
        "border-right-width:2px",
        "border-bottom-width:medium",
        "border-left-width:0",
        "outline-width:thick",
        "vertical-align:-25%",
        "vertical-align:text-bottom",
        "line-height:1.25",
        "line-height:120%",
        "line-height:normal",
        "border-spacing:1px 2px",
        "border-spacing:0",
    ] {
        assert!(
            matches!(parse(input).unwrap(), PropertyResult::Parsed(_)),
            "{input}"
        );
    }
    for input in [
        "height:-1px",
        "min-width:auto",
        "max-height:-1%",
        "padding-top:-0.00001em",
        "padding-left:auto",
        "font-size:-0.00001%",
        "font-size:xxx-large",
        "border-left-width:1%",
        "outline-width:-1px",
        "letter-spacing:1%",
        "line-height:-.1",
        "line-height:1e2",
        "border-spacing:-1px",
        "border-spacing:1%",
        "border-spacing:1px 2px 3px",
    ] {
        assert_eq!(parse(input).unwrap(), PropertyResult::Invalid, "{input}");
    }
}

#[test]
fn color_and_border_style_longhands_apply_their_own_special_keywords() {
    for name in [
        "background-color",
        "border-top-color",
        "border-right-color",
        "border-bottom-color",
        "border-left-color",
    ] {
        let PropertyResult::Parsed(actual) = parse(&format!("{name}:transparent")).unwrap() else {
            panic!()
        };
        assert_eq!(actual.value, SpecifiedValue::Color(Color::Transparent));
        assert!(matches!(
            parse(&format!("{name}:rgb(1,2,3)")).unwrap(),
            PropertyResult::Parsed(_)
        ));
    }
    let PropertyResult::Parsed(actual) = parse("outline-color:invert").unwrap() else {
        panic!()
    };
    assert_eq!(actual.value, SpecifiedValue::Color(Color::Invert));
    for name in [
        "border-top-style",
        "border-right-style",
        "border-bottom-style",
        "border-left-style",
    ] {
        for style in [
            "none", "hidden", "dotted", "dashed", "solid", "double", "groove", "ridge", "inset",
            "outset",
        ] {
            assert!(matches!(
                parse(&format!("{name}:{style}")).unwrap(),
                PropertyResult::Parsed(_)
            ));
        }
    }
    assert!(matches!(
        parse("outline-style:dashed").unwrap(),
        PropertyResult::Parsed(_)
    ));
    for input in [
        "outline-style:hidden",
        "outline-style:solid dotted",
        "background-color:invert",
        "border-left-color:invert",
        "border-top-style:transparent",
    ] {
        assert_eq!(parse(input).unwrap(), PropertyResult::Invalid, "{input}");
    }
}

#[test]
fn integer_property_domains_are_exact_and_property_specific() {
    for (name, raw) in [
        ("z-index", "-999999999999999999999999999999"),
        ("z-index", "+000"),
        ("orphans", "+001"),
        ("widows", "999999999999999999999999999999"),
    ] {
        let PropertyResult::Parsed(actual) = parse(&format!("{name}:{raw}")).unwrap() else {
            panic!()
        };
        assert_eq!(actual.property.descriptor().name, name);
        assert_eq!(
            actual.value,
            SpecifiedValue::Integer(
                iced_css_core::values::Integer::parse(raw, limits().numeric).unwrap()
            )
        );
    }
    assert!(matches!(
        parse("z-index:auto").unwrap(),
        PropertyResult::Parsed(_)
    ));
    for input in [
        "orphans:0",
        "widows:-0",
        "orphans:-1",
        "widows:auto",
        "z-index:1.0",
        "z-index:1e2",
        "z-index:1px",
        "z-index:1 2",
    ] {
        assert_eq!(parse(input).unwrap(), PropertyResult::Invalid, "{input}");
    }
}

#[test]
fn border_spacing_preserves_both_exact_operands() {
    for (raw, first, second) in [("1px", "1px", "1px"), ("1px 2em", "1px", "2em")] {
        let PropertyResult::Parsed(actual) = parse(&format!("border-spacing:{raw}")).unwrap()
        else {
            panic!()
        };
        assert_eq!(
            actual.value,
            SpecifiedValue::LengthPair(
                LengthTarget::parse(first, limits().numeric).unwrap(),
                LengthTarget::parse(second, limits().numeric).unwrap(),
            )
        );
    }
}

#[test]
fn text_decoration_accepts_an_unordered_unique_set_or_none() {
    for input in [
        "text-decoration:underline",
        "text-decoration:blink overline underline line-through",
        r"text-decoration:\75 nderline/**/overline",
        "text-decoration:none",
        "text-decoration:inherit",
    ] {
        assert!(
            matches!(parse(input).unwrap(), PropertyResult::Parsed(_)),
            "{input}"
        );
    }
    for input in [
        "text-decoration:underline underline",
        "text-decoration:none underline",
        "text-decoration:underline,overline",
        "text-decoration:solid",
        "text-decoration:inherit blink",
        "text-decoration:'underline'",
    ] {
        assert_eq!(parse(input).unwrap(), PropertyResult::Invalid, "{input}");
    }
    let PropertyResult::Parsed(first) = parse("text-decoration:underline overline").unwrap() else {
        panic!()
    };
    let PropertyResult::Parsed(second) = parse("text-decoration:overline underline").unwrap()
    else {
        panic!()
    };
    assert_eq!(first.value, second.value);
}

#[test]
fn image_longhands_admit_one_uri_or_none() {
    for name in ["background-image", "list-style-image"] {
        for value in ["url(a.png)", "url('a b')", "none", "inherit"] {
            assert!(matches!(
                parse(&format!("{name}:{value}")).unwrap(),
                PropertyResult::Parsed(_)
            ));
        }
        for value in ["url(a) url(b)", "red", "'a.png'"] {
            assert_eq!(
                parse(&format!("{name}:{value}")).unwrap(),
                PropertyResult::Invalid
            );
        }
        // CSS Images 3: a gradient is a background image, not a list marker.
        assert_eq!(
            matches!(
                parse(&format!("{name}:linear-gradient(red,blue)")).unwrap(),
                PropertyResult::Parsed(_)
            ),
            name == "background-image"
        );
    }
}

#[test]
fn image_uri_retains_decoded_relative_reference_without_resolution() {
    let PropertyResult::Parsed(actual) = parse(r"background-image:url(../a\20 b.png)").unwrap()
    else {
        panic!()
    };
    assert_eq!(actual.value, SpecifiedValue::Uri("../a b.png".into()));
}

#[test]
fn quotes_require_ordered_string_pairs_or_none() {
    for value in [r#"'«' '»' '‹' '›'"#, "'' ''", "none", "inherit", "'a' 'b"] {
        assert!(
            matches!(
                parse(&format!("quotes:{value}")).unwrap(),
                PropertyResult::Parsed(_)
            ),
            "{value}"
        );
    }
    for value in ["'a'", "'a' 'b' 'c'", "none 'a'", "open-quote", "'a', 'b'"] {
        assert_eq!(
            parse(&format!("quotes:{value}")).unwrap(),
            PropertyResult::Invalid,
            "{value}"
        );
    }
}

#[test]
fn quotes_retain_order_and_bound_all_owned_capacities() {
    let input = "quotes:'a' 'b' 'c' 'd'";
    let PropertyResult::Parsed(actual) = parse(input).unwrap() else {
        panic!()
    };
    assert_eq!(
        actual.value,
        SpecifiedValue::Quotes(vec![["a".into(), "b".into()], ["c".into(), "d".into()]])
    );
    for bytes in 32..256 {
        let mut small = limits();
        small.syntax.retained_bytes = NonZeroUsize::new(bytes).unwrap();
        match parse_with_limits(input, small) {
            Ok(PropertyResult::Parsed(actual)) => {
                let SpecifiedValue::Quotes(pairs) = actual.value else {
                    panic!()
                };
                let retained = pairs.capacity() * size_of::<[String; 2]>()
                    + pairs.iter().flatten().map(String::capacity).sum::<usize>();
                assert!(retained <= bytes);
            }
            Err(PropertyError::Syntax(SyntaxError {
                fault: SyntaxFault::Resource(SyntaxResource::Bytes),
                ..
            })) => {}
            other => panic!("{bytes}: {other:?}"),
        }
    }
}

#[test]
fn counter_declarations_preserve_names_defaults_and_signed_values() {
    for input in [
        "counter-reset:chapter section 2",
        "counter-increment:chapter -1 chapter 0",
        r"counter-reset:\63 hapter +0002",
        "counter-reset:none",
        "counter-increment:inherit",
    ] {
        assert!(
            matches!(parse(input).unwrap(), PropertyResult::Parsed(_)),
            "{input}"
        );
    }
    for input in [
        "counter-reset:initial",
        "counter-reset:a none",
        "counter-increment:inherit 1",
        "counter-reset:a 1.0",
        "counter-reset:a 1e2",
        "counter-reset:a, b",
        "counter-reset:'a'",
        "counter-reset:1 a",
    ] {
        assert_eq!(parse(input).unwrap(), PropertyResult::Invalid, "{input}");
    }
}

#[test]
fn counter_values_keep_source_order_repetitions_and_exact_operands() {
    for (property, default) in [("counter-reset", "0"), ("counter-increment", "1")] {
        let PropertyResult::Parsed(actual) = parse(&format!(
            "{property}:Chapter Chapter -99999999999999999999 chapter"
        ))
        .unwrap() else {
            panic!()
        };
        let SpecifiedValue::Counters(actual) = actual.value else {
            panic!()
        };
        assert_eq!(actual.len(), 3);
        for (entry, name, value) in [
            (&actual[0], "Chapter", default),
            (&actual[1], "Chapter", "-99999999999999999999"),
            (&actual[2], "chapter", default),
        ] {
            assert_eq!(entry.name, name);
            assert_eq!(
                entry.value,
                iced_css_core::values::Integer::parse(value, limits().numeric).unwrap()
            );
        }
    }
    let input = "counter-reset:a 1 b -2 c";
    for bytes in 32..512 {
        let mut small = limits();
        small.syntax.retained_bytes = NonZeroUsize::new(bytes).unwrap();
        match parse_with_limits(input, small) {
            Ok(PropertyResult::Parsed(actual)) => {
                let SpecifiedValue::Counters(entries) = actual.value else {
                    panic!()
                };
                assert!(
                    entries.capacity() * size_of::<CounterDirective>()
                        + entries.iter().map(|e| e.name.capacity()).sum::<usize>()
                        <= bytes
                );
            }
            Err(PropertyError::Syntax(SyntaxError {
                fault: SyntaxFault::Resource(SyntaxResource::Bytes),
                ..
            })) => {}
            other => panic!("{bytes}: {other:?}"),
        }
    }
}

#[test]
fn background_position_resolves_keyword_axes_and_preserves_exact_lengths() {
    for (value, x, y) in [
        ("right top", "100%", "0%"),
        ("top center", "50%", "0%"),
        ("center", "50%", "50%"),
        ("bottom", "50%", "100%"),
        ("-1px", "-1px", "50%"),
        ("left -20%", "0%", "-20%"),
        ("2em bottom", "2em", "100%"),
        ("center center", "50%", "50%"),
        ("bottom right", "100%", "100%"),
    ] {
        let PropertyResult::Parsed(actual) =
            parse(&format!("background-position:{value}")).unwrap()
        else {
            panic!("{value}")
        };
        assert_eq!(
            actual.value,
            SpecifiedValue::LengthPair(
                LengthTarget::parse(x, limits().numeric).unwrap(),
                LengthTarget::parse(y, limits().numeric).unwrap()
            ),
            "{value}"
        );
    }
    for value in [
        "top 1px",
        "1px left",
        "left right",
        "top bottom",
        "left top 1px",
        "auto",
        "1",
        "1e2px",
    ] {
        assert_eq!(
            parse(&format!("background-position:{value}")).unwrap(),
            PropertyResult::Invalid,
            "{value}"
        );
    }
}

#[test]
fn clip_rect_accepts_four_length_or_auto_edges_with_uniform_separators() {
    for value in [
        "rect(0, 1px, auto, -2em)",
        "rect(0 1px auto -2em)",
        r"r\65 ct(0,0,0,0)",
        "rect(0,0,0,0",
        "auto",
        "inherit",
    ] {
        assert!(
            matches!(
                parse(&format!("clip:{value}")).unwrap(),
                PropertyResult::Parsed(_)
            ),
            "{value}"
        );
    }
    for value in [
        "rect(0,0 0,0)",
        "rect(0 0,0 0)",
        "rect(0,0,0)",
        "rect(0,0,0,0,0)",
        "rect(1%,0,0,0)",
        "rect(1,0,0,0)",
        "rect(0,0,0,0) auto",
        "circle(0,0,0,0)",
    ] {
        assert_eq!(
            parse(&format!("clip:{value}")).unwrap(),
            PropertyResult::Invalid,
            "{value}"
        );
    }
}

#[test]
fn cursor_requires_a_css22_fallback_after_each_comma_separated_uri_list() {
    for value in [
        "auto",
        "pointer",
        "ne-resize",
        "progress",
        "url(a), url('b'), help",
        r"url(a), \74 ext",
        "inherit",
    ] {
        assert!(
            matches!(
                parse(&format!("cursor:{value}")).unwrap(),
                PropertyResult::Parsed(_)
            ),
            "{value}"
        );
    }
    for value in [
        "url(a)",
        "url(a) pointer",
        "url(a),",
        "url(a), inherit",
        "pointer,help",
        "grab",
        "zoom-in",
        "url(a) 1 2, auto",
    ] {
        assert_eq!(
            parse(&format!("cursor:{value}")).unwrap(),
            PropertyResult::Invalid,
            "{value}"
        );
    }
}

#[test]
fn cursor_retains_uri_priority_and_named_fallback_within_capacity_budget() {
    let input = "cursor:url(a), url(b), pointer";
    for bytes in 32..256 {
        let mut small = limits();
        small.syntax.retained_bytes = NonZeroUsize::new(bytes).unwrap();
        match parse_with_limits(input, small) {
            Ok(PropertyResult::Parsed(actual)) => {
                let SpecifiedValue::Cursor(cursor) = actual.value else {
                    panic!()
                };
                assert_eq!(cursor.uris, ["a", "b"]);
                assert_eq!(cursor.fallback.as_str(), "pointer");
                assert!(
                    cursor.uris.capacity() * size_of::<String>()
                        + cursor.uris.iter().map(String::capacity).sum::<usize>()
                        <= bytes
                );
            }
            Err(PropertyError::Syntax(SyntaxError {
                fault: SyntaxFault::Resource(SyntaxResource::Bytes),
                ..
            })) => {}
            other => panic!("{bytes}: {other:?}"),
        }
    }
}

#[test]
fn font_family_requires_a_prioritized_list_of_names_or_generics() {
    for value in [
        r#""New Century Schoolbook", serif"#,
        "Gill Sans, sans-serif",
        "'serif', SERIF",
        "Times inherit",
        "inherit Times",
        r"\69 nherit Times",
        r"\32 1st Century, fantasy",
        "inherit",
    ] {
        assert!(
            matches!(
                parse(&format!("font-family:{value}")).unwrap(),
                PropertyResult::Parsed(_)
            ),
            "{value}"
        );
    }
    for value in [
        "Red/Black, sans-serif",
        "\"Lucida\" Grande, sans-serif",
        "Ahem!",
        "test@foo",
        "#POUND",
        "Hawaii 5-0",
        "Times, inherit",
        "default",
        "initial",
        "serif,",
        ",serif",
    ] {
        assert_eq!(
            parse(&format!("font-family:{value}")).unwrap(),
            PropertyResult::Invalid,
            "{value}"
        );
    }
}

#[test]
fn font_family_distinguishes_named_and_generic_entries_and_bounds_nested_storage() {
    let input = "font-family:'serif', Gill Sans, SERIF";
    for bytes in 32..512 {
        let mut small = limits();
        small.syntax.retained_bytes = NonZeroUsize::new(bytes).unwrap();
        match parse_with_limits(input, small) {
            Ok(PropertyResult::Parsed(actual)) => {
                let SpecifiedValue::FontFamily(families) = actual.value else {
                    panic!()
                };
                assert_eq!(families.len(), 3);
                assert_eq!(families[0], FontFamily::Quoted("serif".into()));
                assert_eq!(
                    families[1],
                    FontFamily::Identifiers(vec!["Gill".into(), "Sans".into()])
                );
                let FontFamily::Generic(generic) = families[2] else {
                    panic!()
                };
                assert_eq!(generic.as_str(), "serif");
                let retained = families.capacity() * size_of::<FontFamily>()
                    + families
                        .iter()
                        .map(|family| match family {
                            FontFamily::Quoted(name) => name.capacity(),
                            FontFamily::Identifiers(names) => {
                                names.capacity() * size_of::<String>()
                                    + names.iter().map(String::capacity).sum::<usize>()
                            }
                            FontFamily::Generic(_) => 0,
                        })
                        .sum::<usize>();
                assert!(retained <= bytes);
            }
            Err(PropertyError::Syntax(SyntaxError {
                fault: SyntaxFault::Resource(SyntaxResource::Bytes),
                ..
            })) => {}
            other => panic!("{bytes}: {other:?}"),
        }
    }
}

#[test]
fn generated_content_admits_css22_items_and_validates_function_arguments() {
    for value in [
        "normal",
        "none",
        "inherit",
        "'Chapter ' counter(chapter) '. '",
        "counters(section, '.', upper-roman)",
        "counter(x, none)",
        "attr(title) url(a) open-quote no-close-quote",
        "attr(title",
        "counter(x",
        "counters(x, '.'",
    ] {
        assert!(
            matches!(
                parse(&format!("content:{value}")).unwrap(),
                PropertyResult::Parsed(_)
            ),
            "{value}"
        );
    }
    for value in [
        "none 'x'",
        "normal open-quote",
        "attr()",
        "attr('title')",
        "attr(title, x)",
        "counter(none)",
        "counter(x, inherit)",
        "counter(x, custom)",
        "counters(x)",
        "counters(x, '.') junk",
        "counter(x,)",
        "url(a), 'b'",
        "1",
        "env(x)",
    ] {
        assert_eq!(
            parse(&format!("content:{value}")).unwrap(),
            PropertyResult::Invalid,
            "{value}"
        );
    }
}

#[test]
fn generated_content_retains_order_names_styles_and_bounded_payloads() {
    let input = "content:'a' attr(Title) counters(X, '.', none) url(b) open-quote";
    for bytes in 32..1024 {
        let mut small = limits();
        small.syntax.retained_bytes = NonZeroUsize::new(bytes).unwrap();
        match parse_with_limits(input, small) {
            Ok(PropertyResult::Parsed(actual)) => {
                let SpecifiedValue::Content(items) = actual.value else {
                    panic!()
                };
                assert_eq!(items.len(), 5);
                assert_eq!(items[0], ContentItem::Text("a".into()));
                assert_eq!(items[1], ContentItem::Attribute("Title".into()));
                let ContentItem::Counter {
                    name,
                    separator,
                    style,
                } = &items[2]
                else {
                    panic!()
                };
                assert_eq!(name, "X");
                assert_eq!(separator.as_deref(), Some("."));
                assert_eq!(style.as_str(), "none");
                assert_eq!(items[3], ContentItem::Uri("b".into()));
                let ContentItem::Quote(quote) = items[4] else {
                    panic!()
                };
                assert_eq!(quote.as_str(), "open-quote");
                let retained = items.capacity() * size_of::<ContentItem>()
                    + items
                        .iter()
                        .map(|item| match item {
                            ContentItem::Text(s)
                            | ContentItem::Uri(s)
                            | ContentItem::Attribute(s) => s.capacity(),
                            ContentItem::Counter {
                                name, separator, ..
                            } => name.capacity() + separator.as_ref().map_or(0, String::capacity),
                            ContentItem::Quote(_) => 0,
                        })
                        .sum::<usize>();
                assert!(retained <= bytes);
            }
            Err(PropertyError::Syntax(SyntaxError {
                fault: SyntaxFault::Resource(SyntaxResource::Bytes),
                ..
            })) => {}
            other => panic!("{bytes}: {other:?}"),
        }
    }
}

#[test]
fn four_side_shorthands_accept_one_to_four_valid_components_atomically() {
    for input in [
        "margin:-1px auto 2%",
        "padding:1px 2px 3px 4px",
        "border-width:thin 2px thick",
        "border-style:solid hidden",
        "margin:inherit !important",
    ] {
        assert!(
            matches!(parse(input).unwrap(), PropertyResult::Parsed(_)),
            "{input}"
        );
    }
    for input in [
        "margin:1px 2px 3px 4px 5px",
        "padding:1px -1px",
        "border-width:thin 1%",
        "border-style:solid purple",
        "margin:inherit auto",
        "padding:auto",
    ] {
        assert_eq!(parse(input).unwrap(), PropertyResult::Invalid, "{input}");
    }
}

#[test]
fn side_expansion_sets_each_longhand_and_preserves_important() {
    for (value, expected) in [
        ("1px", ["1px", "1px", "1px", "1px"]),
        ("1px 2px", ["1px", "2px", "1px", "2px"]),
        ("1px 2px 3px", ["1px", "2px", "3px", "2px"]),
        ("1px 2px 3px 4px", ["1px", "2px", "3px", "4px"]),
    ] {
        let PropertyResult::Parsed(actual) = parse(&format!("margin:{value} !important")).unwrap()
        else {
            panic!()
        };
        let SpecifiedValue::Expanded(leaves) = actual.value else {
            panic!()
        };
        assert_eq!(leaves.len(), 4);
        for ((leaf, target), expected) in leaves
            .iter()
            .zip(["margin-top", "margin-right", "margin-bottom", "margin-left"])
            .zip(expected)
        {
            assert_eq!(leaf.property.descriptor().name, target);
            assert!(leaf.important);
            assert_eq!(
                leaf.value,
                SpecifiedValue::Length(LengthTarget::parse(expected, limits().numeric).unwrap())
            );
        }
    }
    let PropertyResult::Parsed(actual) = parse("padding:inherit !important").unwrap() else {
        panic!()
    };
    let SpecifiedValue::Expanded(leaves) = actual.value else {
        panic!()
    };
    assert!(
        leaves
            .iter()
            .all(|leaf| leaf.important && leaf.value == SpecifiedValue::Inherit)
    );
}

#[test]
fn border_color_shorthand_keeps_color_functions_as_single_components() {
    for value in ["red", "rgb(1,2,3) transparent blue", "rgb(1,2,3", "inherit"] {
        assert!(
            matches!(
                parse(&format!("border-color:{value}")).unwrap(),
                PropertyResult::Parsed(_)
            ),
            "{value}"
        );
    }
    for value in [
        "red green blue white black",
        "rgb(1,2) red",
        "rgb(f(1),2,3)",
        "red inherit",
        "invert",
        "red,blue",
    ] {
        assert_eq!(
            parse(&format!("border-color:{value}")).unwrap(),
            PropertyResult::Invalid,
            "{value}"
        );
    }
    let PropertyResult::Parsed(actual) = parse("border-color:red rgb(1,2,3) transparent").unwrap()
    else {
        panic!()
    };
    let SpecifiedValue::Expanded(leaves) = actual.value else {
        panic!()
    };
    assert_eq!(
        leaves[0].value,
        SpecifiedValue::Color(Color::Srgb([255, 0, 0]))
    );
    assert_eq!(leaves[2].value, SpecifiedValue::Color(Color::Transparent));
    assert_eq!(leaves[1].value, leaves[3].value);
}

#[test]
fn border_and_outline_shorthands_require_unique_roles_and_reset_omissions() {
    for input in [
        "border:red solid 2px",
        "border-top:thick",
        "border-right:hidden transparent",
        "border-bottom:inherit",
        "border-left:rgb(1,2,3) solid",
        "outline:invert dashed 1px",
        "outline:inherit",
    ] {
        assert!(
            matches!(parse(input).unwrap(), PropertyResult::Parsed(_)),
            "{input}"
        );
    }
    for input in [
        "border:solid dashed",
        "border:1px 2px",
        "border:red blue",
        "border:inherit solid",
        "outline:hidden",
        "border:invert",
        "border:solid 1%",
        "outline:initial",
    ] {
        assert_eq!(parse(input).unwrap(), PropertyResult::Invalid, "{input}");
    }
}

#[test]
fn border_expansion_resets_every_omitted_member_and_sets_all_four_sides() {
    let PropertyResult::Parsed(actual) = parse("border:solid !important").unwrap() else {
        panic!()
    };
    let SpecifiedValue::Expanded(leaves) = actual.value else {
        panic!()
    };
    assert_eq!(leaves.len(), 12);
    for leaf in leaves {
        assert!(leaf.important);
        if leaf.property.descriptor().name.ends_with("-style") {
            let SpecifiedValue::Keyword(style) = leaf.value else {
                panic!()
            };
            assert_eq!(style.as_str(), "solid");
        } else {
            assert_eq!(leaf.value, SpecifiedValue::Initial);
        }
    }
    let PropertyResult::Parsed(actual) = parse("outline:inherit !important").unwrap() else {
        panic!()
    };
    let SpecifiedValue::Expanded(leaves) = actual.value else {
        panic!()
    };
    assert_eq!(leaves.len(), 3);
    assert!(
        leaves
            .iter()
            .all(|leaf| leaf.important && leaf.value == SpecifiedValue::Inherit)
    );
    let PropertyResult::Parsed(actual) = parse("border-left:red").unwrap() else {
        panic!()
    };
    let SpecifiedValue::Expanded(leaves) = actual.value else {
        panic!()
    };
    assert_eq!(leaves.len(), 3);
    assert!(
        leaves
            .iter()
            .all(|leaf| leaf.property.descriptor().name.starts_with("border-left-"))
    );
}

#[test]
fn background_shorthand_combines_css22_roles_and_resets_omissions() {
    for value in [
        "red",
        "url(chess.png) gray 50% repeat fixed",
        "blue url(a) repeat-x scroll left top",
        "bottom right red",
        "inherit",
    ] {
        assert!(
            matches!(
                parse(&format!("background:{value}")).unwrap(),
                PropertyResult::Parsed(_)
            ),
            "{value}"
        );
    }
    for value in [
        "red blue",
        "url(a) url(b)",
        "repeat no-repeat",
        "fixed scroll",
        "left top right",
        "red / cover",
        "red, blue",
        "padding-box",
        "inherit red",
    ] {
        assert_eq!(
            parse(&format!("background:{value}")).unwrap(),
            PropertyResult::Invalid,
            "{value}"
        );
    }
}

#[test]
fn background_expansion_preserves_position_uri_priority_and_storage_bound() {
    let input = "background:url(relative.png) left top !important";
    for bytes in (128..4096).step_by(31) {
        let mut small = limits();
        small.syntax.retained_bytes = NonZeroUsize::new(bytes).unwrap();
        match parse_with_limits(input, small) {
            Ok(PropertyResult::Parsed(actual)) => {
                let SpecifiedValue::Expanded(leaves) = actual.value else {
                    panic!()
                };
                assert_eq!(leaves.len(), 5);
                assert!(leaves.iter().all(|leaf| leaf.important));
                let image = leaves
                    .iter()
                    .find(|leaf| leaf.property.descriptor().name == "background-image")
                    .unwrap();
                assert_eq!(image.value, SpecifiedValue::Uri("relative.png".into()));
                let position = leaves
                    .iter()
                    .find(|leaf| leaf.property.descriptor().name == "background-position")
                    .unwrap();
                assert_eq!(
                    position.value,
                    SpecifiedValue::LengthPair(
                        LengthTarget::parse("0%", limits().numeric).unwrap(),
                        LengthTarget::parse("0%", limits().numeric).unwrap()
                    )
                );
                assert!(
                    leaves
                        .iter()
                        .filter(|leaf| !matches!(
                            leaf.property.descriptor().name,
                            "background-image" | "background-position"
                        ))
                        .all(|leaf| leaf.value == SpecifiedValue::Initial)
                );
                let retained = leaves.capacity() * size_of::<ParsedProperty>()
                    + leaves
                        .iter()
                        .map(|leaf| {
                            if let SpecifiedValue::Uri(uri) = &leaf.value {
                                uri.capacity()
                            } else {
                                0
                            }
                        })
                        .sum::<usize>();
                assert!(retained <= bytes);
            }
            Err(PropertyError::Syntax(SyntaxError {
                fault: SyntaxFault::Resource(SyntaxResource::Bytes),
                ..
            })) => {}
            other => panic!("{bytes}: {other:?}"),
        }
    }
}

#[test]
fn list_style_shorthand_assigns_none_to_the_unspecified_image_and_type() {
    for value in [
        "upper-roman inside",
        "url(a) disc",
        "none",
        "none none",
        "none none inside",
        "none disc",
        "url(a) none",
        "inherit",
    ] {
        assert!(
            matches!(
                parse(&format!("list-style:{value}")).unwrap(),
                PropertyResult::Parsed(_)
            ),
            "{value}"
        );
    }
    for value in [
        "none disc url(a)",
        "none none disc",
        "none none url(a)",
        "none none none",
        "inside outside",
        "disc circle",
        "url(a) url(b)",
        "none inherit",
    ] {
        assert_eq!(
            parse(&format!("list-style:{value}")).unwrap(),
            PropertyResult::Invalid,
            "{value}"
        );
    }
}

#[test]
fn list_style_expansion_preserves_none_semantics_and_explicit_image() {
    for (value, image_none, type_none) in [
        ("none", true, true),
        ("none none", true, true),
        ("none disc", true, false),
        ("url(a) none", false, true),
    ] {
        let PropertyResult::Parsed(actual) =
            parse(&format!("list-style:{value} !important")).unwrap()
        else {
            panic!()
        };
        let SpecifiedValue::Expanded(leaves) = actual.value else {
            panic!()
        };
        assert_eq!(leaves.len(), 3);
        assert!(leaves.iter().all(|leaf| leaf.important));
        let image = &leaves
            .iter()
            .find(|leaf| leaf.property.descriptor().name == "list-style-image")
            .unwrap()
            .value;
        assert_eq!(
            matches!(image, SpecifiedValue::Keyword(k) if k.as_str() == "none"),
            image_none
        );
        let marker = &leaves
            .iter()
            .find(|leaf| leaf.property.descriptor().name == "list-style-type")
            .unwrap()
            .value;
        assert_eq!(
            matches!(marker, SpecifiedValue::Keyword(k) if k.as_str() == "none"),
            type_none
        );
        assert_eq!(
            leaves
                .iter()
                .find(|leaf| leaf.property.descriptor().name == "list-style-position")
                .unwrap()
                .value,
            SpecifiedValue::Initial
        );
    }
}

#[test]
fn font_weight_admits_verified_keywords_and_enumerated_spellings() {
    for value in [
        "normal", "bold", "bolder", "lighter", "100", "200", "300", "400", "500", "600", "700",
        "800", "900", "inherit",
    ] {
        assert!(
            matches!(
                parse(&format!("font-weight:{value}")).unwrap(),
                PropertyResult::Parsed(_)
            ),
            "{value}"
        );
    }
}

#[test]
fn unverified_weight_spellings_are_explicit_errors_not_guessed_validity() {
    for value in ["+100", "0100", "100.0"] {
        assert!(matches!(
            parse(&format!("font-weight:{value}")),
            Err(PropertyError::UnimplementedNumericSpelling(_))
        ));
    }
    // A number that names none of the nine weights is plainly invalid: the
    // declaration is dropped, the stylesheet is not aborted.
    for value in ["150", "550", "1000", "0", "1.5", "-100", "+150", "100.5"] {
        assert_eq!(
            parse(&format!("font-weight:{value}")).unwrap(),
            PropertyResult::Invalid,
            "{value}"
        );
    }
    let PropertyResult::Parsed(actual) = parse("font-weight:700").unwrap() else {
        panic!()
    };
    assert_eq!(
        actual.value,
        SpecifiedValue::Integer(
            iced_css_core::values::Integer::parse("700", limits().numeric).unwrap()
        )
    );
}

#[test]
fn font_shorthand_requires_size_and_family_or_one_system_keyword() {
    for value in [
        "12px/14px sans-serif",
        "80% sans-serif",
        "x-large/110% 'New Century Schoolbook', serif",
        "bold italic large Palatino, serif",
        "normal small-caps 120%/120% fantasy",
        "normal italic 12px serif",
        "normal normal bold 12px serif",
        "300 italic 1.3em/1.7em 'FB Armada', sans-serif",
        "0 serif",
        "12px inherit Times",
        "caption",
        "menu",
        "inherit",
    ] {
        assert!(
            matches!(
                parse(&format!("font:{value}")).unwrap(),
                PropertyResult::Parsed(_)
            ),
            "{value}"
        );
    }
    for value in [
        "12px",
        "bold italic",
        "12px/ serif",
        "12px/1.2",
        "italic oblique 12px serif",
        "normal normal normal normal 12px serif",
        "12px inherit",
        "inherit 12px serif",
        "caption serif",
        "12px/ inherit serif",
    ] {
        assert_eq!(
            parse(&format!("font:{value}")).unwrap(),
            PropertyResult::Invalid,
            "{value}"
        );
    }
}

#[test]
fn font_expansion_preserves_system_requests_defaults_and_shared_normal_roles() {
    let PropertyResult::Parsed(actual) = parse("font:menu !important").unwrap() else {
        panic!()
    };
    let SpecifiedValue::Expanded(leaves) = actual.value else {
        panic!()
    };
    assert_eq!(leaves.len(), 6);
    for leaf in leaves {
        assert!(leaf.important);
        let SpecifiedValue::SystemFont(keyword) = leaf.value else {
            panic!()
        };
        assert_eq!(keyword.as_str(), "menu");
    }
    let PropertyResult::Parsed(actual) = parse("font:normal italic 12px serif").unwrap() else {
        panic!()
    };
    let SpecifiedValue::Expanded(leaves) = actual.value else {
        panic!()
    };
    for (name, expected) in [
        ("font-style", "italic"),
        ("font-variant", "normal"),
        ("font-weight", "normal"),
    ] {
        let SpecifiedValue::Keyword(keyword) = leaves
            .iter()
            .find(|leaf| leaf.property.descriptor().name == name)
            .unwrap()
            .value
        else {
            panic!()
        };
        assert_eq!(keyword.as_str(), expected);
    }
    assert_eq!(
        leaves
            .iter()
            .find(|leaf| leaf.property.descriptor().name == "line-height")
            .unwrap()
            .value,
        SpecifiedValue::Initial
    );
    assert!(matches!(
        parse("font:+100 12px serif"),
        Err(PropertyError::UnimplementedNumericSpelling(_))
    ));
}

#[test]
fn custom_properties_and_var_parse_at_use_not_in_the_sheet() {
    use iced_css_core::values::registry::PropertyId;
    // Any name of two dashes and more declares one; its value is kept as written.
    let PropertyResult::Parsed(custom) = parse("--Main-Color: #f00 ").unwrap() else {
        panic!("custom property");
    };
    assert_eq!(custom.property, PropertyId::custom());
    assert_eq!(
        custom.value,
        SpecifiedValue::Custom {
            name: "--Main-Color".into(),
            raw: "#f00".into(),
        }
    );
    assert!(matches!(
        parse("--x: any(thing) [at all] !important").unwrap(),
        PropertyResult::Parsed(ParsedProperty {
            important: true,
            ..
        })
    ));
    // A value with var() is valid whatever else it holds, and waits.
    let PropertyResult::Parsed(pending) = parse("width: var(--w, 10px)").unwrap() else {
        panic!("pending");
    };
    assert_eq!(
        pending.value,
        SpecifiedValue::Pending {
            raw: "var(--w, 10px)".into(),
            shorthand: None,
        }
    );
    // A shorthand gives each of its longhands the same pending text.
    let PropertyResult::Parsed(margin) = parse("margin: 0 VAR(--m)").unwrap() else {
        panic!("pending shorthand");
    };
    let SpecifiedValue::Expanded(leaves) = &margin.value else {
        panic!("expanded");
    };
    assert_eq!(leaves.len(), 4);
    assert!(leaves.iter().all(|leaf| matches!(
        &leaf.value,
        SpecifiedValue::Pending { raw, shorthand: Some(from) }
            if raw == "0 VAR(--m)" && *from == margin.property
    )));
    // An unknown property stays unknown.
    assert_eq!(parse("nope: var(--x)").unwrap(), PropertyResult::Unknown);
}

#[test]
fn flex_shorthands_expand_as_css_flexbox_gives_them() {
    use iced_css_core::values::registry::PropertyId;
    let leaves = |input: &str| -> Option<Vec<(String, String)>> {
        match parse(input).unwrap() {
            PropertyResult::Parsed(ParsedProperty {
                value: SpecifiedValue::Expanded(leaves),
                ..
            }) => Some(
                leaves
                    .into_iter()
                    .map(|leaf| {
                        let shown = match leaf.value {
                            SpecifiedValue::Keyword(keyword) => keyword.as_str().to_owned(),
                            SpecifiedValue::Number(number) => format!("{number:?}"),
                            SpecifiedValue::Length(LengthTarget::Auto) => "auto".to_owned(),
                            SpecifiedValue::Length(_) => "length".to_owned(),
                            SpecifiedValue::Initial => "initial".to_owned(),
                            SpecifiedValue::Inherit => "inherit".to_owned(),
                            other => format!("{other:?}"),
                        };
                        (leaf.property.descriptor().name.to_owned(), shown)
                    })
                    .collect(),
            ),
            _ => None,
        }
    };
    let number = |text: &str| {
        format!(
            "{:?}",
            iced_css_core::values::Decimal::parse(text, limits().numeric).unwrap()
        )
    };
    let flex = |input: &str| {
        leaves(&format!("flex:{input}")).map(|leaves| {
            assert_eq!(
                leaves
                    .iter()
                    .map(|leaf| leaf.0.as_str())
                    .collect::<Vec<_>>(),
                ["flex-grow", "flex-shrink", "flex-basis"]
            );
            [
                leaves[0].1.clone(),
                leaves[1].1.clone(),
                leaves[2].1.clone(),
            ]
        })
    };
    assert_eq!(
        flex("none"),
        Some([number("0"), number("0"), "auto".into()])
    );
    assert_eq!(
        flex("auto"),
        Some([number("1"), number("1"), "auto".into()])
    );
    assert_eq!(flex("2"), Some([number("2"), number("1"), "length".into()]));
    assert_eq!(
        flex("2 3"),
        Some([number("2"), number("3"), "length".into()])
    );
    assert_eq!(
        flex("2 3 10px"),
        Some([number("2"), number("3"), "length".into()])
    );
    assert_eq!(
        flex("1 1 0"),
        Some([number("1"), number("1"), "length".into()])
    );
    assert_eq!(
        flex("10px"),
        Some([number("1"), number("1"), "length".into()])
    );
    assert_eq!(
        flex("1 30%"),
        Some([number("1"), number("1"), "length".into()])
    );
    assert_eq!(
        flex("1 auto"),
        Some([number("1"), number("1"), "auto".into()])
    );
    assert_eq!(
        flex("inherit").map(|parts| parts[0].clone()),
        Some("inherit".into())
    );
    for bad in ["-1", "1 2 3 4", "10px 20px", "red", "1 -2"] {
        assert_eq!(flex(bad), None, "{bad}");
    }
    assert_eq!(
        leaves("flex-flow:wrap column"),
        Some(vec![
            ("flex-direction".into(), "column".into()),
            ("flex-wrap".into(), "wrap".into())
        ])
    );
    assert_eq!(
        leaves("flex-flow:row-reverse"),
        Some(vec![
            ("flex-direction".into(), "row-reverse".into()),
            ("flex-wrap".into(), "initial".into())
        ])
    );
    assert_eq!(leaves("flex-flow:row column"), None);
    assert_eq!(
        leaves("gap:5px"),
        Some(vec![
            ("row-gap".into(), "length".into()),
            ("column-gap".into(), "length".into())
        ])
    );
    assert_eq!(
        leaves("gap:normal 5px"),
        Some(vec![
            ("row-gap".into(), "normal".into()),
            ("column-gap".into(), "length".into())
        ])
    );
    assert_eq!(leaves("gap:-1px"), None);
    // The longhands on their own.
    for good in [
        "flex-grow:0",
        "flex-shrink:2.5",
        "order:-3",
        "order:0",
        "flex-basis:content",
        "display:inline-flex",
    ] {
        assert!(
            matches!(parse(good).unwrap(), PropertyResult::Parsed(_)),
            "{good}"
        );
    }
    for bad in [
        "flex-grow:-1",
        "order:1.5",
        "flex-basis:-1px",
        "align-items:left",
    ] {
        assert_eq!(parse(bad).unwrap(), PropertyResult::Invalid, "{bad}");
    }
    assert!(PropertyId::lookup("flex").is_some_and(|flex| flex.descriptor().shorthand));
}

#[test]
fn grid_properties_parse_css_grid_track_lists_lines_and_shorthands() {
    use iced_css_core::values::registry::PropertyId;
    let valid = |input: &str| matches!(parse(input).unwrap(), PropertyResult::Parsed(_));
    for good in [
        "grid-template-columns:none",
        "grid-template-columns:100px 1fr 2fr",
        "grid-template-columns:[a] 10% [b c] minmax(10px, 1fr) [d]",
        "grid-template-columns:repeat(3, 1fr)",
        "grid-template-columns:repeat(2, [x] 10px auto)",
        "grid-template-columns:repeat(auto-fill, minmax(100px, 1fr))",
        "grid-template-columns:20px repeat(auto-fit, 50px) 20px",
        "grid-template-rows:fit-content(40px) min-content max-content",
        "grid-template-rows:calc(10px + 5%) 1fr",
        "grid-auto-rows:10px auto",
        "grid-auto-flow:column dense",
        "grid-auto-flow:dense",
        "grid-template-areas:'a a b' \"c . b\"",
        "grid-row-start:2",
        "grid-row-start:-1",
        "grid-row-start:span 2",
        "grid-row-start:2 header",
        "grid-row-start:span header 2",
        "grid-column-end:header",
        "justify-items:center",
        "justify-self:end",
        "justify-content:stretch",
    ] {
        assert!(valid(good), "{good}");
    }
    for bad in [
        "grid-template-columns:-10px",
        "grid-template-columns:minmax(1fr, 10px)",
        "grid-template-columns:repeat(0, 1fr)",
        "grid-template-columns:repeat(auto-fill, 1fr)",
        "grid-template-columns:repeat(auto-fill, 10px) repeat(auto-fit, 10px)",
        "grid-template-columns:repeat(auto-fill, 10px) auto",
        "grid-template-columns:repeat(2, repeat(2, 10px))",
        "grid-template-columns:[a",
        "grid-template-columns:[span] 10px",
        "grid-template-columns:10",
        "grid-auto-rows:[a] 10px",
        "grid-auto-flow:row column",
        "grid-template-areas:'a b' 'a'",
        "grid-template-areas:'a b a'",
        "grid-template-areas:'a #'",
        "grid-row-start:0",
        "grid-row-start:span -1",
        "grid-row-start:span",
        "grid-row-start:2 span x",
        "grid-row-start:auto 2",
    ] {
        assert_eq!(parse(bad).unwrap(), PropertyResult::Invalid, "{bad}");
    }
    let lines = |input: &str| -> Vec<(String, GridLineValue)> {
        let PropertyResult::Parsed(ParsedProperty {
            value: SpecifiedValue::Expanded(leaves),
            ..
        }) = parse(input).unwrap()
        else {
            panic!("{input}")
        };
        leaves
            .into_iter()
            .map(|leaf| {
                let SpecifiedValue::Grid(GridValue::Line(line)) = leaf.value else {
                    panic!("{input}")
                };
                (leaf.property.descriptor().name.to_owned(), line)
            })
            .collect()
    };
    let named = |name: &str| GridLineValue {
        name: Some(name.into()),
        ..GridLineValue::default()
    };
    let number = |number: i32| GridLineValue {
        number,
        ..GridLineValue::default()
    };
    assert_eq!(
        lines("grid-row:1 / span 2"),
        [
            ("grid-row-start".into(), number(1)),
            (
                "grid-row-end".into(),
                GridLineValue {
                    span: true,
                    number: 2,
                    name: None
                }
            ),
        ]
    );
    assert_eq!(
        lines("grid-column:side"),
        [
            ("grid-column-start".into(), named("side")),
            ("grid-column-end".into(), named("side")),
        ]
    );
    assert_eq!(
        lines("grid-column:3"),
        [
            ("grid-column-start".into(), number(3)),
            ("grid-column-end".into(), GridLineValue::default()),
        ]
    );
    // `grid-area` copies a lone name into every part left out.
    assert_eq!(
        lines("grid-area:main")
            .into_iter()
            .map(|(name, line)| (name, line.name))
            .collect::<Vec<_>>(),
        [
            ("grid-row-start".into(), Some("main".into())),
            ("grid-column-start".into(), Some("main".into())),
            ("grid-row-end".into(), Some("main".into())),
            ("grid-column-end".into(), Some("main".into())),
        ]
    );
    assert_eq!(
        lines("grid-area:1 / 2 / 3")
            .into_iter()
            .map(|(_, line)| line.number)
            .collect::<Vec<_>>(),
        [1, 2, 3, 0]
    );
    assert_eq!(
        parse("grid-area:1/2/3/4/5").unwrap(),
        PropertyResult::Invalid
    );
    // The template shorthands set every longhand they name.
    let shape = |input: &str| -> Vec<&'static str> {
        let PropertyResult::Parsed(ParsedProperty {
            value: SpecifiedValue::Expanded(leaves),
            ..
        }) = parse(input).unwrap()
        else {
            panic!("{input}")
        };
        leaves
            .iter()
            .map(|leaf| match &leaf.value {
                SpecifiedValue::Grid(GridValue::None) => "none",
                SpecifiedValue::Grid(GridValue::Tracks(_)) => "tracks",
                SpecifiedValue::Grid(GridValue::Areas(_)) => "areas",
                SpecifiedValue::Grid(GridValue::Flow { column: true, .. }) => "column",
                SpecifiedValue::Grid(GridValue::Flow { .. }) => "row",
                _ => "other",
            })
            .collect()
    };
    assert_eq!(shape("grid-template:none"), ["none", "none", "none"]);
    assert_eq!(
        shape("grid-template:100px 1fr / 50px"),
        ["tracks", "tracks", "none"]
    );
    assert_eq!(
        shape("grid-template:[top] 'a b' 40px [mid] 'c d' / 1fr 2fr"),
        ["tracks", "tracks", "areas"]
    );
    assert_eq!(
        shape("grid:auto-flow dense 40px / 1fr 1fr"),
        ["none", "tracks", "none", "tracks", "tracks", "row"]
    );
    assert_eq!(
        shape("grid:100px / auto-flow"),
        ["tracks", "none", "none", "tracks", "tracks", "column"]
    );
    assert_eq!(
        shape("grid:'a' / 10px"),
        ["tracks", "tracks", "areas", "tracks", "tracks", "row"]
    );
    for bad in [
        "grid-template:100px",
        "grid-template:'a b' / repeat(2, 10px)",
        "grid:auto-flow / auto-flow",
    ] {
        assert_eq!(parse(bad).unwrap(), PropertyResult::Invalid, "{bad}");
    }
    assert!(PropertyId::lookup("grid").is_some_and(|grid| grid.descriptor().shorthand));
}

#[test]
fn transition_properties_parse_lists_easings_and_the_shorthand() {
    let valid = |input: &str| matches!(parse(input).unwrap(), PropertyResult::Parsed(_));
    for good in [
        "transition-property:none",
        "transition-property:all",
        "transition-property:width, color, no-such-thing",
        "transition-duration:1s, 200ms",
        "transition-delay:-1.5s",
        "transition-timing-function:ease, linear, step-start",
        "transition-timing-function:cubic-bezier(0.1, -2, 0.9, 3)",
        "transition-timing-function:steps(4), steps(2, jump-none), steps(1, start)",
        "transition:width 1s",
        "transition:1s ease-in 2s opacity, color 3ms",
        "transition:none",
        "transition:all 0s steps(3, jump-both)",
    ] {
        assert!(valid(good), "{good}");
    }
    for bad in [
        "transition-property:none, width",
        "transition-property:initial",
        "transition-property:width color",
        "transition-duration:-1s",
        "transition-duration:1",
        "transition-duration:1px",
        "transition-timing-function:cubic-bezier(1.5, 0, 0, 1)",
        "transition-timing-function:cubic-bezier(0, 0, 1)",
        "transition-timing-function:steps(0)",
        "transition-timing-function:steps(1, jump-none)",
        "transition-timing-function:bounce",
        "transition:width 1s 2s 3s",
        "transition:width color 1s",
        "transition:none 1s, width 2s",
        "transition:-1s width",
        "transition:width 1s,",
    ] {
        assert_eq!(parse(bad).unwrap(), PropertyResult::Invalid, "{bad}");
    }
    let PropertyResult::Parsed(ParsedProperty {
        value: SpecifiedValue::Expanded(leaves),
        ..
    }) = parse("transition:opacity 1s, 200ms linear").unwrap()
    else {
        panic!()
    };
    let names: Vec<&str> = leaves
        .iter()
        .map(|leaf| leaf.property.descriptor().name)
        .collect();
    assert_eq!(
        names,
        [
            "transition-property",
            "transition-duration",
            "transition-timing-function",
            "transition-delay"
        ]
    );
    assert_eq!(
        leaves[0].value,
        SpecifiedValue::Transition(TransitionValue::Properties(vec![
            TransitionProperty::Named("opacity".into()),
            TransitionProperty::All,
        ]))
    );
    let SpecifiedValue::Transition(TransitionValue::Times(durations)) = &leaves[1].value else {
        panic!()
    };
    assert_eq!(
        durations
            .iter()
            .map(|time| time.milliseconds)
            .collect::<Vec<_>>(),
        [false, true]
    );
    let SpecifiedValue::Transition(TransitionValue::Easings(easings)) = &leaves[2].value else {
        panic!()
    };
    assert_eq!(easings[1], EasingValue::Linear);
    assert!(matches!(easings[0], EasingValue::CubicBezier(_)));
}

#[test]
fn animation_properties_parse_lists_and_the_shorthand() {
    let valid = |input: &str| matches!(parse(input).unwrap(), PropertyResult::Parsed(_));
    for good in [
        "animation-name:none",
        "animation-name:spin, 'fade in', none",
        "animation-duration:1s, 0ms",
        "animation-delay:-2s",
        "animation-iteration-count:infinite, 2.5, 0",
        "animation-direction:alternate-reverse, normal",
        "animation-fill-mode:both",
        "animation-play-state:paused, running",
        "animation-timing-function:steps(3, end)",
        "animation:spin 1s",
        "animation:2s ease-in 1s infinite alternate both paused spin, fade 100ms",
        "animation:none",
        "animation:reverse 3 spin",
    ] {
        assert!(valid(good), "{good}");
    }
    for bad in [
        "animation-name:initial",
        "animation-name:a b",
        "animation-duration:-1s",
        "animation-iteration-count:-1",
        "animation-direction:sideways",
        "animation-fill-mode:forwards backwards",
        "animation-play-state:stopped",
        "animation:spin fade",
        "animation:1s 2s 3s spin",
        "animation:-1s spin",
        "animation:spin 1s,",
    ] {
        assert_eq!(parse(bad).unwrap(), PropertyResult::Invalid, "{bad}");
    }
    let PropertyResult::Parsed(ParsedProperty {
        value: SpecifiedValue::Expanded(leaves),
        ..
    }) = parse("animation:infinite alternate spin 2s, 1s fade").unwrap()
    else {
        panic!()
    };
    let names: Vec<&str> = leaves
        .iter()
        .map(|leaf| leaf.property.descriptor().name)
        .collect();
    assert_eq!(
        names,
        [
            "animation-name",
            "animation-duration",
            "animation-timing-function",
            "animation-delay",
            "animation-iteration-count",
            "animation-direction",
            "animation-fill-mode",
            "animation-play-state"
        ]
    );
    assert_eq!(
        leaves[0].value,
        SpecifiedValue::Transition(TransitionValue::Names(vec![
            Some("spin".into()),
            Some("fade".into())
        ]))
    );
    assert_eq!(
        leaves[5].value,
        SpecifiedValue::Transition(TransitionValue::Keywords(vec!["alternate", "normal"]))
    );
    let SpecifiedValue::Transition(TransitionValue::Counts(counts)) = &leaves[4].value else {
        panic!()
    };
    assert!(counts[0].is_none() && counts[1].is_some());
}

#[test]
fn background_image_takes_a_linear_gradient() {
    let gradient = |input: &str| match parse(&format!("background-image:{input}")).unwrap() {
        PropertyResult::Parsed(ParsedProperty {
            value: SpecifiedValue::Gradient(gradient),
            ..
        }) => Some(gradient),
        _ => None,
    };
    let plain = gradient("linear-gradient(red, blue)").unwrap();
    assert_eq!(plain.direction, GradientDirection::Toward(0, 1));
    assert_eq!(plain.stops.len(), 2);
    assert!(plain.stops.iter().all(|stop| stop.position.is_none()));
    let angled =
        gradient("linear-gradient(45deg, rgb(0, 0, 255) 10%, currentColor, #fff 20px 80%)")
            .unwrap();
    assert!(matches!(
        angled.direction,
        GradientDirection::Angle(_, AngleUnit::Deg)
    ));
    // Two places make two stops; `currentColor` is the element's color.
    assert_eq!(angled.stops.len(), 4);
    assert!(angled.stops[1].color.is_none());
    assert_eq!(
        gradient("linear-gradient(to top right, red, blue)")
            .unwrap()
            .direction,
        GradientDirection::Toward(1, -1)
    );
    for unit in ["0.5turn", "100grad", "1rad", "0"] {
        assert!(
            gradient(&format!("linear-gradient({unit}, red, blue)")).is_some(),
            "{unit}"
        );
    }
    for bad in [
        "linear-gradient(red)",
        "linear-gradient(to middle, red, blue)",
        "linear-gradient(to left right, red, blue)",
        "linear-gradient(red, 50%, blue)",
        "linear-gradient(red 1px 2px 3px, blue)",
        "linear-gradient(10px, red, blue)",
        "linear-gradient(red, blue) red",
        "radial-gradient(red, blue)",
        "repeating-linear-gradient(red, blue)",
        "linear-gradient(red, blue",
    ] {
        assert!(gradient(bad).is_none(), "{bad}");
    }
    // In the shorthand too, and not as a list image.
    let PropertyResult::Parsed(ParsedProperty {
        value: SpecifiedValue::Expanded(leaves),
        ..
    }) = parse("background:linear-gradient(red, blue) no-repeat").unwrap()
    else {
        panic!()
    };
    assert!(matches!(leaves[1].value, SpecifiedValue::Gradient(_)));
    assert_eq!(
        parse("list-style-image:linear-gradient(red, blue)").unwrap(),
        PropertyResult::Invalid
    );
}
