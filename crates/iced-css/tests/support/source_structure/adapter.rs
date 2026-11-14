include!("../source_capitalize/helpers.rs");

use iced_css::{
    capture::ContentCapability,
    evaluation::{
        source_eligibility::Eligibility,
        source_participants::{SourceOuterCategory, SourceParticipants},
        source_structure::*,
        source_structural_events::SourceStructuralEventKind,
        source_whitespace::*,
    },
};
use iced_css_core::{
    formatting::display::{Display, Position},
    text::{
        lf_conversion::LfDecision, line_edges::WhiteSpace,
        preserved_spaces::PreservedSpace, space_collapse::SpaceAction,
    },
};
use std::{iter::FusedIterator, ops::Range};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputFamily { Container, Row, Column, Text, RichText }

#[derive(Debug)]
struct InputNode {
    label: String,
    family: InputFamily,
    display: String,
    white_space: String,
    position: String,
    direction: String,
    unicode_bidi: String,
    operation: String,
    profile: String,
    text: String,
    runs: Option<Vec<Range<usize>>>,
    children: Vec<InputNode>,
}

fn whitespace_limits() -> SourceWhitespaceLimits {
    SourceWhitespaceLimits {
        sources: bound(), events: bound(), input_bytes: bound(), input_scalars: bound(),
        retained_bytes: nz(20_000_000), work: nz(20_000_000),
    }
}
fn structure_limits() -> SourceStructureLimits {
    SourceStructureLimits {
        sources: bound(), records: nz(20_000), retained_bytes: nz(20_000_000),
        work: nz(20_000_000),
    }
}

fn build_node(node: &InputNode, css: &mut String) -> CapturedElement<'static, (), iced::Theme, ()> {
    let children: Vec<_> = node.children.iter().map(|child| build_node(child, css)).collect();
    let element = match node.family {
        InputFamily::Column => adapters::column_from_vec(children, capture_limits()).unwrap().erase(),
        InputFamily::Row => adapters::row_from_vec(children, capture_limits()).unwrap().erase(),
        InputFamily::Container => {
            assert_eq!(children.len(), 1, "input conversion admitted only unary Container");
            adapters::container(children.into_iter().next().unwrap(), capture_limits()).unwrap().erase()
        }
        InputFamily::Text => {
            assert!(children.is_empty() && node.runs.is_none());
            adapters::text::<iced::Theme, ()>(node.text.clone(), capture_limits()).unwrap().erase()
        }
        InputFamily::RichText => {
            assert!(children.is_empty());
            let runs = node.runs.as_ref().expect("RichText retains explicit runs");
            let mut end = 0;
            let spans = runs.iter().map(|run| {
                assert_eq!(run.start, end);
                assert!(run.start <= run.end && node.text.is_char_boundary(run.start) && node.text.is_char_boundary(run.end));
                end = run.end;
                iced::widget::text::Span::new(node.text[run.clone()].to_owned())
            }).collect();
            assert_eq!(end, node.text.len());
            adapters::rich_text_vec::<(), (), iced::Theme, ()>(spans, capture_limits()).unwrap().erase()
        }
    };
    css.push_str(&format!(
        "#{} {{ display:{}; white-space:{}; position:{}; direction:{}; unicode-bidi:{}; text-transform:{}; }}\n",
        node.label, node.display, node.white_space, node.position, node.direction,
        node.unicode_bidi,
        match node.operation.as_str() { "none" => "none", "lower" => "lowercase", "upper" => "uppercase", _ => panic!("operation") },
    ));
    let element = element.style_id(&node.label, attrs()).unwrap();
    match node.profile.as_str() {
        "default" => element,
        "tr" => element.style_attribute("lang", "tr", attrs()).unwrap(),
        _ => panic!("model profile is not a CSS property"),
    }
}

fn actual_tree(root: &InputNode) -> (Construction, String) {
    let mut css = String::new();
    let root = build_node(root, &mut css);
    (root.into_parts().1, css)
}

