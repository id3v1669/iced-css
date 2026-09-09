include!("support/source_participants_helpers.rs");
use iced_css::capture::{
    ContentCapability, ControlContentFamily, ManagedContentFamily, ReplacementCapability,
};
use iced_css::evaluation::source_participants::*;

#[test]
fn original_indices_and_exact_owner_survive_wrapper_drop() {
    let host_a = host(row(vec![wrapped("left"), wrapped("right")]), None, None);
    let host_b = host(row(vec![wrapped("left"), wrapped("right")]), None, None);
    let ua = catalog();
    let sheets = controls(&[], true, 100);
    let bindings = [];
    let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, bundle_limits()).unwrap();
    let display_a = evaluate_source_display(host_a.snapshot().unwrap(), &bundle, limits()).unwrap();
    let display_b = evaluate_source_display(host_b.snapshot().unwrap(), &bundle, limits()).unwrap();
    let eligible_a = evaluate_source_eligibility(&display_a, eligibility_limits()).unwrap();
    let eligible_b = evaluate_source_eligibility(&display_b, eligibility_limits()).unwrap();
    let view = SourceParticipants::new(&eligible_a);
    assert_eq!(view.node_count(), 5);
    assert!(view.get(5).is_none());
    assert!(view.get(usize::MAX).is_none());
    assert_eq!(
        eligible_a
            .ordered()
            .iter()
            .map(|x| x.source)
            .collect::<Vec<_>>(),
        [4, 1, 0, 3, 2]
    );
    for entry in eligible_a.ordered() {
        let item = view.get(entry.source).unwrap();
        assert_eq!(item.source_index(), entry.source);
        assert!(std::ptr::eq(item.eligibility(), &eligible_a));
        assert!(std::ptr::eq(
            item.node(),
            &host_a.snapshot().unwrap().construction().nodes()[entry.source]
        ));
        assert!(std::ptr::eq(
            item.token(),
            eligible_a.token(entry.source).unwrap()
        ));
        assert!(std::ptr::eq(
            item.resolution(),
            display_a.resolution(entry.source).unwrap()
        ));
        assert!(std::ptr::eq(
            item.text_keywords(),
            display_a.text_keywords(entry.source).unwrap()
        ));
        assert!(std::ptr::eq(
            item.bidi_style(),
            display_a.bidi_style(entry.source).unwrap()
        ));
        let other = SourceParticipants::new(&eligible_b)
            .get(entry.source)
            .unwrap();
        assert!(std::ptr::eq(
            other.token(),
            eligible_b.token(entry.source).unwrap()
        ));
        assert!(!std::ptr::eq(item.token(), other.token())); // returned addresses, not token semantic equality
    }
    let child = view.get(0).unwrap();
    drop(view);
    assert!(matches!(child.content(), ContentCapability::Text("text")));
    assert_eq!(
        child.outer_category(),
        SourceOuterCategory::InlineNonReplaced
    );
    assert_eq!(
        child.participation(),
        SourceParticipation::Eligible { flow: Flow::Normal }
    );
}

