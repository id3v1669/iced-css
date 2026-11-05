#[path = "support/clearance_fixture.rs"]
mod support;
use iced_css::evaluation::{
    clearance_bridge::{
        CapturedClearanceError, CapturedClearanceUnsupported, ClearanceOuterContentGeometry,
    },
    sibling_layout::{
        FloatClusterLayoutError, FloatClusterLayoutLimits, SiblingLayoutError, SiblingLayoutLimits,
        build_captured_float_cluster_layout, build_captured_sibling_layout,
    },
    source_display::SourceDisplay,
};
use iced_css_core::{
    formatting::{
        block::{BlockHorizontalError, BlockWidthError},
        clearance_float_cluster::{ClearanceFloatClusterError, ClearanceFloatClusterLimits},
        clearance_owner::ClearanceOwnerError,
        clearance_sequence::{ClearanceSequenceError, ClearanceSequenceLimits},
    },
    values::ConversionError,
};
use support::*;

fn limits() -> FloatClusterLayoutLimits {
    FloatClusterLayoutLimits {
        sources: bound(),
        work: nz(20_000_000),
        retained_bytes: nz(20_000_000),
        scratch_bytes: nz(20_000_000),
        core: ClearanceFloatClusterLimits {
            sources: bound(),
            floats: bound(),
            work: nz(20_000_000),
            retained_bytes: nz(20_000_000),
            scratch_bytes: nz(20_000_000),
        },
    }
}
fn facts_limits() -> SiblingBoxFactsLimits {
    SiblingBoxFactsLimits {
        sources: bound(),
        retained_bytes: nz(20_000_000),
        evaluation: fact_limits(),
    }
}
fn css() -> &'static str {
    "#o { width:100px; } #p { overflow:hidden; }
     #b1 { height:10px; } #f { float:left; width:40px; height:20px; }
     #b2 { float:right; width:30px; height:10px; } #b3 { height:1px; }"
}
fn geometry<'a>(projection: &'a SiblingSourceProjection<'_>) -> ClearanceOuterContentGeometry<'a> {
    ClearanceOuterContentGeometry {
        outer: &projection.nodes()[0],
        content_left: 0.0,
        content_width: 100.0,
        content_top: 0.0,
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SharedFailureStage {
    NormalClearance,
    SourceJoin,
    ChildAdmission,
    ChildVertical,
    ParentHorizontal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SharedFailureCause {
    UnsupportedEquality,
    GeometrySource,
    ChildOverflow,
    PercentageHeightUnrepresentable,
    ParentWidthUnrepresentable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SharedFailure {
    stage: SharedFailureStage,
    source: usize,
    cause: SharedFailureCause,
}

fn legacy_failure(error: SiblingLayoutError) -> SharedFailure {
    match error {
        SiblingLayoutError::Core(ClearanceSequenceError::Placement {
            source,
            error: ClearanceOwnerError::UnsupportedEquality,
        }) => SharedFailure {
            stage: SharedFailureStage::NormalClearance,
            source,
            cause: SharedFailureCause::UnsupportedEquality,
        },
        SiblingLayoutError::Captured(CapturedClearanceError::GeometrySource) => SharedFailure {
            stage: SharedFailureStage::SourceJoin,
            source: 0,
            cause: SharedFailureCause::GeometrySource,
        },
        SiblingLayoutError::Captured(CapturedClearanceError::Unsupported {
            projected,
            property: "overflow",
            reason: CapturedClearanceUnsupported::ChildOverflow,
        }) => SharedFailure {
            stage: SharedFailureStage::ChildAdmission,
            source: projected,
            cause: SharedFailureCause::ChildOverflow,
        },
        SiblingLayoutError::Percentage {
            source,
            property: "height",
            error: ConversionError::Unrepresentable,
        } => SharedFailure {
            stage: SharedFailureStage::ChildVertical,
            source,
            cause: SharedFailureCause::PercentageHeightUnrepresentable,
        },
        SiblingLayoutError::ParentHorizontal(BlockHorizontalError::Width(
            BlockWidthError::Unrepresentable,
        )) => SharedFailure {
            stage: SharedFailureStage::ParentHorizontal,
            source: 1,
            cause: SharedFailureCause::ParentWidthUnrepresentable,
        },
        other => panic!("unexpected legacy shared failure: {other:?}"),
    }
}

fn cluster_failure(error: FloatClusterLayoutError) -> SharedFailure {
    match error {
        FloatClusterLayoutError::Existing(error) => legacy_failure(error),
        FloatClusterLayoutError::Core(ClearanceFloatClusterError::Placement {
            source,
            error: ClearanceOwnerError::UnsupportedEquality,
        }) => SharedFailure {
            stage: SharedFailureStage::NormalClearance,
            source,
            cause: SharedFailureCause::UnsupportedEquality,
        },
        other => panic!("unexpected cluster shared failure: {other:?}"),
    }
}

fn legacy_limits() -> SiblingLayoutLimits {
    SiblingLayoutLimits {
        sources: bound(),
        work: nz(20_000_000),
        retained_bytes: nz(20_000_000),
        scratch_bytes: nz(20_000_000),
        core: ClearanceSequenceLimits {
            sources: bound(),
            work: nz(20_000_000),
            retained_bytes: nz(20_000_000),
        },
    }
}

fn with_host<T>(
    host: &InitialSourceHost,
    css: &str,
    run: impl FnOnce(
        &SiblingSourceProjection<'_>,
        &SourceDisplay<'_, '_, '_>,
        &OwnedStylesheetGraph,
    ) -> T,
) -> T {
    let snapshot = host.snapshot().unwrap();
    let projection = prepare_sibling_source_projection(
        host,
        snapshot,
        SiblingProjectionLimits {
            sources: bound(),
            work: bound(),
            retained_bytes: nz(20_000_000),
        },
    )
    .unwrap();
    let ua = catalog();
    let graph = graph(css, context(100, 100));
    let sheets = controls(&[1], true, 100);
    let bindings = [binding(&graph, GraphOrigin::Author(sheet(1)))];
    let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, bundle_limits()).unwrap();
    let display = evaluate_source_display(snapshot, &bundle, display_limits()).unwrap();
    run(&projection, &display, &graph)
}

#[test]
fn g9a_parent_height_uses_max_float_bottom() {
    let three = format!(
        "{} #b3 {{ float:left; width:40px; height:5px; }} #b4 {{ height:1px; }}",
        css()
    );
    sibling_fixture(7, &three, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, facts_limits()).unwrap();
        let owner =
            build_captured_float_cluster_layout(&facts, geometry(projection), None, None, limits())
                .unwrap();
        assert_eq!(owner.cluster().floats().len(), 3);
        // The source-last float ends25; the earlier left float ends30; normal ends11.
        assert_eq!(owner.children()[3].content().bottom, 25.0);
        assert_eq!(owner.children()[4].content().bottom, 11.0);
        assert_eq!(owner.parent().height().content_bottom, 30.0);
        assert_eq!(owner.parent().height().used_height, 30.0);
        assert_eq!(owner.parent().content().bottom, 30.0);
        for source in 0..7 {
            assert!(std::ptr::eq(
                owner.source(source).unwrap(),
                &projection.nodes()[source]
            ));
        }
        assert_eq!(
            owner
                .children()
                .iter()
                .map(|c| c.source_index())
                .collect::<Vec<_>>(),
            vec![2, 3, 4, 5, 6]
        );
    });
}

#[test]
fn g9a_second_float_auto_width_refused() {
    sibling_fixture(
        6,
        &format!("{} #b2 {{ width:auto; }}", css()),
        |projection, display| {
            let facts = evaluate_sibling_box_facts(projection, display, facts_limits()).unwrap();
            assert!(matches!(
                build_captured_float_cluster_layout(
                    &facts,
                    geometry(projection),
                    None,
                    None,
                    limits()
                ),
                Err(FloatClusterLayoutError::FloatWidth { source: 4 })
            ));
        },
    );
}

#[test]
fn g9a_second_float_auto_height_refused() {
    for height in ["auto", "50%"] {
        sibling_fixture(
            6,
            &format!("{} #b2 {{ height:{height}; }}", css()),
            |projection, display| {
                let facts =
                    evaluate_sibling_box_facts(projection, display, facts_limits()).unwrap();
                assert!(matches!(
                    build_captured_float_cluster_layout(
                        &facts,
                        geometry(projection),
                        None,
                        None,
                        limits()
                    ),
                    Err(FloatClusterLayoutError::FloatHeight { source: 4 })
                ));
            },
        );
    }
    // A definite parent height makes the same percentage an explicit used dimension.
    sibling_fixture(
        6,
        &format!(
            "{} #p {{ height:40px; }} #b2 {{ height:50%; width:10%; min-width:30px; max-width:50px; }}",
            css()
        ),
        |projection, display| {
            let facts = evaluate_sibling_box_facts(projection, display, facts_limits()).unwrap();
            let owner = build_captured_float_cluster_layout(
                &facts,
                geometry(projection),
                None,
                None,
                limits(),
            )
            .unwrap();
            assert_eq!(owner.children()[2].used_height(), 20.0);
            assert_eq!(owner.children()[2].used_width(), 30.0);
            assert_eq!(owner.children()[2].content().left, 70.0);
        },
    );
}

#[test]
fn g9a_float_reentry_refused() {
    let rule = format!(
        "{} #b2 {{ float:none; }} #b3 {{ float:right; width:30px; height:10px; }} #b4 {{ height:1px; }}",
        css()
    );
    sibling_fixture(7, &rule, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, facts_limits()).unwrap();
        assert!(matches!(
            build_captured_float_cluster_layout(&facts, geometry(projection), None, None, limits()),
            Err(FloatClusterLayoutError::ClusterOrder { source: 5 })
        ));
    });
}

