// Expected-free actual-owner adapter for the approved SourceAnalysisInput API.
include!("helpers.rs");

use iced_css::{
    capture::ContentCapability,
    evaluation::{
        source_analysis_input::*,
        source_casing::*,
        source_eligibility::Eligibility,
        source_participants::{SourceOuterCategory, SourceParticipants},
        source_structure::*,
        source_whitespace::*,
    },
};
use iced_css_core::{
    formatting::display::{Display, Position},
    text::{bidi_initial::BidiParagraphMode, line_edges::WhiteSpace},
};
use std::{iter::FusedIterator, ops::Range};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputFamily {
    Container,
    Row,
    Column,
    Text,
    RichText,
}

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

fn build_node(node: &InputNode, css: &mut String) -> CapturedElement<'static, (), iced::Theme, ()> {
    let children: Vec<_> = node
        .children
        .iter()
        .map(|child| build_node(child, css))
        .collect();
    let element = match node.family {
        InputFamily::Column => adapters::column_from_vec(children, capture_limits())
            .unwrap()
            .erase(),
        InputFamily::Row => adapters::row_from_vec(children, capture_limits())
            .unwrap()
            .erase(),
        InputFamily::Container => {
            assert_eq!(
                children.len(),
                1,
                "input conversion admitted only unary Container"
            );
            adapters::container(children.into_iter().next().unwrap(), capture_limits())
                .unwrap()
                .erase()
        }
        InputFamily::Text => {
            assert!(children.is_empty() && node.runs.is_none());
            adapters::text::<iced::Theme, ()>(node.text.clone(), capture_limits())
                .unwrap()
                .erase()
        }
        InputFamily::RichText => {
            assert!(children.is_empty());
            let runs = node.runs.as_ref().expect("RichText retains explicit runs");
            let mut end = 0;
            let spans = runs
                .iter()
                .map(|run| {
                    assert_eq!(run.start, end);
                    assert!(
                        run.start <= run.end
                            && node.text.is_char_boundary(run.start)
                            && node.text.is_char_boundary(run.end)
                    );
                    end = run.end;
                    iced::widget::text::Span::new(node.text[run.clone()].to_owned())
                })
                .collect();
            assert_eq!(end, node.text.len());
            adapters::rich_text_vec::<(), (), iced::Theme, ()>(spans, capture_limits())
                .unwrap()
                .erase()
        }
    };
    css.push_str(&format!(
        "#{} {{ display:{}; white-space:{}; position:{}; direction:{}; unicode-bidi:{}; text-transform:{}; }}\n",
        node.label, node.display, node.white_space, node.position, node.direction,
        node.unicode_bidi,
        match node.operation.as_str() { "none" => "none", "lower" => "lowercase", "upper" => "uppercase", "capitalize" => "capitalize", _ => panic!("operation") },
    ));
    let element = element.style_id(&node.label, attrs()).unwrap();
    match node.profile.as_str() {
        "default" => element,
        "lt" | "tr" | "az" => element
            .style_attribute("lang", node.profile.as_str(), attrs())
            .unwrap(),
        _ => panic!("model profile is not a CSS property"),
    }
}

fn actual_tree(root: &InputNode) -> (Construction, String) {
    let mut css = String::new();
    let root = build_node(root, &mut css);
    (root.into_parts().1, css)
}

