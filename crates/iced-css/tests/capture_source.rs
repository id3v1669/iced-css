use iced_css::capture::{
    CaptureLimits, CapturedElement, adapters,
    source::{ConstructionSource, ConstructionSourceError, ConstructionState},
};
use iced_css_core::{
    selectors::{
        MatchLimits, SelectorLimits, SelectorSource, matching_specificity, parse_selector_list,
    },
    source::{
        AttributeLimits,
        selector_tree::{DocumentLanguage, SelectorTreeLimits},
    },
};
use std::num::NonZeroUsize;
fn n() -> NonZeroUsize {
    NonZeroUsize::new(100000).unwrap()
}
fn limits() -> CaptureLimits {
    CaptureLimits {
        nodes: n(),
        depth: n(),
        retained_bytes: n(),
        work: n(),
    }
}
fn attrs() -> AttributeLimits {
    AttributeLimits {
        entries: n(),
        retained_bytes: n(),
        work: n(),
    }
}
fn source_limits() -> SelectorTreeLimits {
    SelectorTreeLimits {
        entries: n(),
        retained_bytes: n(),
        work: n(),
    }
}
fn text() -> CapturedElement<'static, (), iced::Theme, ()> {
    adapters::text("label", limits()).unwrap().erase()
}
fn tree() -> CapturedElement<'static, (), iced::Theme, ()> {
    let first = adapters::container(text(), limits())
        .unwrap()
        .style_classes(&["first"], attrs())
        .unwrap()
        .style_attribute("lang", "", attrs())
        .unwrap()
        .erase();
    let second = adapters::button(text(), limits())
        .unwrap()
        .style_id("go", attrs())
        .unwrap()
        .erase();
    adapters::row_from_vec(vec![first, second], limits())
        .unwrap()
        .style_attribute("lang", "en-US", attrs())
        .unwrap()
        .erase()
}
fn matches(source: &ConstructionSource<'_>, node: usize, css: &str) -> bool {
    let selectors = parse_selector_list(
        css,
        SelectorLimits {
            work: n(),
            retained_bytes: n(),
            entries: n(),
        },
    )
    .unwrap();
    matching_specificity(
        &selectors,
        source,
        node,
        None,
        MatchLimits {
            work: n(),
            retained_bytes: n(),
        },
    )
    .unwrap()
    .is_some()
}

#[test]
fn captured_relations_language_and_state_feed_complex_selectors() {
    let capture = tree();
    let mut states = [ConstructionState::default(); 5];
    states[0].hovered = true;
    states[3].active = true;
    states[3].focused = true;
    let source = ConstructionSource::new(
        capture.construction(),
        &states,
        DocumentLanguage::Known("fr"),
        source_limits(),
    )
    .unwrap();
    assert_eq!(source.root(), 4);
    assert_eq!(source.parent(3), Some(4));
    assert_eq!(source.previous_element(3), Some(1));
    assert!(matches(
        &source,
        3,
        "row > container.first + button#go:focus:active:lang(en)"
    ));
    assert!(matches(
        &source,
        1,
        "row:hover > container:hover:first-child"
    ));
    assert!(!matches(&source, 0, "text:lang(en)"));
    assert!(matches(&source, 2, "text:lang(en)"));
    assert!(!matches(&source, 4, ":focus"));
    assert!(matches(&source, 4, ":active"));
    assert_eq!(source.parent(99), None);
    assert!(source.facts(99).is_none());
}

#[test]
fn duplicate_ids_state_shape_and_limits_reject_before_exposing_source() {
    let a = text().style_id("duplicate", attrs()).unwrap();
    let b = text().style_id("duplicate", attrs()).unwrap();
    let capture = adapters::row_from_vec(vec![a, b], limits()).unwrap();
    assert!(matches!(
        ConstructionSource::new(
            capture.construction(),
            &[ConstructionState::default(); 3],
            DocumentLanguage::Unknown,
            source_limits()
        ),
        Err(ConstructionSourceError::DuplicateCssId)
    ));
    let capture = tree();
    assert!(matches!(
        ConstructionSource::new(
            capture.construction(),
            &[],
            DocumentLanguage::Unknown,
            source_limits()
        ),
        Err(ConstructionSourceError::StateCount)
    ));
    assert!(matches!(
        ConstructionSource::new(
            capture.construction(),
            &[ConstructionState::default(); 5],
            DocumentLanguage::Known(""),
            source_limits()
        ),
        Err(ConstructionSourceError::EmptyDocumentLanguage)
    ));
    for error in [
        ConstructionSourceError::Entries,
        ConstructionSourceError::Bytes,
        ConstructionSourceError::Work,
    ] {
        let mut small = source_limits();
        match error {
            ConstructionSourceError::Entries => small.entries = NonZeroUsize::MIN,
            ConstructionSourceError::Bytes => small.retained_bytes = NonZeroUsize::MIN,
            ConstructionSourceError::Work => small.work = NonZeroUsize::MIN,
            _ => unreachable!(),
        }
        assert!(
            matches!(ConstructionSource::new(capture.construction(),&[ConstructionState::default();5],DocumentLanguage::Unknown,small),Err(actual) if actual==error)
        );
    }
}

