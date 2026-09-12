// Public integration literals for the approved SourceAnalysisInput API.
include!("adapter.rs");

fn managed(
    label: &str,
    family: InputFamily,
    display: &str,
    direction: &str,
    unicode_bidi: &str,
    children: Vec<InputNode>,
) -> InputNode {
    InputNode {
        label: label.into(),
        family,
        display: display.into(),
        white_space: "normal".into(),
        position: "static".into(),
        direction: direction.into(),
        unicode_bidi: unicode_bidi.into(),
        operation: "none".into(),
        profile: "default".into(),
        text: String::new(),
        runs: None,
        children,
    }
}
fn text(
    label: &str,
    value: &str,
    white_space: &str,
    operation: &str,
    profile: &str,
    unicode_bidi: &str,
) -> InputNode {
    InputNode {
        label: label.into(),
        family: InputFamily::Text,
        display: "inline".into(),
        white_space: white_space.into(),
        position: "static".into(),
        direction: "ltr".into(),
        unicode_bidi: unicode_bidi.into(),
        operation: operation.into(),
        profile: profile.into(),
        text: value.into(),
        runs: None,
        children: vec![],
    }
}
fn rich(label: &str, value: &str, runs: Vec<Range<usize>>, white_space: &str) -> InputNode {
    InputNode {
        label: label.into(),
        family: InputFamily::RichText,
        display: "inline".into(),
        white_space: white_space.into(),
        position: "static".into(),
        direction: "ltr".into(),
        unicode_bidi: "normal".into(),
        operation: "none".into(),
        profile: "default".into(),
        text: value.into(),
        runs: Some(runs),
        children: vec![],
    }
}

macro_rules! with_predecessors {
    ($root:expr,|$whitespace:ident,$casing:ident,$structure:ident|$body:block) => {{
        let (construction, css) = actual_tree(&$root);
        let host = host(construction, None, None);
        let ua = catalog();
        let graph = graph(&css, ua.next_context());
        let sheets = controls(&[1], true, 100);
        let bindings = [binding(&graph, GraphOrigin::Author(sheet(1)))];
        let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, bundle_limits()).unwrap();
        let display = evaluate_source_display(host.snapshot().unwrap(), &bundle, limits()).unwrap();
        let eligibility = evaluate_source_eligibility(&display, eligibility_limits()).unwrap();
        let $whitespace = preprocess_source_whitespace(
            &SourceParticipants::new(&eligibility),
            whitespace_limits(),
        )
        .unwrap();
        let $casing = evaluate_source_casing(&$whitespace, casing_limits()).unwrap();
        let $structure = build_source_structure(&$whitespace, structure_limits()).unwrap();
        $body
    }};
}

#[test]
fn empty_owner_has_zero_allocations_and_one_work() {
    with_predecessors!(
        managed(
            "s",
            InputFamily::Column,
            "none",
            "ltr",
            "normal",
            vec![text("s_0", "hidden", "normal", "none", "default", "normal")]
        ),
        |whitespace, casing, structure| {
            let limits = SourceAnalysisInputLimits {
                sources: nz(1),
                scopes: nz(1),
                output_scalars: nz(1),
                output_bytes: nz(1),
                retained_bytes: nz(1),
                work: nz(1),
            };
            let out = build_source_analysis_input(&structure, &casing, limits).unwrap();
            let r = out.receipt();
            assert_eq!(
                [
                    r.sources,
                    r.records,
                    r.payloads,
                    r.original_scalars,
                    r.scopes,
                    r.domains,
                    r.output_scalars,
                    r.output_bytes,
                    r.scope_capacity,
                    r.scalar_capacity,
                    r.transport_capacity,
                    r.work,
                    r.retained_bytes,
                    r.peak_retained_bytes
                ],
                [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0]
            );
            assert!(
                out.scope(0).is_none()
                    && out.contribution(0).is_none()
                    && out.payload_projection(0).is_none()
            );
            assert!(std::ptr::eq(out.whitespace(), &whitespace));
        }
    );
}