fn family_name(value: &str) -> &'static str {
    match value { "container" => "Container", "row" => "Row", "column" => "Column", "text" => "Text", "rich_text" => "RichText", _ => "Unsupported" }
}
fn disposition_name(value: Eligibility) -> String {
    match value {
        Eligibility::Eligible => "Eligible".into(),
        Eligibility::DisplayNone => "DisplayNone".into(),
        Eligibility::Ancestor { index } => format!("Ancestor:{index}"),
    }
}
fn outer_name(value: SourceOuterCategory) -> &'static str {
    match value {
        SourceOuterCategory::Suppressed => "Suppressed",
        SourceOuterCategory::InlineNonReplaced => "InlineNonReplaced",
        SourceOuterCategory::Block => "Block",
        _ => "Unsupported",
    }
}
fn display_name(value: Display) -> &'static str {
    match value { Display::Inline => "inline", Display::Block => "block", Display::None => "none", _ => "unsupported" }
}
fn position_name(value: Position) -> &'static str {
    match value { Position::Static => "static", Position::Relative => "relative", Position::Absolute => "absolute", Position::Fixed => "fixed" }
}
fn event_name(value: SourceStructuralEventKind) -> &'static str {
    match value { SourceStructuralEventKind::Begin => "Begin", SourceStructuralEventKind::Content => "Content", SourceStructuralEventKind::End => "End" }
}
fn mode_name(value: WhiteSpace) -> &'static str {
    match value { WhiteSpace::Normal => "normal", WhiteSpace::Nowrap => "nowrap", WhiteSpace::Pre => "pre", WhiteSpace::PreWrap => "pre-wrap", WhiteSpace::PreLine => "pre-line" }
}
fn lf_name(value: LfDecision) -> &'static str {
    match value { LfDecision::Keep => "Keep", LfDecision::RemoveWhitespace => "RemoveWhitespace", LfDecision::Space => "Space", LfDecision::RemoveLf => "RemoveLf" }
}
fn preserved_name(value: PreservedSpace) -> &'static str {
    match value {
        PreservedSpace::Ordinary => "Ordinary",
        PreservedSpace::NonBreaking { sequence_end_opportunity: false } => "NonBreakingFalse",
        PreservedSpace::NonBreaking { sequence_end_opportunity: true } => "NonBreakingTrue",
    }
}
fn collapse_name(value: SpaceAction) -> &'static str {
    match value { SpaceAction::Unchanged => "Unchanged", SpaceAction::TabToSpace => "TabToSpace", SpaceAction::RemovedAdjacentSpace => "RemovedAdjacentSpace" }
}
fn structure_kind(value: SourceStructureKind) -> &'static str {
    match value { SourceStructureKind::SourceBlock => "SourceBlock", SourceStructureKind::SourceInline => "SourceInline", SourceStructureKind::AnonymousBlock => "AnonymousBlock", SourceStructureKind::Payload => "Payload" }
}
fn edge_name(value: SourceStructureEdgeKind) -> &'static str {
    match value { SourceStructureEdgeKind::OriginalBegin => "OriginalBegin", SourceStructureEdgeKind::OriginalEnd => "OriginalEnd", SourceStructureEdgeKind::BeforeInterruption => "BeforeInterruption", SourceStructureEdgeKind::AfterInterruption => "AfterInterruption" }
}
fn side_name(value: SourceStructureBoundarySide) -> &'static str {
    match value { SourceStructureBoundarySide::Start => "Start", SourceStructureBoundarySide::End => "End" }
}
fn optional(value: Option<usize>) -> String { value.map_or_else(|| "-".into(), |n| n.to_string()) }
fn encoded(value: &str) -> String {
    let value=value.as_bytes().iter().map(|b|format!("{b:02x}")).collect::<String>();
    if value.is_empty() { "-".into() } else { value }
}
fn runs(value: Option<&[Range<usize>]>) -> String {
    match value {
        None => "-".into(),
        Some([]) => "empty".into(),
        Some(values) => values.iter().map(|r|format!("{}:{}",r.start,r.end)).collect::<Vec<_>>().join(","),
    }
}
fn indices(values: impl Iterator<Item=usize>) -> String {
    let values = values.map(|n| n.to_string()).collect::<Vec<_>>().join(",");
    if values.is_empty() { "-".into() } else { values }
}
fn exact_fused<I: ExactSizeIterator + FusedIterator>(_: &I) {}
fn fused<I: FusedIterator>(_: &I) {}