#[test]
fn authored_rules_cascade_on_captured_sources_and_compute_display() {
    use iced_css_core::{
        cascade::{rule::*, sheets::*, winner_table::PropertyWinners},
        formatting::display::{Display, Float, Flow, Position},
        syntax::*,
        values::{NumericLimits, color::ColorLimits, display::compute_display_style},
    };
    use iced_css_core::{
        stylesheet::imports::{
            ImportBinding, ImportResolution, LoadedGraphLimits, LoadedSheet, expand_loaded_imports,
        },
        syntax::imports::{ImportLimits, ResourceId},
    };
    use std::num::NonZeroU64;
    // Use the production assembler, then apply its records to captured source.
    fn rules(css: &str, first_order: u64) -> iced_css_core::stylesheet::ParsedStylesheet {
        use iced_css_core::stylesheet::*;
        let recovery = RecoveryLimits {
            work: n(),
            retained_bytes: n(),
            nesting: n(),
        };
        let sheet = parse_stylesheet(
            css,
            RuleContext {
                first_order,
                first_declaration: NonZeroU64::new(first_order).unwrap(),
            },
            StylesheetLimits {
                statements: n(),
                record_bytes: n(),
                work: n(),
                rules: RuleAssemblyLimits {
                    recovery,
                    selectors: SelectorLimits {
                        work: n(),
                        retained_bytes: n(),
                        entries: n(),
                    },
                    values: ColorLimits {
                        syntax: StringLimits {
                            work: n(),
                            retained_bytes: n(),
                        },
                        numeric: NumericLimits {
                            work: n(),
                            digits: n(),
                        },
                    },
                    entries: n(),
                    record_bytes: n(),
                    work: n(),
                    name_bytes: n(),
                },
            },
        )
        .unwrap();
        assert!(sheet.rejected.is_empty());
        sheet
    }
    let capture = tree();
    let mut states = [ConstructionState::default(); 5];
    states[3].focused = true;
    let source = ConstructionSource::new(
        capture.construction(),
        &states,
        DocumentLanguage::Unknown,
        source_limits(),
    )
    .unwrap();
    let sheet = SheetId::new(NonZeroU64::MIN);
    let sheets = AuthorSheets::new(
        vec![AuthorSheet {
            id: sheet,
            enabled: true,
            membership: Membership::Persistent,
        }],
        vec![],
        true,
        None,
        SheetLimits {
            entries: n(),
            retained_bytes: n(),
            work: n(),
        },
    )
    .unwrap();
    let author = rules(
        "@import \"theme.css\"; @page {margin:1cm} @media print {button#go:focus {display:block!important}}",
        1,
    );
    let imported = rules(
        "@media screen {.first + button#go:focus {display:none!important;display:block;float:left;position:absolute}}",
        1,
    );
    let bindings = [ImportBinding {
        entry: 0,
        resolution: ImportResolution::Sheet(1),
    }];
    let loaded = [
        LoadedSheet {
            resource: ResourceId::new(NonZeroU64::MIN),
            sheet: &author,
            imports: &bindings,
        },
        LoadedSheet {
            resource: ResourceId::new(NonZeroU64::new(2).unwrap()),
            sheet: &imported,
            imports: &[],
        },
    ];
    let plan = expand_loaded_imports(
        &loaded,
        0,
        RuleContext {
            first_order: 1,
            first_declaration: NonZeroU64::MIN,
        },
        LoadedGraphLimits {
            unavailable: iced_css_core::stylesheet::imports::UnavailableImportPolicy::Record,
            entries: n(),
            record_bytes: n(),
            work: n(),
            tracking: ImportLimits {
                entries: n(),
                retained_bytes: n(),
                work: n(),
                occurrences: NonZeroU64::new(100).unwrap(),
            },
        },
    )
    .unwrap();
    let user = rules("button {display:inline!important}", 10);
    assert_eq!(plan.rules.len(), 1);
    let mut winners = PropertyWinners::new();
    let limits = RuleCascadeLimits {
        matching: MatchLimits {
            work: n(),
            retained_bytes: n(),
        },
        declarations: n(),
    };
    let target = || RuleTarget {
        source: &source,
        node: 3,
        pseudo: None,
    };
    let applied = cascade_positioned_rules(
        &mut winners,
        plan.offers(RuleOrigin::Author(sheet)),
        target(),
        &sheets,
        RuleBatchLimits {
            rules: n(),
            per_rule: limits,
        },
    )
    .unwrap();
    assert_eq!(applied.applied, 1);
    let display_property = iced_css_core::values::registry::PropertyId::lookup("display").unwrap();
    let winner = winners.candidate(display_property).unwrap().unwrap();
    let provenance = plan
        .find_declaration(winner.declaration, n())
        .unwrap()
        .unwrap();
    assert_eq!(provenance.source.get(), 2);
    assert_eq!(provenance.parsed.property, display_property);
    assert!(provenance.parsed.important);

    let parent = compute_display_style(&PropertyWinners::new(), None).unwrap();
    let suppressed = compute_display_style(&winners, Some(&parent)).unwrap();
    assert_eq!(
        (suppressed.display, suppressed.float, suppressed.flow),
        (Display::None, Float::Left, Flow::Suppressed)
    );
    cascade_rules(
        &mut winners,
        user.local_screen_rules()
            .map(|rule| rule.offer(RuleOrigin::User)),
        target(),
        &sheets,
        RuleBatchLimits {
            rules: n(),
            per_rule: limits,
        },
    )
    .unwrap();
    let computed = compute_display_style(&winners, Some(&parent)).unwrap();
    assert_eq!(
        (
            computed.display,
            computed.position,
            computed.float,
            computed.flow
        ),
        (
            Display::Block,
            Position::Absolute,
            Float::None,
            Flow::AbsolutelyPositioned
        )
    );
    assert_eq!(capture.construction().nodes()[3].canonical_type(), "button");
}

