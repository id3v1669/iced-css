// UNCOMPILED literal SourceCapitalize owner fixtures.
// UNCOMPILED integration preparation, all fixture expectations are literal.
include!("adapter.rs");
fn leaf(id: usize, text: &str, operation: &str, lang: Option<&str>) -> InputNode {
    InputNode::Payload {
        id,
        text: text.to_owned(),
        mode: "normal".to_owned(),
        suppressed: false,
        runs: None,
        operation: operation.to_owned(),
        language: lang.map(str::to_owned),
    }
}
macro_rules! with_whitespace {
    ($children:expr,$root_lang:expr,$doc_lang:expr,$extra:expr,|$w:ident|$body:block) => {{
        let (construction, mut css) = actual_tree_with_language(&$children, $root_lang);
        css.push_str($extra);
        let h = host(construction, None, $doc_lang);
        let ua = catalog();
        let g = graph(&css, ua.next_context());
        let sheets = controls(&[1], true, 100);
        let bindings = [binding(&g, GraphOrigin::Author(sheet(1)))];
        let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, bundle_limits()).unwrap();
        let display = evaluate_source_display(h.snapshot().unwrap(), &bundle, limits()).unwrap();
        let eligibility = evaluate_source_eligibility(&display, eligibility_limits()).unwrap();
        let $w = preprocess_source_whitespace(
            &SourceParticipants::new(&eligibility),
            whitespace_limits(),
        )
        .unwrap();
        $body
    }};
}
#[test]
fn first_complete_unit_uses_each_members_original_operation_and_profile() {
    for (left_op, right_op, left_lang, right_lang, want, targets) in [
        (
            "capitalize",
            "capitalize",
            "lt",
            "lt",
            "I",
            vec![true, true],
        ),
        (
            "capitalize",
            "none",
            "lt",
            "lt",
            "I\u{0307}",
            vec![true, false],
        ),
        ("none", "capitalize", "lt", "lt", "i", vec![false, true]),
        (
            "capitalize",
            "capitalize",
            "en",
            "lt",
            "I",
            vec![true, true],
        ),
        (
            "capitalize",
            "capitalize",
            "lt",
            "en",
            "I\u{0307}",
            vec![true, true],
        ),
    ] {
        with_whitespace!(
            [
                leaf(0, "i", left_op, Some(left_lang)),
                leaf(1, "\u{0307}", right_op, Some(right_lang))
            ],
            None,
            None,
            "",
            |w| {
                let out = evaluate_source_casing(&w, casing_limits()).unwrap();
                assert_eq!(out.capitalize_context_count(), 1);
                assert_eq!(out.capitalize_unit_count(), 1);
                let u = out.capitalize_unit(0).unwrap();
                assert_eq!(u.scalar_range(), 0..2);
                assert_eq!(u.word_range(), 0..2);
                assert_eq!(
                    u.scalars()
                        .map(|s| s.is_capitalize_target())
                        .collect::<Vec<_>>(),
                    targets
                );
                assert_eq!(
                    out.scalars()
                        .flat_map(|s| s.presentation().characters())
                        .collect::<String>(),
                    want
                );
            }
        );
    }
}
#[test]
fn complete_unit_policy_literals_preserve_noninitial_content() {
    for (text, want) in [
        ("a\u{0345}", "AΙ"),
        ("\u{0600}a", "\u{0600}A"),
        ("\u{0600}漢", "\u{0600}漢"),
        ("1abc", "1abc"),
        ("_abc", "_abc"),
        ("אabc", "אabc"),
        ("اabc", "اabc"),
        ("漢abc", "漢Abc"),
        ("\u{0301}abc", "\u{0301}Abc"),
        ("\u{0345}abc", "ΙAbc"),
        ("\"questions\"", "\"Questions\""),
        ("can't", "Can't"),
        ("l’arbre", "L’arbre"),
        ("foo-bar", "Foo-Bar"),
        ("foo_bar", "Foo_bar"),
        ("ßabc", "SSabc"),
        ("ǳabc", "Ǳabc"),
        ("qUestions", "QUestions"),
    ] {
        with_whitespace!(
            [leaf(0, text, "capitalize", Some("en"))],
            None,
            None,
            "",
            |w| {
                let out = evaluate_source_casing(&w, casing_limits()).unwrap();
                assert_eq!(
                    out.scalars()
                        .flat_map(|s| s.presentation().characters())
                        .collect::<String>(),
                    want
                );
                if text == "\u{0600}漢" {
                    let unit = out.capitalize_unit(0).unwrap();
                    assert_eq!(unit.scalar_range(), 0..2);
                    assert_eq!(unit.word_range(), 0..2);
                }
            }
        );
    }
    with_whitespace!(
        [leaf(0, "ijsland", "capitalize", Some("nl"))],
        None,
        None,
        "",
        |w| {
            let out = evaluate_source_casing(&w, casing_limits()).unwrap();
            assert_eq!(
                out.source(0).unwrap().profile(),
                SourceCasingProfile::Default
            );
            assert_eq!(
                out.source(0).unwrap().language_status(),
                SourceCasingLanguageStatus::WellFormed
            );
            assert_eq!(
                out.scalars()
                    .flat_map(|s| s.presentation().characters())
                    .collect::<String>(),
                "Ijsland"
            );
        }
    );
}
#[test]
fn no_operative_unit_still_analyzes_context_without_mapping_lane() {
    with_whitespace!(
        [
            leaf(0, "q", "none", None),
            leaf(1, "uestions", "capitalize", None)
        ],
        None,
        None,
        "",
        |w| {
            let out = evaluate_source_casing(&w, casing_limits()).unwrap();
            assert_eq!(out.capitalize_scalar_count(), 8);
            assert_eq!(out.capitalize_context_count(), 1);
            assert_eq!(out.capitalize_unit_count(), 0);
            assert_eq!(out.active_context_count(), 1);
            assert_eq!(out.context_bytes(), 9);
            assert_eq!(out.selection_pass_count(), 0);
            assert_eq!(out.language_view_work(), 49);
            assert_eq!(out.work(), 184);
            assert!(
                out.scalars()
                    .all(|s| s.capitalize_unit().is_none() && !s.is_capitalize_target())
            );
            assert!(
                out.scalars()
                    .all(|s| matches!(s.origin(), SourceCasingOrigin::Untransformed))
            );
            assert_eq!(
                out.scalars()
                    .flat_map(|s| s.presentation().characters())
                    .collect::<String>(),
                "questions"
            );
            let max = nz(1_000_000);
            let wb = iced_css_core::text::word_boundaries::evaluate_word_boundaries(
                "questions",
                iced_css_core::text::word_boundaries::WordBoundaryLimits {
                    input_bytes: max,
                    scalars: max,
                    retained_bytes: max,
                    work: max,
                },
            )
            .unwrap();
            let gb = iced_css_core::text::grapheme_boundaries::evaluate_grapheme_boundaries(
                "questions",
                iced_css_core::text::grapheme_boundaries::GraphemeBoundaryLimits {
                    input_bytes: max,
                    scalars: max,
                    retained_bytes: max,
                    work: max,
                },
            )
            .unwrap();
            assert_eq!(wb.work(), 37);
            assert_eq!(gb.work(), 19);
            assert_eq!(wb.offsets(), &[0, 9]);
            assert_eq!(gb.offsets(), &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
        }
    );
}
#[test]
fn literal_complete_work_and_language_context_precedence() {
    with_whitespace!(
        [
            leaf(0, "i", "capitalize", Some("lt")),
            leaf(1, "\u{0307}", "capitalize", Some("lt"))
        ],
        None,
        None,
        "",
        |w| {
            let out = evaluate_source_casing(&w, casing_limits()).unwrap();
            assert_eq!(out.language_view_work(), 65);
            assert_eq!(out.work(), 566);
            assert_eq!(out.capitalize_scalar_count(), 2);
            let mut lim = casing_limits();
            lim.work = nz(566);
            assert_eq!(evaluate_source_casing(&w, lim).unwrap().work(), 566);
            for work in [565, 559, 552, 548, 543] {
                let mut lim = casing_limits();
                lim.work = nz(work);
                assert!(matches!(
                    evaluate_source_casing(&w, lim),
                    Err(SourceCasingError::Work)
                ));
            }
            let mut lim = casing_limits();
            lim.language_bytes = nz(1);
            lim.work = nz(69);
            assert!(matches!(
                evaluate_source_casing(&w, lim),
                Err(SourceCasingError::LanguageBytes { source: 0 })
            ));
            let mut lim = casing_limits();
            lim.context_bytes = nz(2);
            lim.work = nz(525);
            assert!(matches!(
                evaluate_source_casing(&w, lim),
                Err(SourceCasingError::ContextBytes { start: 0 })
            ));
        }
    );
}
#[test]
fn empty_payload_and_inline_block_context_edges() {
    with_whitespace!(
        [
            leaf(0, "", "capitalize", Some("tr")),
            InputNode::Empty { id: 1 }
        ],
        None,
        None,
        "",
        |w| {
            let out = evaluate_source_casing(&w, casing_limits()).unwrap();
            assert_eq!(out.capitalize_scalar_count(), 0);
            assert_eq!(out.capitalize_context_count(), 0);
            assert_eq!(out.capitalize_unit_count(), 0);
            assert_eq!(
                out.source(0).unwrap().operation(),
                Some(SourceCasingOperation::Capitalize)
            );
            assert_eq!(
                out.source(0).unwrap().profile(),
                SourceCasingProfile::Turkish
            );
        }
    );
    with_whitespace!(
        [
            leaf(0, "i", "capitalize", Some("lt")),
            InputNode::Empty { id: 2 },
            leaf(1, "\u{0307}", "capitalize", Some("lt"))
        ],
        None,
        None,
        "",
        |w| {
            let out = evaluate_source_casing(&w, casing_limits()).unwrap();
            assert_eq!(out.capitalize_context_count(), 1);
            assert_eq!(out.capitalize_unit(0).unwrap().scalar_range(), 0..2);
            assert_eq!(
                out.scalars()
                    .flat_map(|s| s.presentation().characters())
                    .collect::<String>(),
                "I"
            );
        }
    );
    with_whitespace!(
        [
            leaf(0, "i", "capitalize", Some("lt")),
            InputNode::Block {
                id: 0,
                suppressed: false,
                children: vec![]
            },
            leaf(1, "\u{0307}", "capitalize", Some("lt"))
        ],
        None,
        None,
        "",
        |w| {
            let out = evaluate_source_casing(&w, casing_limits()).unwrap();
            assert_eq!(out.capitalize_context_count(), 2);
            assert_eq!(out.capitalize_unit_count(), 2);
            assert_eq!(
                out.scalars()
                    .flat_map(|s| s.presentation().characters())
                    .collect::<String>(),
                "I\u{0307}"
            );
        }
    );
}
#[test]
fn original_rich_crlf_and_supplementary_identity_survive() {
    let rich = InputNode::Payload {
        id: 0,
        text: "é\r\n漢Σ".into(),
        mode: "pre-line".into(),
        suppressed: false,
        runs: Some(vec![0..3, 3..9]),
        operation: "capitalize".into(),
        language: Some("en".into()),
    };
    // Exercises every adapter helper and all public observations; no dead helper copy.
    observe(
        0,
        &[InputNode::Block {
            id: 0,
            suppressed: false,
            children: vec![
                InputNode::Empty { id: 1 },
                rich,
                leaf(2, "𐐨a", "capitalize", Some("en")),
            ],
        }],
    );
    with_whitespace!(
        [leaf(0, "𐐨a", "capitalize", Some("en"))],
        None,
        None,
        "",
        |w| {
            let out = evaluate_source_casing(&w, casing_limits()).unwrap();
            assert_eq!(out.capitalize_unit(0).unwrap().scalar_range(), 0..1);
            assert_eq!(
                out.scalar(0).unwrap().whitespace_scalar().source_range(),
                0..4
            );
            assert_eq!(
                out.scalars()
                    .flat_map(|s| s.presentation().characters())
                    .collect::<String>(),
                "𐐀a"
            );
        }
    );
}
#[test]
fn existing_operations_share_lanes_and_hidden_sources_stay_hidden() {
    with_whitespace!(
        [
            leaf(0, "a", "upper", Some("en")),
            leaf(1, " b", "capitalize", Some("en"))
        ],
        None,
        None,
        "",
        |w| {
            let out = evaluate_source_casing(&w, casing_limits()).unwrap();
            assert_eq!(out.selection_pass_count(), 1);
            assert_eq!(
                out.scalars()
                    .flat_map(|s| s.presentation().characters())
                    .collect::<String>(),
                "A B"
            );
        }
    );
    with_whitespace!(
        [leaf(0, "x", "capitalize", Some("tr"))],
        None,
        None,
        "#scope {display:none;}",
        |w| {
            let mut lim = casing_limits();
            lim.work = nz(1);
            lim.retained_bytes = nz(1);
            let out = evaluate_source_casing(&w, lim).unwrap();
            assert_eq!(out.work(), 1);
            assert_eq!(out.capitalize_scalar_count(), 0);
            assert_eq!(out.capitalize_unit_count(), 0);
            assert_eq!(out.retained_bytes(), 0);
        }
    );
    with_whitespace!(
        [leaf(0, "x", "none", None)],
        None,
        None,
        "#scope {text-transform:capitalize;}",
        |w| {
            let out = evaluate_source_casing(&w, casing_limits()).unwrap();
            assert_eq!(out.capitalize_scalar_count(), 0);
            assert_eq!(out.capitalize_context_count(), 0);
        }
    );
}
#[test]
fn capitalize_real_language_inheritance_clear_and_malformed_diagnostics() {
    with_whitespace!(
        [
            leaf(0, "i", "capitalize", None),
            leaf(1, " i", "capitalize", Some("")),
            leaf(2, " i", "capitalize", Some("tr\0")),
            InputNode::Empty { id: 3 }
        ],
        None,
        Some("tr"),
        "",
        |w| {
            let out = evaluate_source_casing(&w, casing_limits()).unwrap();
            let by_id = |id: &str| {
                out.sources()
                    .find(|s| s.participant().node().metadata().css_id() == Some(id))
                    .unwrap()
            };
            for id in ["scope", "p0", "i3"] {
                assert_eq!(by_id(id).profile(), SourceCasingProfile::Turkish);
                assert_eq!(
                    by_id(id).language_status(),
                    SourceCasingLanguageStatus::WellFormed
                );
            }
            assert_eq!(by_id("scope").operation(), None);
            assert_eq!(by_id("i3").operation(), None);
            assert_eq!(
                by_id("p1").language_status(),
                SourceCasingLanguageStatus::Unknown
            );
            assert_eq!(by_id("p1").profile(), SourceCasingProfile::Default);
            assert_eq!(
                by_id("p2").language_status(),
                SourceCasingLanguageStatus::Malformed
            );
            assert_eq!(by_id("p2").profile(), SourceCasingProfile::Default);
            assert_eq!(
                by_id("p2")
                    .participant()
                    .node()
                    .metadata()
                    .attribute("lang"),
                Some("tr\0")
            );
            assert_eq!(out.language_bytes(), 9);
            assert_eq!(out.known_language_count(), 4);
            assert_eq!(
                out.scalars()
                    .flat_map(|s| s.presentation().characters())
                    .collect::<String>(),
                "İ I I"
            );
        }
    );
}