#[test]
fn ancestor_suppression_wins_over_own_flow_and_keeps_control_recipe() {
    let checkbox = adapters::checkbox::<(), iced::Theme, ()>(false, capture_limits())
        .unwrap()
        .style_id("check", attrs())
        .unwrap()
        .erase();
    let subtree = adapters::container(
        adapters::row_from_vec(
            vec![named("normal"), named("float"), named("absolute"), checkbox],
            capture_limits(),
        )
        .unwrap(),
        capture_limits(),
    )
    .unwrap()
    .style_id("hidden", attrs())
    .unwrap()
    .erase();
    let owner = host(row(vec![subtree]), None, None);
    let ua = catalog();
    let sheets = controls(&[1], true, 100);
    let graph = graph(
        "#hidden{display:none} #float{float:left} #absolute{position:absolute} #check{display:inline}",
        ua.next_context(),
    );
    let bindings = [binding(&graph, GraphOrigin::Author(sheet(1)))];
    let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, bundle_limits()).unwrap();
    let display = evaluate_source_display(owner.snapshot().unwrap(), &bundle, limits()).unwrap();
    let eligible = evaluate_source_eligibility(&display, eligibility_limits()).unwrap();
    let view = SourceParticipants::new(&eligible);
    // Construction appends each parent: children0..3, inner row4, hidden container5, root6.
    for (i, flow) in [
        (0, Flow::Normal),
        (1, Flow::Floating),
        (2, Flow::AbsolutelyPositioned),
    ] {
        let item = view.get(i).unwrap();
        assert_eq!(item.resolution().flow, flow);
        assert_eq!(
            item.participation(),
            SourceParticipation::SuppressedByAncestor { index: 5 }
        );
        assert_eq!(item.outer_category(), SourceOuterCategory::Suppressed);
        assert!(std::ptr::eq(item.token(), eligible.token(i).unwrap()));
    }
    let control = view.get(3).unwrap();
    assert!(matches!(
        control.content(),
        ContentCapability::CheckboxLabel(None)
    ));
    assert_eq!(
        control.content().recipe_requirement(),
        Some(ControlContentFamily::Checkbox)
    );
    assert_eq!(control.outer_category(), SourceOuterCategory::Suppressed);
    assert_eq!(
        view.get(5).unwrap().participation(),
        SourceParticipation::DisplayNone
    );
}

#[test]
fn managed_structure_and_control_outer_categories_remain_separate() {
    for (css, expected) in [
        (
            "#child{display:inline}",
            SourceOuterCategory::InlineRecipeRequired(ControlContentFamily::Space),
        ),
        (
            "#child{display:inline-block}",
            SourceOuterCategory::InlineBlock,
        ),
        (
            "#child{display:inline-table}",
            SourceOuterCategory::InlineTable,
        ),
        ("#child{display:block}", SourceOuterCategory::Block),
        ("#child{display:list-item}", SourceOuterCategory::ListItem),
        ("#child{display:table-cell}", SourceOuterCategory::TableCell),
        (
            "#child{display:inline;float:left}",
            SourceOuterCategory::Block,
        ),
    ] {
        let space: CapturedElement<'static, (), iced::Theme, ()> =
            adapters::space(capture_limits())
                .unwrap()
                .style_id("child", attrs())
                .unwrap()
                .erase();
        let owner = host(row(vec![space]), None, None);
        let ua = catalog();
        let sheets = controls(&[1], true, 100);
        let graph = graph(css, ua.next_context());
        let bindings = [binding(&graph, GraphOrigin::Author(sheet(1)))];
        let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, bundle_limits()).unwrap();
        let display =
            evaluate_source_display(owner.snapshot().unwrap(), &bundle, limits()).unwrap();
        let eligible = evaluate_source_eligibility(&display, eligibility_limits()).unwrap();
        let view = SourceParticipants::new(&eligible);
        let item = view.get(0).unwrap();
        assert_eq!(item.outer_category(), expected);
        assert_eq!(
            item.content().replacement(),
            ReplacementCapability::RecipeRequired(ControlContentFamily::Space)
        );
        assert!(matches!(
            view.get(1).unwrap().content(),
            ContentCapability::Managed(ManagedContentFamily::Row)
        ));
    }
}

fn assert_child_projection(
    construction: Construction,
    css: &str,
    expected: SourceOuterCategory,
    expected_display: Display,
    expected_flow: Flow,
) {
    let owner = host(construction, None, None);
    let ua = catalog();
    let sheets = controls(&[1], true, 100);
    let graph = graph(css, ua.next_context());
    let bindings = [binding(&graph, GraphOrigin::Author(sheet(1)))];
    let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, bundle_limits()).unwrap();
    let display = evaluate_source_display(owner.snapshot().unwrap(), &bundle, limits()).unwrap();
    let eligible = evaluate_source_eligibility(&display, eligibility_limits()).unwrap();
    let item = SourceParticipants::new(&eligible).get(0).unwrap();
    assert_eq!(item.outer_category(), expected, "{css}");
    assert_eq!(item.resolution().display, expected_display, "{css}");
    assert_eq!(item.resolution().flow, expected_flow, "{css}");
    assert!(std::ptr::eq(
        item.resolution(),
        display.resolution(0).unwrap()
    ));
    assert!(std::ptr::eq(item.token(), eligible.token(0).unwrap()));
}