#[test]
fn zero_native_space_hints_keep_first_child_siblings_and_inherited_language() {
    let first: CapturedElement<'_, (), iced::Theme, ()> =
        adapters::space(limits()).unwrap().width(0.0).erase();
    let second = adapters::space(limits()).unwrap().height(0.0).erase();
    let column = adapters::column_from_vec(vec![first, second], limits())
        .unwrap()
        .style_attribute("lang", "en-GB", attrs())
        .unwrap();
    let states = [ConstructionState::default(); 3];
    let source = ConstructionSource::new(
        column.construction(),
        &states,
        DocumentLanguage::Unknown,
        source_limits(),
    )
    .unwrap();
    assert!(matches(&source, 0, "column > space:first-child:lang(en)"));
    assert!(matches(&source, 1, "space:first-child + space:lang(en)"));
    assert!(!matches(&source, 1, "space:first-child"));
}

#[test]
fn captured_stack_selectors_keep_zero_size_source_children() {
    let first: CapturedElement<'_, (), iced::Theme, ()> = adapters::space(limits())
        .unwrap()
        .width(0.0)
        .height(0.0)
        .erase();
    let second = adapters::space(limits()).unwrap().erase();
    let stack = adapters::stack_from_vec(vec![first, second], limits())
        .unwrap()
        .style_classes(&["layers"], attrs())
        .unwrap();
    let states = [ConstructionState::default(); 3];
    let source = ConstructionSource::new(
        stack.construction(),
        &states,
        DocumentLanguage::Unknown,
        source_limits(),
    )
    .unwrap();
    assert!(matches(&source, 0, "stack.layers > space:first-child"));
    assert!(matches(
        &source,
        1,
        "stack.layers > space:first-child + space"
    ));
    assert!(!matches(&source, 1, "space:first-child"));
}

#[test]
fn grid_append_keeps_root_attributes_and_ordered_selector_relationships() {
    let first: CapturedElement<'_, (), iced::Theme, ()> = adapters::space(limits())
        .unwrap()
        .width(0.0)
        .height(0.0)
        .erase();
    let incoming =
        adapters::row_from_vec(vec![adapters::space(limits()).unwrap().erase()], limits())
            .unwrap()
            .style_id("tail", attrs())
            .unwrap();
    let grid = adapters::grid_from_vec(vec![first], limits())
        .unwrap()
        .style_classes(&["cells"], attrs())
        .unwrap()
        .style_attribute("lang", "en-GB", attrs())
        .unwrap()
        .push(incoming, limits())
        .unwrap();
    let states = [ConstructionState::default(); 4];
    let source = ConstructionSource::new(
        grid.construction(),
        &states,
        DocumentLanguage::Unknown,
        source_limits(),
    )
    .unwrap();
    assert!(matches(
        &source,
        0,
        "grid.cells > space:first-child:lang(en)"
    ));
    assert!(matches(
        &source,
        2,
        "grid.cells > space:first-child + row#tail"
    ));
    assert!(matches(
        &source,
        1,
        "grid.cells > row#tail > space:lang(en)"
    ));
}