#[test]
fn own_base_mode_ignores_inline_direction_and_preserves_controls() {
    with_predecessors!(
        managed(
            "s",
            InputFamily::Column,
            "block",
            "ltr",
            "normal",
            vec![managed(
                "s_0",
                InputFamily::Row,
                "inline",
                "rtl",
                "normal",
                vec![text(
                    "s_0_0",
                    "\u{202a}א\n\u{202c}",
                    "pre",
                    "none",
                    "default",
                    "normal"
                )]
            )]
        ),
        |_whitespace, casing, structure| {
            let out = build_source_analysis_input(&structure, &casing, analysis_limits()).unwrap();
            let scope = out.scope(0).unwrap();
            assert_eq!(
                scope.participation(),
                SourceAnalysisParticipation::InlineSequence
            );
            assert_eq!(scope.mode(), BidiParagraphMode::LeftToRight);
            assert!(matches!(
                scope.base_origin(),
                SourceAnalysisBaseOrigin::OwnBlock(_)
            ));
            let domain = scope.inline_sequence().unwrap();
            assert_eq!(domain.text(), "\u{202a}א\n\u{202c}");
            assert_eq!(domain.scalar_count(), 4);
            for local in 0..4 {
                let origin = domain.origin(local).unwrap();
                assert_eq!(origin.analysis_index(), local);
                assert_eq!(
                    origin.character(),
                    domain.text().chars().nth(local).unwrap()
                );
            }
            assert!(domain.origin(4).is_none());
        }
    );
}

#[test]
fn expansion_deletion_whitespace_suppression_and_empty_payload_are_distinct() {
    with_predecessors!(
        managed(
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
                text("s_5", "", "normal", "none", "default", "normal")
            ]
        ),
        |_whitespace, casing, structure| {
            let out = build_source_analysis_input(&structure, &casing, analysis_limits()).unwrap();
            assert_eq!(out.scope_count(), 1);
            assert_eq!(out.contribution_count(), 5);
            assert_eq!(
                out.scope(0).unwrap().inline_sequence().unwrap().text(),
                "SSi "
            );
            let expected = [("SS", 2), ("i", 1), ("", 0), (" ", 1), ("", 0)];
            for (i, (value, n)) in expected.into_iter().enumerate() {
                let c = out.contribution(i).unwrap();
                assert_eq!(c.presentation().characters().collect::<String>(), value);
                assert_eq!(c.analysis_range().len(), n);
                assert!(c.payload().kind() == SourceStructureKind::Payload);
            }
            let payloads = structure
                .records()
                .filter(|r| r.kind() == SourceStructureKind::Payload)
                .collect::<Vec<_>>();
            let empty = out.payload_projection(payloads[5].index()).unwrap();
            assert!(
                empty.original_range().is_empty()
                    && empty.transport_range().is_empty()
                    && empty.analysis_range().is_empty()
            );
            assert!(
                empty
                    .domain()
                    .is_same_domain(&out.scope(0).unwrap().inline_sequence().unwrap())
            );
        }
    );
}

#[test]
fn scope_contribution_member_and_projection_identity_require_input_owner() {
    with_predecessors!(
        managed(
            "s",
            InputFamily::Column,
            "block",
            "ltr",
            "normal",
            vec![text("s_0", "x", "normal", "none", "default", "normal")]
        ),
        |_whitespace, casing, structure| {
            let a = build_source_analysis_input(&structure, &casing, analysis_limits()).unwrap();
            let b = build_source_analysis_input(&structure, &casing, analysis_limits()).unwrap();
            let sa = a.scope(0).unwrap();
            let sb = b.scope(0).unwrap();
            assert!(!sa.is_same_scope(&sb));
            let da = sa.inline_sequence().unwrap();
            let db = sb.inline_sequence().unwrap();
            assert!(!da.is_same_domain(&db));
            let ca = a.contribution(0).unwrap();
            let cb = b.contribution(0).unwrap();
            assert!(!ca.is_same_contribution(&cb));
            let ma = da.origin(0).unwrap();
            let mb = db.origin(0).unwrap();
            assert!(!ma.is_same_origin(&mb));
            let payload = ca.payload().index();
            assert!(
                !a.payload_projection(payload)
                    .unwrap()
                    .is_same_projection(&b.payload_projection(payload).unwrap())
            );
            assert!(sa.record().is_same_record(&sb.record()));
            assert!(ca.casing_scalar().index() == cb.casing_scalar().index());
        }
    );
}