#[test]
fn every_computed_display_has_a_distinct_projection_requirement() {
    // These literals test only the new projection; selector/display algorithms keep their own tests.
    for (keyword, display, outer) in [
        (
            "inline",
            Display::Inline,
            SourceOuterCategory::InlineNonReplaced,
        ),
        ("block", Display::Block, SourceOuterCategory::Block),
        (
            "list-item",
            Display::ListItem,
            SourceOuterCategory::ListItem,
        ),
        (
            "inline-block",
            Display::InlineBlock,
            SourceOuterCategory::InlineBlock,
        ),
        ("table", Display::Table, SourceOuterCategory::Table),
        (
            "inline-table",
            Display::InlineTable,
            SourceOuterCategory::InlineTable,
        ),
        (
            "table-row-group",
            Display::TableRowGroup,
            SourceOuterCategory::TableRowGroup,
        ),
        (
            "table-header-group",
            Display::TableHeaderGroup,
            SourceOuterCategory::TableHeaderGroup,
        ),
        (
            "table-footer-group",
            Display::TableFooterGroup,
            SourceOuterCategory::TableFooterGroup,
        ),
        (
            "table-row",
            Display::TableRow,
            SourceOuterCategory::TableRow,
        ),
        (
            "table-column-group",
            Display::TableColumnGroup,
            SourceOuterCategory::TableColumnGroup,
        ),
        (
            "table-column",
            Display::TableColumn,
            SourceOuterCategory::TableColumn,
        ),
        (
            "table-cell",
            Display::TableCell,
            SourceOuterCategory::TableCell,
        ),
        (
            "table-caption",
            Display::TableCaption,
            SourceOuterCategory::TableCaption,
        ),
        ("none", Display::None, SourceOuterCategory::Suppressed),
    ] {
        let css = format!("#child{{display:{keyword}}}");
        assert_child_projection(
            row(vec![named("child")]),
            &css,
            outer,
            display,
            if display == Display::None {
                Flow::Suppressed
            } else {
                Flow::Normal
            },
        );
    }
}

#[test]
fn projection_uses_actual_root_float_and_absolute_resolution_without_reapplying_it() {
    for (css, expected, display, flow, root) in [
        (
            "#child{display:inline-table}",
            SourceOuterCategory::Table,
            Display::Table,
            Flow::Normal,
            true,
        ),
        (
            "#child{display:inline}",
            SourceOuterCategory::Block,
            Display::Block,
            Flow::Normal,
            true,
        ),
        (
            "#child{display:list-item}",
            SourceOuterCategory::ListItem,
            Display::ListItem,
            Flow::Normal,
            true,
        ),
        (
            "#child{display:inline-table;float:left}",
            SourceOuterCategory::Table,
            Display::Table,
            Flow::Floating,
            false,
        ),
        (
            "#child{display:inline-block;float:right}",
            SourceOuterCategory::Block,
            Display::Block,
            Flow::Floating,
            false,
        ),
        (
            "#child{display:inline-table;position:absolute}",
            SourceOuterCategory::Table,
            Display::Table,
            Flow::AbsolutelyPositioned,
            false,
        ),
        (
            "#child{display:table-cell;position:fixed}",
            SourceOuterCategory::Block,
            Display::Block,
            Flow::AbsolutelyPositioned,
            false,
        ),
        (
            "#child{display:inline;float:left;position:absolute}",
            SourceOuterCategory::Block,
            Display::Block,
            Flow::AbsolutelyPositioned,
            false,
        ),
    ] {
        let construction = if root {
            named("child").into_parts().1
        } else {
            row(vec![named("child")])
        };
        assert_child_projection(construction, css, expected, display, flow);
    }
}