#[test]
fn deferred_row_push_keeps_void_children_in_css_selector_order() {
    let first: CapturedElement<'_, (), iced::Theme, ()> = adapters::space(limits())
        .unwrap()
        .width(0.0)
        .height(iced::Length::Fill)
        .erase();
    let row = adapters::row_from_vec(
        Vec::<CapturedElement<'_, (), iced::Theme, ()>>::new(),
        limits(),
    )
    .unwrap()
    .style_classes(&["managed"], attrs())
    .unwrap()
    .push(first, limits())
    .unwrap()
    .push(adapters::space(limits()).unwrap(), limits())
    .unwrap();
    let states = [ConstructionState::default(); 3];
    let source = ConstructionSource::new(
        row.construction(),
        &states,
        DocumentLanguage::Unknown,
        source_limits(),
    )
    .unwrap();
    assert!(matches(&source, 0, "row.managed > space:first-child"));
    assert!(matches(
        &source,
        1,
        "row.managed > space:first-child + space"
    ));
}

#[test]
fn deferred_column_push_keeps_void_children_in_css_selector_order() {
    let first: CapturedElement<'_, (), iced::Theme, ()> = adapters::space(limits())
        .unwrap()
        .width(0.0)
        .height(iced::Length::Fill)
        .erase();
    let column = adapters::column_from_vec(
        Vec::<CapturedElement<'_, (), iced::Theme, ()>>::new(),
        limits(),
    )
    .unwrap()
    .style_classes(&["managed"], attrs())
    .unwrap()
    .push(first, limits())
    .unwrap()
    .push(adapters::space(limits()).unwrap(), limits())
    .unwrap();
    let states = [ConstructionState::default(); 3];
    let source = ConstructionSource::new(
        column.construction(),
        &states,
        DocumentLanguage::Unknown,
        source_limits(),
    )
    .unwrap();
    assert!(matches(&source, 0, "column.managed > space:first-child"));
    assert!(matches(
        &source,
        1,
        "column.managed > space:first-child + space"
    ));
}

#[test]
fn stack_underlay_precedes_base_in_selectors_and_keeps_root_metadata() {
    let base: CapturedElement<'_, (), iced::Theme, ()> = adapters::space(limits())
        .unwrap()
        .style_id("base", attrs())
        .unwrap()
        .erase();
    let underlay = adapters::column_from_vec(
        vec![adapters::space(limits()).unwrap().width(0.0).erase()],
        limits(),
    )
    .unwrap()
    .style_id("under", attrs())
    .unwrap();
    let stack = adapters::stack_from_vec(vec![base], limits())
        .unwrap()
        .style_classes(&["layers"], attrs())
        .unwrap()
        .style_attribute("lang", "en-GB", attrs())
        .unwrap()
        .push_under(underlay, limits())
        .unwrap();
    let states = [ConstructionState::default(); 4];
    let source = ConstructionSource::new(
        stack.construction(),
        &states,
        DocumentLanguage::Unknown,
        source_limits(),
    )
    .unwrap();
    assert!(matches(
        &source,
        0,
        "stack.layers > column#under:first-child > space:lang(en)"
    ));
    assert!(matches(
        &source,
        1,
        "stack.layers > column#under:first-child"
    ));
    assert!(matches(
        &source,
        2,
        "stack.layers > column#under + space#base:lang(en)"
    ));
}

#[test]
fn scrollable_source_role_and_native_id_preserve_css_attributes() {
    let child = adapters::column_from_vec(
        vec![adapters::space(limits()).unwrap().width(0.0).erase()],
        limits(),
    )
    .unwrap();
    let scroll = adapters::scrollable::<(), iced::Theme, (), _>(child, limits())
        .unwrap()
        .style_id("css-scroll", attrs())
        .unwrap()
        .style_classes(&["pane"], attrs())
        .unwrap()
        .style_attribute("lang", "en-US", attrs())
        .unwrap()
        .id("native-scroll")
        .horizontal();
    let states = [ConstructionState::default(); 3];
    let source = ConstructionSource::new(
        scroll.construction(),
        &states,
        DocumentLanguage::Unknown,
        source_limits(),
    )
    .unwrap();
    assert!(matches(&source, 2, "scrollable#css-scroll.pane"));
    assert!(matches(
        &source,
        1,
        "scrollable.pane > column:first-child:lang(en)"
    ));
    assert!(matches(
        &source,
        0,
        "scrollable#css-scroll > column > space:first-child:lang(en)"
    ));
    assert!(!matches(&source, 2, "#native-scroll"));
}