#[test]
fn iterators_are_exact_fused_and_scope_subranges_match() {
    with_predecessors!(
        managed(
            "s",
            InputFamily::Column,
            "block",
            "ltr",
            "normal",
            vec![rich("s_0", "é\r\n漢", vec![0..3, 3..7], "pre")]
        ),
        |_whitespace, casing, structure| {
            let out = build_source_analysis_input(&structure, &casing, analysis_limits()).unwrap();
            let mut scopes = out.scopes();
            exact_fused(&scopes);
            assert_eq!(scopes.size_hint(), (1, Some(1)));
            let scope = scopes.next().unwrap();
            assert_eq!(scopes.size_hint(), (0, Some(0)));
            assert!(scopes.next().is_none() && scopes.next().is_none());
            let mut all = out.contributions();
            exact_fused(&all);
            assert_eq!(all.size_hint(), (3, Some(3)));
            assert_eq!(
                scope
                    .contributions()
                    .map(|c| c.original_index())
                    .collect::<Vec<_>>(),
                scope.original_range().collect::<Vec<_>>()
            );
            while all.next().is_some() {}
            assert_eq!(all.size_hint(), (0, Some(0)));
            assert!(all.next().is_none() && all.next().is_none());
        }
    );
}

#[test]
fn blockchildren_and_anonymous_inherited_bases_remain_observable_without_domains() {
    with_predecessors!(
        managed(
            "s",
            InputFamily::Column,
            "block",
            "rtl",
            "normal",
            vec![managed(
                "s_0",
                InputFamily::Row,
                "inline",
                "ltr",
                "normal",
                vec![
                    managed(
                        "s_0_0",
                        InputFamily::Row,
                        "block",
                        "ltr",
                        "normal",
                        vec![text("s_0_0_0", "a", "normal", "none", "default", "normal")]
                    ),
                    text("s_0_1", "b", "normal", "none", "default", "normal"),
                    managed("s_0_2", InputFamily::Row, "block", "ltr", "normal", vec![])
                ]
            )]
        ),
        |_whitespace, casing, structure| {
            let out = build_source_analysis_input(&structure, &casing, analysis_limits()).unwrap();
            assert!(out.scopes().any(|s| s.participation()
                == SourceAnalysisParticipation::BlockChildren
                && s.inline_sequence().is_none()
                && s.original_range().is_empty()));
            let anonymous = out
                .scopes()
                .find(|s| s.record().kind() == SourceStructureKind::AnonymousBlock)
                .unwrap();
            assert!(matches!(
                anonymous.base_origin(),
                SourceAnalysisBaseOrigin::AnonymousInherited(_)
            ));
            assert_eq!(anonymous.base_origin().participant().source_index(), 5);
            assert_eq!(anonymous.base_origin().direction().as_str(), "rtl");
            assert_eq!(anonymous.mode(), BidiParagraphMode::RightToLeft);
        }
    );
}

