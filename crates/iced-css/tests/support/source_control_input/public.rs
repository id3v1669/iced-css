// Review artifact. Root installs as an integration test; no compiler result claimed.
include!("setup.rs");
use iced_css::evaluation::source_control_input::*;
use iced_css_core::text::bidi_initial::BidiParagraphMode;
use iced_css_core::values::bidi::ComputedDirection;

fn control_limits() -> SourceControlInputLimits {
    SourceControlInputLimits {
        sources: nz(usize::MAX),
        records: nz(usize::MAX),
        originals: nz(usize::MAX),
        scopes: nz(usize::MAX),
        depth: nz(usize::MAX),
        output_scalars: nz(usize::MAX),
        output_bytes: nz(usize::MAX),
        retained_bytes: nz(usize::MAX),
        scratch_bytes: nz(usize::MAX),
        peak_bytes: nz(usize::MAX),
        work: nz(usize::MAX),
    }
}
fn recipe(value: &str, base: &str, direction: &str, bidi: &str) -> InputNode {
    managed(
        "B",
        InputFamily::Column,
        "block",
        base,
        "normal",
        vec![managed(
            "I",
            InputFamily::Row,
            "inline",
            direction,
            bidi,
            vec![text("T", value, "pre", "none", "default", "normal")],
        )],
    )
}
#[test]
fn preserved_five_witness_tapes_and_origins() {
    for (value, bidi, nested, expected, work) in [
        (
            "a\nb",
            "embed",
            false,
            "\u{202b}a\u{202c}\n\u{202b}b\u{202c}",
            52,
        ),
        (
            "a\nb",
            "bidi-override",
            false,
            "\u{202e}a\u{202c}\n\u{202e}b\u{202c}",
            52,
        ),
        (
            "a\nb",
            "embed",
            true,
            "\u{202a}\u{202e}a\u{202c}\u{202c}\n\u{202a}\u{202e}b\u{202c}\u{202c}",
            69,
        ),
        (
            "\u{202c}a\nb",
            "embed",
            false,
            "\u{202b}\u{202c}a\u{202c}\n\u{202b}b\u{202c}",
            56,
        ),
        (
            "\u{2067}a\nb",
            "embed",
            false,
            "\u{202b}\u{2067}a\u{202c}\n\u{202b}b\u{202c}",
            56,
        ),
    ] {
        let root = if nested {
            managed(
                "B",
                InputFamily::Column,
                "block",
                "ltr",
                "normal",
                vec![managed(
                    "I",
                    InputFamily::Row,
                    "inline",
                    "ltr",
                    "embed",
                    vec![managed(
                        "J",
                        InputFamily::Row,
                        "inline",
                        "rtl",
                        "bidi-override",
                        vec![text("T", value, "pre", "none", "default", "normal")],
                    )],
                )],
            )
        } else {
            recipe(value, "ltr", "rtl", bidi)
        };
        with_predecessors!(root, |_whitespace, casing, structure| {
            let owner = build_source_control_input(&structure, &casing, control_limits()).unwrap();
            assert!(std::ptr::eq(owner.structure(), &structure));
            assert!(std::ptr::eq(owner.casing(), &casing));
            assert_eq!(owner.receipt().work, work);
            let domain = owner.scope(0).unwrap().inline_sequence().unwrap();
            assert_eq!(domain.text(), expected);
            assert_eq!(domain.scope().record().index(), 0);
            let mut original = 0;
            let mut occurrence = 0;
            let mut offset = 0;
            for (ordinal, ch) in expected.chars().enumerate() {
                let token = domain.token(ordinal).unwrap();
                assert_eq!(token.character(), ch);
                assert_eq!(token.ordinal(), ordinal);
                assert_eq!(token.byte_range(), offset..offset + ch.len_utf8());
                offset += ch.len_utf8();
                match token.origin() {
                    SourceControlOrigin::Literal(literal) => {
                        assert_eq!(literal.original_index(), original);
                        assert_eq!(literal.member_index(), 0);
                        assert_eq!(literal.character(), value.chars().nth(original).unwrap());
                        assert_eq!(
                            literal.payload().record().index(),
                            if nested { 4 } else { 3 }
                        );
                        assert!(literal.token().is_same_token(&token));
                        assert!(
                            owner
                                .contribution(original)
                                .unwrap()
                                .literal_token(0)
                                .unwrap()
                                .is_same_token(&token)
                        );
                        original += 1;
                    }
                    SourceControlOrigin::Synthetic(synthetic) => {
                        assert_eq!(synthetic.occurrence_index(), occurrence);
                        occurrence += 1;
                        assert!(synthetic.token().is_same_token(&token));
                        let fragment = synthetic.cause_fragment().index();
                        let expected_fragment = if nested {
                            match ordinal {
                                0 | 4 | 6 | 10 => 1,
                                1 | 3 | 7 | 9 => 2,
                                _ => panic!("unexpected synthetic ordinal"),
                            }
                        } else {
                            1
                        };
                        assert_eq!(fragment, expected_fragment);
                        assert_eq!(
                            synthetic.cause().source_index(),
                            if nested { 3 - expected_fragment } else { 1 }
                        );
                        let is_start = ordinal < if nested { 2 } else { 1 };
                        let is_end =
                            ordinal >= expected.chars().count() - if nested { 2 } else { 1 };
                        match synthetic.placement() {
                            SourceControlPlacement::Break { anchor, side } => {
                                assert!(!is_start && !is_end);
                                assert_eq!(anchor.member_index(), 0);
                                assert_eq!(anchor.character(), '\n');
                                assert_eq!(
                                    anchor.original_index(),
                                    if value.starts_with('a') { 1 } else { 2 }
                                );
                                let b = expected.chars().position(|c| c == '\n').unwrap();
                                assert_eq!(
                                    side,
                                    if ordinal < b {
                                        SourceControlBreakSide::Before
                                    } else {
                                        SourceControlBreakSide::After
                                    }
                                );
                            }
                            SourceControlPlacement::FragmentEdge(edge) => {
                                assert!(is_start || is_end);
                                assert_eq!(edge.record().index(), expected_fragment);
                                assert_eq!(
                                    edge.kind(),
                                    if is_start {
                                        SourceStructureEdgeKind::OriginalBegin
                                    } else {
                                        SourceStructureEdgeKind::OriginalEnd
                                    }
                                );
                            }
                        }
                    }
                }
            }
            assert_eq!(original, value.chars().count());
            assert!(domain.token(domain.scalar_count()).is_none());
            let r = owner.receipt();
            assert_eq!(
                r.retained_bytes,
                r.transport_capacity
                    + r.scope_capacity * r.scope_element_bytes
                    + r.payload_capacity * r.payload_element_bytes
                    + r.original_capacity * r.original_element_bytes
                    + r.token_capacity * r.token_element_bytes
            );
            assert_eq!(r.scratch_bytes, r.frame_capacity * r.frame_element_bytes);
            assert_eq!(r.peak_retained_bytes, r.retained_bytes + r.scratch_bytes);
        });
    }
}