#[test]
fn g9a_source_host_join_rejected() {
    let first = captured_siblings(6);
    let second = captured_siblings(6);
    with_host(&first, css(), |projection, display, _| {
        let facts = evaluate_sibling_box_facts(projection, display, facts_limits()).unwrap();
        with_host(&second, css(), |foreign, _, _| {
            assert!(!std::ptr::eq(&projection.nodes()[0], &foreign.nodes()[0]));
            assert!(matches!(build_captured_float_cluster_layout(&facts, geometry(foreign), None, None, limits()), Err(FloatClusterLayoutError::Existing(SiblingLayoutError::Captured(iced_css::evaluation::clearance_bridge::CapturedClearanceError::GeometrySource)))));
        });
        let owner =
            build_captured_float_cluster_layout(&facts, geometry(projection), None, None, limits())
                .unwrap();
        assert!(std::ptr::eq(
            owner.source(0).unwrap(),
            &projection.nodes()[0]
        ));
    });
}

#[test]
fn g9a_facade_one_float_complete_observation_parity() {
    let base = "#o { width:100px; } #p { overflow:hidden; padding-top:1px; padding-bottom:1px; }
        #b1 { height:10px; margin-bottom:2px; } #f { float:left; width:40px; height:20px; }
        #b2 { height:1px; clear:both; margin-bottom:-2px; } #b3 { height:1px; margin-top:3px; }";
    for extra in [
        "",
        "#f { padding:2px; border-style:solid; border-width:1px; margin-left:-2px; margin-right:3px; }",
        "#f { position:relative; left:3px; top:2px; margin-top:-0px; }",
        "#f { width:30%; min-width:40px; max-width:50px; }",
    ] {
        sibling_fixture(6, &format!("{base} {extra}"), |projection, display| {
            let facts = evaluate_sibling_box_facts(projection, display, facts_limits()).unwrap();
            let owner = build_captured_float_cluster_layout(
                &facts,
                geometry(projection),
                None,
                None,
                limits(),
            )
            .unwrap();
            let legacy = build_captured_sibling_layout(
                &facts,
                geometry(projection),
                SiblingLayoutLimits {
                    sources: bound(),
                    work: nz(20_000_000),
                    retained_bytes: nz(20_000_000),
                    scratch_bytes: nz(20_000_000),
                    core: ClearanceSequenceLimits {
                        sources: bound(),
                        work: nz(20_000_000),
                        retained_bytes: nz(20_000_000),
                    },
                },
            )
            .unwrap();
            // Both debug observations contain every private geometry/provenance field;
            // additionally compare float bits because ordinary PartialEq merges signed zero.
            assert_eq!(
                format!("{:?}", owner.parent()),
                format!("{:?}", legacy.parent())
            );
            assert_eq!(
                format!("{:?}", owner.children()),
                format!("{:?}", legacy.children())
            );
            for (a, b) in owner.children().iter().zip(legacy.children()) {
                assert_eq!(
                    [
                        a.content().left,
                        a.content().right,
                        a.content().top,
                        a.content().bottom,
                        a.used_margin_top(),
                        a.used_margin_bottom(),
                        a.used_width(),
                        a.used_height()
                    ]
                    .map(f32::to_bits),
                    [
                        b.content().left,
                        b.content().right,
                        b.content().top,
                        b.content().bottom,
                        b.used_margin_top(),
                        b.used_margin_bottom(),
                        b.used_width(),
                        b.used_height()
                    ]
                    .map(f32::to_bits)
                );
                assert_eq!(
                    a.computed_width(),
                    facts.row(a.source_index()).unwrap().width
                );
                assert_eq!(
                    a.computed_height(),
                    facts.row(a.source_index()).unwrap().height
                );
            }
        });
    }
}