#[test]
fn empty_payloads_at_equal_offsets_authenticate_distinct_domain_scopes() {
    with_predecessors!(
        managed(
            "s",
            InputFamily::Column,
            "block",
            "ltr",
            "normal",
            vec![managed(
                "s_0",
                InputFamily::Row,
                "inline",
                "ltr",
                "normal",
                vec![
                    text("s_0_0", "", "normal", "none", "default", "normal"),
                    managed("s_0_1", InputFamily::Row, "block", "ltr", "normal", vec![]),
                    text("s_0_2", "", "normal", "none", "default", "normal")
                ]
            )]
        ),
        |_whitespace, casing, structure| {
            let out = build_source_analysis_input(&structure, &casing, analysis_limits()).unwrap();
            let values = structure
                .records()
                .filter(|r| r.kind() == SourceStructureKind::Payload)
                .map(|r| out.payload_projection(r.index()).unwrap())
                .collect::<Vec<_>>();
            assert_eq!(values.len(), 2);
            assert_eq!(values[0].transport_range(), values[1].transport_range());
            assert_ne!(values[0].scope().index(), values[1].scope().index());
            assert!(!values[0].is_same_projection(&values[1]));
            assert!(!values[0].domain().is_same_domain(&values[1].domain()));
        }
    );
}

#[test]
fn equal_local_raw_coordinates_do_not_authenticate_a_foreign_domain() {
    with_predecessors!(
        managed(
            "s",
            InputFamily::Column,
            "block",
            "ltr",
            "normal",
            vec![
                managed(
                    "s_0",
                    InputFamily::Row,
                    "block",
                    "ltr",
                    "normal",
                    vec![text("s_0_0", "a", "normal", "none", "default", "normal")]
                ),
                managed(
                    "s_1",
                    InputFamily::Row,
                    "block",
                    "ltr",
                    "normal",
                    vec![text("s_1_0", "b", "normal", "none", "default", "normal")]
                )
            ]
        ),
        |_whitespace, casing, structure| {
            let out = build_source_analysis_input(&structure, &casing, analysis_limits()).unwrap();
            let domains = out
                .scopes()
                .filter_map(|s| s.inline_sequence())
                .collect::<Vec<_>>();
            assert_eq!(domains.len(), 2);
            let a = domains[0].origin(0).unwrap();
            let b = domains[1].origin(0).unwrap();
            assert_eq!(a.member_index(), b.member_index());
            assert!(!a.is_same_origin(&b));
            assert!(!domains[0].is_same_domain(&domains[1]));
        }
    );
}

#[test]
fn owner_mismatch_is_pointer_identity_and_first_gate() {
    with_predecessors!(
        managed("s", InputFamily::Column, "block", "ltr", "normal", vec![]),
        |wa, _ca, sa| {
            with_predecessors!(
                managed("s", InputFamily::Column, "block", "ltr", "normal", vec![]),
                |wb, cb, _sb| {
                    assert!(!std::ptr::eq(&wa, &wb));
                    let tiny = SourceAnalysisInputLimits {
                        sources: nz(1),
                        scopes: nz(1),
                        output_scalars: nz(1),
                        output_bytes: nz(1),
                        retained_bytes: nz(1),
                        work: nz(1),
                    };
                    assert!(matches!(
                        build_source_analysis_input(&sa, &cb, tiny),
                        Err(SourceAnalysisInputError::OwnerMismatch)
                    ));
                }
            );
        }
    );
}

#[test]
fn first_eligible_nonnormal_precedes_output_limits() {
    with_predecessors!(
        managed(
            "s",
            InputFamily::Column,
            "block",
            "ltr",
            "embed",
            vec![managed(
                "s_0",
                InputFamily::Row,
                "inline",
                "ltr",
                "bidi-override",
                vec![text("s_0_0", "xx", "normal", "none", "default", "normal")]
            )]
        ),
        |_whitespace, casing, structure| {
            let mut lim = analysis_limits();
            lim.output_scalars = nz(1);
            lim.output_bytes = nz(1);
            assert!(matches!(
                build_source_analysis_input(&structure, &casing, lim),
                Err(SourceAnalysisInputError::Nonnormal { source: 2, .. })
            ));
        }
    );
}

