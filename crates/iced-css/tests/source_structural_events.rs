include!("support/source_structural_events_helpers.rs");
use iced_css::capture::ContentCapability;
use iced_css::evaluation::{source_participants::SourceParticipants, source_structural_events::*};

fn event_limits(sources: usize, events: usize, work: usize) -> SourceStructuralEventLimits {
    SourceStructuralEventLimits {
        sources: nz(sources),
        events: nz(events),
        work: nz(work),
    }
}
fn ample_events() -> SourceStructuralEventLimits {
    event_limits(100_000, 100_000, 100_000)
}
fn error_of<T>(value: Result<T, SourceStructuralEventError>) -> SourceStructuralEventError {
    match value {
        Ok(_) => panic!("expected admission rejection"),
        Err(error) => error,
    }
}

#[test]
fn exact_literal_branch_trace_original_identity_and_every_remaining_count() {
    let host = host(row(vec![wrapped("left"), wrapped("right")]), None, None);
    let ua = catalog();
    let sheets = controls(&[], true, 100);
    let bindings = [];
    let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, bundle_limits()).unwrap();
    let display = evaluate_source_display(host.snapshot().unwrap(), &bundle, limits()).unwrap();
    let eligibility = evaluate_source_eligibility(&display, eligibility_limits()).unwrap();
    let participants = SourceParticipants::new(&eligibility);
    let mut stream = source_structural_events(&participants, event_limits(5, 12, 18)).unwrap();
    assert_eq!(stream.source_count(), 5);
    assert_eq!(stream.content_count(), 2);
    assert_eq!(stream.event_count(), 12);
    assert_eq!(stream.work(), 18);
    assert!(std::ptr::eq(stream.eligibility(), &eligibility));
    drop(participants); // stream must borrow eligibility directly
    use SourceStructuralEventKind::{Begin as B, Content as C, End as E};
    let expected = [
        (B, 4),
        (B, 1),
        (B, 0),
        (C, 0),
        (E, 0),
        (E, 1),
        (B, 3),
        (B, 2),
        (C, 2),
        (E, 2),
        (E, 3),
        (E, 4),
    ];
    for (i, (kind, source)) in expected.into_iter().enumerate() {
        assert_eq!(stream.len(), 12 - i);
        assert_eq!(stream.size_hint(), (12 - i, Some(12 - i)));
        let event = stream.next().unwrap();
        assert_eq!(event.kind(), kind);
        let item = event.participant();
        assert_eq!(item.source_index(), source);
        assert!(std::ptr::eq(
            item.token(),
            eligibility.token(source).unwrap()
        ));
        assert!(std::ptr::eq(
            item.node(),
            &host.snapshot().unwrap().construction().nodes()[source]
        ));
        assert!(std::ptr::eq(
            item.resolution(),
            display.resolution(source).unwrap()
        ));
        assert!(std::ptr::eq(
            item.node().metadata(),
            host.snapshot().unwrap().construction().nodes()[source].metadata()
        ));
        assert_eq!(
            item.node().parent(),
            host.snapshot().unwrap().construction().nodes()[source].parent()
        );
    }
    assert_eq!(stream.len(), 0);
    assert_eq!(stream.size_hint(), (0, Some(0)));
    assert!(stream.next().is_none());
    assert!(stream.next().is_none());
    assert_eq!(stream.work(), 18); // prepaid initial total, not next() call count
}

#[test]
fn every_early_stop_is_a_prefix_with_fixed_prepaid_account_and_surviving_item() {
    let host = host(row(vec![wrapped("a"), wrapped("b")]), None, None);
    let ua = catalog();
    let sheets = controls(&[], true, 100);
    let bindings = [];
    let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, bundle_limits()).unwrap();
    let display = evaluate_source_display(host.snapshot().unwrap(), &bundle, limits()).unwrap();
    let eligibility = evaluate_source_eligibility(&display, eligibility_limits()).unwrap();
    for stop in 0..=12 {
        let participants = SourceParticipants::new(&eligibility);
        let mut stream = source_structural_events(&participants, event_limits(5, 12, 18)).unwrap();
        for _ in 0..stop {
            assert!(stream.next().is_some());
        }
        assert_eq!(stream.len(), 12 - stop);
        assert_eq!(stream.work(), 18);
        assert_eq!(stream.source_count(), 5);
        assert_eq!(stream.content_count(), 2);
        assert_eq!(stream.event_count(), 12);
    }
    let participants = SourceParticipants::new(&eligibility);
    let mut stream = source_structural_events(&participants, event_limits(5, 12, 18)).unwrap();
    let event = stream.next().unwrap();
    drop(stream);
    drop(participants);
    assert_eq!(event.kind(), SourceStructuralEventKind::Begin);
    assert!(std::ptr::eq(
        event.participant().token(),
        eligibility.token(4).unwrap()
    ));
}