fn hex(s: &str) -> String {
    let v = s
        .as_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    if v.is_empty() { "-".into() } else { v }
}
fn family_name(value: &str) -> &'static str {
    match value {
        "container" => "Container",
        "row" => "Row",
        "column" => "Column",
        "text" => "Text",
        "rich_text" => "RichText",
        _ => panic!("unsupported captured family"),
    }
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
        _ => panic!("unsupported fixture outer category"),
    }
}
fn display_name(value: Display) -> &'static str {
    match value {
        Display::Inline => "inline",
        Display::Block => "block",
        Display::None => "none",
        _ => panic!("unsupported fixture display"),
    }
}
fn position_name(value: Position) -> &'static str {
    match value {
        Position::Static => "static",
        Position::Relative => "relative",
        Position::Absolute => "absolute",
        Position::Fixed => "fixed",
    }
}
fn opt(v: Option<usize>) -> String {
    v.map_or_else(|| "-".into(), |x| x.to_string())
}
fn range(r: Range<usize>) -> String {
    format!("{}:{}", r.start, r.end)
}
fn whitespace_mode(v: WhiteSpace) -> &'static str {
    match v {
        WhiteSpace::Normal => "normal",
        WhiteSpace::Nowrap => "nowrap",
        WhiteSpace::Pre => "pre",
        WhiteSpace::PreWrap => "pre-wrap",
        WhiteSpace::PreLine => "pre-line",
    }
}
fn kind(v: SourceStructureKind) -> &'static str {
    match v {
        SourceStructureKind::SourceBlock => "SourceBlock",
        SourceStructureKind::SourceInline => "SourceInline",
        SourceStructureKind::AnonymousBlock => "AnonymousBlock",
        SourceStructureKind::Payload => "Payload",
    }
}
fn edge_kind(v: SourceStructureEdgeKind) -> &'static str {
    match v {
        SourceStructureEdgeKind::OriginalBegin => "OriginalBegin",
        SourceStructureEdgeKind::OriginalEnd => "OriginalEnd",
        SourceStructureEdgeKind::BeforeInterruption => "BeforeInterruption",
        SourceStructureEdgeKind::AfterInterruption => "AfterInterruption",
    }
}
fn mapping_name(v: iced_css_core::text::casing_data::CaseMapping) -> &'static str {
    match v {
        iced_css_core::text::casing_data::CaseMapping::Identity => "Identity",
        iced_css_core::text::casing_data::CaseMapping::Replacement(_) => "Replacement",
    }
}
fn mapping_characters(s: &SourceCasingScalar<'_, '_, '_, '_, '_, '_, '_>) -> String {
    let values: Vec<char> = match s.mapping() {
        iced_css_core::text::casing_data::CaseMapping::Identity => {
            vec![s.whitespace_scalar().normalized()]
        }
        iced_css_core::text::casing_data::CaseMapping::Replacement(cs) => cs.to_vec(),
    };
    if values.is_empty() {
        "-".into()
    } else {
        values
            .iter()
            .map(|c| format!("{:x}", *c as u32))
            .collect::<Vec<_>>()
            .join(",")
    }
}
fn casing_operation(v: Option<SourceCasingOperation>) -> &'static str {
    match v {
        None | Some(SourceCasingOperation::None) => "none",
        Some(SourceCasingOperation::Lower) => "lower",
        Some(SourceCasingOperation::Upper) => "upper",
        Some(SourceCasingOperation::Capitalize) => "capitalize",
    }
}
fn casing_profile(v: SourceCasingProfile) -> &'static str {
    match v {
        SourceCasingProfile::Default => "default",
        SourceCasingProfile::Lithuanian => "lt",
        SourceCasingProfile::Turkish => "tr",
        SourceCasingProfile::Azeri => "az",
    }
}
fn indices(values: impl Iterator<Item = usize>) -> String {
    let value = values.map(|n| n.to_string()).collect::<Vec<_>>().join(",");
    if value.is_empty() { "-".into() } else { value }
}
fn participation(v: SourceAnalysisParticipation) -> &'static str {
    match v {
        SourceAnalysisParticipation::Empty => "Empty",
        SourceAnalysisParticipation::BlockChildren => "BlockChildren",
        SourceAnalysisParticipation::InlineSequence => "InlineSequence",
    }
}
fn mode(v: BidiParagraphMode) -> &'static str {
    match v {
        BidiParagraphMode::LeftToRight => "LeftToRight",
        BidiParagraphMode::RightToLeft => "RightToLeft",
        BidiParagraphMode::AutoLeftToRight => panic!("mapped scopes never expose auto mode"),
    }
}
fn presentation(v: SourceCasingPresentation) -> (&'static str, String) {
    match v {
        SourceCasingPresentation::SuppressedByWhitespace => ("SuppressedByWhitespace", "-".into()),
        SourceCasingPresentation::Scalar(c) => ("Scalar", format!("{:x}", c as u32)),
        SourceCasingPresentation::Replacement(cs) => (
            "Replacement",
            if cs.is_empty() {
                "-".into()
            } else {
                cs.iter()
                    .map(|c| format!("{:x}", *c as u32))
                    .collect::<Vec<_>>()
                    .join(",")
            },
        ),
    }
}
fn exact_fused<I: ExactSizeIterator + FusedIterator>(_: &I) {}