#[test]
fn g9a_one_float_explicit_error_stage_parity() {
    const MAX: &str = "340282346638528859811704183484516925440px";
    let base = "#o { width:100px; } #p { overflow:hidden; }
        #b1 { height:10px; } #f { float:left; width:40px; height:20px; }
        #b2 { height:1px; clear:none; } #b3 { height:1px; }";

    let equality = format!("{base} #f {{ height:0px; }} #b2 {{ clear:both; }}");
    sibling_fixture(6, &equality, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, facts_limits()).unwrap();
        let expected = SharedFailure {
            stage: SharedFailureStage::NormalClearance,
            source: 4,
            cause: SharedFailureCause::UnsupportedEquality,
        };
        assert_eq!(
            legacy_failure(
                build_captured_sibling_layout(&facts, geometry(projection), legacy_limits(),)
                    .unwrap_err(),
            ),
            expected,
        );
        assert_eq!(
            cluster_failure(
                build_captured_float_cluster_layout(
                    &facts,
                    geometry(projection),
                    None,
                    None,
                    limits(),
                )
                .unwrap_err(),
            ),
            expected,
        );
    });

    let first = captured_siblings(6);
    let foreign_host = captured_siblings(6);
    with_host(&first, base, |projection, display, _| {
        let facts = evaluate_sibling_box_facts(projection, display, facts_limits()).unwrap();
        with_host(&foreign_host, base, |foreign, _, _| {
            let expected = SharedFailure {
                stage: SharedFailureStage::SourceJoin,
                source: 0,
                cause: SharedFailureCause::GeometrySource,
            };
            assert_eq!(
                legacy_failure(
                    build_captured_sibling_layout(&facts, geometry(foreign), legacy_limits(),)
                        .unwrap_err(),
                ),
                expected,
            );
            assert_eq!(
                cluster_failure(
                    build_captured_float_cluster_layout(
                        &facts,
                        geometry(foreign),
                        None,
                        None,
                        limits(),
                    )
                    .unwrap_err(),
                ),
                expected,
            );
        });
    });

    let held_parent = format!("{base} #p {{ width:{MAX}; margin-left:{MAX}; margin-right:auto; }}");
    sibling_fixture(6, &held_parent, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, facts_limits()).unwrap();
        let expected = SharedFailure {
            stage: SharedFailureStage::ParentHorizontal,
            source: 1,
            cause: SharedFailureCause::ParentWidthUnrepresentable,
        };
        assert_eq!(
            legacy_failure(
                build_captured_sibling_layout(&facts, geometry(projection), legacy_limits(),)
                    .unwrap_err(),
            ),
            expected,
        );
        assert_eq!(
            cluster_failure(
                build_captured_float_cluster_layout(
                    &facts,
                    geometry(projection),
                    None,
                    None,
                    limits(),
                )
                .unwrap_err(),
            ),
            expected,
        );
    });

    let admission = format!("{held_parent} #b1 {{ overflow:scroll; }}");
    sibling_fixture(6, &admission, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, facts_limits()).unwrap();
        let expected = SharedFailure {
            stage: SharedFailureStage::ChildAdmission,
            source: 2,
            cause: SharedFailureCause::ChildOverflow,
        };
        assert_eq!(
            legacy_failure(
                build_captured_sibling_layout(&facts, geometry(projection), legacy_limits(),)
                    .unwrap_err(),
            ),
            expected,
        );
        assert_eq!(
            cluster_failure(
                build_captured_float_cluster_layout(
                    &facts,
                    geometry(projection),
                    None,
                    None,
                    limits(),
                )
                .unwrap_err(),
            ),
            expected,
        );
    });

    let vertical = format!("{held_parent} #p {{ height:{MAX}; }} #b1 {{ height:200%; }}");
    sibling_fixture(6, &vertical, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, facts_limits()).unwrap();
        let expected = SharedFailure {
            stage: SharedFailureStage::ChildVertical,
            source: 2,
            cause: SharedFailureCause::PercentageHeightUnrepresentable,
        };
        assert_eq!(
            legacy_failure(
                build_captured_sibling_layout(&facts, geometry(projection), legacy_limits(),)
                    .unwrap_err(),
            ),
            expected,
        );
        assert_eq!(
            cluster_failure(
                build_captured_float_cluster_layout(
                    &facts,
                    geometry(projection),
                    None,
                    None,
                    limits(),
                )
                .unwrap_err(),
            ),
            expected,
        );
    });
}

