use iced_css_core::{
    formatting::{
        block::AutoLength,
        clearance::ClearSide,
        clearance_owner::*,
        display::{Display, DisplayResolution, Float, Flow, Position},
        floats::{Band, FloatError, FloatSide},
    },
    identity::IdAllocator,
    source::{Metadata, SourceNode},
};
use std::num::NonZeroU64;
fn sources() -> [SourceNode; 4] {
    let mut ids = IdAllocator::new(NonZeroU64::new(4).unwrap());
    let mut nodes = std::array::from_fn(|_| SourceNode {
        instance: ids.allocate().unwrap(),
        parent: None,
        children: vec![],
        metadata: Metadata::default(),
    });
    nodes[0].children = nodes[1..].iter().map(|n| n.instance).collect();
    for i in 1..4 {
        nodes[i].parent = Some(nodes[0].instance);
    }
    nodes
}
fn edges() -> OwnerEdges {
    OwnerEdges {
        top: 0.,
        right: 0.,
        bottom: 0.,
        left: 0.,
    }
}
fn display() -> DisplayResolution {
    DisplayResolution {
        display: Display::Block,
        position: Position::Static,
        float: Float::None,
        flow: Flow::Normal,
    }
}
fn input(s: &[SourceNode]) -> ClearanceOwnerInput<'_> {
    let child = |index| ChildFacts {
        source: &s[index],
        display: display(),
        context: ChildContext::OrdinaryNonBfc,
        content: OwnerContent::NoOwnContentNonReplaced,
        generated: OwnerGenerated::None,
        // Explicit asserts positive computed CSS height and used height after min/max.
        height: OwnerHeight::Explicit(1.),
        margin: edges().into(),
        border: edges(),
        padding: edges(),
        clear: ClearSide::None,
    };
    let mut result = ClearanceOwnerInput {
        parent: ParentFacts {
            source: &s[0],
            display: display(),
            content: OwnerContent::NoOwnContentNonReplaced,
            generated: OwnerGenerated::None,
            bfc: ParentBfc::NewWithEmptyPrefix,
            content_top: 0.,
            containing: Band {
                left: 0.,
                right: 100.,
            },
            border: OwnerEdges {
                top: 1.,
                bottom: 1.,
                ..edges()
            },
            padding: edges(),
        },
        before: child(1),
        float: child(2),
        after: child(3),
        float_width: 10.,
    };
    result.before.margin.bottom = 4.;
    result.float.display.float = Float::Left;
    result.float.display.flow = Flow::Floating;
    result.float.context = ChildContext::FloatBfc;
    result.float.height = OwnerHeight::Explicit(2.);
    result.after.margin.top = 3.;
    result.after.clear = ClearSide::Both;
    result
}
#[test]
fn negative_clearance_retains_real_source_membership_and_barrier() {
    let s = sources();
    let owner = {
        let facts = input(&s);
        build_clearance_owner(&s, facts).unwrap()
    };
    assert!(std::ptr::eq(owner.sources(), s.as_slice()));
    for (i, n) in s.iter().enumerate() {
        assert!(std::ptr::eq(owner.source(i).unwrap(), n));
    }
    assert!(owner.source(4).is_none());
    assert_eq!(
        owner.before(),
        BlockVertical {
            top: 0.,
            bottom: 1.
        }
    );
    assert_eq!(
        (owner.placed_float().top, owner.placed_float().bottom),
        (5., 7.)
    );
    assert_eq!(
        owner.after(),
        BlockVertical {
            top: 7.,
            bottom: 8.
        }
    );
    assert_eq!(owner.hypothetical_top(), 5.);
    assert_eq!(owner.zero_clearance_top(), Some(8.));
    assert_eq!(owner.clearance(), Some(-1.));
    let pair = [
        MarginMember {
            source_index: 1,
            side: MarginSide::Bottom,
            value: 4.,
        },
        MarginMember {
            source_index: 3,
            side: MarginSide::Top,
            value: 3.,
        },
    ];
    assert_eq!(
        owner.hypothetical_group().id(),
        MarginGroupId::HypotheticalPair
    );
    assert_eq!(owner.hypothetical_group().members(), pair);
    assert_eq!(
        (
            owner.hypothetical_group().positive(),
            owner.hypothetical_group().negative()
        ),
        (4., 0.)
    );
    assert_eq!(owner.actual_groups().len(), 2);
    assert_eq!(owner.barrier_source_index(), Some(3));
    for (i, id) in [MarginGroupId::BeforeBarrier, MarginGroupId::AfterBarrier]
        .into_iter()
        .enumerate()
    {
        assert_eq!(owner.actual_groups()[i].id(), id);
        assert_eq!(owner.actual_groups()[i].members(), &pair[i..i + 1]);
    }
}
#[test]
fn signed_amounts_and_absence_preserve_the_right_groups() {
    let s = sources();
    for (m2, clear, amount, top) in [
        (2., ClearSide::Both, Some(0.), 7.),
        (1., ClearSide::Both, Some(1.), 7.),
        (3., ClearSide::None, None, 5.),
        (3., ClearSide::Right, None, 5.),
        (7., ClearSide::Both, None, 8.),
    ] {
        let mut f = input(&s);
        f.after.margin.top = m2;
        f.after.clear = clear;
        let o = build_clearance_owner(&s, f).unwrap();
        assert_eq!(o.clearance(), amount);
        assert_eq!(o.after().top, top);
        if amount.is_some() {
            assert_eq!(o.actual_groups().len(), 2);
            assert_eq!(o.barrier_source_index(), Some(3));
            assert_eq!(o.zero_clearance_top(), Some(5. + m2));
        } else {
            assert_eq!(o.actual_groups().len(), 1);
            assert_eq!(o.actual_groups()[0].id(), MarginGroupId::ActualPair);
            assert_eq!(
                o.actual_groups()[0].members(),
                o.hypothetical_group().members()
            );
            assert_eq!(o.zero_clearance_top(), None);
            assert_eq!(o.barrier_source_index(), None);
        }
    }
    let mut f = input(&s);
    f.after.margin.top = 6.;
    assert_eq!(
        build_clearance_owner(&s, f).unwrap_err(),
        ClearanceOwnerError::UnsupportedEquality
    );
}
#[test]
fn contexts_are_required_even_without_content_and_clear() {
    let s = sources();
    for i in 1..4 {
        for context in [
            ChildContext::OrdinaryNonBfc,
            ChildContext::FloatBfc,
            ChildContext::OtherNewBfc,
            ChildContext::Unknown,
        ] {
            if context
                == if i == 2 {
                    ChildContext::FloatBfc
                } else {
                    ChildContext::OrdinaryNonBfc
                }
            {
                continue;
            }
            let mut f = input(&s);
            f.after.clear = ClearSide::None;
            match i {
                1 => f.before.context = context,
                2 => f.float.context = context,
                _ => f.after.context = context,
            };
            assert_eq!(
                build_clearance_owner(&s, f).unwrap_err(),
                ClearanceOwnerError::Unsupported {
                    source: i,
                    reason: OwnerProfileReason::ChildContext
                }
            );
        }
    }
}
#[test]
fn complete_display_tuples_precede_context_admission() {
    let s = sources();
    for i in 0..4 {
        for field in 0..4 {
            let mut f = input(&s);
            f.after.context = ChildContext::Unknown;
            let d = match i {
                0 => &mut f.parent.display,
                1 => &mut f.before.display,
                2 => &mut f.float.display,
                _ => &mut f.after.display,
            };
            match field {
                0 => d.display = Display::Inline,
                1 => d.position = Position::Relative,
                2 => d.float = if i == 2 { Float::None } else { Float::Left },
                _ => d.flow = if i == 2 { Flow::Normal } else { Flow::Floating },
            };
            assert_eq!(
                build_clearance_owner(&s, f).unwrap_err(),
                ClearanceOwnerError::Unsupported {
                    source: i,
                    reason: OwnerProfileReason::DisplayOrFlow
                }
            );
        }
    }
}
#[test]
fn shape_and_pointer_checks_are_bounded_and_ordered() {
    let s = sources();
    let huge = vec![s[0].clone(); 10_000];
    assert_eq!(
        build_clearance_owner(&huge, input(&s)).unwrap_err(),
        ClearanceOwnerError::NodeCount
    );
    for i in 0..4 {
        let mut bad = s.clone();
        bad[i].children = vec![s[0].instance; 10_000];
        assert_eq!(
            build_clearance_owner(&bad, input(&s)).unwrap_err(),
            ClearanceOwnerError::ChildCount { source: i }
        );
    }
    for i in 0..4 {
        let mut f = input(&s);
        let foreign = s[i].clone();
        match i {
            0 => f.parent.source = &foreign,
            1 => f.before.source = &foreign,
            2 => f.float.source = &foreign,
            _ => f.after.source = &foreign,
        };
        assert_eq!(
            build_clearance_owner(&s, f).unwrap_err(),
            ClearanceOwnerError::SourcePointer { source: i }
        );
    }
    let mut f = input(&s);
    f.before.source = &s[2];
    assert_eq!(
        build_clearance_owner(&s, f).unwrap_err(),
        ClearanceOwnerError::SourcePointer { source: 1 }
    );
    for i in 0..4 {
        let mut bad = s.clone();
        bad[i].parent = if i == 0 { Some(s[1].instance) } else { None };
        assert_eq!(
            build_clearance_owner(&bad, input(&bad)).unwrap_err(),
            ClearanceOwnerError::ParentMembership { source: i }
        );
    }
    let mut bad = s.clone();
    bad[0].children.swap(0, 1);
    assert_eq!(
        build_clearance_owner(&bad, input(&bad)).unwrap_err(),
        ClearanceOwnerError::ChildMembership { source: 0, slot: 0 }
    );
    for first in 0..4 {
        for second in first + 1..4 {
            let mut bad = s.clone();
            bad[second].instance = bad[first].instance;
            assert_eq!(
                build_clearance_owner(&bad, input(&bad)).unwrap_err(),
                ClearanceOwnerError::DuplicateInstance { first, second }
            );
        }
    }
}
#[test]
fn profile_rejections_are_typed() {
    let s = sources();
    for i in 1..4 {
        for h in [
            OwnerHeight::Auto,
            OwnerHeight::Percentage(1.),
            OwnerHeight::Percentage(f32::NAN),
        ] {
            let mut f = input(&s);
            match i {
                1 => f.before.height = h,
                2 => f.float.height = h,
                _ => f.after.height = h,
            };
            assert_eq!(
                build_clearance_owner(&s, f).unwrap_err(),
                ClearanceOwnerError::Unsupported {
                    source: i,
                    reason: OwnerProfileReason::HeightKind
                }
            );
        }
    }
    for (case, source, reason) in [
        (0, 0, OwnerProfileReason::ParentContext),
        (1, 0, OwnerProfileReason::ParentIsolation),
        (2, 0, OwnerProfileReason::ParentIsolation),
        (3, 1, OwnerProfileReason::NonpositiveBlockHeight),
        (4, 3, OwnerProfileReason::NegativeMargin),
        (5, 1, OwnerProfileReason::HorizontalMargin),
        (6, 3, OwnerProfileReason::ChildDecoration),
        (7, 2, OwnerProfileReason::FloatMargin),
        (8, 1, OwnerProfileReason::ClearOnBeforeOrFloat),
        (9, 2, OwnerProfileReason::ClearOnBeforeOrFloat),
        (10, 2, OwnerProfileReason::FloatTooWide),
    ] {
        let mut f = input(&s);
        match case {
            0 => f.parent.bfc = ParentBfc::OtherOrUnknown,
            1 => f.parent.border.top = 0.,
            2 => f.parent.border.bottom = 0.,
            3 => f.before.height = OwnerHeight::Explicit(0.),
            4 => f.after.margin.top = -1.,
            5 => f.before.margin.left = AutoLength::Value(1.),
            6 => f.after.padding.top = 1.,
            7 => f.float.margin.bottom = 1.,
            8 => f.before.clear = ClearSide::Left,
            9 => f.float.clear = ClearSide::Both,
            _ => f.float_width = 101.,
        };
        assert_eq!(
            build_clearance_owner(&s, f).unwrap_err(),
            ClearanceOwnerError::Unsupported { source, reason }
        );
    }
    for i in 0..4 {
        for content in [
            OwnerContent::TextOrRichText,
            OwnerContent::Replaced,
            OwnerContent::RecipeRequired,
            OwnerContent::OpaqueOrUnknown,
        ] {
            let mut f = input(&s);
            match i {
                0 => f.parent.content = content,
                1 => f.before.content = content,
                2 => f.float.content = content,
                _ => f.after.content = content,
            };
            assert_eq!(
                build_clearance_owner(&s, f).unwrap_err(),
                ClearanceOwnerError::Unsupported {
                    source: i,
                    reason: OwnerProfileReason::Content
                }
            );
        }
        for g in [OwnerGenerated::Present, OwnerGenerated::Unresolved] {
            let mut f = input(&s);
            match i {
                0 => f.parent.generated = g,
                1 => f.before.generated = g,
                2 => f.float.generated = g,
                _ => f.after.generated = g,
            };
            assert_eq!(
                build_clearance_owner(&s, f).unwrap_err(),
                ClearanceOwnerError::Unsupported {
                    source: i,
                    reason: OwnerProfileReason::GeneratedContent
                }
            );
        }
    }
}
#[test]
fn stages_and_checked_float_producer_report_the_actual_failure() {
    let s = sources();
    for (case, stage) in [
        (0, "before_top"),
        (1, "before_bottom"),
        (2, "float_top"),
        (3, "hypothetical_top"),
        (4, "zero_clearance_top"),
        (5, "after_bottom"),
    ] {
        let mut f = input(&s);
        match case {
            0 => {
                f.parent.content_top = f32::MAX;
                f.before.margin.top = f32::MAX;
            }
            1 => {
                f.parent.content_top = f32::MAX;
                f.before.height = OwnerHeight::Explicit(f32::MAX);
            }
            2 => {
                f.parent.content_top = f32::MAX;
                f.before.margin.bottom = f32::MAX;
            }
            3 => {
                f.parent.content_top = f32::MAX;
                f.before.margin.bottom = 0.;
                f.float.height = OwnerHeight::Explicit(0.);
                f.after.margin.top = f32::MAX;
            }
            4 => {
                // Baseline overflows while strict H < F reaches required clearance.
                f.before.margin.bottom = f32::MAX * 0.75;
                f.after.margin.top = f32::MAX * 0.75;
                f.float.height = OwnerHeight::Explicit(f32::MAX * 0.125);
            }
            _ => {
                f.after.height = OwnerHeight::Explicit(f32::MAX);
                f.parent.content_top = f32::MAX / 2.;
                f.after.clear = ClearSide::None;
            }
        }
        assert_eq!(
            build_clearance_owner(&s, f).unwrap_err(),
            ClearanceOwnerError::Unrepresentable { stage }
        );
    }
    let mut f = input(&s);
    f.parent.content_top = f32::MAX;
    f.before.margin.bottom = 0.;
    f.float.height = OwnerHeight::Explicit(f32::MAX);
    assert_eq!(
        build_clearance_owner(&s, f).unwrap_err(),
        ClearanceOwnerError::FloatPlacement(FloatError::Unrepresentable)
    );
}
#[test]
fn degenerate_float_and_signed_origins_use_real_placement() {
    let s = sources();
    for (side, expected) in [
        (Float::Left, FloatSide::Left),
        (Float::Right, FloatSide::Right),
    ] {
        let mut f = input(&s);
        f.float.display.float = side;
        f.float_width = 0.;
        f.parent.content_top = -10.;
        let o = build_clearance_owner(&s, f).unwrap();
        assert_eq!(o.placed_float().side, expected);
        assert_eq!(o.placed_float().left, o.placed_float().right);
        assert_eq!(o.after().top, -3.);
        let mut f = input(&s);
        f.float.display.float = side;
        f.float.height = OwnerHeight::Explicit(0.);
        assert_eq!(
            build_clearance_owner(&s, f).unwrap_err(),
            ClearanceOwnerError::UnsupportedEquality
        );
    }
    let mut f = input(&s);
    f.before.margin.bottom = f32::MAX * 0.75;
    f.after.margin.top = f32::MAX * 0.75;
    f.float.height = OwnerHeight::Explicit(0.);
    f.after.clear = ClearSide::None;
    let o = build_clearance_owner(&s, f).unwrap();
    assert_eq!(o.zero_clearance_top(), None);
    assert_eq!(o.after().top, f32::MAX * 0.75);
    let mut f = input(&s);
    f.parent.containing = Band {
        left: -f32::MAX,
        right: f32::MAX,
    };
    f.float_width = f32::MAX;
    assert!(build_clearance_owner(&s, f).is_ok());
}
#[test]
fn invalid_measurements_are_rejected_by_source_and_field() {
    let s = sources();
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        for (case, source, field) in [
            (0, 0, "content_top"),
            (1, 0, "containing_left"),
            (2, 0, "containing_right"),
            (3, 1, "height"),
            (4, 2, "height"),
            (5, 3, "height"),
            (6, 2, "float_width"),
        ] {
            let mut f = input(&s);
            match case {
                0 => f.parent.content_top = bad,
                1 => f.parent.containing.left = bad,
                2 => f.parent.containing.right = bad,
                3 => f.before.height = OwnerHeight::Explicit(bad),
                4 => f.float.height = OwnerHeight::Explicit(bad),
                5 => f.after.height = OwnerHeight::Explicit(bad),
                _ => f.float_width = bad,
            };
            assert_eq!(
                build_clearance_owner(&s, f).unwrap_err(),
                ClearanceOwnerError::InvalidNumber { source, field }
            );
        }
        for source in 0..4 {
            for field in ["margin", "border", "padding"] {
                if source == 0 && field == "margin" {
                    continue;
                }
                for edge in 0..4 {
                    let mut f = input(&s);
                    if field == "margin" {
                        let c = match source {
                            1 => &mut f.before,
                            2 => &mut f.float,
                            _ => &mut f.after,
                        };
                        match edge {
                            0 => c.margin.top = bad,
                            1 => c.margin.right = AutoLength::Value(bad),
                            2 => c.margin.bottom = bad,
                            _ => c.margin.left = AutoLength::Value(bad),
                        }
                    } else {
                        let e = if source == 0 {
                            if field == "border" {
                                &mut f.parent.border
                            } else {
                                &mut f.parent.padding
                            }
                        } else {
                            let c = match source {
                                1 => &mut f.before,
                                2 => &mut f.float,
                                _ => &mut f.after,
                            };
                            if field == "border" {
                                &mut c.border
                            } else {
                                &mut c.padding
                            }
                        };
                        match edge {
                            0 => e.top = bad,
                            1 => e.right = bad,
                            2 => e.bottom = bad,
                            _ => e.left = bad,
                        };
                    }
                    assert_eq!(
                        build_clearance_owner(&s, f).unwrap_err(),
                        ClearanceOwnerError::InvalidNumber { source, field }
                    );
                }
            }
        }
    }
    for (case, source, field) in [
        (0, 0, "containing_order"),
        (1, 2, "float_width"),
        (2, 2, "height"),
        (3, 0, "border"),
        (4, 1, "padding"),
    ] {
        let mut f = input(&s);
        match case {
            0 => {
                f.parent.containing = Band {
                    left: 10.,
                    right: 0.,
                }
            }
            1 => f.float_width = -1.,
            2 => f.float.height = OwnerHeight::Explicit(-1.),
            3 => f.parent.border.left = -1.,
            _ => f.before.padding.bottom = -1.,
        };
        assert_eq!(
            build_clearance_owner(&s, f).unwrap_err(),
            ClearanceOwnerError::InvalidNumber { source, field }
        );
    }
    // Within an edge record finiteness precedes range, then profile checks.
    let mut f = input(&s);
    f.before.margin.top = -1.;
    f.before.margin.left = AutoLength::Value(f32::NAN);
    assert_eq!(
        build_clearance_owner(&s, f).unwrap_err(),
        ClearanceOwnerError::InvalidNumber {
            source: 1,
            field: "margin"
        }
    );
    let mut f = input(&s);
    f.parent.border.top = -1.;
    f.parent.border.right = f32::INFINITY;
    assert_eq!(
        build_clearance_owner(&s, f).unwrap_err(),
        ClearanceOwnerError::InvalidNumber {
            source: 0,
            field: "border"
        }
    );
}

