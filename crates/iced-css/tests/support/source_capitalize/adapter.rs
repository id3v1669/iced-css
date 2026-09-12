// Uncompiled actual-owner input adapter. No expected scalar results embedded.
include!("helpers.rs");
use iced_css::evaluation::{
    source_participants::SourceParticipants, source_structural_events::SourceStructuralEventKind,
    source_whitespace::*,
};
use iced_css_core::text::{
    lf_conversion::LfDecision, line_edges::WhiteSpace, preserved_spaces::PreservedSpace,
    space_collapse::SpaceAction, whitespace_preprocessing::WhitespaceRendering,
};

#[derive(Debug)]
enum InputNode {
    Payload {
        id: usize,
        text: String,
        mode: String,
        suppressed: bool,
        runs: Option<Vec<std::ops::Range<usize>>>,
        operation: String,
        language: Option<String>,
    },
    Empty {
        id: usize,
    },
    Block {
        id: usize,
        suppressed: bool,
        children: Vec<InputNode>,
    },
}
fn whitespace_limits() -> SourceWhitespaceLimits {
    SourceWhitespaceLimits {
        sources: bound(),
        events: bound(),
        input_bytes: bound(),
        input_scalars: bound(),
        retained_bytes: nz(20_000_000),
        work: nz(20_000_000),
    }
}
fn build_node(node: &InputNode, css: &mut String) -> CapturedElement<'static, (), iced::Theme, ()> {
    let (label, display, mode, suppressed, element) = match node {
        InputNode::Payload {
            id,
            text,
            mode,
            suppressed,
            runs,
            operation,
            language,
        } => {
            let element = if let Some(runs) = runs {
                let mut end = 0;
                for r in runs {
                    assert_eq!(r.start, end);
                    assert!(text.is_char_boundary(r.start) && text.is_char_boundary(r.end));
                    assert!(r.start <= r.end);
                    end = r.end;
                }
                assert_eq!(end, text.len());
                let spans: Vec<iced::widget::text::Span<'static, (), iced::Font>> = runs
                    .iter()
                    .map(|r| iced::widget::text::Span::new(text[r.clone()].to_owned()))
                    .collect();
                adapters::rich_text_vec::<(), (), iced::Theme, ()>(spans, capture_limits())
                    .unwrap()
                    .erase()
            } else {
                adapters::text::<iced::Theme, ()>(text.clone(), capture_limits())
                    .unwrap()
                    .erase()
            };
            css.push_str(&format!(
                "#p{id} {{ text-transform: {}; }}\n",
                match operation.as_str() {
                    "lower" => "lowercase",
                    "upper" => "uppercase",
                    "none" => "none",
                    other => other,
                }
            ));
            let element = if let Some(lang) = language {
                element.style_attribute("lang", lang, attrs()).unwrap()
            } else {
                element
            };
            (
                format!("p{id}"),
                "inline",
                mode.as_str(),
                *suppressed,
                element,
            )
        }
        InputNode::Empty { id } => (
            format!("i{id}"),
            "inline",
            "normal",
            false,
            adapters::row_from_vec::<(), iced::Theme, ()>(vec![], capture_limits())
                .unwrap()
                .erase(),
        ),
        InputNode::Block {
            id,
            suppressed,
            children,
        } => {
            let children = children.iter().map(|c| build_node(c, css)).collect();
            (
                format!("b{id}"),
                "block",
                "normal",
                *suppressed,
                adapters::row_from_vec(children, capture_limits())
                    .unwrap()
                    .erase(),
            )
        }
    };
    css.push_str(&format!(
        "#{label} {{ display: {}; white-space: {mode}; }}\n",
        if suppressed { "none" } else { display }
    ));
    element.style_id(&label, attrs()).unwrap()
}
fn actual_tree(children: &[InputNode]) -> (Construction, String) {
    actual_tree_with_language(children, None)
}
fn actual_tree_with_language(
    children: &[InputNode],
    language: Option<&str>,
) -> (Construction, String) {
    let mut css = String::from("#scope { display: block; }\n");
    let children = children.iter().map(|c| build_node(c, &mut css)).collect();
    let root = adapters::column_from_vec(children, capture_limits())
        .unwrap()
        .erase()
        .style_id("scope", attrs())
        .unwrap();
    let root = if let Some(lang) = language {
        root.style_attribute("lang", lang, attrs()).unwrap()
    } else {
        root
    };
    (root.into_parts().1, css)
}
fn verify_payloads(
    nodes: &[InputNode],
    snapshot: &iced_css::host::source_publication::InitialSourceSnapshot,
) {
    for input in nodes {
        match input {
            InputNode::Payload {
                id,
                text,
                runs,
                language,
                ..
            } => {
                let label = format!("p{id}");
                let node = snapshot
                    .construction()
                    .nodes()
                    .iter()
                    .find(|n| n.metadata().css_id() == Some(label.as_str()))
                    .unwrap();
                assert_eq!(node.metadata().attribute("lang"), language.as_deref());
                match (runs, node.content_capability()) {
                    (None, iced_css::capture::ContentCapability::Text(actual)) => {
                        assert_eq!(actual, text.as_str())
                    }
                    (
                        Some(expected),
                        iced_css::capture::ContentCapability::RichText {
                            text: actual,
                            runs: actual_runs,
                        },
                    ) => {
                        assert_eq!(actual, text.as_str());
                        assert_eq!(actual_runs, expected.as_slice());
                    }
                    _ => panic!("captured payload family/runs differ"),
                }
            }
            InputNode::Block { children, .. } => verify_payloads(children, snapshot),
            InputNode::Empty { .. } => {}
        }
    }
}
fn lf_code(value: LfDecision) -> u8 {
    match value {
        LfDecision::Keep => 0,
        LfDecision::RemoveWhitespace => 1,
        LfDecision::Space => 2,
        LfDecision::RemoveLf => 3,
    }
}
fn preserved_code(value: PreservedSpace) -> u8 {
    match value {
        PreservedSpace::Ordinary => 0,
        PreservedSpace::NonBreaking {
            sequence_end_opportunity: false,
        } => 1,
        PreservedSpace::NonBreaking {
            sequence_end_opportunity: true,
        } => 2,
    }
}
fn adjacent_code(value: SpaceAction) -> u8 {
    match value {
        SpaceAction::Unchanged => 0,
        SpaceAction::TabToSpace => 1,
        SpaceAction::RemovedAdjacentSpace => 2,
    }
}
fn rendering_code(value: WhitespaceRendering) -> u8 {
    match value {
        WhitespaceRendering::Keep => 0,
        WhitespaceRendering::Space => 1,
        WhitespaceRendering::RemoveWhitespace => 2,
        WhitespaceRendering::RemoveLf => 3,
        WhitespaceRendering::RemoveAdjacentSpace => 4,
    }
}
fn mode_name(value: WhiteSpace) -> &'static str {
    match value {
        WhiteSpace::Normal => "normal",
        WhiteSpace::Nowrap => "nowrap",
        WhiteSpace::Pre => "pre",
        WhiteSpace::PreWrap => "pre-wrap",
        WhiteSpace::PreLine => "pre-line",
    }
}
fn kind_name(value: SourceStructuralEventKind) -> &'static str {
    match value {
        SourceStructuralEventKind::Begin => "B",
        SourceStructuralEventKind::Content => "C",
        SourceStructuralEventKind::End => "E",
    }
}
fn iterator_contract<I: ExactSizeIterator + std::iter::FusedIterator>(_: &I) {}
fn observe(case: usize, children: &[InputNode]) {
    let (construction, css) = actual_tree(children);
    let host = host(construction, None, None);
    let snapshot = host.snapshot().unwrap();
    verify_payloads(children, snapshot);
    let ua = catalog();
    let graph = graph(&css, ua.next_context());
    let sheets = controls(&[1], true, 100);
    let bindings = [binding(&graph, GraphOrigin::Author(sheet(1)))];
    let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, bundle_limits()).unwrap();
    let display = evaluate_source_display(snapshot, &bundle, limits()).unwrap();
    let eligibility = evaluate_source_eligibility(&display, eligibility_limits()).unwrap();
    let participants = SourceParticipants::new(&eligibility);
    let output = preprocess_source_whitespace(&participants, whitespace_limits()).unwrap();
    drop(participants);
    assert!(std::ptr::eq(output.eligibility(), &eligibility));
    assert!(output.peak_retained_bytes() >= output.retained_bytes());
    println!(
        "H\t{case}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        output.source_count(),
        output.content_count(),
        output.event_count(),
        output.input_bytes(),
        output.input_scalar_count(),
        output.scalar_count(),
        output.work()
    );
    for (index, node) in snapshot.construction().nodes().iter().enumerate() {
        println!("M\t{case}\t{}\t{index}", node.metadata().css_id().unwrap());
    }
    let events = output.events();
    iterator_contract(&events);
    for e in events {
        let p = e.participant();
        let range = e.scalar_range();
        assert!(std::ptr::eq(
            p.token(),
            eligibility.token(p.source_index()).unwrap()
        ));
        assert!(std::ptr::eq(
            p.node(),
            &snapshot.construction().nodes()[p.source_index()]
        ));
        assert_eq!(output.event(e.index()).unwrap().scalar_range(), range);
        println!(
            "E\t{case}\t{}\t{}\t{}\t{}\t{}\t{}",
            e.index(),
            kind_name(e.kind()),
            p.node().metadata().css_id().unwrap(),
            p.source_index(),
            range.start,
            range.end
        );
    }
    let scalars = output.scalars();
    iterator_contract(&scalars);
    for s in scalars {
        let p = s.participant();
        let r = s.source_range();
        assert!(std::ptr::eq(
            p.token(),
            eligibility.token(p.source_index()).unwrap()
        ));
        assert!(std::ptr::eq(
            p.resolution(),
            display.resolution(p.source_index()).unwrap()
        ));
        assert_eq!(output.scalar(s.index()).unwrap().source_range(), r);
        println!(
            "S\t{case}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            s.index(),
            p.node().metadata().css_id().unwrap(),
            p.source_index(),
            r.start,
            r.end,
            s.normalized() as u32,
            mode_name(s.white_space()),
            lf_code(s.lf_decision()),
            preserved_code(s.preserved_space()),
            adjacent_code(s.adjacent_action()),
            rendering_code(s.rendering()),
            s.rendered_scalar().map_or(-1, |c| c as i64)
        );
    }
    observe_casing(case, &output);
    assert!(output.event(output.event_count()).is_none());
    assert!(output.scalar(output.scalar_count()).is_none());
    assert!(output.event(usize::MAX).is_none());
    assert!(output.scalar(usize::MAX).is_none());
    assert!(output.between_events(0, 0).is_none());
    assert!(output.between_events(usize::MAX, 0).is_none());
    for left in 0..output.event_count() {
        for right in left + 1..output.event_count() {
            let a = output.event(left).unwrap();
            let b = output.event(right).unwrap();
            let interval = output.between_events(left, right).unwrap();
            assert_eq!(
                interval.scalar_range(),
                a.scalar_range().end..b.scalar_range().start
            );
            assert_eq!(interval.event_range(), left + 1..right);
            let ev = interval.events();
            let sc = interval.scalars();
            iterator_contract(&ev);
            iterator_contract(&sc);
            assert_eq!(
                ev.map(|e| e.index()).collect::<Vec<_>>(),
                (left + 1..right).collect::<Vec<_>>()
            );
            assert_eq!(
                sc.map(|s| s.index()).collect::<Vec<_>>(),
                interval.scalar_range().collect::<Vec<_>>()
            );
        }
    }
}

include!("casing_observer.rs");