#[test]
fn g9a_fact_report_geometry_parity() {
    use iced::advanced::subscription;
    use iced::futures::{self, Stream};
    use iced_css::diagnostics::{
        DiagnosticHost,
        border::{BorderCaptureLimits, BorderDeclarationBinding, BorderHostLimits},
    };
    use iced_css_core::{
        diagnostics::{Delivery, Provenance, QueueLimits},
        values::{RangeEvent, registry::PropertyId},
    };
    use std::{
        pin::Pin,
        sync::Arc,
        task::{Context, Poll, Waker},
    };
    fn stream(host: &DiagnosticHost) -> Pin<Box<dyn Stream<Item = Delivery> + Send>> {
        subscription::into_recipes(host.subscription())
            .pop()
            .unwrap()
            .stream(Box::pin(futures::stream::pending()))
    }
    let source = captured_siblings(6);
    let mut diagnostics =
        DiagnosticHost::new(NonZeroU64::MIN, QueueLimits::new(32, 1_048_576).unwrap());
    let mut reporter = diagnostics
        .bind_border_reporting(
            &source,
            BorderHostLimits {
                live_causes: nz(32),
                live_bytes: nz(1_048_576),
                transaction_bytes: nz(4_194_304),
                publication_work: nz(1_000_000),
            },
        )
        .unwrap();
    let rule = format!(
        "{} #b1 {{ border-top-style:solid; border-top-width:0.000000000000000000000000000000000000000000000000001px; }}",
        css()
    );
    with_host(&source, &rule, |projection, display, graph| {
        let provenance = Arc::new(Provenance {
            logical_name: "g9a.css".into(),
            revision: NonZeroU64::MIN,
        });
        let bindings: Vec<_> = graph
            .rules()
            .flat_map(|rule| {
                let provenance = Arc::clone(&provenance);
                (0..rule.rule.declarations.len()).map(move |ordinal| {
                    let declaration =
                        NonZeroU64::new(rule.context.first_declaration.get() + ordinal as u64)
                            .unwrap();
                    BorderDeclarationBinding {
                        graph,
                        resource: rule.source,
                        occurrence: rule.occurrence,
                        declaration,
                        logical_key: declaration,
                        provenance: Arc::clone(&provenance),
                    }
                })
            })
            .collect();
        let ordinary = evaluate_sibling_box_facts(projection, display, facts_limits()).unwrap();
        let reporting = evaluate_sibling_box_facts_reporting(
            projection,
            display,
            facts_limits(),
            &reporter,
            &bindings,
            BorderCaptureLimits {
                observations: nz(64),
                bindings: nz(128),
                capture_bytes: nz(262_144),
                prepared_bytes: nz(2_097_152),
                capture_work: nz(1_000_000),
                provenance_work: nz(1_000_000),
                preparation_work: nz(1_000_000),
            },
        )
        .unwrap();
        for index in 0..projection.nodes().len() {
            assert_eq!(ordinary.row(index), reporting.facts.row(index));
        }
        let a = build_captured_float_cluster_layout(
            &ordinary,
            geometry(projection),
            None,
            None,
            limits(),
        )
        .unwrap();
        let b = build_captured_float_cluster_layout(
            &reporting.facts,
            geometry(projection),
            None,
            None,
            limits(),
        )
        .unwrap();
        assert_eq!(format!("{:?}", a.parent()), format!("{:?}", b.parent()));
        assert_eq!(format!("{:?}", a.children()), format!("{:?}", b.children()));
        let mut delivery = stream(&diagnostics);
        assert!(matches!(
            delivery
                .as_mut()
                .poll_next(&mut Context::from_waker(Waker::noop())),
            Poll::Pending
        ));
        let publication = reporter.publish(reporting.reporting.unwrap()).unwrap();
        assert_eq!((publication.new_causes, publication.queued), (1, 1));
        match delivery
            .as_mut()
            .poll_next(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(Some(Delivery::Report(report))) => {
                assert_eq!(
                    report.diagnostic.instance,
                    Some(projection.nodes()[2].instance)
                );
                assert_eq!(
                    report.diagnostic.property,
                    PropertyId::lookup("border-top-width")
                );
                assert_eq!(
                    report.diagnostic.kind,
                    iced_css_core::diagnostics::DiagnosticKind::Range(
                        RangeEvent::PositiveRoundedToZero
                    )
                );
                assert_eq!(report.diagnostic.result.unwrap().to_bits(), 0);
            }
            _ => panic!("explicit publication must deliver exactly the captured border event"),
        }
        assert!(matches!(
            delivery
                .as_mut()
                .poll_next(&mut Context::from_waker(Waker::noop())),
            Poll::Pending
        ));
    });
}