#[test]
fn strict_owner_rejects_signed_horizontal_margins_for_both_normal_roles() {
    let s = sources();
    for source in [1, 3] {
        for edge in ["left", "right"] {
            for (value, reason) in [
                (1.0, OwnerProfileReason::HorizontalMargin),
                (-1.0, OwnerProfileReason::NegativeMargin),
            ] {
                let mut facts = input(&s);
                let child = if source == 1 {
                    &mut facts.before
                } else {
                    &mut facts.after
                };
                if edge == "left" {
                    child.margin.left = AutoLength::Value(value);
                } else {
                    child.margin.right = AutoLength::Value(value);
                }
                assert_eq!(
                    build_clearance_owner(&s, facts).unwrap_err(),
                    ClearanceOwnerError::Unsupported { source, reason }
                );
            }
        }
    }
}

#[derive(Clone, Copy)]
enum StrictAutoCase {
    LeftAuto,
    RightAuto,
    BothAuto,
    RightNumber(f32),
    NegativeTop,
    BorderLeft,
    NonpositiveHeight,
}

fn strict_auto_error(nested: bool, case: StrictAutoCase) -> ClearanceOwnerError {
    let flat_sources = sources();
    let nested_sources = nested_sources();
    let source_slice = if nested {
        &nested_sources[1..]
    } else {
        &flat_sources[..]
    };
    let mut facts = input(source_slice);
    match case {
        StrictAutoCase::LeftAuto => facts.before.margin.left = AutoLength::Auto,
        StrictAutoCase::RightAuto => facts.before.margin.right = AutoLength::Auto,
        StrictAutoCase::BothAuto => {
            facts.before.margin.left = AutoLength::Auto;
            facts.before.margin.right = AutoLength::Auto;
        }
        StrictAutoCase::RightNumber(value) => {
            facts.before.margin.left = AutoLength::Auto;
            facts.before.margin.right = AutoLength::Value(value);
        }
        StrictAutoCase::NegativeTop => {
            facts.before.margin.left = AutoLength::Auto;
            facts.before.margin.top = -1.0;
        }
        StrictAutoCase::BorderLeft => {
            facts.before.margin.left = AutoLength::Auto;
            facts.before.border.left = 1.0;
        }
        StrictAutoCase::NonpositiveHeight => {
            facts.before.margin.left = AutoLength::Auto;
            facts.before.height = OwnerHeight::Explicit(0.0);
        }
    }
    if nested {
        build_nested_clearance_owner(&nested_sources, outer(&nested_sources), facts).unwrap_err()
    } else {
        build_clearance_owner(&flat_sources, facts).unwrap_err()
    }
}