#[test]
fn tooltip_source_children_exist_without_an_open_overlay() {
    let a = adapters::space(limits())
        .unwrap()
        .style_id("anchor", attrs())
        .unwrap();
    let b = adapters::column_from_vec(
        vec![adapters::space(limits()).unwrap().width(0.0).erase()],
        limits(),
    )
    .unwrap()
    .style_id("tip", attrs())
    .unwrap();
    let t = adapters::tooltip::<(), iced::Theme, (), _, _>(
        a,
        b,
        iced::widget::tooltip::Position::Bottom,
        limits(),
    )
    .unwrap()
    .style_classes(&["help"], attrs())
    .unwrap()
    .style_attribute("lang", "en-GB", attrs())
    .unwrap();
    let states = [ConstructionState::default(); 4];
    let source = ConstructionSource::new(
        t.construction(),
        &states,
        DocumentLanguage::Unknown,
        source_limits(),
    )
    .unwrap();
    assert!(matches(
        &source,
        0,
        "tooltip.help > space#anchor:first-child"
    ));
    assert!(matches(
        &source,
        2,
        "tooltip.help > space#anchor + column#tip"
    ));
    assert!(matches(
        &source,
        1,
        "tooltip.help > column#tip > space:lang(en)"
    ));
}

#[test]
fn hover_top_source_remains_a_sibling_without_native_hover_state() {
    let base = adapters::space(limits()).unwrap();
    let top = adapters::row_from_vec(
        vec![adapters::space(limits()).unwrap().width(0.0).erase()],
        limits(),
    )
    .unwrap()
    .style_id("top", attrs())
    .unwrap();
    let h = adapters::hover::<(), iced::Theme, (), _, _>(base, top, limits())
        .unwrap()
        .style_classes(&["shell"], attrs())
        .unwrap()
        .style_attribute("lang", "en-US", attrs())
        .unwrap();
    let states = [ConstructionState::default(); 4];
    let source = ConstructionSource::new(
        h.construction(),
        &states,
        DocumentLanguage::Unknown,
        source_limits(),
    )
    .unwrap();
    assert!(matches(&source, 0, "hover.shell > space:first-child"));
    assert!(matches(&source, 2, "hover.shell > space + row#top"));
    assert!(matches(
        &source,
        1,
        "hover.shell > row#top > space:lang(en)"
    ));
}

#[test]
fn pin_source_parent_exists_despite_native_state_forwarding() {
    let child =
        adapters::button::<(), iced::Theme, (), _>(adapters::space(limits()).unwrap(), limits())
            .unwrap();
    let pin = adapters::pin(child, limits())
        .unwrap()
        .style_classes(&["positioned"], attrs())
        .unwrap()
        .style_attribute("lang", "en-US", attrs())
        .unwrap()
        .x(10.0)
        .y(20.0);
    let states = [ConstructionState::default(); 3];
    let source = ConstructionSource::new(
        pin.construction(),
        &states,
        DocumentLanguage::Unknown,
        source_limits(),
    )
    .unwrap();
    assert!(matches(
        &source,
        1,
        "pin.positioned > button:first-child:lang(en)"
    ));
    assert!(matches(
        &source,
        0,
        "pin.positioned > button > space:first-child:lang(en)"
    ));
}

#[test]
fn mouse_area_source_identity_survives_callback_configuration() {
    let child = adapters::pin::<(), iced::Theme, (), _>(
        adapters::space(limits()).unwrap().width(0.0),
        limits(),
    )
    .unwrap();
    let area = adapters::mouse_area(child, limits())
        .unwrap()
        .style_classes(&["events"], attrs())
        .unwrap()
        .style_attribute("lang", "en-US", attrs())
        .unwrap()
        .on_press(())
        .on_release(())
        .interaction(iced::mouse::Interaction::Pointer);
    let states = [ConstructionState::default(); 3];
    let source = ConstructionSource::new(
        area.construction(),
        &states,
        DocumentLanguage::Unknown,
        source_limits(),
    )
    .unwrap();
    assert!(matches(
        &source,
        1,
        "mouse_area.events > pin:first-child:lang(en)"
    ));
    assert!(matches(
        &source,
        0,
        "mouse_area.events > pin > space:first-child:lang(en)"
    ));
}