fn observe(case_id: &str, root: &InputNode) {
    println!("C\t{case_id}");
    let (construction, css) = actual_tree(root);
    let host = host(construction, None, None);
    let snapshot = host.snapshot().unwrap();
    let ua = catalog();
    let graph = graph(&css, ua.next_context());
    let sheets = controls(&[1], true, 100);
    let bindings = [binding(&graph, GraphOrigin::Author(sheet(1)))];
    let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, bundle_limits()).unwrap();
    let display = evaluate_source_display(snapshot, &bundle, limits()).unwrap();
    let eligibility = evaluate_source_eligibility(&display, eligibility_limits()).unwrap();
    let participants = SourceParticipants::new(&eligibility);
    let whitespace = preprocess_source_whitespace(&participants, whitespace_limits()).unwrap();
    assert!(std::ptr::eq(whitespace.eligibility(), &eligibility));

    for source in 0..snapshot.construction().nodes().len() {
        let participant = participants.get(source).unwrap();
        let node = participant.node();
        assert!(std::ptr::eq(participant.token(), eligibility.token(source).unwrap()));
        assert!(std::ptr::eq(participant.resolution(), display.resolution(source).unwrap()));
        match participant.content() {
            ContentCapability::Text(text) => assert_eq!(node.text(),Some(text)),
            ContentCapability::RichText { text, runs } => { assert_eq!(node.text(),Some(text)); assert_eq!(node.text_runs(),Some(runs)); }
            ContentCapability::Managed(_) => assert!(node.text().is_none() && node.text_runs().is_none()),
            _ => panic!("fixture admitted an unexpected content family"),
        }
        println!("O\t{case_id}\t{source}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            node.metadata().css_id().unwrap(), family_name(node.canonical_type()), optional(node.parent()),
            disposition_name(participant.disposition()), optional(eligibility.ordinal(source)), outer_name(participant.outer_category()),
            display_name(participant.resolution().display), position_name(participant.resolution().position),
            participant.text_keywords().white_space().keyword().as_str(), participant.text_keywords().text_transform().keyword().as_str(),
            participant.bidi_style().direction().as_str(), participant.bidi_style().unicode_bidi().as_str(),
            node.metadata().attribute("lang").unwrap_or("-"), node.text().map_or_else(||"-".into(),encoded), runs(node.text_runs()));
    }
    assert!(participants.get(snapshot.construction().nodes().len()).is_none());
    let events = whitespace.events(); exact_fused(&events);
    for event in events {
        let range = event.scalar_range(); let participant = event.participant();
        assert!(std::ptr::eq(participant.token(), eligibility.token(participant.source_index()).unwrap()));
        println!("E\t{case_id}\t{}\t{}\t{}\t{}\t{}\t{}", event.index(), event_name(event.kind()),
            participant.source_index(), participant.node().metadata().css_id().unwrap(), range.start, range.end);
    }
    let scalars = whitespace.scalars(); exact_fused(&scalars);
    for scalar in scalars {
        let participant = scalar.participant(); let range = scalar.source_range();
        let joined = whitespace.scalar(scalar.index()).unwrap();
        let join_ok = std::ptr::eq(participant.token(), joined.participant().token());
        println!("W\t{case_id}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            scalar.index(), participant.source_index(), participant.node().metadata().css_id().unwrap(), range.start, range.end,
            scalar.normalized() as u32, mode_name(scalar.white_space()), lf_name(scalar.lf_decision()),
            preserved_name(scalar.preserved_space()), collapse_name(scalar.adjacent_action()),
            scalar.rendered_scalar().map_or_else(|| "-".into(), |c| (c as u32).to_string()), usize::from(join_ok));
    }

    let output = match build_source_structure(&whitespace, structure_limits()) {
        Ok(output) => output,
        Err(SourceStructureError::Gap { left, right, events, scalars, witness }) => {
            let (kind, source, index) = match witness {
                SourceStructureGapWitness::UnresolvedEmptyInlineGap { source, event } => ("UnresolvedEmptyInlineGap", source, event),
                SourceStructureGapWitness::UnresolvedCollapsibleElementGap { source, event } => ("UnresolvedCollapsibleElementGap", source, event),
                SourceStructureGapWitness::UnresolvedLiteralBidiGap { source, scalar } => ("UnresolvedLiteralBidiGap", source, scalar),
            };
            println!("X\t{case_id}\tGap\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{kind}\t{source}\t{index}\t1",
                left.source, left.event, right.source, right.event, events.start, events.end, scalars.start, scalars.end);
            println!("Z\t{case_id}");
            return;
        }
        Err(error) => panic!("unexpected resource/invariant error for generous fixture: {error:?}"),
    };
    assert!(std::ptr::eq(output.whitespace(), &whitespace));
    let receipt = output.receipt();
    println!("Q\t{case_id}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t1\t1",
        receipt.sources, receipt.events, receipt.scalars, receipt.contents, receipt.blocks, receipt.inline_sources,
        receipt.continuations, receipt.anonymous_blocks, receipt.records, receipt.gap_events, receipt.gap_scalars,
        receipt.work, receipt.fact_capacity, receipt.frame_capacity, receipt.record_capacity,
        receipt.frame_element_bytes, receipt.record_element_bytes, receipt.retained_bytes, receipt.peak_retained_bytes);
    assert_eq!(output.source_count(), receipt.sources); assert_eq!(output.record_count(), receipt.records);
    assert_eq!(output.work(), receipt.work); assert_eq!(output.retained_bytes(), receipt.retained_bytes);
    assert_eq!(output.peak_retained_bytes(), receipt.peak_retained_bytes);
    let records = output.records(); exact_fused(&records); assert_eq!(records.len(), output.record_count());
    assert_eq!(records.size_hint(),(output.record_count(),Some(output.record_count())));
    let second = build_source_structure(&whitespace, structure_limits()).unwrap();
    for (ordinal,record) in records.enumerate() {
        assert_eq!(record.index(),ordinal);
        let direct = output.record(record.index()).unwrap();
        let identity_ok = record.is_same_record(&direct) && !record.is_same_record(&second.record(record.index()).unwrap());
        assert!(std::ptr::eq(record.owner(), &output));
        let parent = record.formatting_parent().map(|r| r.index());
        if let Some(parent_record)=record.formatting_parent() { assert!(parent_record.is_same_record(&output.record(parent_record.index()).unwrap())); }
        let source = record.source_index();
        let participant_ok = match record.participant() {
            Some(p) => source == Some(p.source_index()) && std::ptr::eq(p.token(), eligibility.token(p.source_index()).unwrap()),
            None => source.is_none(),
        };
        let (sr,ss,sk,se,sc,er,es,ek,ee,ec) = match (record.start_edge(), record.end_edge()) {
            (Some(start), Some(end)) => {
                assert_eq!(start.side(), SourceStructureBoundarySide::Start); assert_eq!(end.side(), SourceStructureBoundarySide::End);
                assert!(start.is_same_edge(&direct.start_edge().unwrap()) && end.is_same_edge(&direct.end_edge().unwrap()));
                assert!(!start.is_same_edge(&second.record(record.index()).unwrap().start_edge().unwrap()));
                assert!(start.record().is_same_record(&record)); assert!(end.record().is_same_record(&record));
                assert!(std::ptr::eq(start.event().participant().token(),whitespace.event(start.event().index()).unwrap().participant().token()));
                assert!(std::ptr::eq(end.event().participant().token(),whitespace.event(end.event().index()).unwrap().participant().token()));
                assert!(!end.is_same_edge(&second.record(record.index()).unwrap().end_edge().unwrap()));
                (optional(Some(start.record().index())),side_name(start.side()),edge_name(start.kind()), optional(Some(start.event().index())), optional(Some(start.cut())),
                 optional(Some(end.record().index())),side_name(end.side()),edge_name(end.kind()), optional(Some(end.event().index())), optional(Some(end.cut())))
            }
            (None,None) => ("-".into(),"-","-", "-".into(), "-".into(), "-".into(),"-","-", "-".into(), "-".into()),
            _ => panic!("one-sided public edge"),
        };
        let (content_event, content_source, scalar_start, scalar_end) = match record.content_event() {
            Some(event) => { let range=event.scalar_range(); assert!(std::ptr::eq(event.participant().token(),whitespace.event(event.index()).unwrap().participant().token())); (optional(Some(event.index())), optional(Some(event.participant().source_index())), optional(Some(range.start)), optional(Some(range.end))) },
            None => ("-".into(), "-".into(), "-".into(), "-".into()),
        };
        let (ae,ai,ar,ao_record,ab_start_record,as_side,as_record,ass,ask,ase,ascut,ab_end_record,ae_side,ae_record,aes,aek,aee,aecut) = match record.anonymous_origin() {
            Some(origin) => {
                let start=origin.start_boundary(); let end=origin.end_boundary();
                assert!(origin.record().is_same_record(&record)); assert!(start.record().is_same_record(&record)); assert!(end.record().is_same_record(&record));
                assert!(origin.enclosing_block().is_same_record(&output.record(origin.enclosing_block().index()).unwrap()));
                assert!(start.is_same_boundary(&direct.anonymous_origin().unwrap().start_boundary()));
                assert!(end.is_same_boundary(&direct.anonymous_origin().unwrap().end_boundary()));
                assert!(!start.is_same_boundary(&end) && !start.is_same_boundary(&second.record(record.index()).unwrap().anonymous_origin().unwrap().start_boundary()));
                assert!(!end.is_same_boundary(&second.record(record.index()).unwrap().anonymous_origin().unwrap().end_boundary()));
                let ss=start.source_support(); let es=end.source_support();
                assert_eq!(ss.side(),SourceStructureBoundarySide::Start); assert_eq!(es.side(),SourceStructureBoundarySide::End);
                assert!(ss.record().is_same_record(&output.record(ss.record().index()).unwrap()));
                assert!(es.record().is_same_record(&output.record(es.record().index()).unwrap()));
                assert!(ss.is_same_edge(&output.record(ss.record().index()).unwrap().start_edge().unwrap()));
                assert!(es.is_same_edge(&output.record(es.record().index()).unwrap().end_edge().unwrap()));
                assert!(std::ptr::eq(origin.inheritance_source().token(),eligibility.token(origin.inheritance_source().source_index()).unwrap()));
                (optional(Some(origin.enclosing_block().index())), optional(Some(origin.inheritance_source().source_index())),
                 format!("{:?}", origin.reason()),optional(Some(origin.record().index())),optional(Some(start.record().index())),side_name(start.side()),
                 optional(Some(ss.record().index())),side_name(ss.side()),edge_name(ss.kind()),optional(Some(ss.event().index())),optional(Some(ss.cut())),
                 optional(Some(end.record().index())),side_name(end.side()),optional(Some(es.record().index())),side_name(es.side()),edge_name(es.kind()),optional(Some(es.event().index())),optional(Some(es.cut())))
            }
            None => ("-".into(),"-".into(),"-".into(),"-".into(),"-".into(),"-","-".into(),"-","-","-".into(),"-".into(),"-".into(),"-","-".into(),"-","-","-".into(),"-".into()),
        };
        let mut children=record.children(); fused(&children); assert_eq!(children.size_hint().0,0); assert!(children.size_hint().1.unwrap()<=output.record_count());
        let child_ids=indices(children.by_ref().map(|r|{assert!(r.is_same_record(&output.record(r.index()).unwrap()));r.index()}));
        assert_eq!(children.size_hint(),(0,Some(0))); assert!(children.next().is_none() && children.next().is_none());
        println!("R\t{case_id}\t{}\t{}\t{}\t{}\t{sr}\t{ss}\t{sk}\t{se}\t{sc}\t{er}\t{es}\t{ek}\t{ee}\t{ec}\t{content_event}\t{content_source}\t{scalar_start}\t{scalar_end}\t{ae}\t{ai}\t{ar}\t{ao_record}\t{ab_start_record}\t{as_side}\t{as_record}\t{ass}\t{ask}\t{ase}\t{ascut}\t{ab_end_record}\t{ae_side}\t{ae_record}\t{aes}\t{aek}\t{aee}\t{aecut}\t{child_ids}\t{}\t{}",
            record.index(), structure_kind(record.kind()), optional(parent), optional(source), usize::from(identity_ok), usize::from(participant_ok));
    }
    for source in 0..snapshot.construction().nodes().len() {
        let mut fragments=output.fragments(source).expect("valid original source"); fused(&fragments);
        assert_eq!(fragments.size_hint().0,0); assert!(fragments.size_hint().1.unwrap()<=output.record_count());
        let value=indices(fragments.by_ref().map(|r|{assert!(r.is_same_record(&output.record(r.index()).unwrap()));r.index()}));
        assert_eq!(fragments.size_hint(),(0,Some(0))); assert!(fragments.next().is_none()&&fragments.next().is_none());
        println!("F\t{case_id}\t{source}\tSome\t{value}");
    }
    assert!(output.fragments(snapshot.construction().nodes().len()).is_none());
    assert!(output.record(output.record_count()).is_none() && output.record(usize::MAX).is_none());
    let mut exhausted_records=output.records(); while exhausted_records.next().is_some() {}
    assert_eq!((exhausted_records.len(),exhausted_records.size_hint()),(0,(0,Some(0))));
    assert!(exhausted_records.next().is_none()&&exhausted_records.next().is_none());
    println!("F\t{case_id}\tinvalid\tNone\t-");
    match output.root() {
        None => assert_eq!(output.record_count(),0),
        Some(root) => { assert_eq!(root.index(),0); assert!(root.is_same_record(&output.record(0).unwrap())); }
    }
    println!("Z\t{case_id}");
}