#[test]
fn g9a_decorations_offsets_and_signed_float_extent() {
    let decorated = format!(
        "{} #f {{ padding:2px; border-style:solid; border-width:1px; }} #b3 {{ float:left; width:40px; height:5px; }} #b4 {{ height:1px; }}",
        css()
    );
    sibling_fixture(7, &decorated, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, facts_limits()).unwrap();
        let owner =
            build_captured_float_cluster_layout(&facts, geometry(projection), None, None, limits())
                .unwrap();
        let first = &owner.children()[1];
        assert_eq!(
            (
                first.border().left,
                first.border().right,
                first.border().top,
                first.border().bottom
            ),
            (0.0, 46.0, 10.0, 36.0)
        );
        assert_eq!(
            (
                first.padding().left,
                first.padding().right,
                first.content().left,
                first.content().right
            ),
            (1.0, 45.0, 3.0, 43.0)
        );
        assert_eq!(
            (
                owner.children()[3].content().left,
                owner.children()[3].content().top
            ),
            (46.0, 20.0)
        );
        assert_eq!(owner.parent().height().content_bottom, 36.0);
    });
    let shifted = format!("{} #b2 {{ position:relative; left:3px; top:5px; }}", css());
    sibling_fixture(6, &shifted, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, facts_limits()).unwrap();
        let owner =
            build_captured_float_cluster_layout(&facts, geometry(projection), None, None, limits())
                .unwrap();
        let second = &owner.children()[2];
        assert_eq!((second.content().left, second.content().top), (70.0, 10.0));
        let offset = second.relative_offset();
        assert_eq!((offset.left, offset.top), (3.0, 5.0));
        assert_eq!(
            (
                second.content().left + offset.left,
                second.content().top + offset.top
            ),
            (73.0, 15.0)
        );
        // Relative offsets change painting geometry; original float bands and parent height remain.
        assert_eq!(
            (
                owner.cluster().floats()[1].placed_float().left,
                owner.cluster().floats()[1].placed_float().top
            ),
            (70.0, 10.0)
        );
        assert_eq!(owner.parent().height().content_bottom, 30.0);
    });
    sibling_fixture(
        6,
        &format!("{} #f {{ margin-left:-2px; margin-right:3px; }}", css()),
        |projection, display| {
            let facts = evaluate_sibling_box_facts(projection, display, facts_limits()).unwrap();
            let owner = build_captured_float_cluster_layout(
                &facts,
                geometry(projection),
                None,
                None,
                limits(),
            )
            .unwrap();
            let placed = owner.cluster().floats()[0].placed_float();
            assert_eq!((placed.left, placed.right), (0.0, 41.0));
            assert_eq!(
                (
                    owner.children()[1].content().left,
                    owner.children()[1].content().right
                ),
                (-2.0, 38.0)
            );
        },
    );
    sibling_fixture(
        6,
        &format!("{} #b2 {{ width:1px; margin-left:-2px; }}", css()),
        |projection, display| {
            use iced_css_core::formatting::{
                clearance_float_cluster::ClearanceFloatClusterError,
                clearance_owner::{ClearanceOwnerError, OwnerProfileReason},
            };
            let facts = evaluate_sibling_box_facts(projection, display, facts_limits()).unwrap();
            assert!(matches!(
                build_captured_float_cluster_layout(
                    &facts,
                    geometry(projection),
                    None,
                    None,
                    limits()
                ),
                Err(FloatClusterLayoutError::Core(
                    ClearanceFloatClusterError::Profile(ClearanceOwnerError::Unsupported {
                        source: 4,
                        reason: OwnerProfileReason::NegativeFloatOuterExtent
                    })
                ))
            ));
        },
    );
}