#[test]
fn raw_empty_and_rich_content_keep_occurrences_without_whitespace_grouping() {
    let plain: CapturedElement<'static, (), iced::Theme, ()> =
        adapters::text(" \r\n\t", capture_limits()).unwrap().erase();
    let empty: CapturedElement<'static, (), iced::Theme, ()> =
        adapters::text("", capture_limits()).unwrap().erase();
    let spans: Vec<iced::widget::text::Span<'static, (), iced::Font>> = vec![
        iced::widget::text::Span::new(""),
        iced::widget::text::Span::new("é\r\n"),
    ];
    let rich: CapturedElement<'static, (), iced::Theme, ()> =
        adapters::rich_text_vec::<(), (), iced::Theme, ()>(spans, capture_limits())
            .unwrap()
            .erase();
    let empty_rich: CapturedElement<'static, (), iced::Theme, ()> =
        adapters::rich_text_new::<(), (), iced::Theme, ()>(capture_limits())
            .unwrap()
            .erase();
    let host = host(row(vec![plain, empty, rich, empty_rich]), None, None);
    let ua = catalog();
    let sheets = controls(&[], true, 100);
    let bindings = [];
    let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, bundle_limits()).unwrap();
    let display = evaluate_source_display(host.snapshot().unwrap(), &bundle, limits()).unwrap();
    let eligibility = evaluate_source_eligibility(&display, eligibility_limits()).unwrap();
    let participants = SourceParticipants::new(&eligibility);
    let mut stream = source_structural_events(&participants, event_limits(5, 14, 20)).unwrap();
    assert_eq!(stream.content_count(), 4);
    let mut content_sources = Vec::new();
    for event in stream.by_ref() {
        if event.kind() != SourceStructuralEventKind::Content {
            continue;
        }
        let participant = event.participant();
        content_sources.push(participant.source_index());
        match participant.content() {
            ContentCapability::Text(text) => {
                assert_eq!(
                    text,
                    if participant.source_index() == 0 {
                        " \r\n\t"
                    } else {
                        ""
                    }
                );
                assert!(std::ptr::eq(text, participant.node().text().unwrap()));
            }
            ContentCapability::RichText { text, runs } => {
                if participant.source_index() == 2 {
                    assert_eq!(text, "é\r\n");
                    assert_eq!(runs, [0..0, 0..4]);
                } else {
                    assert_eq!(text, "");
                    assert!(runs.is_empty());
                }
                assert!(std::ptr::eq(runs, participant.node().text_runs().unwrap()));
            }
            _ => panic!("Content cannot refer to a managed boundary"),
        }
    }
    assert_eq!(content_sources, [0, 1, 2, 3]);
    assert_eq!(stream.work(), 20);
}

#[test]
fn deep_ascent_closes_one_actual_ancestor_per_event() {
    let depth = 64;
    let mut child = text();
    for _ in 0..depth {
        child = adapters::container(child, capture_limits())
            .unwrap()
            .erase();
    }
    let host = host(child.into_parts().1, None, None);
    let ua = catalog();
    let sheets = controls(&[], true, 100);
    let bindings = [];
    let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, bundle_limits()).unwrap();
    let display = evaluate_source_display(host.snapshot().unwrap(), &bundle, limits()).unwrap();
    let eligibility = evaluate_source_eligibility(&display, eligibility_limits()).unwrap();
    let participants = SourceParticipants::new(&eligibility);
    let mut stream = source_structural_events(
        &participants,
        event_limits(depth + 1, 2 * (depth + 1) + 1, 1 + 3 * (depth + 1) + 1),
    )
    .unwrap();
    for source in (0..=depth).rev() {
        let event = stream.next().unwrap();
        assert_eq!(event.kind(), SourceStructuralEventKind::Begin);
        assert_eq!(event.participant().source_index(), source);
    }
    assert_eq!(
        stream.next().unwrap().kind(),
        SourceStructuralEventKind::Content
    );
    for source in 0..=depth {
        let event = stream.next().unwrap();
        assert_eq!(event.kind(), SourceStructuralEventKind::End);
        assert_eq!(event.participant().source_index(), source);
    }
    assert!(stream.next().is_none());
}

