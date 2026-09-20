// Include after public.rs in the same support module. Supplemental captures unexecuted.
#[test]
fn repeated_breaks_have_distinct_actual_anchors() {
    with_predecessors!(
        recipe("a\nb\nc", "ltr", "rtl", "embed"),
        |_whitespace, casing, structure| {
            let out = build_source_control_input(&structure, &casing, control_limits()).unwrap();
            let domain = out.scope(0).unwrap().inline_sequence().unwrap();
            assert_eq!(
                domain.text(),
                "\u{202b}a\u{202c}\n\u{202b}b\u{202c}\n\u{202b}c\u{202c}"
            );
            let mut anchors = Vec::new();
            for i in 0..domain.scalar_count() {
                if let SourceControlOrigin::Synthetic(s) = domain.token(i).unwrap().origin()
                    && let SourceControlPlacement::Break { anchor, side } = s.placement()
                {
                    assert_eq!(anchor.member_index(), 0);
                    assert_eq!(anchor.character(), '\n');
                    anchors.push((anchor.original_index(), side));
                }
            }
            assert_eq!(
                anchors,
                vec![
                    (1, SourceControlBreakSide::Before),
                    (1, SourceControlBreakSide::After),
                    (3, SourceControlBreakSide::Before),
                    (3, SourceControlBreakSide::After)
                ]
            );
        }
    );
}
#[test]
fn empty_nested_and_sibling_payloads_preserve_distinct_causes() {
    for nested in [false, true] {
        let j = managed(
            "J",
            InputFamily::Row,
            "inline",
            "ltr",
            "bidi-override",
            vec![text("TJ", "", "pre", "none", "default", "normal")],
        );
        let i = managed(
            "I",
            InputFamily::Row,
            "inline",
            "rtl",
            "embed",
            if nested {
                vec![text("TI", "", "pre", "none", "default", "normal"), j]
            } else {
                vec![text("TI", "", "pre", "none", "default", "normal")]
            },
        );
        let children = if nested {
            vec![i]
        } else {
            vec![
                i,
                managed(
                    "J",
                    InputFamily::Row,
                    "inline",
                    "ltr",
                    "bidi-override",
                    vec![text("TJ", "", "pre", "none", "default", "normal")],
                ),
            ]
        };
        let root = managed("B", InputFamily::Column, "block", "ltr", "normal", children);
        with_predecessors!(root, |_whitespace, casing, structure| {
            let out = build_source_control_input(&structure, &casing, control_limits()).unwrap();
            let d = out.scope(0).unwrap().inline_sequence().unwrap();
            assert_eq!(
                d.text(),
                if nested {
                    "\u{202b}\u{202d}\u{202c}\u{202c}"
                } else {
                    "\u{202b}\u{202c}\u{202d}\u{202c}"
                }
            );
            assert_eq!(out.payload_count(), 2);
            assert_eq!(out.contribution_count(), 0);
            let p = out.payload(0).unwrap();
            let q = out.payload(1).unwrap();
            assert_eq!(p.original_range(), q.original_range());
            assert!(p.original_range().is_empty());
            assert!(!p.is_same_payload(&q));
            assert!(p.token_range().is_empty() && q.token_range().is_empty());
            let causes = (0..4)
                .map(|n| match d.token(n).unwrap().origin() {
                    SourceControlOrigin::Synthetic(s) => {
                        assert_eq!(s.occurrence_index(), n);
                        match s.placement() {
                            SourceControlPlacement::FragmentEdge(edge) => assert_eq!(
                                edge.kind(),
                                if if nested { n < 2 } else { n % 2 == 0 } {
                                    SourceStructureEdgeKind::OriginalBegin
                                } else {
                                    SourceStructureEdgeKind::OriginalEnd
                                }
                            ),
                            _ => panic!("empty payload cannot anchor B"),
                        };
                        s.cause_fragment().index()
                    }
                    _ => panic!("empty payload has no literals"),
                })
                .collect::<Vec<_>>();
            assert_ne!(causes[0], causes[if nested { 1 } else { 2 }]);
            assert_eq!(causes[0], causes[if nested { 3 } else { 1 }]);
            assert_eq!(
                causes[if nested { 1 } else { 2 }],
                causes[2 + usize::from(!nested)]
            );
        });
    }
}
#[test]
fn override_blocks_and_interior_b_interruption_reject() {
    // Block Embed is admitted (no inline embedding effect); only bidi-override rejects.
    {
        let bidi = "bidi-override";
        let root = managed(
            "B",
            InputFamily::Column,
            "block",
            "ltr",
            bidi,
            vec![text("T", "a\nb", "pre", "none", "default", "normal")],
        );
        with_predecessors!(root, |_whitespace, casing, structure| {
            let e = build_source_control_input(&structure, &casing, control_limits()).unwrap_err();
            assert!(matches!(
                e.kind,
                SourceControlInputErrorKind::Unsupported(SourceControlUnsupported::BlockBidi(_))
            ));
            let block = structure
                .records()
                .find(|r| r.kind() == SourceStructureKind::SourceBlock)
                .unwrap();
            assert_eq!(
                (e.source, e.record, e.original, e.member),
                (block.source_index(), None, None, None)
            );
        });
    }
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
                text("A", "a\nb", "pre", "none", "default", "normal"),
                managed(
                    "K",
                    InputFamily::Column,
                    "block",
                    "ltr",
                    "normal",
                    vec![text("X", "x", "pre", "none", "default", "normal")],
                ),
                text("C", "c", "pre", "none", "default", "normal"),
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
        let frame = structure
            .records()
            .find(|r| r.kind() == SourceStructureKind::SourceInline)
            .unwrap();
        assert_eq!(
            (e.source, e.record, e.original, e.member),
            (frame.source_index(), Some(frame.index()), None, None)
        );
    });
}
