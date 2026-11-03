//! CF34 integration oracles exercise actual graph evaluation and the shared queue.
use super::*;
use iced::advanced::subscription;
use iced::futures::{self, Stream};
use iced_css::diagnostics::{DiagnosticHost, border::*};
use iced_css::evaluation::sibling_box_facts::{
    evaluate_sibling_box_facts_reporting, evaluate_sibling_box_facts_with_font_medium_reporting,
};
use iced_css::evaluation::source_display::SourceDisplay;
use iced_css_core::diagnostics::{
    Delivery, Diagnostic, DiagnosticKind, Origin, Provenance, QueueLimits,
};
use iced_css_core::values::{NonnegativeF32, RangeEvent, registry::PropertyId};
use std::{
    pin::Pin,
    sync::Arc,
    task::{Context, Poll, Waker},
};

const H: &str = "1000000000000000000000000000000000000000";
const Z: &str = "0.000000000000000000000000000000000000000000000000001";
const MAX: &str = "340282346638528859811704183484516925440";
const MIN_SUB: &str = "0.00000000000000000000000000000000000000000000140129846432481707092372958328991613128026194187651577175706828388979108268586060148663818836212158203125";

fn host_limits() -> BorderHostLimits {
    BorderHostLimits {
        live_causes: nz(32),
        live_bytes: nz(1_048_576),
        transaction_bytes: nz(4_194_304),
        publication_work: nz(1_000_000),
    }
}
fn capture_limits() -> BorderCaptureLimits {
    BorderCaptureLimits {
        observations: nz(64),
        bindings: nz(128),
        capture_bytes: nz(262_144),
        prepared_bytes: nz(2_097_152),
        capture_work: nz(1_000_000),
        provenance_work: nz(1_000_000),
        preparation_work: nz(1_000_000),
    }
}
fn diagnostics(count: usize) -> DiagnosticHost {
    DiagnosticHost::new(NonZeroU64::MIN, QueueLimits::new(count, 1_048_576).unwrap())
}
fn top(value: &str) -> String {
    format!("#p {{ border-top-style:solid; border-top-width:{value}px; }}")
}
fn mapping(
    graph: &OwnedStylesheetGraph,
    revision: u64,
    key_offset: u64,
) -> Vec<BorderDeclarationBinding<'_>> {
    let provenance = Arc::new(Provenance {
        logical_name: "fixture.css".into(),
        revision: NonZeroU64::new(revision).unwrap(),
    });
    graph
        .rules()
        .flat_map(|rule| {
            let source = Arc::clone(&provenance);
            (0..rule.rule.declarations.len()).map(move |ordinal| {
                let declaration =
                    NonZeroU64::new(rule.context.first_declaration.get() + ordinal as u64).unwrap();
                BorderDeclarationBinding {
                    graph,
                    resource: rule.source,
                    occurrence: rule.occurrence,
                    declaration,
                    logical_key: NonZeroU64::new(declaration.get() + key_offset).unwrap(),
                    provenance: Arc::clone(&source),
                }
            })
        })
        .collect()
}