#[test]
fn table_pane_and_child_owning_families_preserve_typed_source_roles() {
    let table = Construction::table(
        1,
        1,
        vec![text().into_parts().1, text().into_parts().1],
        capture_limits(),
    )
    .unwrap();
    let table_families: Vec<_> = table
        .nodes()
        .iter()
        .filter_map(|node| match node.content_capability() {
            ContentCapability::Managed(family) => Some(family),
            ContentCapability::Text(_) => None,
            _ => panic!("unexpected table source capability"),
        })
        .collect();
    assert_eq!(
        table_families,
        [
            ManagedContentFamily::TableHeaderCell,
            ManagedContentFamily::TableRow,
            ManagedContentFamily::TableHeaderGroup,
            ManagedContentFamily::TableCell,
            ManagedContentFamily::TableRow,
            ManagedContentFamily::TableRowGroup,
            ManagedContentFamily::Table
        ]
    );
    assert_eq!(
        table
            .nodes()
            .iter()
            .filter(|node| matches!(node.content_capability(), ContentCapability::Text(_)))
            .count(),
        2
    );
    let controls = adapters::pane_controls_dynamic(text(), text(), capture_limits()).unwrap();
    let title = adapters::pane_title(text(), capture_limits())
        .unwrap()
        .controls(controls, capture_limits())
        .unwrap();
    let pane = adapters::pane_content(text(), capture_limits())
        .unwrap()
        .title_bar(title, capture_limits())
        .unwrap();
    let nodes = pane.construction().nodes();
    let descriptors: Vec<_> = nodes
        .iter()
        .filter_map(|node| match node.content_capability() {
            ContentCapability::Managed(family) => Some((family, node.role())),
            _ => None,
        })
        .collect();
    assert_eq!(
        descriptors,
        [
            (ManagedContentFamily::PaneControls, Some("controls")),
            (ManagedContentFamily::PaneTitle, Some("title")),
            (ManagedContentFamily::PaneContent, None)
        ]
    );
    assert_eq!(
        nodes
            .iter()
            .filter_map(|node| match node.content_capability() {
                ContentCapability::Text(_) => node.role(),
                _ => None,
            })
            .collect::<Vec<_>>(),
        ["content", "full", "compact", "body"]
    );
    let button = adapters::button(text(), capture_limits()).unwrap();
    let opaque = adapters::opaque(text(), capture_limits()).unwrap();
    let button_nodes = button.construction().nodes();
    assert!(matches!(
        button_nodes[1].content_capability(),
        ContentCapability::RecipeRequired(ControlContentFamily::Button)
    ));
    assert!(matches!(
        button_nodes[0].content_capability(),
        ContentCapability::Text(_)
    ));
    assert_eq!(button_nodes[0].parent(), Some(1));
    let opaque_nodes = opaque.construction().nodes();
    assert!(matches!(
        opaque_nodes[1].content_capability(),
        ContentCapability::Managed(ManagedContentFamily::Opaque)
    ));
    assert_eq!(opaque_nodes[0].parent(), Some(1));
}

#[cfg(feature = "image-without-codecs")]
#[test]
fn inline_replaced_and_other_inline_atomic_use_same_borrowed_image_evidence() {
    for (css, outer, display) in [
        (
            "image{display:inline}",
            SourceOuterCategory::InlineReplaced,
            Display::Inline,
        ),
        (
            "image{display:inline-block}",
            SourceOuterCategory::InlineBlock,
            Display::InlineBlock,
        ),
        (
            "image{display:inline-table}",
            SourceOuterCategory::InlineTable,
            Display::InlineTable,
        ),
    ] {
        let image = adapters::image(
            iced::widget::image::Handle::from_rgba(1, 1, vec![0u8; 4]),
            capture_limits(),
        )
        .unwrap();
        let parent =
            adapters::container::<(), iced::Theme, iced::Renderer, _>(image, capture_limits())
                .unwrap();
        assert_child_projection(parent.into_parts().1, css, outer, display, Flow::Normal);
    }
}
