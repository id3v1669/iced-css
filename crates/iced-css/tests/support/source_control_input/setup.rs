// Expected-free actual-owner adapter for the approved SourceAnalysisInput API.
include!("../source_analysis_input/helpers.rs");

use iced_css::{
    evaluation::{
        source_analysis_input::*,
        source_casing::*,
        source_participants::SourceParticipants,
        source_structure::*,
        source_whitespace::*,
    },
};
use std::ops::Range;

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
