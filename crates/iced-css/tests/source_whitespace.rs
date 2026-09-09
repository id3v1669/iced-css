// UNCOMPILED integration fixtures. Install with adapter/helpers at reviewed paths.
include!("support/source_whitespace/adapter.rs");
fn leaf(id: usize, text: &str, mode: &str) -> InputNode {
    InputNode::Payload {
        id,
        text: text.to_owned(),
        mode: mode.to_owned(),
        suppressed: false,
        runs: None,
    }
}
macro_rules! with_output {
    ($children:expr,$extra:expr,|$output:ident,$eligibility:ident,$display:ident| $body:block) => {{
        let (construction, mut css) = actual_tree(&$children);
        css.push_str($extra);
        let h = host(construction, None, None);
        let ua = catalog();
        let g = graph(&css, ua.next_context());
        let sheets = controls(&[1], true, 100);
        let bindings = [binding(&g, GraphOrigin::Author(sheet(1)))];
        let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, bundle_limits()).unwrap();
        let $display = evaluate_source_display(h.snapshot().unwrap(), &bundle, limits()).unwrap();
        let $eligibility = evaluate_source_eligibility(&$display, eligibility_limits()).unwrap();
        let participants = SourceParticipants::new(&$eligibility);
        let $output = preprocess_source_whitespace(&participants, whitespace_limits()).unwrap();
        drop(participants);
        $body
    }};
}
#[test]
fn normal007_surviving_source_identity_and_dual_causes() {
    with_output!(
        [
            leaf(0, "X ", "normal"),
            leaf(1, " \n   ", "normal"),
            leaf(2, "X", "normal")
        ],
        "",
        |out, eligibility, display| {
            let rows: Vec<_> = out.scalars().collect();
            assert_eq!(rows.len(), 8);
            assert_eq!(rows[1].source_range(), 1..2);
            assert_eq!(rows[1].rendered_scalar(), Some(' '));
            assert_eq!(rows[1].participant().node().metadata().css_id(), Some("p0"));
            assert_eq!(rows[3].lf_decision(), LfDecision::Space);
            assert_eq!(rows[3].adjacent_action(), SpaceAction::RemovedAdjacentSpace);
            assert_eq!(
                rows[3].rendering(),
                WhitespaceRendering::RemoveAdjacentSpace
            );
            assert_eq!(rows[3].participant().node().metadata().css_id(), Some("p1"));
            assert!(std::ptr::eq(out.eligibility(), &eligibility));
            let p = rows[1].participant();
            assert!(std::ptr::eq(
                p.resolution(),
                display.resolution(p.source_index()).unwrap()
            ));
            assert_eq!(
                rows.iter()
                    .filter_map(|r| r.rendered_scalar())
                    .collect::<String>(),
                "X X"
            );
        }
    );
}
#[test]
fn rich_crlf_crosses_original_runs_without_losing_aggregate_ranges() {
    let children = vec![InputNode::Payload {
        id: 0,
        text: "é\r\n漢".into(),
        mode: "pre-wrap".into(),
        suppressed: false,
        runs: Some(vec![0..0, 0..3, 3..7]),
    }];
    with_output!(children, "", |out, eligibility, display| {
        let r = out.scalar(1).unwrap();
        assert_eq!(r.normalized(), '\n');
        assert_eq!(r.source_range(), 2..4);
        assert_eq!(out.input_scalar_count(), 4);
        assert_eq!(out.scalar_count(), 3);
        assert_eq!(r.lf_decision(), LfDecision::Keep);
        assert_eq!(r.preserved_space(), PreservedSpace::Ordinary);
        let p = r.participant();
        match p.content() {
            iced_css::capture::ContentCapability::RichText { text, runs } => {
                assert_eq!(text, "é\r\n漢");
                assert_eq!(runs, &[0..0, 0..3, 3..7]);
            }
            _ => panic!("rich owner"),
        }
        assert!(std::ptr::eq(
            p.token(),
            eligibility.token(p.source_index()).unwrap()
        ));
        assert!(std::ptr::eq(
            p.resolution(),
            display.resolution(p.source_index()).unwrap()
        ));
    });
}
#[test]
fn exact_work_limits_do_not_depend_on_allocator_capacity() {
    for (text, expected) in [("", 25), ("\r\n", 43), ("𠀀", 46)] {
        with_output!(
            [leaf(0, text, "normal")],
            "",
            |out, eligibility, display| {
                assert_eq!(out.work(), expected);
                let participants = SourceParticipants::new(&eligibility);
                let mut exact = whitespace_limits();
                exact.work = nz(expected);
                assert_eq!(
                    preprocess_source_whitespace(&participants, exact)
                        .unwrap()
                        .work(),
                    expected
                );
                let mut short = whitespace_limits();
                short.work = nz(expected - 1);
                assert!(matches!(
                    preprocess_source_whitespace(&participants, short),
                    Err(SourceWhitespaceError::Work)
                ));
                let mut huge = whitespace_limits();
                huge.retained_bytes = NonZeroUsize::MAX;
                assert_eq!(
                    preprocess_source_whitespace(&participants, huge)
                        .unwrap()
                        .work(),
                    expected
                );
                assert!(display.resolution(0).is_some());
            }
        );
    }
}
#[test]
fn empty_content_and_block_anchors_are_distinct_and_intervals_checked() {
    let children = vec![
        leaf(0, " ", "normal"),
        InputNode::Empty { id: 2 },
        InputNode::Block {
            id: 0,
            suppressed: false,
            children: vec![],
        },
        leaf(1, "", "normal"),
    ];
    with_output!(children, "", |out, eligibility, display| {
        assert_eq!(out.scalar_count(), 1);
        assert_eq!(out.content_count(), 2);
        let contents: Vec<_> = out
            .events()
            .filter(|e| e.kind() == SourceStructuralEventKind::Content)
            .collect();
        assert_eq!(contents[1].scalar_range(), 1..1);
        let iter = {
            let gap = out
                .between_events(contents[0].index(), contents[1].index())
                .unwrap();
            assert_eq!(gap.scalar_range(), 1..1);
            assert!(gap.events().len() >= 4);
            gap.events()
        };
        iterator_contract(&iter);
        assert!(iter.count() >= 4);
        assert!(
            out.between_events(contents[1].index(), contents[0].index())
                .is_none()
        );
        assert!(out.between_events(0, usize::MAX).is_none());
        assert!(std::ptr::eq(out.eligibility(), &eligibility));
        assert!(display.resolution(0).is_some());
    });
}
#[test]
fn all_suppressed_and_eligible_unsupported_sources_are_distinct() {
    with_output!(
        [leaf(0, "\r\n", "normal")],
        "#scope { display:none; }",
        |out, eligibility, display| {
            assert_eq!(
                (
                    out.source_count(),
                    out.content_count(),
                    out.event_count(),
                    out.scalar_count(),
                    out.work(),
                    out.retained_bytes(),
                    out.peak_retained_bytes()
                ),
                (0, 0, 0, 0, 2, 0, 0)
            );
            assert!(out.events().next().is_none());
            assert!(out.scalars().next().is_none());
            assert!(std::ptr::eq(out.eligibility(), &eligibility));
            assert!(display.resolution(0).is_some());
        }
    );
    // Existing literal family fixture: a Slider/Checkbox recipe is not silently removed.
    let control: CapturedElement<'static, (), iced::Theme, ()> =
        adapters::checkbox(false, capture_limits()).unwrap().erase();
    let h = host(row(vec![control]), None, None);
    let ua = catalog();
    let sheets = controls(&[], true, 100);
    let bindings = [];
    let b = DisplayRuleBundle::new(&ua, &bindings, &sheets, bundle_limits()).unwrap();
    let d = evaluate_source_display(h.snapshot().unwrap(), &b, limits()).unwrap();
    let e = evaluate_source_eligibility(&d, eligibility_limits()).unwrap();
    assert!(matches!(
        preprocess_source_whitespace(&SourceParticipants::new(&e), whitespace_limits()),
        Err(SourceWhitespaceError::Structural(_))
    ));
}
#[test]
fn expected_free_adapter_checks_real_blocks_mixed_modes_and_all_views() {
    observe(
        0,
        &[
            leaf(0, " ", "normal"),
            InputNode::Empty { id: 7 },
            leaf(1, "\t", "nowrap"),
            InputNode::Block {
                id: 0,
                suppressed: false,
                children: vec![leaf(2, "漢\n漢", "normal")],
            },
            leaf(3, " ", "pre-wrap"),
        ],
    );
    // Exercise inherited existing helper paths without changing their implementations.
    let _ = wrapped("helper");
    let _ = named("named");
    let _ = text();
}
#[test]
fn input_and_guaranteed_minimum_payload_limits_are_indexed() {
    with_output!(
        [leaf(7, "ab", "normal")],
        "",
        |out, eligibility, display| {
            let source = out.scalar(0).unwrap().participant().source_index();
            let participants = SourceParticipants::new(&eligibility);
            let mut bytes = whitespace_limits();
            bytes.input_bytes = nz(1);
            assert!(
                matches!(preprocess_source_whitespace(&participants,bytes),Err(SourceWhitespaceError::InputBytes{source:s}) if s==source)
            );
            let mut scalars = whitespace_limits();
            scalars.input_scalars = nz(1);
            assert!(
                matches!(preprocess_source_whitespace(&participants,scalars),Err(SourceWhitespaceError::InputScalars{source:s}) if s==source)
            );
            // k=2/e=5: outer + structural reservation + consumer = 14 before decode.
            // The first scalar consumes work 15; the second scalar count must fail
            // before its work charge when both remaining allowances are exhausted.
            let mut scalar_precedence = whitespace_limits();
            scalar_precedence.work = nz(15);
            scalar_precedence.input_scalars = nz(1);
            assert!(
                matches!(preprocess_source_whitespace(&participants,scalar_precedence),Err(SourceWhitespaceError::InputScalars{source:s}) if s==source)
            );
            // Payload byte admission precedes any scalar decode/work charge.
            let mut byte_precedence = whitespace_limits();
            byte_precedence.work = nz(14);
            byte_precedence.input_bytes = nz(1);
            assert!(
                matches!(preprocess_source_whitespace(&participants,byte_precedence),Err(SourceWhitespaceError::InputBytes{source:s}) if s==source)
            );
            // Every nonempty event record has at least a usize source, so one byte is
            // below the requested minimum on supported Rust targets; no prior capacity reused.
            let mut storage = whitespace_limits();
            storage.retained_bytes = nz(1);
            assert!(matches!(
                preprocess_source_whitespace(&participants, storage),
                Err(SourceWhitespaceError::Bytes)
            ));
            let mut work = whitespace_limits();
            work.work = nz(1);
            assert!(matches!(
                preprocess_source_whitespace(&participants, work),
                Err(SourceWhitespaceError::Work)
            ));
            assert!(display.resolution(source).is_some());
        }
    );
}
