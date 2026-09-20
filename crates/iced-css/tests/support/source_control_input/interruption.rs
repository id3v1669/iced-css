// Installed sixth-witness success oracle (Row/pre capture); the original Column rejection
// fixture in public.rs was converted to a success assertion under the amended plan.
#[test]
fn accepted_single_normal_block_interruption_preserves_all_origins() {
    let mut root = managed(
        "B",
        InputFamily::Row,
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
                    InputFamily::Row,
                    "block",
                    "ltr",
                    "normal",
                    vec![text("X", "x", "pre", "none", "default", "normal")],
                ),
                text("C", "b", "pre", "none", "default", "normal"),
            ],
        )],
    );
    root.white_space = "pre".into();
    root.children[0].white_space = "pre".into();
    root.children[0].children[1].white_space = "pre".into();
    with_predecessors!(root, |_whitespace, casing, structure| {
        let inventory = [
            (SourceStructureKind::SourceBlock, Some(5), None),
            (SourceStructureKind::AnonymousBlock, None, Some(0)),
            (SourceStructureKind::SourceInline, Some(4), Some(1)),
            (SourceStructureKind::SourceInline, Some(0), Some(2)),
            (SourceStructureKind::Payload, Some(0), Some(3)),
            (SourceStructureKind::SourceBlock, Some(2), Some(0)),
            (SourceStructureKind::SourceInline, Some(1), Some(5)),
            (SourceStructureKind::Payload, Some(1), Some(6)),
            (SourceStructureKind::AnonymousBlock, None, Some(0)),
            (SourceStructureKind::SourceInline, Some(4), Some(8)),
            (SourceStructureKind::SourceInline, Some(3), Some(9)),
            (SourceStructureKind::Payload, Some(3), Some(10)),
        ];
        assert_eq!(structure.record_count(), inventory.len());
        for (index, (kind, source, parent)) in inventory.into_iter().enumerate() {
            let record = structure.record(index).unwrap();
            assert_eq!(
                (
                    record.kind(),
                    record.source_index(),
                    record.formatting_parent().map(|p| p.index())
                ),
                (kind, source, parent)
            );
        }
        let owner = build_source_control_input(&structure, &casing, control_limits()).unwrap();
        assert_eq!(
            (
                owner.scope_count(),
                owner.domain_count(),
                owner.payload_count(),
                owner.contribution_count(),
                owner.token_count()
            ),
            (4, 3, 3, 3, 7)
        );
        let root_scope = owner.scope(0).unwrap();
        assert_eq!(root_scope.record().index(), 0);
        assert_eq!(
            root_scope.participation(),
            SourceAnalysisParticipation::BlockChildren
        );
        assert!(root_scope.inline_sequence().is_none());
        for (record, text, payload, original, source, inherited) in [
            (1, "\u{202b}a\u{202c}", 4, 0, 0, true),
            (5, "x", 7, 1, 1, false),
            (8, "\u{202b}b\u{202c}", 11, 2, 3, true),
        ] {
            let scope = (0..owner.scope_count())
                .map(|i| owner.scope(i).unwrap())
                .find(|s| s.record().index() == record)
                .unwrap();
            assert_eq!(scope.mode(), BidiParagraphMode::LeftToRight);
            let base = scope.base_origin();
            assert_eq!(base.record().index(), record);
            assert_eq!(base.direction().as_str(), "ltr");
            if inherited {
                assert!(matches!(
                    base,
                    SourceControlBaseOrigin::AnonymousInherited(_)
                ));
                assert_eq!(base.participant().source_index(), 5);
                assert_eq!(
                    scope
                        .record()
                        .anonymous_origin()
                        .unwrap()
                        .enclosing_block()
                        .index(),
                    0
                );
            } else {
                assert!(matches!(base, SourceControlBaseOrigin::OwnBlock(_)));
                assert_eq!(base.participant().source_index(), 2);
            }
            let domain = scope.inline_sequence().unwrap();
            assert_eq!(domain.text(), text);
            assert_eq!(domain.scalar_count(), text.chars().count());
            let literal_ordinal = usize::from(inherited);
            let token = domain.token(literal_ordinal).unwrap();
            let SourceControlOrigin::Literal(literal) = token.origin() else {
                panic!("literal source member lost")
            };
            assert_eq!(
                (
                    literal.original_index(),
                    literal.member_index(),
                    literal.payload().record().index()
                ),
                (original, 0, payload)
            );
            assert!(literal.scope().is_same_scope(&scope));
            let scalar = literal.contribution().whitespace_scalar();
            assert_eq!(
                (scalar.index(), scalar.participant().source_index()),
                (original, source)
            );
            assert_eq!(scalar.source_range(), 0..1);
            assert_eq!(literal.character(), ['a', 'x', 'b'][original]);
            assert!(literal.token().is_same_token(&token));
            assert!(
                owner
                    .contribution(original)
                    .unwrap()
                    .literal_token(0)
                    .unwrap()
                    .is_same_token(&token)
            );
            assert!(literal.contribution().member(1).is_none());
            assert_eq!(
                literal.payload().token_range(),
                literal_ordinal..literal_ordinal + 1
            );
            assert_eq!(
                literal.contribution().token_range(),
                literal_ordinal..literal_ordinal + 1
            );
            let mut offset = 0;
            for (ordinal, ch) in text.chars().enumerate() {
                let t = domain.token(ordinal).unwrap();
                assert_eq!((t.ordinal(), t.character()), (ordinal, ch));
                assert_eq!(t.byte_range(), offset..offset + ch.len_utf8());
                offset += ch.len_utf8();
            }
            assert!(domain.token(domain.scalar_count()).is_none());
        }
        for (domain_record, ordinal, fragment, side, kind, event, event_source, cut, occurrence) in [
            (
                1,
                0,
                2,
                SourceStructureBoundarySide::Start,
                SourceStructureEdgeKind::OriginalBegin,
                1,
                4,
                2,
                0,
            ),
            (
                1,
                2,
                2,
                SourceStructureBoundarySide::End,
                SourceStructureEdgeKind::BeforeInterruption,
                5,
                2,
                5,
                1,
            ),
            (
                8,
                0,
                9,
                SourceStructureBoundarySide::Start,
                SourceStructureEdgeKind::AfterInterruption,
                9,
                2,
                10,
                0,
            ),
            (
                8,
                2,
                9,
                SourceStructureBoundarySide::End,
                SourceStructureEdgeKind::OriginalEnd,
                13,
                4,
                13,
                1,
            ),
        ] {
            let scope = (0..owner.scope_count())
                .map(|i| owner.scope(i).unwrap())
                .find(|s| s.record().index() == domain_record)
                .unwrap();
            let domain = scope.inline_sequence().unwrap();
            let token = domain.token(ordinal).unwrap();
            let SourceControlOrigin::Synthetic(synthetic) = token.origin() else {
                panic!("synthetic edge lost")
            };
            assert_eq!(
                (
                    synthetic.occurrence_index(),
                    synthetic.cause().source_index(),
                    synthetic.cause_fragment().index()
                ),
                (occurrence, 4, fragment)
            );
            assert_eq!(synthetic.cause().bidi_style().direction().as_str(), "rtl");
            assert_eq!(
                synthetic.cause().bidi_style().unicode_bidi().as_str(),
                "embed"
            );
            assert!(synthetic.token().is_same_token(&token));
            assert!(synthetic.domain().is_same_domain(&domain));
            let SourceControlPlacement::FragmentEdge(edge) = synthetic.placement() else {
                panic!("interruption must remain an actual edge, not synthetic B anchor")
            };
            assert_eq!(
                (
                    edge.record().index(),
                    edge.side(),
                    edge.kind(),
                    edge.event().index(),
                    edge.event().participant().source_index(),
                    edge.cut()
                ),
                (fragment, side, kind, event, event_source, cut)
            );
        }
        // Work/resource accounting for this witness is asserted in interruption_matrix.rs.
    });
}