#[test]
fn strict_auto_margin_errors_preserve_numeric_precedence() {
    for nested in [false, true] {
        let source = if nested { 2 } else { 1 };
        for case in [
            StrictAutoCase::LeftAuto,
            StrictAutoCase::RightAuto,
            StrictAutoCase::BothAuto,
            StrictAutoCase::RightNumber(1.0),
            StrictAutoCase::BorderLeft,
        ] {
            assert_eq!(
                strict_auto_error(nested, case),
                ClearanceOwnerError::Unsupported {
                    source,
                    reason: OwnerProfileReason::AutoHorizontalMargin,
                }
            );
        }
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(
                strict_auto_error(nested, StrictAutoCase::RightNumber(bad)),
                ClearanceOwnerError::InvalidNumber {
                    source,
                    field: "margin",
                }
            );
        }
        for case in [
            StrictAutoCase::NegativeTop,
            StrictAutoCase::RightNumber(-1.0),
        ] {
            assert_eq!(
                strict_auto_error(nested, case),
                ClearanceOwnerError::Unsupported {
                    source,
                    reason: OwnerProfileReason::NegativeMargin,
                }
            );
        }
        assert_eq!(
            strict_auto_error(nested, StrictAutoCase::NonpositiveHeight),
            ClearanceOwnerError::Unsupported {
                source,
                reason: OwnerProfileReason::NonpositiveBlockHeight,
            }
        );

        let flat_sources = sources();
        let nested_sources = nested_sources();
        let source_slice = if nested {
            &nested_sources[1..]
        } else {
            &flat_sources[..]
        };
        let float_source = if nested { 3 } else { 2 };
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut facts = input(source_slice);
            facts.float.margin.left = AutoLength::Auto;
            facts.float.margin.right = AutoLength::Value(bad);
            let error = if nested {
                build_nested_clearance_owner(&nested_sources, outer(&nested_sources), facts)
                    .unwrap_err()
            } else {
                build_clearance_owner(&flat_sources, facts).unwrap_err()
            };
            assert_eq!(
                error,
                ClearanceOwnerError::InvalidNumber {
                    source: float_source,
                    field: "margin",
                }
            );
        }
        for (value, reason) in [
            (AutoLength::Auto, OwnerProfileReason::AutoHorizontalMargin),
            (AutoLength::Value(1.0), OwnerProfileReason::FloatMargin),
            (AutoLength::Value(-1.0), OwnerProfileReason::NegativeMargin),
        ] {
            let mut facts = input(source_slice);
            facts.float.margin.left = value;
            let error = if nested {
                build_nested_clearance_owner(&nested_sources, outer(&nested_sources), facts)
                    .unwrap_err()
            } else {
                build_clearance_owner(&flat_sources, facts).unwrap_err()
            };
            assert_eq!(
                error,
                ClearanceOwnerError::Unsupported {
                    source: float_source,
                    reason,
                }
            );
        }
    }
}
#[test]
fn exact_stages_preserve_subnormals_and_rounding_created_equality() {
    let s = sources();
    let tiny = f32::from_bits(1);
    let mut f = input(&s);
    f.before.height = OwnerHeight::Explicit(tiny);
    f.before.margin.bottom = tiny;
    f.float.height = OwnerHeight::Explicit(tiny);
    f.after.margin.top = 0.;
    f.after.height = OwnerHeight::Explicit(tiny);
    let o = build_clearance_owner(&s, f).unwrap();
    assert_eq!(o.before().bottom.to_bits(), 1);
    assert_eq!(o.hypothetical_top().to_bits(), 2);
    assert_eq!(o.zero_clearance_top().unwrap().to_bits(), 2);
    assert_eq!(o.after().top.to_bits(), 3);
    assert_eq!(o.after().bottom.to_bits(), 4);
    assert_eq!(o.clearance().unwrap().to_bits(), 1);
    // Each positive explicit used height is still positive; rounding it at a large
    // origin can make the published float bottom equal H. No epsilon breaks ties.
    let mut f = input(&s);
    f.parent.content_top = f32::MAX / 2.;
    assert_eq!(
        build_clearance_owner(&s, f).unwrap_err(),
        ClearanceOwnerError::UnsupportedEquality
    );
    let mut f = input(&s);
    f.parent.content_top = -0.;
    f.before.margin.top = -0.;
    f.after.clear = ClearSide::None;
    let o = build_clearance_owner(&s, f).unwrap();
    assert_eq!(o.before().top.to_bits(), 0);
}
#[test]
fn both_isolation_edges_and_role_independent_height_categories_are_checked() {
    let s = sources();
    let mut f = input(&s);
    f.parent.padding = f.parent.border;
    f.parent.border = edges();
    assert!(build_clearance_owner(&s, f).is_ok());
    for source in [1, 3] {
        for height in [0., -0., -1.] {
            let mut f = input(&s);
            if source == 1 {
                f.before.height = OwnerHeight::Explicit(height);
            } else {
                f.after.height = OwnerHeight::Explicit(height);
            }
            assert_eq!(
                build_clearance_owner(&s, f).unwrap_err(),
                ClearanceOwnerError::Unsupported {
                    source,
                    reason: OwnerProfileReason::NonpositiveBlockHeight
                }
            );
        }
    }
    // Independent registries issue equal numeric IDs; ownership is pointer-bound.
    let other = sources();
    let mut f = input(&s);
    f.after.source = &other[3];
    assert_eq!(other[3].instance, s[3].instance);
    assert_eq!(
        build_clearance_owner(&s, f).unwrap_err(),
        ClearanceOwnerError::SourcePointer { source: 3 }
    );
}