fn assert_domain_error(
    construction: Construction,
    css: &str,
    source: usize,
    reason: SourceStructuralDomainReason,
) {
    let host = host(construction, None, None);
    let ua = catalog();
    let sheets = controls(&[1], true, 100);
    let graph = graph(css, ua.next_context());
    let bindings = [binding(&graph, GraphOrigin::Author(sheet(1)))];
    let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, bundle_limits()).unwrap();
    let display = evaluate_source_display(host.snapshot().unwrap(), &bundle, limits()).unwrap();
    let eligibility = evaluate_source_eligibility(&display, eligibility_limits()).unwrap();
    let participants = SourceParticipants::new(&eligibility);
    assert_eq!(
        error_of(source_structural_events(&participants, ample_events())),
        SourceStructuralEventError::Domain { source, reason }
    );
}

#[test]
fn whole_domain_rejection_respects_flow_family_display_and_original_source_order() {
    use SourceStructuralDomainReason::{Display as D, Family as F, Flow as W};
    // First failing eligible original index, not preorder ordinal or the last source.
    let control = || {
        adapters::checkbox::<(), iced::Theme, ()>(false, capture_limits())
            .unwrap()
            .style_id("bad", attrs())
            .unwrap()
            .erase()
    };
    assert_domain_error(
        row(vec![control(), named("later")]),
        "#bad{display:inline}",
        0,
        F,
    );
    assert_domain_error(row(vec![control()]), "#bad{display:table;float:left}", 0, W);
    assert_domain_error(row(vec![control()]), "#bad{display:table}", 0, F);
    assert_domain_error(row(vec![named("bad")]), "#bad{display:block}", 0, D);
    assert_domain_error(row(vec![named("bad")]), "#bad{position:absolute}", 0, W);
    assert_domain_error(
        row(vec![wrapped("leaf")]),
        "container{display:inline-block}",
        1,
        D,
    );
    // A Text root is computed Block by §9.7 and fails Display before defensive Root.
    assert_domain_error(named("bad").into_parts().1, "#bad{display:inline}", 0, D);
}

#[test]
fn unsupported_suppressed_sources_are_absent_but_supported_raw_siblings_remain() {
    let control = adapters::checkbox::<(), iced::Theme, ()>(false, capture_limits())
        .unwrap()
        .erase();
    let hidden = adapters::container(control, capture_limits())
        .unwrap()
        .style_id("hidden", attrs())
        .unwrap()
        .erase();
    let host = host(row(vec![hidden, named("kept")]), None, None);
    let ua = catalog();
    let sheets = controls(&[1], true, 100);
    let graph = graph("#hidden{display:none}", ua.next_context());
    let bindings = [binding(&graph, GraphOrigin::Author(sheet(1)))];
    let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, bundle_limits()).unwrap();
    let display = evaluate_source_display(host.snapshot().unwrap(), &bundle, limits()).unwrap();
    let eligibility = evaluate_source_eligibility(&display, eligibility_limits()).unwrap();
    assert_eq!(
        eligibility.disposition(0),
        Some(Eligibility::Ancestor { index: 1 })
    );
    let participants = SourceParticipants::new(&eligibility);
    let events = source_structural_events(&participants, event_limits(2, 5, 8)).unwrap();
    use SourceStructuralEventKind::{Begin as B, Content as C, End as E};
    assert_eq!(
        events
            .map(|e| (e.kind(), e.participant().source_index()))
            .collect::<Vec<_>>(),
        [(B, 3), (B, 2), (C, 2), (E, 2), (E, 3)]
    );
}