fn print_error(case: &str, error: &SourceAnalysisInputError) {
    match error {
        SourceAnalysisInputError::OwnerMismatch => println!("X\t{case}\tOwnerMismatch"),
        SourceAnalysisInputError::Sources => println!("X\t{case}\tSources"),
        SourceAnalysisInputError::Scopes => println!("X\t{case}\tScopes"),
        SourceAnalysisInputError::OutputScalars => println!("X\t{case}\tOutputScalars"),
        SourceAnalysisInputError::OutputBytes => println!("X\t{case}\tOutputBytes"),
        SourceAnalysisInputError::Work => println!("X\t{case}\tWork"),
        SourceAnalysisInputError::Bytes => println!("X\t{case}\tBytes"),
        SourceAnalysisInputError::Allocation => println!("X\t{case}\tAllocation"),
        SourceAnalysisInputError::Nonnormal { source, value } => {
            println!("X\t{case}\tNonnormal\t{source}\t{}", value.as_str())
        }
        SourceAnalysisInputError::Invariant { record, reason } => {
            println!("X\t{case}\tInvariant\t{}\t{reason:?}", opt(*record))
        }
    }
}

/// Constructs all real predecessor owners from the recipe and observes only public actual values.
/// No expected result, range, tree, text, presentation, or error tag enters this function.
fn observe_with_raw<F>(case: &str, root: &InputNode, mut raw: F)
where
    F: FnMut(
        &str,
        usize,
        &str,
        BidiParagraphMode,
        &mut dyn FnMut(usize) -> (usize, usize, char, usize),
    ),
{
    println!("B\t{case}");
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
    let casing = evaluate_source_casing(&whitespace, casing_limits()).unwrap();

    // Original captured-source authentication includes suppressed content bytes/runs and computed bidi style.
    for source in 0..snapshot.construction().nodes().len() {
        let p = participants.get(source).unwrap();
        let n = p.node();
        assert!(std::ptr::eq(p.token(), eligibility.token(source).unwrap()));
        assert!(std::ptr::eq(
            p.resolution(),
            display.resolution(source).unwrap()
        ));
        let (text, runs) = match p.content() {
            ContentCapability::Text(t) => (hex(t), "-".into()),
            ContentCapability::RichText { text, runs } => (
                hex(text),
                if runs.is_empty() {
                    "empty".into()
                } else {
                    runs.iter()
                        .map(|r| range(r.clone()))
                        .collect::<Vec<_>>()
                        .join(",")
                },
            ),
            ContentCapability::Managed(_) => ("-".into(), "-".into()),
            _ => panic!("unsupported fixture content"),
        };
        println!(
            "O\t{case}\t{source}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            n.metadata().css_id().unwrap(),
            family_name(n.canonical_type()),
            opt(n.parent()),
            disposition_name(p.disposition()),
            opt(eligibility.ordinal(source)),
            outer_name(p.outer_category()),
            display_name(p.resolution().display),
            position_name(p.resolution().position),
            p.text_keywords().white_space().keyword().as_str(),
            p.text_keywords().text_transform().keyword().as_str(),
            p.bidi_style().direction().as_str(),
            p.bidi_style().unicode_bidi().as_str(),
            n.metadata().attribute("lang").unwrap_or("-"),
            text,
            runs
        );
    }
    for e in whitespace.events() {
        let p = e.participant();
        assert!(std::ptr::eq(
            p.token(),
            eligibility.token(p.source_index()).unwrap()
        ));
        println!(
            "E\t{case}\t{}\t{:?}\t{}\t{}",
            e.index(),
            e.kind(),
            p.source_index(),
            range(e.scalar_range())
        );
    }
    for s in whitespace.scalars() {
        let p = s.participant();
        println!(
            "W\t{case}\t{}\t{}\t{}\t{}\t{}\t{}\t{:?}\t{:?}\t{:?}\t{}",
            s.index(),
            p.source_index(),
            range(s.source_range()),
            s.normalized() as u32,
            whitespace_mode(s.white_space()),
            opt(s.rendered_scalar().map(|c| c as usize)),
            s.lf_decision(),
            s.preserved_space(),
            s.adjacent_action(),
            usize::from(std::ptr::eq(
                p.token(),
                eligibility.token(p.source_index()).unwrap()
            ))
        );
    }
    println!(
        "K\t{case}\tH\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        casing.source_count(),
        casing.scalar_count(),
        casing.capitalize_scalar_count(),
        casing.capitalize_context_count(),
        casing.capitalize_unit_count(),
        casing.active_context_count(),
        casing.context_bytes(),
        casing.selection_pass_count()
    );
    for source in casing.sources() {
        let index = source.source_index();
        let direct = casing.source(index).unwrap();
        let p = source.participant();
        assert_eq!(source.ordinal(), eligibility.ordinal(index).unwrap());
        assert_eq!(
            (source.operation(), source.profile()),
            (direct.operation(), direct.profile())
        );
        assert!(std::ptr::eq(p.token(), eligibility.token(index).unwrap()));
        assert!(std::ptr::eq(
            p.resolution(),
            display.resolution(index).unwrap()
        ));
        println!(
            "K\t{case}\tS\t{}\t{}\t{}\t{}\t{}",
            index,
            source.ordinal(),
            source
                .operation()
                .map(|op| casing_operation(Some(op)))
                .unwrap_or("absent"),
            casing_profile(source.profile()),
            usize::from(std::ptr::eq(p.token(), direct.participant().token()))
        );
    }
    let units = casing.capitalize_units();
    exact_fused(&units);
    assert_eq!(
        units.size_hint(),
        (
            casing.capitalize_unit_count(),
            Some(casing.capitalize_unit_count())
        )
    );
    for unit in units {
        let direct = casing.capitalize_unit(unit.index()).unwrap();
        assert_eq!(
            (unit.scalar_range(), unit.word_range()),
            (direct.scalar_range(), direct.word_range())
        );
        let targets = indices(
            unit.scalars()
                .filter(|s| s.is_capitalize_target())
                .map(|s| s.index()),
        );
        println!(
            "K\t{case}\tU\t{}\t{}\t{}\t{}",
            unit.index(),
            range(unit.scalar_range()),
            range(unit.word_range()),
            targets
        );
    }
    for s in casing.scalars() {
        let ws = s.whitespace_scalar();
        let source = s.source();
        let (tag, chars) = presentation(s.presentation());
        let unit = s.capitalize_unit().map(|u| u.index());
        println!(
            "A\t{case}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            s.index(),
            source.source_index(),
            ws.index(),
            range(ws.source_range()),
            mapping_name(s.mapping()),
            mapping_characters(&s),
            tag,
            chars,
            casing_operation(source.operation()),
            casing_profile(source.profile()),
            opt(unit),
            usize::from(s.is_capitalize_target()),
            usize::from(std::ptr::eq(
                ws.participant().token(),
                source.participant().token()
            ))
        );
    }

    let structure = match build_source_structure(&whitespace, structure_limits()) {
        Ok(v) => v,
        Err(SourceStructureError::Gap {
            left,
            right,
            events,
            scalars,
            witness,
        }) => {
            let (tag, source, index) = match witness {
                SourceStructureGapWitness::UnresolvedEmptyInlineGap { source, event } => {
                    ("UnresolvedEmptyInlineGap", source, event)
                }
                SourceStructureGapWitness::UnresolvedCollapsibleElementGap { source, event } => {
                    ("UnresolvedCollapsibleElementGap", source, event)
                }
                SourceStructureGapWitness::UnresolvedLiteralBidiGap { source, scalar } => {
                    ("UnresolvedLiteralBidiGap", source, scalar)
                }
            };
            println!(
                "G\t{case}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{tag}\t{source}\t{index}",
                left.source,
                left.event,
                right.source,
                right.event,
                events.start,
                events.end,
                scalars.start,
                scalars.end
            );
            println!("Z\t{case}");
            return;
        }
        Err(e) => panic!("generous predecessor setup failed: {e:?}"),
    };
    for r in structure.records() {
        let direct = structure.record(r.index()).unwrap();
        let start = r
            .start_edge()
            .map(|e| format!("{}:{}", edge_kind(e.kind()), e.event().index()))
            .unwrap_or_else(|| "-".into());
        let end = r
            .end_edge()
            .map(|e| format!("{}:{}", edge_kind(e.kind()), e.event().index()))
            .unwrap_or_else(|| "-".into());
        assert!(r.is_same_record(&direct));
        println!(
            "R\t{case}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            r.index(),
            kind(r.kind()),
            opt(r.formatting_parent().map(|p| p.index())),
            opt(r.source_index()),
            opt(r.content_event().map(|e| e.index())),
            start,
            end,
            usize::from(std::ptr::eq(r.owner(), &structure))
        );
    }
    let input = match build_source_analysis_input(&structure, &casing, analysis_limits()) {
        Ok(v) => v,
        Err(e) => {
            print_error(case, &e);
            println!("Z\t{case}");
            return;
        }
    };
    assert!(std::ptr::eq(input.structure(), &structure));
    assert!(std::ptr::eq(input.casing(), &casing));
    assert!(std::ptr::eq(input.whitespace(), structure.whitespace()));
    let rc = input.receipt();
    assert_eq!(
        (
            input.work(),
            input.retained_bytes(),
            input.peak_retained_bytes()
        ),
        (rc.work, rc.retained_bytes, rc.peak_retained_bytes)
    );
    println!(
        "H\t{case}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        rc.sources,
        rc.records,
        rc.payloads,
        rc.original_scalars,
        rc.scopes,
        rc.domains,
        rc.output_scalars,
        rc.output_bytes,
        rc.scope_capacity,
        rc.scalar_capacity,
        rc.transport_capacity,
        rc.scope_element_bytes,
        rc.scalar_element_bytes,
        rc.work,
        rc.retained_bytes,
        rc.peak_retained_bytes,
        input.scope_count(),
        input.domain_count(),
        input.contribution_count(),
        input.work(),
        input.retained_bytes(),
        input.peak_retained_bytes()
    );
    let mut scopes = input.scopes();
    exact_fused(&scopes);
    assert_eq!(
        scopes.size_hint(),
        (input.scope_count(), Some(input.scope_count()))
    );
    while let Some(scope) = scopes.next() {
        let remaining = input.scope_count() - scope.index() - 1;
        assert_eq!(scopes.size_hint(), (remaining, Some(remaining)));
        let rec = scope.record();
        assert!(rec.is_same_record(&structure.record(rec.index()).unwrap()));
        let base = scope.base_origin();
        let (bk, bp) = match &base {
            SourceAnalysisBaseOrigin::OwnBlock(_) => ("OwnBlock", base.participant()),
            SourceAnalysisBaseOrigin::AnonymousInherited(_) => {
                ("AnonymousInherited", base.participant())
            }
        };
        assert!(base.scope().is_same_scope(&scope));
        assert!(base.record().is_same_record(&rec));
        assert!(std::ptr::eq(
            bp.token(),
            eligibility.token(bp.source_index()).unwrap()
        ));
        let direct_scope = input.scope(scope.index()).unwrap();
        println!(
            "Q\t{case}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            scope.index(),
            rec.index(),
            participation(scope.participation()),
            range(scope.record_range()),
            range(scope.original_range()),
            range(scope.transport_range()),
            range(scope.analysis_range()),
            mode(scope.mode()),
            bk,
            bp.source_index(),
            base.direction().as_str(),
            usize::from(std::ptr::eq(scope.owner(), &input)),
            usize::from(scope.is_same_scope(&direct_scope))
        );
        let mut cs = scope.contributions();
        exact_fused(&cs);
        let n = scope.original_range().len();
        assert_eq!(cs.size_hint(), (n, Some(n)));
        for expected in scope.original_range() {
            let c = cs.next().unwrap();
            assert_eq!(c.original_index(), expected);
            let remaining = scope.original_range().end - expected - 1;
            assert_eq!(cs.size_hint(), (remaining, Some(remaining)));
            assert!(c.is_same_contribution(&input.contribution(expected).unwrap()));
        }
        assert_eq!(cs.size_hint(), (0, Some(0)));
        assert!(cs.next().is_none() && cs.next().is_none());
        if let Some(domain) = scope.inline_sequence() {
            assert!(domain.scope().is_same_scope(&scope));
            println!(
                "D\t{case}\t{}\t{}\t{}\t{}\t{}",
                scope.index(),
                hex(domain.text()),
                domain.scalar_count(),
                mode(scope.mode()),
                usize::from(domain.is_same_domain(&scope.inline_sequence().unwrap()))
            );
            for local in 0..domain.scalar_count() {
                let origin = domain.origin(local).unwrap();
                let c = origin.contribution();
                assert!(
                    c.is_same_contribution(&input.contribution(origin.original_index()).unwrap())
                );
                println!(
                    "M\t{case}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                    scope.index(),
                    local,
                    origin.analysis_index(),
                    origin.original_index(),
                    origin.member_index(),
                    origin.character() as u32,
                    c.payload().index(),
                    usize::from(origin.is_same_origin(&domain.origin(local).unwrap()))
                );
            }
            assert!(domain.origin(domain.scalar_count()).is_none());
            let mut origin = |local| {
                let a = domain
                    .origin(local)
                    .expect("raw local scalar joins mapped domain");
                (
                    a.original_index(),
                    a.member_index(),
                    a.character(),
                    usize::from(a.is_same_origin(&domain.origin(local).unwrap())),
                )
            };
            raw(
                case,
                scope.index(),
                domain.text(),
                scope.mode(),
                &mut origin,
            );
        }
    }
    assert_eq!(scopes.size_hint(), (0, Some(0)));
    assert!(scopes.next().is_none() && scopes.next().is_none());
    let mut contributions = input.contributions();
    exact_fused(&contributions);
    let cn = input.contribution_count();
    assert_eq!(contributions.size_hint(), (cn, Some(cn)));
    while let Some(c) = contributions.next() {
        let remaining = cn - c.original_index() - 1;
        assert_eq!(contributions.size_hint(), (remaining, Some(remaining)));
        let cs = c.casing_scalar();
        let ws = c.whitespace_scalar();
        let (tag, chars) = presentation(c.presentation());
        let scope = c.scope();
        assert_eq!(cs.index(), c.original_index());
        assert_eq!(ws.index(), c.original_index());
        assert!(std::ptr::eq(
            cs.source().participant().token(),
            ws.participant().token()
        ));
        assert!(
            c.payload()
                .is_same_record(&structure.record(c.payload().index()).unwrap())
        );
        println!(
            "C\t{case}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            c.original_index(),
            scope.index(),
            c.payload().index(),
            cs.source().source_index(),
            range(c.transport_range()),
            range(c.analysis_range()),
            tag,
            chars,
            usize::from(std::ptr::eq(c.owner(), &input)),
            usize::from(scope.is_same_scope(&input.scope(scope.index()).unwrap())),
            usize::from(c.is_same_contribution(&input.contribution(c.original_index()).unwrap()))
        );
    }
    assert_eq!(contributions.size_hint(), (0, Some(0)));
    assert!(contributions.next().is_none() && contributions.next().is_none());
    assert!(input.contribution(input.contribution_count()).is_none());
    assert!(input.scope(input.scope_count()).is_none());
    for r in structure.records() {
        if r.kind() == SourceStructureKind::Payload {
            let p = input.payload_projection(r.index()).unwrap();
            assert!(p.payload().is_same_record(&r));
            let canonical = input
                .scope(p.scope().index())
                .unwrap()
                .inline_sequence()
                .unwrap();
            assert!(
                p.scope().is_same_scope(&p.domain().scope())
                    && p.domain().is_same_domain(&canonical)
            );
            println!(
                "P\t{case}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                r.index(),
                p.scope().index(),
                range(p.original_range()),
                range(p.transport_range()),
                range(p.analysis_range()),
                range(p.domain_byte_range()),
                range(p.domain_scalar_range()),
                usize::from(p.is_same_projection(&input.payload_projection(r.index()).unwrap()))
            );
        } else {
            assert!(input.payload_projection(r.index()).is_none());
        }
    }
    assert!(input.payload_projection(structure.record_count()).is_none());
    assert!(input.payload_projection(usize::MAX).is_none());
    println!("Z\t{case}");
}

fn observe(case: &str, root: &InputNode) {
    observe_with_raw(case, root, |_, _, _, _, _| {});
}
