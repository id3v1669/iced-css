fn interruption_recipe(base: &str, middle: &str, direction: &str, bidi: &str) -> InputNode {
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
            vec![
                text("A", "a", "pre", "none", "default", "normal"),
                managed(
                    "K",
                    InputFamily::Column,
                    "block",
                    middle,
                    "normal",
                    vec![text("X", "x", "pre", "none", "default", "normal")],
                ),
                text("C", "b", "pre", "none", "default", "normal"),
            ],
        )],
    )
}

#[test]
fn single_interruption_direction_matrix_and_work_boundaries() {
    for base in ["ltr", "rtl"] {
        for middle in ["ltr", "rtl"] {
            for (direction, bidi, opening) in [
                ("ltr", "embed", '\u{202a}'),
                ("rtl", "embed", '\u{202b}'),
                ("ltr", "bidi-override", '\u{202d}'),
                ("rtl", "bidi-override", '\u{202e}'),
            ] {
                let root = interruption_recipe(base, middle, direction, bidi);
                with_predecessors!(root, |_whitespace, casing, structure| {
                    let mut limits = control_limits();
                    limits.work = nz(83);
                    let owner = build_source_control_input(&structure, &casing, limits).unwrap();
                    assert_eq!(owner.receipt().work, 83);
                    assert_eq!(owner.receipt().frame_visits_at_breaks, 0);
                    assert_eq!(owner.receipt().output_bytes, 15);
                    let expected = [
                        format!("{opening}a\u{202c}"),
                        "x".into(),
                        format!("{opening}b\u{202c}"),
                    ];
                    for (index, expected) in expected.iter().enumerate() {
                        let scope = owner.scope(index + 1).unwrap();
                        assert_eq!(scope.inline_sequence().unwrap().text(), expected);
                        assert_eq!(
                            scope.base_origin().direction().as_str(),
                            if index == 1 { middle } else { base }
                        );
                    }
                    for (work, source, record) in
                        [(21, None, None), (22, Some(4), Some(2)), (82, None, None)]
                    {
                        limits.work = nz(work);
                        let error =
                            build_source_control_input(&structure, &casing, limits).unwrap_err();
                        assert!(matches!(
                            error.kind,
                            SourceControlInputErrorKind::Limit(SourceControlLimit::Work)
                        ));
                        assert_eq!(
                            (error.source, error.record, error.original, error.member),
                            (source, record, None, None)
                        );
                    }
                });
            }
        }
    }
}

#[test]
fn single_interruption_rejects_presented_breaks_and_empty_domains() {
    for slot in 0..3 {
        for value in ["", "a\nb", "\n"] {
            let mut root = interruption_recipe("ltr", "ltr", "rtl", "embed");
            let inline = &mut root.children[0];
            let content = match slot {
                0 => &mut inline.children[0],
                1 => &mut inline.children[1].children[0],
                _ => &mut inline.children[2],
            };
            content.text = value.into();
            with_predecessors!(root, |_whitespace, casing, structure| {
                // Observed first-interruption identity, never a hardcoded source/record.
                let first = structure
                    .records()
                    .find(|record| {
                        record.kind() == SourceStructureKind::SourceInline
                            && record.end_edge().unwrap().kind()
                                == SourceStructureEdgeKind::BeforeInterruption
                    })
                    .unwrap();
                let error =
                    build_source_control_input(&structure, &casing, control_limits()).unwrap_err();
                assert!(matches!(
                    error.kind,
                    SourceControlInputErrorKind::Unsupported(
                        SourceControlUnsupported::InterruptedInline(
                            SourceStructureEdgeKind::BeforeInterruption
                        )
                    )
                ));
                assert_eq!(
                    (error.source, error.record, error.original, error.member),
                    (first.source_index(), Some(first.index()), None, None)
                );
            });
        }
    }
}