#[test]
fn fully_suppressed_scope_and_empty_managed_root_have_exact_distinct_counts() {
    for suppress in [false, true] {
        let host = host(row(vec![]), None, None);
        let ua = catalog();
        let sheets = controls(&[1], true, 100);
        // Root computed display resolves inline to Block when not suppressed.
        let graph = graph(
            if suppress {
                "row{display:none}"
            } else {
                "row{display:inline}"
            },
            ua.next_context(),
        );
        let bindings = [binding(&graph, GraphOrigin::Author(sheet(1)))];
        let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, bundle_limits()).unwrap();
        let display = evaluate_source_display(host.snapshot().unwrap(), &bundle, limits()).unwrap();
        let eligibility = evaluate_source_eligibility(&display, eligibility_limits()).unwrap();
        let participants = SourceParticipants::new(&eligibility);
        let mut events = source_structural_events(
            &participants,
            if suppress {
                event_limits(1, 1, 1)
            } else {
                event_limits(1, 2, 4)
            },
        )
        .unwrap();
        assert_eq!(events.source_count(), usize::from(!suppress));
        assert_eq!(events.content_count(), 0);
        assert_eq!(events.event_count(), if suppress { 0 } else { 2 });
        assert_eq!(events.work(), if suppress { 1 } else { 4 });
        if !suppress {
            let begin = events.next().unwrap();
            assert_eq!(begin.kind(), SourceStructuralEventKind::Begin);
            assert_eq!(begin.participant().resolution().display, Display::Block);
            assert_eq!(
                events.next().unwrap().kind(),
                SourceStructuralEventKind::End
            );
        }
        assert!(events.next().is_none());
        assert!(events.next().is_none());
    }
}

#[test]
fn exact_prepaid_boundaries_and_domain_before_event_limit() {
    for invalid in [false, true] {
        let child = if invalid {
            adapters::checkbox::<(), iced::Theme, ()>(false, capture_limits())
                .unwrap()
                .erase()
        } else {
            text()
        };
        let host = host(row(vec![child]), None, None);
        let ua = catalog();
        let sheets = controls(&[], true, 100);
        let bindings = [];
        let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, bundle_limits()).unwrap();
        let display = evaluate_source_display(host.snapshot().unwrap(), &bundle, limits()).unwrap();
        let eligibility = evaluate_source_eligibility(&display, eligibility_limits()).unwrap();
        let participants = SourceParticipants::new(&eligibility);
        assert_eq!(
            error_of(source_structural_events(
                &participants,
                event_limits(1, 1, 1)
            )),
            SourceStructuralEventError::Sources
        );
        assert_eq!(
            error_of(source_structural_events(
                &participants,
                event_limits(2, 1, 2)
            )),
            SourceStructuralEventError::Work
        );
        if invalid {
            assert_eq!(
                error_of(source_structural_events(
                    &participants,
                    event_limits(2, 1, 3)
                )),
                SourceStructuralEventError::Domain {
                    source: 0,
                    reason: SourceStructuralDomainReason::Family
                }
            );
        } else {
            assert_eq!(
                error_of(source_structural_events(
                    &participants,
                    event_limits(2, 4, 3)
                )),
                SourceStructuralEventError::Events
            );
            assert_eq!(
                error_of(source_structural_events(
                    &participants,
                    event_limits(2, 5, 7)
                )),
                SourceStructuralEventError::Work
            );
            let mut exact = source_structural_events(&participants, event_limits(2, 5, 8)).unwrap();
            assert_eq!(exact.work(), 8);
            assert_eq!(exact.len(), 5);
            assert!(exact.next().is_some());
            assert_eq!(exact.work(), 8);
            let huge = source_structural_events(
                &participants,
                event_limits(usize::MAX, usize::MAX, usize::MAX),
            )
            .unwrap();
            assert_eq!(huge.event_count(), 5);
            assert_eq!(huge.work(), 8);
        }
    }
}