fn nested_sources() -> [SourceNode; 5] {
    let mut ids = IdAllocator::new(NonZeroU64::new(5).unwrap());
    let mut s = std::array::from_fn(|_| SourceNode {
        instance: ids.allocate().unwrap(),
        parent: None,
        children: vec![],
        metadata: Metadata::default(),
    });
    s[0].children = vec![s[1].instance];
    s[1].parent = Some(s[0].instance);
    s[1].children = s[2..].iter().map(|n| n.instance).collect();
    for i in 2..5 {
        s[i].parent = Some(s[1].instance);
    }
    s
}
fn outer(s: &[SourceNode]) -> OuterFacts<'_> {
    OuterFacts {
        source: &s[0],
        display: display(),
        content: OwnerContent::NoOwnContentNonReplaced,
        generated: OwnerGenerated::None,
    }
}
#[test]
fn nested_owner_retains_outer_and_maps_shared_solver_indices() {
    let nested = nested_sources();
    let root = sources();
    let a = build_clearance_owner(&root, input(&root)).unwrap();
    let b = build_nested_clearance_owner(&nested, outer(&nested), input(&nested[1..])).unwrap();
    assert!(std::ptr::eq(b.sources(), nested.as_slice()));
    assert_eq!(
        b.source_indices(),
        OwnerSourceIndices {
            parent: 1,
            before: 2,
            float: 3,
            after: 4
        }
    );
    assert_eq!(b.before(), a.before());
    assert_eq!(b.after(), a.after());
    assert_eq!(b.placed_float(), a.placed_float());
    assert_eq!(b.hypothetical_top(), a.hypothetical_top());
    assert_eq!(b.zero_clearance_top(), a.zero_clearance_top());
    assert_eq!(b.clearance(), a.clearance());
    assert_eq!(b.barrier_source_index(), Some(4));
    assert_eq!(b.hypothetical_group().members()[0].source_index, 2);
    assert_eq!(b.hypothetical_group().members()[1].source_index, 4);
    assert_eq!(b.sources()[1].parent, Some(b.sources()[0].instance));
    assert_eq!(
        build_clearance_owner(&nested, input(&nested[1..])).unwrap_err(),
        ClearanceOwnerError::NodeCount
    );
}