#[test]
fn single_interruption_does_not_regenerate_literal_controls() {
    for literal in ['\u{202c}', '\u{2067}'] {
        let mut root = interruption_recipe("ltr", "ltr", "rtl", "embed");
        root.children[0].children[0].text = format!("{literal}a");
        with_predecessors!(root, |_whitespace, casing, structure| {
            let owner = build_source_control_input(&structure, &casing, control_limits()).unwrap();
            let before = owner.scope(1).unwrap().inline_sequence().unwrap();
            let after = owner.scope(3).unwrap().inline_sequence().unwrap();
            assert_eq!(before.text(), format!("\u{202b}{literal}a\u{202c}"));
            assert_eq!(after.text(), "\u{202b}b\u{202c}");
            let token = before.token(1).unwrap();
            let SourceControlOrigin::Literal(origin) = token.origin() else {
                panic!("user control became synthetic")
            };
            assert_eq!(origin.character(), literal);
            assert_eq!(
                origin.contribution().whitespace_scalar().source_range(),
                0..3
            );
            assert_eq!(owner.receipt().synthetic_scalars, 4);
        });
    }
}

#[test]
fn single_interruption_rejects_extra_topology() {
    for variant in 0..4 {
        let mut root = interruption_recipe("ltr", "ltr", "rtl", "embed");
        let children = &mut root.children[0].children;
        match variant {
            0 | 3 => {
                let before = children.remove(0);
                children.insert(
                    0,
                    managed(
                        "J",
                        InputFamily::Row,
                        "inline",
                        "ltr",
                        if variant == 3 { "embed" } else { "normal" },
                        vec![before],
                    ),
                );
            }
            1 => children.push(text("D", "d", "pre", "none", "default", "normal")),
            _ => children.insert(
                2,
                managed(
                    "L",
                    InputFamily::Column,
                    "block",
                    "ltr",
                    "normal",
                    vec![text("Y", "y", "pre", "none", "default", "normal")],
                ),
            ),
        }
        with_predecessors!(root, |_whitespace, casing, structure| {
            let first = structure
                .records()
                .find(|record| {
                    record.kind() == SourceStructureKind::SourceInline
                        && record
                            .participant()
                            .unwrap()
                            .bidi_style()
                            .unicode_bidi()
                            .as_str()
                            != "normal"
                        && record.end_edge().unwrap().kind()
                            == SourceStructureEdgeKind::BeforeInterruption
                })
                .unwrap();
            let error =
                build_source_control_input(&structure, &casing, control_limits()).unwrap_err();
            assert!(matches!(
                error.kind,
                SourceControlInputErrorKind::Unsupported(
                    SourceControlUnsupported::InterruptedInline(
                        SourceStructureEdgeKind::BeforeInterruption
                    )
                )
            ));
            assert_eq!(
                (error.source, error.record, error.original, error.member),
                (first.source_index(), Some(first.index()), None, None)
            );
        });
    }
}

#[test]
fn single_interruption_keeps_nonempty_collapsed_spaces() {
    for slot in 0..3 {
        let mut root = interruption_recipe("ltr", "ltr", "rtl", "embed");
        let inline = &mut root.children[0];
        let content = match slot {
            0 => &mut inline.children[0],
            1 => &mut inline.children[1].children[0],
            _ => &mut inline.children[2],
        };
        content.text = " ".into();
        content.white_space = "normal".into();
        with_predecessors!(root, |_whitespace, casing, structure| {
            assert_eq!(
                casing
                    .scalar(slot)
                    .unwrap()
                    .presentation()
                    .characters()
                    .count(),
                1
            );
            let owner = build_source_control_input(&structure, &casing, control_limits()).unwrap();
            let domain = owner.scope(slot + 1).unwrap().inline_sequence().unwrap();
            assert_eq!(
                domain.text(),
                if slot == 1 { " " } else { "\u{202b} \u{202c}" }
            );
        });
    }
}

