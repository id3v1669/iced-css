// Include after public.rs and additional.rs. Root owns compilation and wrapper installation.
fn qualification_error_context(error: &SourceControlInputError) -> [Option<usize>; 4] {
    [error.source, error.record, error.original, error.member]
}

fn qualification_two_domains() -> InputNode {
    managed(
        "B",
        InputFamily::Column,
        "block",
        "rtl",
        "normal",
        vec![
            managed(
                "K",
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
                    vec![text("T", "x", "pre", "none", "default", "normal")],
                )],
            ),
            managed(
                "L",
                InputFamily::Column,
                "block",
                "rtl",
                "normal",
                vec![managed(
                    "J",
                    InputFamily::Row,
                    "inline",
                    "ltr",
                    "bidi-override",
                    vec![text("U", "y", "pre", "none", "default", "normal")],
                )],
            ),
        ],
    )
}

#[test]
fn normal_anonymous_domains_inherit_base_and_depth_names_anonymous_record() {
    let root = managed(
        "B",
        InputFamily::Column,
        "block",
        "rtl",
        "normal",
        vec![managed(
            "I",
            InputFamily::Row,
            "inline",
            "ltr",
            "normal",
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
        let out = build_source_control_input(&structure, &casing, control_limits()).unwrap();
        let old = build_source_analysis_input(&structure, &casing, analysis_limits()).unwrap();
        assert_eq!((out.scope_count(), out.domain_count()), (4, 3));
        assert_eq!(out.receipt().synthetic_scalars, 0);
        assert_eq!(out.scope(0).unwrap().participation(), SourceAnalysisParticipation::BlockChildren);
        let enclosing = structure.root().unwrap();
        for (scope_index, expected, inherited) in [(1, "a", true), (2, "x", false), (3, "b", true)] {
            let scope = out.scope(scope_index).unwrap();
            let domain = scope.inline_sequence().unwrap();
            let old_scope = old.scope(scope_index).unwrap();
            assert_eq!(domain.text(), expected);
            assert_eq!(domain.text(), old_scope.inline_sequence().unwrap().text());
            assert_eq!(scope.mode(), old_scope.mode());
            let base = scope.base_origin();
            assert!(base.record().is_same_record(&scope.record()));
            if inherited {
                assert!(matches!(base, SourceControlBaseOrigin::AnonymousInherited(_)));
                assert_eq!(base.direction(), ComputedDirection::RightToLeft);
                assert_eq!(base.participant().source_index(), enclosing.source_index().unwrap());
                assert!(scope.record().anonymous_origin().unwrap().enclosing_block().is_same_record(&enclosing));
            } else {
                assert!(matches!(base, SourceControlBaseOrigin::OwnBlock(_)));
                assert_eq!(base.direction(), ComputedDirection::LeftToRight);
            }
            let token = domain.token(0).unwrap();
            match token.origin() {
                SourceControlOrigin::Literal(origin) => {
                    assert_eq!(origin.original_index(), scope_index - 1);
                    assert!(origin.scope().is_same_scope(&scope));
                }
                SourceControlOrigin::Synthetic(_) => panic!("Normal input emitted a CSS control"),
            }
        }
        let anonymous = out.scope(1).unwrap().record();
        assert_eq!(anonymous.kind(), SourceStructureKind::AnonymousBlock);
        let mut limits = control_limits();
        limits.depth = nz(1);
        let error = build_source_control_input(&structure, &casing, limits).unwrap_err();
        assert!(matches!(error.kind, SourceControlInputErrorKind::Limit(SourceControlLimit::Depth)));
        assert_eq!(qualification_error_context(&error), [None, Some(anonymous.index()), None, None]);
    });
}

#[test]
fn multiple_domains_reset_synthetic_ordinals_and_preserve_original_indices() {
    with_predecessors!(qualification_two_domains(), |_whitespace, casing, structure| {
        let out = build_source_control_input(&structure, &casing, control_limits()).unwrap();
        assert_eq!((out.scope_count(), out.domain_count(), out.token_count()), (3, 2, 6));
        let first = out.scope(1).unwrap().inline_sequence().unwrap();
        let second = out.scope(2).unwrap().inline_sequence().unwrap();
        assert!(!first.is_same_domain(&second));
        assert_eq!(first.text(), "\u{202b}x\u{202c}");
        assert_eq!(second.text(), "\u{202d}y\u{202c}");
        for (domain, original) in [(first, 0), (second, 1)] {
            for (token_index, occurrence) in [(0, 0), (2, 1)] {
                let token = domain.token(token_index).unwrap();
                assert_eq!(token.ordinal(), token_index);
                match token.origin() {
                    SourceControlOrigin::Synthetic(origin) => {
                        assert_eq!(origin.occurrence_index(), occurrence);
                        assert!(origin.domain().is_same_domain(&domain));
                        assert!(origin.token().is_same_token(&token));
                        match origin.placement() {
                            SourceControlPlacement::FragmentEdge(edge) => {
                                assert!(edge.record().is_same_record(&origin.cause_fragment()));
                                assert_eq!(edge.kind(), if token_index == 0 { SourceStructureEdgeKind::OriginalBegin } else { SourceStructureEdgeKind::OriginalEnd });
                            }
                            SourceControlPlacement::Break { .. } => panic!("B-free domain has a break placement"),
                        }
                    }
                    SourceControlOrigin::Literal(_) => panic!("missing synthetic edge"),
                }
            }
            match domain.token(1).unwrap().origin() {
                SourceControlOrigin::Literal(origin) => {
                    assert_eq!((origin.original_index(), origin.member_index()), (original, 0));
                    assert_eq!(origin.payload().token_range(), 1..2);
                }
                SourceControlOrigin::Synthetic(_) => panic!("literal source member was replaced"),
            }
        }
        let a = match first.token(0).unwrap().origin() { SourceControlOrigin::Synthetic(s) => s, _ => unreachable!() };
        let b = match second.token(0).unwrap().origin() { SourceControlOrigin::Synthetic(s) => s, _ => unreachable!() };
        assert!(!a.is_same_origin(&b));
        assert!(!a.cause_fragment().is_same_record(&b.cause_fragment()));
    });
}

#[test]
fn pointer_mismatch_precedes_all_control_limits() {
    with_predecessors!(qualification_two_domains(), |wa, _ca, sa| {
        with_predecessors!(qualification_two_domains(), |wb, cb, _sb| {
            assert!(!std::ptr::eq(&wa, &wb));
            let one = nz(1);
            let limits = SourceControlInputLimits {
                sources: one, records: one, originals: one, scopes: one, depth: one,
                output_scalars: one, output_bytes: one, retained_bytes: one,
                scratch_bytes: one, peak_bytes: one, work: one,
            };
            let error = build_source_control_input(&sa, &cb, limits).unwrap_err();
            assert!(matches!(error.kind, SourceControlInputErrorKind::OwnerMismatch));
            assert_eq!(qualification_error_context(&error), [None; 4]);
        });
    });
}

#[test]
fn exact_and_one_less_control_limits_preserve_phase_and_departing_contexts() {
    with_predecessors!(qualification_two_domains(), |_whitespace, casing, structure| {
        let out = build_source_control_input(&structure, &casing, control_limits()).unwrap();
        let r = out.receipt();
        assert_eq!((r.sources, r.records, r.original_scalars, r.scopes, r.max_depth), (7, 9, 2, 3, 4));
        assert_eq!((r.literal_scalars, r.synthetic_scalars, r.output_scalars, r.output_bytes, r.work), (2, 4, 6, 14, 58));
        let second_scope = out.scope(2).unwrap();
        let second_domain = second_scope.inline_sequence().unwrap();
        let departing = match second_domain.token(2).unwrap().origin() { SourceControlOrigin::Synthetic(s) => s.cause_fragment(), _ => unreachable!() };
        let first_payload = out.payload(0).unwrap().record();
        let deepest = first_payload.formatting_parent().unwrap();
        let mut exact = control_limits();
        exact.sources = nz(r.sources);
        exact.records = nz(r.records);
        exact.originals = nz(r.original_scalars);
        exact.scopes = nz(r.scopes);
        exact.depth = nz(r.max_depth);
        exact.output_scalars = nz(r.output_scalars);
        exact.output_bytes = nz(r.output_bytes);
        exact.work = nz(r.work);
        assert_eq!(build_source_control_input(&structure, &casing, exact).unwrap().receipt().work, 58);
        for limit in [SourceControlLimit::Sources, SourceControlLimit::Records, SourceControlLimit::Originals, SourceControlLimit::Scopes, SourceControlLimit::Depth, SourceControlLimit::OutputScalars, SourceControlLimit::OutputBytes, SourceControlLimit::Work] {
            let mut short = exact;
            let expected_context = match limit {
                SourceControlLimit::Sources => { short.sources = nz(r.sources - 1); [None; 4] }
                SourceControlLimit::Records => { short.records = nz(r.records - 1); [None; 4] }
                SourceControlLimit::Originals => { short.originals = nz(r.original_scalars - 1); [None; 4] }
                SourceControlLimit::Scopes => { short.scopes = nz(r.scopes - 1); [second_scope.record().source_index(), Some(second_scope.record().index()), None, None] }
                SourceControlLimit::Depth => { short.depth = nz(r.max_depth - 1); [deepest.source_index(), Some(deepest.index()), None, None] }
                SourceControlLimit::OutputScalars => { short.output_scalars = nz(r.output_scalars - 1); [departing.source_index(), Some(departing.index()), None, None] }
                SourceControlLimit::OutputBytes => { short.output_bytes = nz(r.output_bytes - 1); [departing.source_index(), Some(departing.index()), None, None] }
                SourceControlLimit::Work => { short.work = nz(r.work - 1); [None; 4] }
                _ => unreachable!(),
            };
            let error = build_source_control_input(&structure, &casing, short).unwrap_err();
            assert!(matches!(error.kind, SourceControlInputErrorKind::Limit(actual) if actual == limit), "expected {limit:?}, got {error:?}");
            assert_eq!(qualification_error_context(&error), expected_context, "{limit:?}");
        }
        let mut combined = exact;
        combined.sources = nz(1);
        combined.records = nz(1);
        combined.work = nz(1);
        let error = build_source_control_input(&structure, &casing, combined).unwrap_err();
        assert!(matches!(error.kind, SourceControlInputErrorKind::Limit(SourceControlLimit::Sources)));
        assert_eq!(qualification_error_context(&error), [None; 4]);
        let mut combined = exact;
        combined.output_scalars = nz(5);
        combined.output_bytes = nz(13);
        let error = build_source_control_input(&structure, &casing, combined).unwrap_err();
        assert!(matches!(error.kind, SourceControlInputErrorKind::Limit(SourceControlLimit::OutputScalars)));
        assert_eq!(qualification_error_context(&error), [departing.source_index(), Some(departing.index()), None, None]);
    });
}