#[test]
fn supplemental_directions_bases_and_non_lf_b() {
    // Unicode 17 DerivedBidiClass: each listed separator is B. New captures unexecuted.
    for base in ["ltr", "rtl"] {
        for direction in ["ltr", "rtl"] {
            for bidi in ["embed", "bidi-override"] {
                for separator in ['\n', '\u{1c}', '\u{1d}', '\u{1e}', '\u{85}', '\u{2029}'] {
                    let opening = match (direction, bidi) {
                        ("ltr", "embed") => '\u{202a}',
                        ("rtl", "embed") => '\u{202b}',
                        ("ltr", _) => '\u{202d}',
                        _ => '\u{202e}',
                    };
                    let value = format!("a{separator}b");
                    with_predecessors!(
                        recipe(&value, base, direction, bidi),
                        |_whitespace, casing, structure| {
                            let out =
                                build_source_control_input(&structure, &casing, control_limits())
                                    .unwrap();
                            assert_eq!(
                                out.scope(0).unwrap().inline_sequence().unwrap().text(),
                                format!("{opening}a\u{202c}{separator}{opening}b\u{202c}")
                            );
                            let scope = out.scope(0).unwrap();
                            assert_eq!(
                                scope.mode(),
                                if base == "ltr" {
                                    BidiParagraphMode::LeftToRight
                                } else {
                                    BidiParagraphMode::RightToLeft
                                }
                            );
                            assert_eq!(
                                scope.base_origin().direction(),
                                if base == "ltr" {
                                    ComputedDirection::LeftToRight
                                } else {
                                    ComputedDirection::RightToLeft
                                }
                            );
                            assert!(matches!(
                                scope.base_origin(),
                                SourceControlBaseOrigin::OwnBlock(_)
                            ));
                            assert_eq!(out.receipt().frame_visits_at_breaks, 6);
                            assert_eq!(out.receipt().work, 52);
                            assert_eq!(
                                out.contribution(1).unwrap().member(0).unwrap().character(),
                                separator
                            );
                        }
                    );
                }
            }
        }
    }
}
#[test]
fn normal_overlap_and_exact_work_limit() {
    with_predecessors!(
        recipe("a\nb", "ltr", "rtl", "normal"),
        |_whitespace, casing, structure| {
            let old = build_source_analysis_input(&structure, &casing, analysis_limits()).unwrap();
            let new = build_source_control_input(&structure, &casing, control_limits()).unwrap();
            assert_eq!(
                new.scope(0).unwrap().inline_sequence().unwrap().text(),
                old.scope(0).unwrap().inline_sequence().unwrap().text()
            );
            assert_eq!(new.receipt().work, 44);
            assert_eq!(new.receipt().synthetic_scalars, 0);
            let mut l = control_limits();
            l.work = nz(44);
            assert!(build_source_control_input(&structure, &casing, l).is_ok());
            l.work = nz(43);
            let e = build_source_control_input(&structure, &casing, l).unwrap_err();
            assert!(matches!(
                e.kind,
                SourceControlInputErrorKind::Limit(SourceControlLimit::Work)
            ));
        }
    );
}
#[test]
fn supplemental_boundary_rejections() {
    for value in ["\na", "a\n", "a\n\nb"] {
        with_predecessors!(
            recipe(value, "ltr", "rtl", "embed"),
            |_whitespace, casing, structure| {
                let e =
                    build_source_control_input(&structure, &casing, control_limits()).unwrap_err();
                let side = if value.starts_with('\n') {
                    SourceControlBreakSide::Before
                } else {
                    SourceControlBreakSide::After
                };
                assert!(
                    matches!(e.kind,SourceControlInputErrorKind::Unsupported(SourceControlUnsupported::BreakBoundary(actual)) if actual==side)
                );
                let payload = structure
                    .records()
                    .find(|r| r.kind() == SourceStructureKind::Payload)
                    .unwrap();
                assert_eq!(
                    (e.source, e.record, e.original, e.member),
                    (
                        payload.source_index(),
                        Some(payload.index()),
                        Some(if value.starts_with('\n') { 0 } else { 1 }),
                        Some(0)
                    )
                );
            }
        );
    }
}
#[test]
fn supplemental_empty_and_zero_output_preservation() {
    for (value, expected, members) in [("", "\u{202b}\u{202c}", 0), ("ß", "\u{202b}SS\u{202c}", 2)]
    {
        let mut root = recipe(value, "ltr", "rtl", "embed");
        root.children[0].children[0].operation = "upper".into();
        with_predecessors!(root, |_whitespace, casing, structure| {
            let out = build_source_control_input(&structure, &casing, control_limits()).unwrap();
            assert_eq!(
                out.scope(0).unwrap().inline_sequence().unwrap().text(),
                expected
            );
            assert_eq!(out.payload_count(), 1);
            if members == 0 {
                assert_eq!(out.contribution_count(), 0);
                assert_eq!(out.payload(0).unwrap().token_range(), 1..1);
            } else {
                assert_eq!(out.contribution(0).unwrap().member_count(), 2);
                for m in 0..2 {
                    let literal = out.contribution(0).unwrap().member(m).unwrap();
                    assert_eq!(literal.original_index(), 0);
                    assert_eq!(literal.member_index(), m);
                }
            }
        });
    }
    let root = managed(
        "s",
        InputFamily::Column,
        "block",
        "ltr",
        "normal",
        vec![
            text("s_0", "ß", "normal", "upper", "default", "normal"),
            text("s_1", "I", "normal", "lower", "tr", "normal"),
            text("s_2", "\u{307}", "normal", "lower", "tr", "normal"),
            text("s_3", " ", "normal", "none", "default", "normal"),
            text("s_4", " ", "normal", "none", "default", "normal"),
            text("s_5", "", "normal", "none", "default", "normal"),
        ],
    );
    with_predecessors!(root, |_whitespace, casing, structure| {
        let out = build_source_control_input(&structure, &casing, control_limits()).unwrap();
        assert_eq!(
            out.scope(0).unwrap().inline_sequence().unwrap().text(),
            "SSi "
        );
        assert_eq!(out.payload_count(), 6);
        assert_eq!(out.contribution_count(), 5);
        for (i, n) in [2, 1, 0, 1, 0].into_iter().enumerate() {
            assert_eq!(out.contribution(i).unwrap().member_count(), n);
        }
        assert!(out.payload(5).unwrap().original_range().is_empty());
    });
}
#[test]
fn preserved_sixth_interruption_rejected() {
    let root = managed(
        "B",
        InputFamily::Column,
        "block",
        "ltr",
        "normal",
        vec![managed(
            "I",
            InputFamily::Row,
            "inline",
            "rtl",
            "embed",
            vec![
                text("A", "a", "pre", "none", "default", "normal"),
                managed(
                    "K",
                    InputFamily::Column,
                    "block",
                    "ltr",
                    "normal",
                    vec![text("X", "x", "pre", "none", "default", "normal")],
                ),
                text("C", "b", "pre", "none", "default", "normal"),
            ],
        )],
    );
    with_predecessors!(root, |_whitespace, casing, structure| {
        let e = build_source_control_input(&structure, &casing, control_limits()).unwrap_err();
        assert!(matches!(
            e.kind,
            SourceControlInputErrorKind::Unsupported(SourceControlUnsupported::InterruptedInline(
                SourceStructureEdgeKind::BeforeInterruption
            ))
        ));
        assert_eq!(
            (e.source, e.record, e.original, e.member),
            (Some(4), Some(2), None, None)
        );
    });
}