#[test]
fn native_float_source_stays_distinct_before_overlay_materialization() {
    let child =
        adapters::button::<(), iced::Theme, (), _>(adapters::space(limits()).unwrap(), limits())
            .unwrap();
    let f = adapters::float(child, limits())
        .unwrap()
        .style_classes(&["floating"], attrs())
        .unwrap()
        .style_attribute("lang", "en-US", attrs())
        .unwrap()
        .scale(2.0)
        .translate(|_, _| iced::Vector::new(3.0, 4.0));
    let states = [ConstructionState::default(); 3];
    let source = ConstructionSource::new(
        f.construction(),
        &states,
        DocumentLanguage::Unknown,
        source_limits(),
    )
    .unwrap();
    assert!(matches(
        &source,
        1,
        "float.floating > button:first-child:lang(en)"
    ));
    assert!(matches(
        &source,
        0,
        "float.floating > button > space:first-child:lang(en)"
    ));
}

#[test]
fn themer_preserves_source_parentage_through_native_theme_configuration() {
    let child =
        adapters::button::<(), iced::Theme, (), _>(adapters::space(limits()).unwrap(), limits())
            .unwrap();
    let t = adapters::themer(Some(iced::Theme::Dark), child, limits())
        .unwrap()
        .style_classes(&["theme"], attrs())
        .unwrap()
        .style_attribute("lang", "en-US", attrs())
        .unwrap()
        .text_color(|_| iced::Color::BLACK)
        .background(|_| iced::Color::WHITE.into());
    let states = [ConstructionState::default(); 3];
    let source = ConstructionSource::new(
        t.construction(),
        &states,
        DocumentLanguage::Unknown,
        source_limits(),
    )
    .unwrap();
    assert!(matches(
        &source,
        1,
        "themer.theme > button:first-child:lang(en)"
    ));
    assert!(matches(
        &source,
        0,
        "themer.theme > button > space:first-child:lang(en)"
    ));
}

#[test]
fn sensor_native_key_transitions_do_not_replace_css_identity() {
    let native_key = String::from("native-key");
    let child = adapters::column_from_vec(
        vec![adapters::space(limits()).unwrap().width(0.0).erase()],
        limits(),
    )
    .unwrap();
    let s = adapters::sensor::<(), iced::Theme, (), _>(child, limits())
        .unwrap()
        .style_id("css-sensor", attrs())
        .unwrap()
        .style_attribute("lang", "en-US", attrs())
        .unwrap()
        .key(7u32)
        .key_ref(native_key.as_str())
        .on_hide(());
    let states = [ConstructionState::default(); 3];
    let source = ConstructionSource::new(
        s.construction(),
        &states,
        DocumentLanguage::Unknown,
        source_limits(),
    )
    .unwrap();
    assert!(matches(
        &source,
        1,
        "sensor#css-sensor > column:first-child:lang(en)"
    ));
    assert!(matches(
        &source,
        0,
        "sensor#css-sensor > column > space:first-child:lang(en)"
    ));
    assert!(!matches(&source, 2, "#native-key"));
}

#[test]
fn horizontal_and_vertical_rules_share_the_rule_source_type() {
    let a = adapters::horizontal_rule::<iced::Theme>(0.0, limits())
        .unwrap()
        .erase::<(), iced::Theme, ()>();
    let b = adapters::vertical_rule::<iced::Theme>(0.0, limits())
        .unwrap()
        .erase::<(), iced::Theme, ()>();
    let c = adapters::column_from_vec(vec![a, b], limits())
        .unwrap()
        .style_classes(&["rules"], attrs())
        .unwrap()
        .style_attribute("lang", "en-US", attrs())
        .unwrap();
    let states = [ConstructionState::default(); 3];
    let source = ConstructionSource::new(
        c.construction(),
        &states,
        DocumentLanguage::Unknown,
        source_limits(),
    )
    .unwrap();
    assert!(matches(
        &source,
        0,
        "column.rules > rule:first-child:lang(en)"
    ));
    assert!(matches(
        &source,
        1,
        "column.rules > rule:first-child + rule:lang(en)"
    ));
}

