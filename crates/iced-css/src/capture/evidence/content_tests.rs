use super::*;
use crate::capture::{
    ContentCapability, ControlContentFamily, ManagedContentFamily, ReplacementCapability,
};
#[derive(Debug, PartialEq, Eq)]
enum Expected {
    Text,
    Rich,
    Checkbox,
    Toggler,
    Radio,
    Managed(ManagedContentFamily),
    Recipe(ControlContentFamily),
}
// Exhaustiveness is deliberate: every new private Kind needs an explicit fixture decision.
fn acknowledge_kind(kind: &Kind) {
    match kind {
        Kind::Table => {}
        Kind::TableHeaderGroup => {}
        Kind::TableRowGroup => {}
        Kind::TableRow => {}
        Kind::TableHeaderCell => {}
        Kind::TableCell => {}
        Kind::PaneGrid => {}
        Kind::PaneControls => {}
        Kind::PaneTitle => {}
        Kind::PaneContent => {}
        Kind::Container => {}
        Kind::Row => {}
        Kind::Column => {}
        Kind::KeyedColumn => {}
        Kind::Stack => {}
        Kind::Grid => {}
        Kind::Scrollable => {}
        Kind::Tooltip => {}
        Kind::Hover => {}
        Kind::Pin => {}
        Kind::MouseArea => {}
        Kind::Float => {}
        Kind::Themer => {}
        Kind::Sensor => {}
        Kind::Opaque => {}
        Kind::Button => {}
        Kind::PickList => {}
        Kind::Rule => {}
        Kind::ProgressBar => {}
        Kind::Slider => {}
        Kind::VerticalSlider => {}
        Kind::Space => {}
        Kind::Text(_)
        | Kind::RichText { .. }
        | Kind::Checkbox(_)
        | Kind::Toggler(_)
        | Kind::Radio(_) => {}
        #[cfg(feature = "image-without-codecs")]
        Kind::Image(_) => {}
        #[cfg(feature = "svg")]
        Kind::Svg(_) => {}
    }
}
#[test]
fn every_committed_kind_has_its_explicit_content_mapping() {
    let cases = vec![
        (Kind::Table, Expected::Managed(ManagedContentFamily::Table)),
        (
            Kind::TableHeaderGroup,
            Expected::Managed(ManagedContentFamily::TableHeaderGroup),
        ),
        (
            Kind::TableRowGroup,
            Expected::Managed(ManagedContentFamily::TableRowGroup),
        ),
        (
            Kind::TableRow,
            Expected::Managed(ManagedContentFamily::TableRow),
        ),
        (
            Kind::TableHeaderCell,
            Expected::Managed(ManagedContentFamily::TableHeaderCell),
        ),
        (
            Kind::TableCell,
            Expected::Managed(ManagedContentFamily::TableCell),
        ),
        (
            Kind::PaneGrid,
            Expected::Managed(ManagedContentFamily::PaneGrid),
        ),
        (
            Kind::PaneControls,
            Expected::Managed(ManagedContentFamily::PaneControls),
        ),
        (
            Kind::PaneTitle,
            Expected::Managed(ManagedContentFamily::PaneTitle),
        ),
        (
            Kind::PaneContent,
            Expected::Managed(ManagedContentFamily::PaneContent),
        ),
        (
            Kind::Container,
            Expected::Managed(ManagedContentFamily::Container),
        ),
        (Kind::Row, Expected::Managed(ManagedContentFamily::Row)),
        (
            Kind::Column,
            Expected::Managed(ManagedContentFamily::Column),
        ),
        (
            Kind::KeyedColumn,
            Expected::Managed(ManagedContentFamily::KeyedColumn),
        ),
        (Kind::Stack, Expected::Managed(ManagedContentFamily::Stack)),
        (Kind::Grid, Expected::Managed(ManagedContentFamily::Grid)),
        (
            Kind::Scrollable,
            Expected::Managed(ManagedContentFamily::Scrollable),
        ),
        (
            Kind::Tooltip,
            Expected::Managed(ManagedContentFamily::Tooltip),
        ),
        (Kind::Hover, Expected::Managed(ManagedContentFamily::Hover)),
        (Kind::Pin, Expected::Managed(ManagedContentFamily::Pin)),
        (
            Kind::MouseArea,
            Expected::Managed(ManagedContentFamily::MouseArea),
        ),
        (Kind::Float, Expected::Managed(ManagedContentFamily::Float)),
        (
            Kind::Themer,
            Expected::Managed(ManagedContentFamily::Themer),
        ),
        (
            Kind::Sensor,
            Expected::Managed(ManagedContentFamily::Sensor),
        ),
        (
            Kind::Opaque,
            Expected::Managed(ManagedContentFamily::Opaque),
        ),
        (Kind::Button, Expected::Recipe(ControlContentFamily::Button)),
        (
            Kind::PickList,
            Expected::Recipe(ControlContentFamily::PickList),
        ),
        (Kind::Rule, Expected::Recipe(ControlContentFamily::Rule)),
        (
            Kind::ProgressBar,
            Expected::Recipe(ControlContentFamily::ProgressBar),
        ),
        (Kind::Slider, Expected::Recipe(ControlContentFamily::Slider)),
        (
            Kind::VerticalSlider,
            Expected::Recipe(ControlContentFamily::VerticalSlider),
        ),
        (Kind::Space, Expected::Recipe(ControlContentFamily::Space)),
        (Kind::Text(String::from("plain")), Expected::Text),
        (
            Kind::RichText {
                text: String::from("é"),
                runs: vec![0..0, 0..2],
            },
            Expected::Rich,
        ),
        (Kind::Checkbox(Some(String::new())), Expected::Checkbox),
        (Kind::Toggler(None), Expected::Toggler),
        (Kind::Radio(String::new()), Expected::Radio),
    ];
    assert_eq!(cases.len(), 37);
    for (kind, expected) in cases {
        acknowledge_kind(&kind);
        // Synthetic standalone records qualify the accessor mapping only, not valid control topology.
        let node = ConstructionNode {
            kind,
            parent: None,
            role: None,
            metadata: Default::default(),
            source_key: None,
        };
        let actual = match node.content_capability() {
            ContentCapability::Text(value) => {
                assert_eq!(value, "plain");
                Expected::Text
            }
            ContentCapability::RichText { text, runs } => {
                assert_eq!(text, "é");
                assert_eq!(runs, [0..0, 0..2]);
                Expected::Rich
            }
            ContentCapability::CheckboxLabel(label) => {
                assert_eq!(label, Some(""));
                Expected::Checkbox
            }
            ContentCapability::TogglerLabel(label) => {
                assert_eq!(label, None);
                Expected::Toggler
            }
            ContentCapability::RadioLabel(label) => {
                assert_eq!(label, "");
                Expected::Radio
            }
            ContentCapability::Managed(family) => Expected::Managed(family),
            ContentCapability::RecipeRequired(family) => Expected::Recipe(family),
            #[cfg(feature = "image-without-codecs")]
            ContentCapability::Image(_) => {
                panic!("resource variants qualified by real adapter fixtures")
            }
            #[cfg(feature = "svg")]
            ContentCapability::Svg(_) => {
                panic!("resource variants qualified by real adapter fixtures")
            }
        };
        assert_eq!(actual, expected);
        let replacement = match expected {
            Expected::Text | Expected::Rich | Expected::Managed(_) => {
                ReplacementCapability::NonReplaced
            }
            Expected::Checkbox => {
                ReplacementCapability::RecipeRequired(ControlContentFamily::Checkbox)
            }
            Expected::Toggler => {
                ReplacementCapability::RecipeRequired(ControlContentFamily::Toggler)
            }
            Expected::Radio => ReplacementCapability::RecipeRequired(ControlContentFamily::Radio),
            Expected::Recipe(family) => ReplacementCapability::RecipeRequired(family),
        };
        assert_eq!(node.content_capability().replacement(), replacement);
    }
}