#[test]
fn normal_interruption_keeps_existing_work_and_domains() {
    let root = interruption_recipe("rtl", "ltr", "rtl", "normal");
    with_predecessors!(root, |_whitespace, casing, structure| {
        let mut limits = control_limits();
        limits.work = nz(63);
        let owner = build_source_control_input(&structure, &casing, limits).unwrap();
        assert_eq!(owner.receipt().work, 63);
        assert_eq!(owner.receipt().synthetic_scalars, 0);
        for (index, expected) in ["a", "x", "b"].into_iter().enumerate() {
            assert_eq!(
                owner
                    .scope(index + 1)
                    .unwrap()
                    .inline_sequence()
                    .unwrap()
                    .text(),
                expected
            );
        }
    });
}

#[test]
fn single_interruption_uses_eligible_roles_instead_of_source_numbers() {
    let mut root = interruption_recipe("ltr", "ltr", "rtl", "embed");
    root.children[0].children[0] = rich("A", "a", std::iter::once(0..1).collect(), "pre");
    let mut hidden = text("hidden", "h", "pre", "none", "default", "normal");
    hidden.display = "none".into();
    root.children.insert(0, hidden);
    with_predecessors!(root, |_whitespace, casing, structure| {
        assert_eq!(
            (structure.source_count(), structure.record_count()),
            (6, 12)
        );
        assert_eq!(structure.root().unwrap().source_index(), Some(6));
        assert_eq!(structure.record(2).unwrap().source_index(), Some(5));
        let owner = build_source_control_input(&structure, &casing, control_limits()).unwrap();
        assert_eq!(owner.receipt().work, 83);
        assert_eq!(
            owner
                .scope(1)
                .unwrap()
                .base_origin()
                .participant()
                .source_index(),
            6
        );
        let domain = owner.scope(1).unwrap().inline_sequence().unwrap();
        assert_eq!(domain.text(), "\u{202b}a\u{202c}");
        let SourceControlOrigin::Synthetic(origin) = domain.token(0).unwrap().origin() else {
            panic!("missing opening")
        };
        assert_eq!(origin.cause().source_index(), 5);
        let SourceControlOrigin::Literal(origin) = domain.token(1).unwrap().origin() else {
            panic!("missing rich content")
        };
        assert_eq!(
            origin
                .contribution()
                .whitespace_scalar()
                .participant()
                .source_index(),
            1
        );
    });
}

#[test]
fn single_interruption_rejects_override_blocks_before_inventory() {
    // Plan §4: bidi-override on B or K in the six-source graph is rejected as BlockBidi by the
    // source pass, which precedes the interruption inventory; the block's observed
    // source wins over the cached interruption witness.
    for block in ["B", "K"] {
        {
            let bidi = "bidi-override";
            let mut root = interruption_recipe("ltr", "ltr", "rtl", "embed");
            let target = if block == "B" {
                &mut root
            } else {
                &mut root.children[0].children[1]
            };
            target.unicode_bidi = bidi.into();
            with_predecessors!(root, |_whitespace, casing, structure| {
                let rejected = structure
                    .records()
                    .find(|record| {
                        record.kind() == SourceStructureKind::SourceBlock
                            && record
                                .participant()
                                .unwrap()
                                .bidi_style()
                                .unicode_bidi()
                                .as_str()
                                == bidi
                    })
                    .unwrap();
                // A work allowance of 7 (1 setup + 6 sources) still reaches BlockBidi, so the
                // rejection precedes the 12-unit inventory charge.
                let mut limits = control_limits();
                limits.work = nz(7);
                let error = build_source_control_input(&structure, &casing, limits).unwrap_err();
                assert!(matches!(
                    error.kind,
                    SourceControlInputErrorKind::Unsupported(SourceControlUnsupported::BlockBidi(_))
                ));
                assert_eq!(
                    (error.source, error.record, error.original, error.member),
                    (rejected.source_index(), None, None, None)
                );
            });
        }
    }
}