#[test]
fn horizontal_and_vertical_progress_bars_share_the_progress_bar_source_type() {
    let a = adapters::progress_bar::<iced::Theme>(0.0..=100.0, 25.0, limits())
        .unwrap()
        .erase::<(), iced::Theme, ()>();
    let b = adapters::progress_bar::<iced::Theme>(0.0..=100.0, 75.0, limits())
        .unwrap()
        .vertical()
        .erase::<(), iced::Theme, ()>();
    let c = adapters::column_from_vec(vec![a, b], limits())
        .unwrap()
        .style_classes(&["progress"], attrs())
        .unwrap()
        .style_attribute("lang", "en-US", attrs())
        .unwrap();
    let states = [ConstructionState::default(); 3];
    let source = ConstructionSource::new(
        c.construction(),
        &states,
        DocumentLanguage::Unknown,
        source_limits(),
    )
    .unwrap();
    assert!(matches(
        &source,
        0,
        "column.progress > progress_bar:first-child:lang(en)"
    ));
    assert!(matches(
        &source,
        1,
        "column.progress > progress_bar:first-child + progress_bar:lang(en)"
    ));
}

#[test]
fn event_opaque_retains_descendant_selectors() {
    let child =
        adapters::button::<(), iced::Theme, (), _>(adapters::space(limits()).unwrap(), limits())
            .unwrap();
    let c = adapters::opaque(child, limits())
        .unwrap()
        .style_id("shield", attrs())
        .unwrap()
        .style_attribute("lang", "en-US", attrs())
        .unwrap();
    let states = [ConstructionState::default(); 3];
    let source = ConstructionSource::new(
        c.construction(),
        &states,
        DocumentLanguage::Unknown,
        source_limits(),
    )
    .unwrap();
    assert!(matches(
        &source,
        1,
        "opaque#shield > button:first-child:lang(en)"
    ));
    assert!(matches(
        &source,
        0,
        "opaque#shield > button > space:first-child:lang(en)"
    ));
}

#[test]
fn slider_orientations_keep_distinct_canonical_types() {
    let h = adapters::slider::<u8, _, iced::Theme>(0..=100, 25, |v| v, limits())
        .unwrap()
        .erase::<u8, iced::Theme, ()>();
    let v = adapters::vertical_slider::<u8, _, iced::Theme>(0..=100, 25, |v| v, limits())
        .unwrap()
        .erase::<u8, iced::Theme, ()>();
    let c = adapters::column_from_vec(vec![h, v], limits())
        .unwrap()
        .style_attribute("lang", "en-US", attrs())
        .unwrap();
    let states = [ConstructionState::default(); 3];
    let source = ConstructionSource::new(
        c.construction(),
        &states,
        DocumentLanguage::Unknown,
        source_limits(),
    )
    .unwrap();
    assert!(matches(&source, 0, "column > slider:first-child:lang(en)"));
    assert!(matches(
        &source,
        1,
        "column > slider + vertical_slider:lang(en)"
    ));
}

#[test]
fn checkbox_labels_preserve_control_selectors_without_synthetic_text_children() {
    let first = adapters::checkbox::<bool, iced::Theme, ()>(true, limits())
        .unwrap()
        .style_classes(&["choice"], attrs())
        .unwrap()
        .label("old", limits())
        .unwrap()
        .label("new", limits())
        .unwrap()
        .erase();
    let second = adapters::checkbox::<bool, iced::Theme, ()>(false, limits())
        .unwrap()
        .erase();
    let column = adapters::column_from_vec(vec![first, second], limits())
        .unwrap()
        .style_attribute("lang", "en-US", attrs())
        .unwrap();
    let states = [ConstructionState::default(); 3];
    let source = ConstructionSource::new(
        column.construction(),
        &states,
        DocumentLanguage::Unknown,
        source_limits(),
    )
    .unwrap();
    assert!(matches(
        &source,
        0,
        "column > checkbox.choice:first-child:lang(en)"
    ));
    assert!(matches(&source, 1, "checkbox.choice + checkbox:lang(en)"));
    assert!(
        !column
            .construction()
            .nodes()
            .iter()
            .any(|node| node.parent() == Some(0))
    );
    assert_eq!(column.construction().nodes()[0].text(), Some("new"));
    assert_eq!(column.construction().nodes()[1].text(), None);
}

#[test]
fn radio_label_stays_on_the_control_source_before_its_authored_sibling() {
    let radio =
        adapters::radio::<(), iced::Theme, (), _>("internal label", 1, None, |_| (), limits())
            .unwrap()
            .style_classes(&["choice"], attrs())
            .unwrap()
            .erase();
    let column = adapters::column_from_vec(vec![radio, text()], limits()).unwrap();
    let states = [ConstructionState::default(); 3];
    let source = ConstructionSource::new(
        column.construction(),
        &states,
        DocumentLanguage::Unknown,
        source_limits(),
    )
    .unwrap();
    assert!(matches(&source, 0, "column > radio.choice:first-child"));
    assert!(matches(&source, 1, "radio.choice + text"));
    assert_eq!(
        column.construction().nodes()[0].text(),
        Some("internal label")
    );
    assert!(
        !column
            .construction()
            .nodes()
            .iter()
            .any(|node| node.parent() == Some(0))
    );
}