// Every invocation borrows the SAME source host; only admitted CSS owners vary.
fn with_graph<T>(
    source: &InitialSourceHost,
    css: &str,
    run: impl FnOnce(
        &SiblingSourceProjection<'_>,
        &SourceDisplay<'_, '_, '_>,
        &OwnedStylesheetGraph,
    ) -> T,
) -> T {
    let snapshot = source.snapshot().unwrap();
    let projection = prepare_sibling_source_projection(
        source,
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
fn batch(
    source: &InitialSourceHost,
    reporter: &BorderReporter<'_>,
    css: &str,
    revision: u64,
    key_offset: u64,
    medium: Option<f32>,
) -> BorderObservationBatch {
    with_graph(source, css, |projection, display, graph| {
        let bindings = mapping(graph, revision, key_offset);
        let result = match medium {
            Some(medium) => evaluate_sibling_box_facts_with_font_medium_reporting(
                projection,
                display,
                medium,
                sibling_fact_limits(),
                reporter,
                &bindings,
                capture_limits(),
            ),
            None => evaluate_sibling_box_facts_reporting(
                projection,
                display,
                sibling_fact_limits(),
                reporter,
                &bindings,
                capture_limits(),
            ),
        }
        .unwrap();
        result.reporting.unwrap()
    })
}
fn stream(host: &DiagnosticHost) -> Pin<Box<dyn Stream<Item = Delivery> + Send>> {
    subscription::into_recipes(host.subscription())
        .pop()
        .unwrap()
        .stream(Box::pin(futures::stream::pending()))
}
fn drain(host: &DiagnosticHost) -> Vec<Delivery> {
    let mut stream = stream(host);
    let mut output = Vec::new();
    while let Poll::Ready(Some(delivery)) = stream
        .as_mut()
        .poll_next(&mut Context::from_waker(Waker::noop()))
    {
        output.push(delivery);
    }
    output
}
fn reports(host: &DiagnosticHost) -> Vec<Diagnostic> {
    drain(host)
        .into_iter()
        .map(|delivery| match delivery {
            Delivery::Report(report) => report.diagnostic,
            Delivery::Omissions { .. } => panic!("unexpected omission"),
        })
        .collect()
}

// Removing conversion-event filtering or swapping side properties breaks these payloads.
#[test]
fn cf34_upper_and_positive_zero() {
    let source = captured_siblings(5);
    let mut host = diagnostics(32);
    let mut reporter = host.bind_border_reporting(&source, host_limits()).unwrap();
    let css =
        format!("#p {{ border-style:solid; border-top-width:{H}px; border-right-width:{Z}px; }}");
    let result = reporter
        .publish(batch(&source, &reporter, &css, 1, 0, None))
        .unwrap();
    assert_eq!((result.new_causes, result.queued), (2, 2));
    let delivered = reports(&host);
    assert_eq!(delivered.len(), 2);
    for (report, (property, bits, event)) in delivered.iter().zip([
        (
            "border-top-width",
            f32::MAX.to_bits(),
            RangeEvent::UpperSaturation,
        ),
        ("border-right-width", 0, RangeEvent::PositiveRoundedToZero),
    ]) {
        assert_eq!(report.property, PropertyId::lookup(property));
        assert_eq!(report.result.unwrap().to_bits(), bits);
        assert_eq!(report.kind, DiagnosticKind::Range(event));
        assert!(report.instance.is_some());
    }
}

#[test]
fn cf34_exact_and_ordinary_silent() {
    let source = captured_siblings(5);
    let mut host = diagnostics(32);
    let mut reporter = host.bind_border_reporting(&source, host_limits()).unwrap();
    for value in [
        format!("{MAX}px"),
        format!("{MIN_SUB}px"),
        "0.1px".into(),
        "thin".into(),
        "medium".into(),
        "thick".into(),
        "-0px".into(),
    ] {
        let css = format!("#p {{border-top-style:solid;border-top-width:{value}}}");
        let result = reporter
            .publish(batch(&source, &reporter, &css, 1, 0, None))
            .unwrap();
        assert_eq!((result.new_causes, result.queued), (0, 0), "{value}");
    }
    assert!(reports(&host).is_empty());
    with_graph(&source, &top(MIN_SUB), |projection, display, _| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        assert_eq!(facts.row(1).unwrap().border_width[0].pixels.to_bits(), 1);
        assert_eq!(facts.row(1).unwrap().border_width[0].event, None);
    });
}

#[test]
fn cf34_shorthand_four_side_identity() {
    let source = captured_siblings(5);
    let mut host = diagnostics(32);
    let mut reporter = host.bind_border_reporting(&source, host_limits()).unwrap();
    let css = format!("#p {{border-style:solid;border-width:{H}px}}");
    assert_eq!(
        reporter
            .publish(batch(&source, &reporter, &css, 1, 0, None))
            .unwrap()
            .queued,
        4
    );
    let delivered = reports(&host);
    assert_eq!(delivered.len(), 4);
    let mut original = None;
    for (report, property) in delivered.iter().zip([
        "border-top-width",
        "border-right-width",
        "border-bottom-width",
        "border-left-width",
    ]) {
        assert_eq!(report.property, PropertyId::lookup(property));
        let Origin::Authored { source, span } = &report.origin else {
            panic!("authored origin required")
        };
        let identity = (
            source.logical_name.clone(),
            source.revision,
            span.bytes.clone(),
        );
        if let Some(old) = &original {
            assert_eq!(&identity, old);
        } else {
            original = Some(identity);
        }
    }
}

#[test]
fn cf34_actual_winner_override() {
    let source = captured_siblings(5);
    let mut host = diagnostics(32);
    let mut reporter = host.bind_border_reporting(&source, host_limits()).unwrap();
    let css = format!("#p {{border-style:solid;border-width:{H}px;border-left-width:1px}}");
    assert_eq!(
        reporter
            .publish(batch(&source, &reporter, &css, 1, 0, None))
            .unwrap()
            .queued,
        3
    );
    let delivered = reports(&host);
    assert_eq!(
        delivered.iter().map(|r| r.property).collect::<Vec<_>>(),
        [
            "border-top-width",
            "border-right-width",
            "border-bottom-width"
        ]
        .map(PropertyId::lookup)
    );
}

#[test]
fn cf34_repeat_clear_recur() {
    let source = captured_siblings(5);
    let mut host = diagnostics(32);
    let mut reporter = host.bind_border_reporting(&source, host_limits()).unwrap();
    let first = reporter
        .publish(batch(&source, &reporter, &top(H), 1, 0, None))
        .unwrap();
    let repeated = reporter
        .publish(batch(&source, &reporter, &top(H), 1, 0, None))
        .unwrap();
    let cleared = reporter
        .publish(batch(&source, &reporter, &top("1"), 1, 0, None))
        .unwrap();
    let recurred = reporter
        .publish(batch(&source, &reporter, &top(H), 1, 0, None))
        .unwrap();
    assert_eq!(
        (
            first.new_causes,
            repeated.suppressed,
            cleared.cleared,
            recurred.new_causes
        ),
        (1, 1, 1, 1)
    );
    assert_eq!(reports(&host).len(), 2);
}

#[test]
fn cf34_revision_and_declaration_identity() {
    let source = captured_siblings(5);
    let mut host = diagnostics(32);
    let mut reporter = host.bind_border_reporting(&source, host_limits()).unwrap();
    assert_eq!(
        reporter
            .publish(batch(&source, &reporter, &top(H), 1, 0, None))
            .unwrap()
            .new_causes,
        1
    );
    assert_eq!(
        reporter
            .publish(batch(&source, &reporter, &top(H), 2, 0, None))
            .unwrap()
            .suppressed,
        1
    );
    assert_eq!(
        reporter
            .publish(batch(&source, &reporter, &top(H), 2, 1000, None))
            .unwrap()
            .new_causes,
        1
    );
    assert_eq!(reports(&host).len(), 2);
}

#[test]
fn cf34_em_ex_context_identity() {
    for unit in ["em", "ex"] {
        let source = captured_siblings(5);
        let mut host = diagnostics(32);
        let mut reporter = host.bind_border_reporting(&source, host_limits()).unwrap();
        let css = format!("#p {{border-top-style:solid;border-top-width:{H}{unit}}}");
        for (medium, new, suppressed) in [(16.0, 1, 0), (32.0, 1, 0), (32.0, 0, 1)] {
            let result = reporter
                .publish(batch(&source, &reporter, &css, 1, 0, Some(medium)))
                .unwrap();
            assert_eq!((result.new_causes, result.suppressed), (new, suppressed));
        }
        let delivered = reports(&host);
        assert_eq!(delivered.len(), 2);
        assert!(delivered.iter().all(|r| r.result == Some(f32::MAX)));
    }
}

#[test]
fn cf34_reverse_observation_publication() {
    let source = captured_siblings(5);
    let mut host = diagnostics(32);
    let mut reporter = host.bind_border_reporting(&source, host_limits()).unwrap();
    let stale = batch(&source, &reporter, &top(H), 1, 0, None);
    let newer = batch(&source, &reporter, &top("1"), 1, 0, None);
    assert_eq!(reporter.publish(newer).unwrap().queued, 0);
    assert_eq!(
        reporter.publish(stale).unwrap_err(),
        BorderReportingError::StaleBatch
    );
    assert!(reports(&host).is_empty());
    assert_eq!(
        reporter
            .publish(batch(&source, &reporter, &top(H), 1, 0, None))
            .unwrap()
            .queued,
        1
    );
}

#[test]
fn cf34_binding_once() {
    let source = captured_siblings(5);
    let mut host = diagnostics(32);
    let reporter = host.bind_border_reporting(&source, host_limits()).unwrap();
    assert!(matches!(
        host.bind_border_reporting(&source, host_limits()),
        Err(BorderReportingError::AlreadyBound)
    ));
    drop(reporter);
    let other = captured_siblings(5);
    assert!(matches!(
        host.bind_border_reporting(&other, host_limits()),
        Err(BorderReportingError::AlreadyBound)
    ));
}

#[test]
fn cf34_closed_during_staging() {
    let source = captured_siblings(5);
    let mut host = diagnostics(32);
    let mut reporter = host.bind_border_reporting(&source, host_limits()).unwrap();
    let captured = batch(&source, &reporter, &top(H), 1, 0, None);
    drop(host);
    assert_eq!(
        reporter.publish(captured).unwrap_err(),
        BorderReportingError::Closed
    );
}

#[test]
fn cf34_original_coordinates() {
    let source = captured_siblings(5);
    let mut host = diagnostics(32);
    let mut reporter = host.bind_border_reporting(&source, host_limits()).unwrap();
    let css = format!("/*\u{e9}*/\r\n#p{{border-top-width:{H}px;border-top-style:solid}}");
    reporter
        .publish(batch(&source, &reporter, &css, 7, 0, None))
        .unwrap();
    let delivered = reports(&host);
    assert_eq!(delivered.len(), 1);
    let Origin::Authored { source, span } = &delivered[0].origin else {
        panic!("authored provenance")
    };
    assert_eq!(source.logical_name, "fixture.css");
    assert_eq!(source.revision.get(), 7);
    assert_eq!(
        (span.bytes.start, span.line.get(), span.column.get()),
        (11, 2, 4)
    );
    assert_eq!(&css[span.bytes.clone()], format!("border-top-width:{H}px;"));
}

#[test]
fn cf34_delayed_original_provenance() {
    let mut host = diagnostics(32);
    let original_instance;
    let css = top(H);
    let start = css.find("border-top-width:").unwrap();
    let end = start + "border-top-width:".len() + H.len() + "px;".len();
    {
        let source = captured_siblings(5);
        original_instance = source
            .engine()
            .instance(source.snapshot().unwrap().token(3).unwrap())
            .unwrap();
        let mut reporter = host.bind_border_reporting(&source, host_limits()).unwrap();
        reporter
            .publish(batch(&source, &reporter, &css, 9, 0, None))
            .unwrap();
    }
    let delivered = reports(&host);
    assert_eq!(delivered.len(), 1);
    assert_eq!(delivered[0].instance, Some(original_instance));
    assert_eq!(delivered[0].result, Some(f32::MAX));
    let Origin::Authored { source, span } = &delivered[0].origin else {
        panic!("authored provenance")
    };
    assert_eq!(
        (source.logical_name.as_str(), source.revision.get()),
        ("fixture.css", 9)
    );
    assert_eq!(span.bytes, start..end);
    assert_eq!((span.line.get(), span.column.get()), (1, start + 1));
}

fn font_shaped() -> Diagnostic {
    Diagnostic::range(
        NonnegativeF32 {
            value: f32::MAX,
            event: Some(RangeEvent::UpperSaturation),
        },
        Origin::Synthetic("font fixture".into()),
        None,
        PropertyId::lookup("font-size").unwrap(),
    )
    .unwrap()
}

#[test]
fn cf34_shared_queue_capacity() {
    let source = captured_siblings(5);
    let mut host = diagnostics(1);
    host.producer().report(font_shaped()).unwrap();
    let mut reporter = host.bind_border_reporting(&source, host_limits()).unwrap();
    let result = reporter
        .publish(batch(&source, &reporter, &top(H), 1, 0, None))
        .unwrap();
    assert_eq!(
        (result.new_causes, result.queued, result.omitted_count),
        (1, 0, 1)
    );
    let deliveries = drain(&host);
    assert_eq!(deliveries.len(), 2);
    assert!(deliveries.iter().any(|d| matches!(d, Delivery::Omissions { summary, .. } if summary.count == 1 && !summary.overflow)));
    assert!(deliveries.iter().any(|d| matches!(d, Delivery::Report(r) if r.diagnostic.property == PropertyId::lookup("font-size"))));
}

#[test]
fn cf34_omitted_suppressed_clear_recur() {
    let source = captured_siblings(5);
    let mut host = diagnostics(1);
    host.producer().report(font_shaped()).unwrap();
    let mut reporter = host.bind_border_reporting(&source, host_limits()).unwrap();
    assert_eq!(
        reporter
            .publish(batch(&source, &reporter, &top(H), 1, 0, None))
            .unwrap()
            .omitted_count,
        1
    );
    let retry = reporter
        .publish(batch(&source, &reporter, &top(H), 1, 0, None))
        .unwrap();
    assert_eq!((retry.suppressed, retry.omitted_count), (1, 0));
    assert_eq!(
        reporter
            .publish(batch(&source, &reporter, &top("1"), 1, 0, None))
            .unwrap()
            .cleared,
        1
    );
    assert_eq!(
        reporter
            .publish(batch(&source, &reporter, &top(H), 1, 0, None))
            .unwrap()
            .omitted_count,
        1
    );
    assert!(
        drain(&host)
            .iter()
            .any(|d| matches!(d, Delivery::Omissions { summary, .. } if summary.count == 2))
    );
}

#[test]
fn cf34_equivalent_numeric_spelling() {
    let source = captured_siblings(5);
    let mut host = diagnostics(32);
    let mut reporter = host.bind_border_reporting(&source, host_limits()).unwrap();
    assert_eq!(
        reporter
            .publish(batch(&source, &reporter, &top(H), 1, 0, None))
            .unwrap()
            .queued,
        1
    );
    assert_eq!(
        reporter
            .publish(batch(
                &source,
                &reporter,
                &top(&format!("{H}.000")),
                1,
                0,
                None
            ))
            .unwrap()
            .suppressed,
        1
    );
    assert_eq!(reports(&host).len(), 1);
}

#[test]
fn cf34_suppressed_and_inherited_silent() {
    let source = captured_siblings(5);
    let mut host = diagnostics(32);
    let mut reporter = host.bind_border_reporting(&source, host_limits()).unwrap();
    let css = format!(
        "#p {{border-top-style:solid;border-top-width:{H}px;border-right-style:solid;border-right-width:{Z}px}} #b1 {{border-style:solid;border-top-width:inherit;border-right-width:inherit}} #b2 {{border-top-style:none;border-top-width:{H}em;border-right-style:hidden;border-right-width:{H}ex}}"
    );
    let result = reporter
        .publish(batch(&source, &reporter, &css, 1, 0, None))
        .unwrap();
    assert_eq!((result.new_causes, result.queued), (2, 2));
    let expected = source
        .engine()
        .instance(source.snapshot().unwrap().token(3).unwrap())
        .unwrap();
    assert!(
        reports(&host)
            .iter()
            .all(|report| report.instance == Some(expected))
    );
}

#[test]
fn cf34_wrong_source_binding() {
    let source = captured_siblings(5);
    let wrong = captured_siblings(5);
    let mut host = diagnostics(32);
    let reporter = host.bind_border_reporting(&source, host_limits()).unwrap();
    with_graph(&wrong, &top(H), |projection, display, graph| {
        let result = evaluate_sibling_box_facts_reporting(
            projection,
            display,
            sibling_fact_limits(),
            &reporter,
            &mapping(graph, 1, 0),
            capture_limits(),
        )
        .unwrap();
        assert!(matches!(
            result.reporting,
            Err(BorderReportingError::WrongSource)
        ));
    });
    assert!(reports(&host).is_empty());
}

#[test]
fn cf34_late_fact_error_wins() {
    let source = captured_siblings(5);
    let mut host = diagnostics(32);
    let reporter = host.bind_border_reporting(&source, host_limits()).unwrap();
    let css = format!("{} #b2:before {{ content:\"x\" }}", top(H));
    with_graph(&source, &css, |projection, display, graph| {
        let expected = evaluate_sibling_box_facts(projection, display, sibling_fact_limits());
        assert!(matches!(
            expected,
            Err(SiblingBoxFactsError::Evaluation(
                ClearanceBoxFactsError::Unsupported {
                    original: 2,
                    target: BoxFactTarget::Before,
                    reason: BoxFactUnsupported::GeneratedContent,
                    ..
                }
            ))
        ));
        for observation_limit in [64, 1] {
            let limits = BorderCaptureLimits {
                observations: nz(observation_limit),
                ..capture_limits()
            };
            let actual = evaluate_sibling_box_facts_reporting(
                projection,
                display,
                sibling_fact_limits(),
                &reporter,
                &mapping(graph, 1, 0),
                limits,
            );
            assert!(matches!(
                actual,
                Err(SiblingBoxFactsError::Evaluation(
                    ClearanceBoxFactsError::Unsupported {
                        original: 2,
                        target: BoxFactTarget::Before,
                        reason: BoxFactUnsupported::GeneratedContent,
                        ..
                    }
                ))
            ));
        }
    });
    assert!(reports(&host).is_empty());
}

#[test]
fn cf34_mapping_error_nonfatal() {
    let source = captured_siblings(5);
    let mut host = diagnostics(32);
    let reporter = host.bind_border_reporting(&source, host_limits()).unwrap();
    with_graph(&source, &top(H), |projection, display, graph| {
        let ordinary =
            evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        let missing = evaluate_sibling_box_facts_reporting(
            projection,
            display,
            sibling_fact_limits(),
            &reporter,
            &[],
            capture_limits(),
        )
        .unwrap();
        assert_eq!(ordinary.receipt(), missing.facts.receipt());
        assert!(matches!(
            missing.reporting,
            Err(BorderReportingError::MissingDeclaration)
        ));
        let mut duplicate = mapping(graph, 1, 0);
        duplicate[1].logical_key = duplicate[0].logical_key;
        let conflicting = evaluate_sibling_box_facts_reporting(
            projection,
            display,
            sibling_fact_limits(),
            &reporter,
            &duplicate,
            capture_limits(),
        )
        .unwrap();
        assert!(matches!(
            conflicting.reporting,
            Err(BorderReportingError::ConflictingMapping)
        ));
        let mut invalid = mapping(graph, 1, 0);
        invalid[0].provenance = Arc::new(Provenance {
            logical_name: String::new(),
            revision: NonZeroU64::MIN,
        });
        let empty_name = evaluate_sibling_box_facts_reporting(
            projection,
            display,
            sibling_fact_limits(),
            &reporter,
            &invalid,
            capture_limits(),
        )
        .unwrap();
        assert!(matches!(
            empty_name.reporting,
            Err(BorderReportingError::InvalidProvenance)
        ));
        let other_graph = super::graph(&top(H), context(100, 100));
        let mut wrong_graph = mapping(graph, 1, 0);
        wrong_graph[0].graph = &other_graph;
        let unmapped_graph = evaluate_sibling_box_facts_reporting(
            projection,
            display,
            sibling_fact_limits(),
            &reporter,
            &wrong_graph,
            capture_limits(),
        )
        .unwrap();
        assert_eq!(ordinary.receipt(), unmapped_graph.facts.receipt());
        assert!(matches!(
            unmapped_graph.reporting,
            Err(BorderReportingError::InvalidProvenance)
        ));
        let mut wrong_resource = mapping(graph, 1, 0);
        wrong_resource[0].resource = ResourceId::new(NonZeroU64::new(999).unwrap());
        let unmapped_resource = evaluate_sibling_box_facts_reporting(
            projection,
            display,
            sibling_fact_limits(),
            &reporter,
            &wrong_resource,
            capture_limits(),
        )
        .unwrap();
        assert_eq!(ordinary.receipt(), unmapped_resource.facts.receipt());
        assert!(matches!(
            unmapped_resource.reporting,
            Err(BorderReportingError::InvalidProvenance)
        ));
    });
    assert!(reports(&host).is_empty());
}

#[test]
fn cf34_capture_budget_boundaries() {
    let source = captured_siblings(5);
    let mut host = diagnostics(32);
    let reporter = host.bind_border_reporting(&source, host_limits()).unwrap();
    with_graph(&source, &top(H), |projection, display, graph| {
        let bindings = mapping(graph, 1, 0);
        let evaluate = |limits| {
            evaluate_sibling_box_facts_reporting(
                projection,
                display,
                sibling_fact_limits(),
                &reporter,
                &bindings,
                limits,
            )
            .unwrap()
        };
        let measured = evaluate(capture_limits()).reporting.unwrap().receipt();
        assert_eq!(
            (
                measured.observations,
                measured.events,
                measured.capture_work
            ),
            (20, 1, 26)
        );
        let limits = BorderCaptureLimits {
            observations: nz(20),
            capture_work: nz(26),
            capture_bytes: nz(measured.capture_bytes),
            ..capture_limits()
        };
        assert!(evaluate(limits).reporting.is_ok());
        for (low, expected) in [
            (
                BorderCaptureLimits {
                    observations: nz(19),
                    ..limits
                },
                BorderLimit::Observations,
            ),
            (
                BorderCaptureLimits {
                    capture_work: nz(25),
                    ..limits
                },
                BorderLimit::CaptureWork,
            ),
            (
                BorderCaptureLimits {
                    capture_bytes: nz(measured.capture_bytes - 1),
                    ..limits
                },
                BorderLimit::CaptureBytes,
            ),
        ] {
            assert!(
                matches!(evaluate(low).reporting, Err(BorderReportingError::Limit(limit)) if limit == expected)
            );
        }
    });
}

#[test]
fn cf34_nonreporting_receipts_unchanged() {
    let source = captured_siblings(5);
    let mut host = diagnostics(32);
    let reporter = host.bind_border_reporting(&source, host_limits()).unwrap();
    with_graph(&source, &top(H), |projection, display, graph| {
        let bindings = mapping(graph, 1, 0);
        let ordinary =
            evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
        let reported = evaluate_sibling_box_facts_reporting(
            projection,
            display,
            sibling_fact_limits(),
            &reporter,
            &bindings,
            capture_limits(),
        )
        .unwrap();
        assert_eq!(ordinary.receipt(), reported.facts.receipt());
        for index in 0..5 {
            assert_eq!(ordinary.row(index), reported.facts.row(index));
        }
        let font_ordinary = evaluate_sibling_box_facts_with_font_medium(
            projection,
            display,
            16.0,
            sibling_fact_limits(),
        )
        .unwrap();
        let font_reported = evaluate_sibling_box_facts_with_font_medium_reporting(
            projection,
            display,
            16.0,
            sibling_fact_limits(),
            &reporter,
            &bindings,
            capture_limits(),
        )
        .unwrap();
        assert_eq!(font_ordinary.receipt(), font_reported.facts.receipt());
        for index in 0..5 {
            assert_eq!(font_ordinary.row(index), font_reported.facts.row(index));
        }
    });
}

#[test]
fn cf34_import_occurrence_keys() {
    use iced_css_core::stylesheet::imports::ImportResolution;
    let source = captured_siblings(5);
    let mut host = diagnostics(32);
    let reporter = host.bind_border_reporting(&source, host_limits()).unwrap();
    let snapshot = source.snapshot().unwrap();
    let projection = prepare_sibling_source_projection(
        &source,
        snapshot,
        SiblingProjectionLimits {
            sources: bound(),
            work: bound(),
            retained_bytes: nz(20_000_000),
        },
    )
    .unwrap();
    let graph = loaded(
        vec![
            resource(
                1,
                "@import 'child'; @import 'child';",
                vec![
                    ImportBinding {
                        entry: 0,
                        resolution: ImportResolution::Sheet(1),
                    },
                    ImportBinding {
                        entry: 1,
                        resolution: ImportResolution::Sheet(1),
                    },
                ],
            ),
            resource(
                2,
                &format!("#p{{border-style:solid;border-width:{H}px}}"),
                vec![],
            ),
        ],
        context(100, 100),
    );
    let ua = catalog();
    let sheets = controls(&[1], true, 100);
    let graph_bindings = [binding(&graph, GraphOrigin::Author(sheet(1)))];
    let bundle = DisplayRuleBundle::new(&ua, &graph_bindings, &sheets, bundle_limits()).unwrap();
    let display = evaluate_source_display(snapshot, &bundle, display_limits()).unwrap();
    let mut bindings = mapping(&graph, 1, 0);
    assert_eq!(bindings.len(), 4);
    assert_ne!(bindings[1].occurrence, bindings[3].occurrence);
    let success = evaluate_sibling_box_facts_reporting(
        &projection,
        &display,
        sibling_fact_limits(),
        &reporter,
        &bindings,
        capture_limits(),
    )
    .unwrap();
    assert_eq!(success.reporting.unwrap().receipt().events, 4);
    let original_occurrence = bindings[3].occurrence;
    bindings[3].occurrence = bindings[1].occurrence;
    let wrong_occurrence = evaluate_sibling_box_facts_reporting(
        &projection,
        &display,
        sibling_fact_limits(),
        &reporter,
        &bindings,
        capture_limits(),
    )
    .unwrap();
    assert!(matches!(
        wrong_occurrence.reporting,
        Err(BorderReportingError::InvalidProvenance)
    ));
    bindings[3].occurrence = original_occurrence;
    bindings[3].logical_key = bindings[1].logical_key;
    let conflict = evaluate_sibling_box_facts_reporting(
        &projection,
        &display,
        sibling_fact_limits(),
        &reporter,
        &bindings,
        capture_limits(),
    )
    .unwrap();
    assert!(matches!(
        conflict.reporting,
        Err(BorderReportingError::ConflictingMapping)
    ));
}