#[test]
fn nested_gate_reports_full_graph_indices_before_geometry() {
    let mut s = nested_sources();
    s[0].children[0] = s[2].instance;
    assert_eq!(
        build_nested_clearance_owner(&s, outer(&s), input(&s[1..])).unwrap_err(),
        ClearanceOwnerError::ChildMembership { source: 0, slot: 0 }
    );
    s[0].children[0] = s[1].instance;
    s[1].children.swap(0, 1);
    assert_eq!(
        build_nested_clearance_owner(&s, outer(&s), input(&s[1..])).unwrap_err(),
        ClearanceOwnerError::ChildMembership { source: 1, slot: 0 }
    );
    s[1].children.swap(0, 1);
    s[4].parent = Some(s[0].instance);
    assert_eq!(
        build_nested_clearance_owner(&s, outer(&s), input(&s[1..])).unwrap_err(),
        ClearanceOwnerError::ParentMembership { source: 4 }
    );
    s[4].parent = Some(s[1].instance);
    let foreign = nested_sources();
    let mut f = input(&s[1..]);
    f.after.source = &foreign[4];
    assert_eq!(
        build_nested_clearance_owner(&s, outer(&s), f).unwrap_err(),
        ClearanceOwnerError::SourcePointer { source: 4 }
    );
    let mut f = input(&s[1..]);
    f.float.height = OwnerHeight::Explicit(-1.);
    assert_eq!(
        build_nested_clearance_owner(&s, outer(&s), f).unwrap_err(),
        ClearanceOwnerError::InvalidNumber {
            source: 3,
            field: "height"
        }
    );
    let mut f = input(&s[1..]);
    f.parent.padding.top = -1.;
    assert_eq!(
        build_nested_clearance_owner(&s, outer(&s), f).unwrap_err(),
        ClearanceOwnerError::InvalidNumber {
            source: 1,
            field: "padding"
        }
    );
    for (display_value, content_value, generated, reason) in [
        (
            DisplayResolution {
                display: Display::None,
                ..display()
            },
            OwnerContent::NoOwnContentNonReplaced,
            OwnerGenerated::None,
            OwnerProfileReason::DisplayOrFlow,
        ),
        (
            display(),
            OwnerContent::TextOrRichText,
            OwnerGenerated::None,
            OwnerProfileReason::Content,
        ),
        (
            display(),
            OwnerContent::NoOwnContentNonReplaced,
            OwnerGenerated::Present,
            OwnerProfileReason::GeneratedContent,
        ),
    ] {
        let o = OuterFacts {
            source: &s[0],
            display: display_value,
            content: content_value,
            generated,
        };
        assert_eq!(
            build_nested_clearance_owner(&s, o, input(&s[1..])).unwrap_err(),
            ClearanceOwnerError::Unsupported { source: 0, reason }
        );
    }
    s[4].instance = s[3].instance;
    assert_eq!(
        build_nested_clearance_owner(&s, outer(&s), input(&s[1..])).unwrap_err(),
        ClearanceOwnerError::DuplicateInstance {
            first: 3,
            second: 4
        }
    );
    s[4].children.push(s[0].instance);
    assert_eq!(
        build_nested_clearance_owner(&s, outer(&s), input(&s[1..])).unwrap_err(),
        ClearanceOwnerError::ChildCount { source: 4 }
    );
}