#[test]
fn mixed_block_inline_boundaries_are_not_split_or_grouped() {
    let block = adapters::container(named("raw"), capture_limits())
        .unwrap()
        .style_id("block", attrs())
        .unwrap()
        .erase();
    let inline = adapters::container(block, capture_limits())
        .unwrap()
        .style_id("inline", attrs())
        .unwrap()
        .erase();
    let host = host(row(vec![inline]), None, None);
    let ua = catalog();
    let sheets = controls(&[1], true, 100);
    let graph = graph(
        "#inline{display:inline} #block{display:block} #raw{white-space:pre;unicode-bidi:embed;direction:rtl}",
        ua.next_context(),
    );
    let bindings = [binding(&graph, GraphOrigin::Author(sheet(1)))];
    let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, bundle_limits()).unwrap();
    let display = evaluate_source_display(host.snapshot().unwrap(), &bundle, limits()).unwrap();
    let eligibility = evaluate_source_eligibility(&display, eligibility_limits()).unwrap();
    let participants = SourceParticipants::new(&eligibility);
    let mut events = source_structural_events(&participants, event_limits(4, 9, 14)).unwrap();
    use SourceStructuralEventKind::{Begin as B, Content as C, End as E};
    for (kind, source) in [
        (B, 3),
        (B, 2),
        (B, 1),
        (B, 0),
        (C, 0),
        (E, 0),
        (E, 1),
        (E, 2),
        (E, 3),
    ] {
        let event = events.next().unwrap();
        assert_eq!(
            (event.kind(), event.participant().source_index()),
            (kind, source)
        );
        if source == 2 {
            assert_eq!(event.participant().resolution().display, Display::Inline);
        }
        if source == 1 {
            assert_eq!(event.participant().resolution().display, Display::Block);
        }
        assert!(std::ptr::eq(
            event.participant().bidi_style(),
            display.bidi_style(source).unwrap()
        ));
        assert!(std::ptr::eq(
            event.participant().text_keywords(),
            display.text_keywords(source).unwrap()
        ));
    }
    assert!(events.next().is_none());
}

#[test]
fn column_is_the_third_explicitly_admitted_managed_family() {
    let column = adapters::column_from_vec(vec![text()], capture_limits()).unwrap();
    let host = host(column.into_parts().1, None, None);
    let ua = catalog();
    let sheets = controls(&[], true, 100);
    let bindings = [];
    let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, bundle_limits()).unwrap();
    let display = evaluate_source_display(host.snapshot().unwrap(), &bundle, limits()).unwrap();
    let eligibility = evaluate_source_eligibility(&display, eligibility_limits()).unwrap();
    let participants = SourceParticipants::new(&eligibility);
    let events = source_structural_events(&participants, event_limits(2, 5, 8)).unwrap();
    use SourceStructuralEventKind::{Begin as B, Content as C, End as E};
    assert_eq!(
        events
            .map(|e| (e.kind(), e.participant().source_index()))
            .collect::<Vec<_>>(),
        [(B, 1), (B, 0), (C, 0), (E, 0), (E, 1)]
    );
}

#[cfg(any(feature = "image-without-codecs", feature = "svg"))]
#[test]
fn resource_families_reject_when_eligible_and_vanish_only_when_suppressed() {
    let mut resources = Vec::new();
    #[cfg(feature = "image-without-codecs")]
    resources.push(
        adapters::image(
            iced::widget::image::Handle::from_rgba(1, 1, vec![0u8; 4]),
            capture_limits(),
        )
        .unwrap()
        .into_parts()
        .1,
    );
    #[cfg(feature = "svg")]
    resources.push(
        adapters::svg::<iced::Theme>(
            iced::widget::svg::Handle::from_memory(b"<svg/>".to_vec()),
            capture_limits(),
        )
        .unwrap()
        .into_parts()
        .1,
    );
    for construction in resources {
        let host = host(construction, None, None);
        let ua = catalog();
        for suppressed in [false, true] {
            let sheets = controls(&[1], true, 100);
            let graph = graph(
                if suppressed {
                    "*{display:none}"
                } else {
                    "*{display:block}"
                },
                ua.next_context(),
            );
            let bindings = [binding(&graph, GraphOrigin::Author(sheet(1)))];
            let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, bundle_limits()).unwrap();
            let display =
                evaluate_source_display(host.snapshot().unwrap(), &bundle, limits()).unwrap();
            let eligibility = evaluate_source_eligibility(&display, eligibility_limits()).unwrap();
            let participants = SourceParticipants::new(&eligibility);
            if suppressed {
                let mut stream =
                    source_structural_events(&participants, event_limits(1, 1, 1)).unwrap();
                assert_eq!(stream.source_count(), 0);
                assert_eq!(stream.work(), 1);
                assert!(stream.next().is_none());
            } else {
                assert_eq!(
                    error_of(source_structural_events(&participants, ample_events())),
                    SourceStructuralEventError::Domain {
                        source: 0,
                        reason: SourceStructuralDomainReason::Family
                    }
                );
            }
        }
    }
}