#[test]
fn optional_control_label_edits_keep_distinct_checkbox_and_toggler_types() {
    let checkbox = adapters::checkbox::<bool, iced::Theme, ()>(false, limits())
        .unwrap()
        .label("checkbox", limits())
        .unwrap()
        .erase();
    let toggler = adapters::toggler::<bool, iced::Theme, ()>(true, limits())
        .unwrap()
        .style_classes(&["choice"], attrs())
        .unwrap()
        .label("first", limits())
        .unwrap()
        .label("replacement", limits())
        .unwrap()
        .erase();
    let column = adapters::column_from_vec(vec![checkbox, toggler], limits()).unwrap();
    let states = [ConstructionState::default(); 3];
    let source = ConstructionSource::new(
        column.construction(),
        &states,
        DocumentLanguage::Unknown,
        source_limits(),
    )
    .unwrap();
    assert!(matches(&source, 0, "column > checkbox:first-child"));
    assert!(matches(&source, 1, "checkbox + toggler.choice"));
    assert_eq!(column.construction().nodes()[0].text(), Some("checkbox"));
    assert_eq!(column.construction().nodes()[1].text(), Some("replacement"));
    assert!(
        !column
            .construction()
            .nodes()
            .iter()
            .any(|node| node.parent() == Some(1))
    );
}

#[test]
fn empty_pick_list_still_precedes_its_authored_source_sibling() {
    let pick =
        adapters::pick_list::<u8, _, _, (), iced::Theme, ()>([], None::<u8>, |_| (), limits())
            .unwrap()
            .style_classes(&["empty"], attrs())
            .unwrap()
            .erase();
    let column = adapters::column_from_vec(vec![pick, text()], limits()).unwrap();
    let states = [ConstructionState::default(); 3];
    let source = ConstructionSource::new(
        column.construction(),
        &states,
        DocumentLanguage::Unknown,
        source_limits(),
    )
    .unwrap();
    assert!(matches(&source, 0, "column > pick_list.empty:first-child"));
    assert!(matches(&source, 1, "pick_list + text"));
    assert_eq!(column.construction().nodes()[0].text(), None);
    assert!(
        !column
            .construction()
            .nodes()
            .iter()
            .any(|node| node.parent() == Some(0))
    );
}

#[test]
fn pane_controls_exposes_full_then_compact_to_selectors() {
    let controls = adapters::pane_controls_dynamic(
        text().style_classes(&["full"], attrs()).unwrap(),
        text().style_classes(&["compact"], attrs()).unwrap(),
        limits(),
    )
    .unwrap()
    .style_classes(&["tools"], attrs())
    .unwrap();
    let states = [ConstructionState::default(); 3];
    let source = ConstructionSource::new(
        controls.construction(),
        &states,
        DocumentLanguage::Unknown,
        source_limits(),
    )
    .unwrap();
    assert!(matches(
        &source,
        0,
        "pane_controls.tools > text.full:first-child"
    ));
    assert!(matches(&source, 1, "pane_controls > .full + text.compact"));
    assert!(!matches(&source, 1, "pane_controls > text:first-child"));
}

#[test]
fn pane_title_precedes_body_after_replacement_and_preserves_parent_classes() {
    let title = adapters::pane_title(text(), limits())
        .unwrap()
        .style_classes(&["heading"], attrs())
        .unwrap()
        .controls(adapters::pane_controls(text(), limits()).unwrap(), limits())
        .unwrap();
    let pane = adapters::pane_content(adapters::container(text(), limits()).unwrap(), limits())
        .unwrap()
        .style_classes(&["pane"], attrs())
        .unwrap()
        .title_bar(adapters::pane_title(text(), limits()).unwrap(), limits())
        .unwrap()
        .title_bar(title, limits())
        .unwrap();
    let states = vec![ConstructionState::default(); pane.construction().nodes().len()];
    let source = ConstructionSource::new(
        pane.construction(),
        &states,
        DocumentLanguage::Unknown,
        source_limits(),
    )
    .unwrap();
    assert!(matches(
        &source,
        3,
        "pane_content.pane > pane_title.heading:first-child"
    ));
    assert!(matches(&source, 5, "pane_title + container"));
    assert!(matches(&source, 0, "pane_title > text:first-child"));
    assert!(matches(&source, 2, "pane_title > text + pane_controls"));
    assert!(matches(&source, 1, "pane_controls > text:first-child"));
}