#[test]
fn later_eligible_nonnormal_preflight_precedes_earlier_output_exhaustion() {
    with_predecessors!(
        managed(
            "s",
            InputFamily::Column,
            "block",
            "ltr",
            "normal",
            vec![
                text("s_0", "xx", "normal", "none", "default", "normal"),
                managed("s_1", InputFamily::Row, "inline", "ltr", "embed", vec![])
            ]
        ),
        |_whitespace, casing, structure| {
            let mut lim = analysis_limits();
            lim.output_scalars = nz(1);
            lim.output_bytes = nz(1);
            assert!(matches!(
                build_source_analysis_input(&structure, &casing, lim),
                Err(SourceAnalysisInputError::Nonnormal { source: 1, .. })
            ));
        }
    );
}

#[test]
fn public_count_limit_categories_and_fixed_requested_byte_preflight() {
    with_predecessors!(
        managed(
            "s",
            InputFamily::Column,
            "block",
            "ltr",
            "normal",
            vec![text("s_0", "SS", "normal", "none", "default", "normal")]
        ),
        |_whitespace, casing, structure| {
            let mut lim = analysis_limits();
            lim.sources = nz(1);
            assert!(matches!(
                build_source_analysis_input(&structure, &casing, lim),
                Err(SourceAnalysisInputError::Sources)
            ));
            let mut lim = analysis_limits();
            lim.output_scalars = nz(1);
            assert!(matches!(
                build_source_analysis_input(&structure, &casing, lim),
                Err(SourceAnalysisInputError::OutputScalars)
            ));
            let mut lim = analysis_limits();
            lim.output_bytes = nz(1);
            assert!(matches!(
                build_source_analysis_input(&structure, &casing, lim),
                Err(SourceAnalysisInputError::OutputBytes)
            ));
            let mut lim = analysis_limits();
            lim.retained_bytes = nz(1);
            assert!(matches!(
                build_source_analysis_input(&structure, &casing, lim),
                Err(SourceAnalysisInputError::Bytes)
            ));
        }
    );
    with_predecessors!(
        managed(
            "s",
            InputFamily::Column,
            "block",
            "ltr",
            "normal",
            vec![
                managed("s_0", InputFamily::Row, "block", "ltr", "normal", vec![]),
                managed("s_1", InputFamily::Row, "block", "ltr", "normal", vec![])
            ]
        ),
        |_w, c, s| {
            let mut lim = analysis_limits();
            lim.scopes = nz(1);
            assert!(matches!(
                build_source_analysis_input(&s, &c, lim),
                Err(SourceAnalysisInputError::Scopes)
            ));
        }
    );
}

#[test]
fn public_zero_request_allocations_and_three_live_capacity_receipt() {
    with_predecessors!(
        managed("s", InputFamily::Column, "block", "ltr", "normal", vec![]),
        |_w, c, s| {
            let out = build_source_analysis_input(&s, &c, analysis_limits()).unwrap();
            let r = out.receipt();
            assert!(r.scope_capacity >= 1);
            assert_eq!((r.scalar_capacity, r.transport_capacity), (0, 0));
            assert_eq!(r.retained_bytes, r.scope_capacity * r.scope_element_bytes);
        }
    );
    with_predecessors!(
        managed(
            "s",
            InputFamily::Column,
            "block",
            "ltr",
            "normal",
            vec![managed(
                "s_0",
                InputFamily::Row,
                "inline",
                "ltr",
                "normal",
                vec![
                    managed("s_0_0", InputFamily::Row, "block", "ltr", "normal", vec![]),
                    text("s_0_1", "x", "normal", "none", "default", "normal"),
                    managed("s_0_2", InputFamily::Row, "block", "ltr", "normal", vec![])
                ]
            )]
        ),
        |_w, c, s| {
            let out = build_source_analysis_input(&s, &c, analysis_limits()).unwrap();
            let r = out.receipt();
            assert_eq!(
                (
                    r.sources,
                    r.records,
                    r.original_scalars,
                    r.output_scalars,
                    r.output_bytes,
                    r.scopes,
                    r.domains,
                    r.work
                ),
                (5, 11, 1, 1, 1, 6, 3, 33)
            );
            assert!(r.scope_capacity >= 6 && r.scalar_capacity >= 1 && r.transport_capacity >= 1);
            assert_eq!(r.retained_bytes, r.peak_retained_bytes);
        }
    );
}

#[test]
fn real_recipe_hand_work_anchors_and_representable_one_short() {
    with_predecessors!(
        managed("s", InputFamily::Column, "none", "ltr", "normal", vec![]),
        |_w, c, s| {
            let mut exact = analysis_limits();
            exact.work = nz(1);
            assert_eq!(
                build_source_analysis_input(&s, &c, exact).unwrap().work(),
                1
            );
        }
    );
    with_predecessors!(
        managed("s", InputFamily::Column, "block", "ltr", "normal", vec![]),
        |_w, c, s| {
            for (limit, ok) in [(5, true), (4, false)] {
                let mut x = analysis_limits();
                x.work = nz(limit);
                assert_eq!(build_source_analysis_input(&s, &c, x).is_ok(), ok);
            }
        }
    );
    let all_inline = managed(
        "s",
        InputFamily::Column,
        "block",
        "ltr",
        "normal",
        vec![
            managed("s_0", InputFamily::Row, "inline", "ltr", "normal", vec![]),
            text("s_1", "", "normal", "none", "default", "normal"),
            rich("s_2", "", vec![], "normal"),
            rich("s_3", "", std::iter::once(0..0).collect(), "normal"),
        ],
    );
    with_predecessors!(all_inline, |_w, c, s| {
        for (limit, ok) in [(23, true), (22, false)] {
            let mut x = analysis_limits();
            x.work = nz(limit);
            assert_eq!(build_source_analysis_input(&s, &c, x).is_ok(), ok);
        }
    });
    let nested = managed(
        "s",
        InputFamily::Column,
        "block",
        "ltr",
        "normal",
        vec![managed(
            "s_0",
            InputFamily::Container,
            "inline",
            "ltr",
            "normal",
            vec![managed(
                "s_0_0",
                InputFamily::Row,
                "inline",
                "ltr",
                "normal",
                vec![
                    managed(
                        "s_0_0_0",
                        InputFamily::Row,
                        "block",
                        "ltr",
                        "normal",
                        vec![],
                    ),
                    managed(
                        "s_0_0_1",
                        InputFamily::Row,
                        "block",
                        "ltr",
                        "normal",
                        vec![],
                    ),
                ],
            )],
        )],
    );
    with_predecessors!(nested, |_w, c, s| {
        for (limit, ok) in [(25, true), (24, false)] {
            let mut x = analysis_limits();
            x.work = nz(limit);
            assert_eq!(build_source_analysis_input(&s, &c, x).is_ok(), ok);
        }
    });
    let separated = managed(
        "s",
        InputFamily::Column,
        "block",
        "ltr",
        "normal",
        vec![managed(
            "s_0",
            InputFamily::Row,
            "inline",
            "ltr",
            "normal",
            vec![
                managed("s_0_0", InputFamily::Row, "block", "ltr", "normal", vec![]),
                text("s_0_1", "x", "normal", "none", "default", "normal"),
                managed("s_0_2", InputFamily::Row, "block", "ltr", "normal", vec![]),
            ],
        )],
    );
    with_predecessors!(separated, |_w, c, s| {
        for (limit, ok) in [(33, true), (32, false)] {
            let mut x = analysis_limits();
            x.work = nz(limit);
            assert_eq!(build_source_analysis_input(&s, &c, x).is_ok(), ok);
        }
    });
}

#[test]
fn expected_free_observer_path_is_live_in_public_install() {
    observe(
        "public-smoke",
        &managed(
            "s",
            InputFamily::Column,
            "block",
            "ltr",
            "normal",
            vec![
                text("s_0", "x", "normal", "none", "default", "normal"),
                rich("s_1", "é\r\n漢", vec![0..3, 3..7], "pre"),
            ],
        ),
    );
}
