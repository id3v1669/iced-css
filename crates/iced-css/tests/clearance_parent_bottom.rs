#[path = "support/clearance_fixture.rs"]
mod support;
use iced_css::evaluation::clearance_bridge::*;
use iced_css_core::formatting::block_bottom::*;
use support::*;
const CSS: &str = "#o, #p, #b1, #f, #b2 {display:block; margin:0; padding:0; border-style:none;} #p {overflow:hidden; padding-top:1px; padding-bottom:1px;} #b1 {height:1px; margin-bottom:4px;} #f {float:left; width:10px; height:2px;} #b2 {height:1px; margin-top:3px; clear:both;}";
fn fixture(
    css: &str,
    check: impl FnOnce(
        &ClearanceSourceProjection<'_>,
        &iced_css::evaluation::source_display::SourceDisplay<'_, '_, '_>,
    ),
) {
    let host = captured_shape(true);
    let snapshot = host.snapshot().unwrap();
    let projection = prepare_clearance_source_projection(
        &host,
        snapshot,
        ClearanceProjectionLimits {
            work: nz(36),
            retained_bytes: bound(),
        },
    )
    .unwrap();
    let ua = catalog();
    let graph = graph(css, context(100, 100));
    let sheets = controls(&[1], true, 100);
    let bindings = [binding(&graph, GraphOrigin::Author(sheet(1)))];
    let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, bundle_limits()).unwrap();
    let display = evaluate_source_display(snapshot, &bundle, display_limits()).unwrap();
    check(&projection, &display);
}

fn outer<'p>(
    facts: &ClearanceBoxFacts<'p, '_, '_, '_, '_, '_>,
) -> ClearanceOuterContentGeometry<'p> {
    ClearanceOuterContentGeometry {
        outer: &facts.projection().nodes()[0],
        content_left: 0.,
        content_width: 100.,
        content_top: 10.,
    }
}
#[test]
fn actual_css_edges_signed_margins_and_once_counted_child_extent() {
    for (suffix, edges, height) in [
        ("", [20., 20., 20.], 8.),
        ("#p {margin-bottom:4px;}", [20., 20., 24.], 8.),
        ("#p {margin-bottom:-4px;}", [20., 20., 16.], 8.),
        ("#p {margin-bottom:-20px;}", [20., 20., 0.], 8.),
        (
            "#p {padding-bottom:3px; margin-bottom:4px;}",
            [22., 22., 26.],
            8.,
        ),
        ("#p {height:2px; margin-bottom:4px;}", [14., 14., 18.], 2.),
        (
            "#b2 {margin-bottom:3px;} #p {margin-bottom:4px;}",
            [23., 23., 27.],
            11.,
        ),
        ("#p {border-style:hidden;}", [20., 20., 20.], 8.),
    ] {
        fixture(&format!("{CSS} {suffix}"), |p, d| {
            let facts = evaluate_clearance_box_facts(p, d, fact_limits()).unwrap();
            let got = build_captured_clearance_with_parent_bottom(&facts, outer(&facts)).unwrap();
            let b = got.bottom();
            assert_eq!(
                [b.padding_bottom, b.border_bottom, b.margin_bottom].map(f32::to_bits),
                edges.map(f32::to_bits)
            );
            assert_eq!(got.parent().height().used_height, height);
            assert_eq!(
                got.computed_margin_bottom(),
                facts.row(1).unwrap().margin[2]
            );
            assert!(std::ptr::eq(
                got.parent().parent().owner().sources(),
                p.nodes()
            ));
        });
    }
}
#[test]
fn actual_computed_provenance_auto_fixed_initial_inherited_and_underflow() {
    let initial = "#o,#p,#b1,#f,#b2 {display:block;padding:0;border-style:none;} #o,#b1,#f,#b2 {margin:0;} #p {overflow:hidden;margin-top:0;margin-right:0;margin-left:0;padding-top:1px;padding-bottom:1px;} #b1 {height:1px;margin-bottom:4px;} #f {float:left;width:10px;height:2px;} #b2 {height:1px;margin-top:3px;clear:both;}";
    for (css, pixels, origin, rounded) in [
        (CSS.to_owned(), 0., BoxFactOrigin::Specified, false),
        (initial.to_owned(), 0., BoxFactOrigin::Initial, false),
        (
            format!("{CSS} #p {{margin-bottom:-0px;}}"),
            0.,
            BoxFactOrigin::Specified,
            false,
        ),
        (
            format!("{CSS} #o {{margin-bottom:4px;}} #p {{margin-bottom:inherit;}}"),
            4.,
            BoxFactOrigin::Inherited { original_parent: 4 },
            false,
        ),
        (
            format!(
                "{CSS} #p {{margin-bottom:0.000000000000000000000000000000000000000000000000001px;}}"
            ),
            0.,
            BoxFactOrigin::Specified,
            true,
        ),
    ] {
        fixture(&css, |p, d| {
            let facts = evaluate_clearance_box_facts(p, d, fact_limits()).unwrap();
            let got = build_captured_clearance_with_parent_bottom(&facts, outer(&facts)).unwrap();
            assert_eq!(
                got.computed_margin_bottom(),
                BoxLengthFact::Absolute(AbsoluteBoxFact {
                    pixels,
                    origin,
                    rounded_to_zero: rounded
                })
            );
            assert_eq!(
                got.computed_margin_bottom(),
                facts.row(1).unwrap().margin[2]
            );
            assert_eq!(got.bottom().used_margin_bottom.to_bits(), pixels.to_bits());
            assert_eq!(
                got.bottom().margin_resolution,
                BottomMarginResolution::Absolute
            );
            assert_eq!(got.bottom().margin_bottom, 20. + pixels);
            let mut copied = got.bottom();
            copied.margin_bottom = 99.;
            assert_ne!(copied, got.bottom());
        });
    }
    fixture(&format!("{CSS} #p {{margin-bottom:auto;}}"), |p, d| {
        let facts = evaluate_clearance_box_facts(p, d, fact_limits()).unwrap();
        let got = build_captured_clearance_with_parent_bottom(&facts, outer(&facts)).unwrap();
        assert_eq!(
            got.computed_margin_bottom(),
            BoxLengthFact::Auto {
                origin: BoxFactOrigin::Specified
            }
        );
        assert_eq!(
            got.bottom().margin_resolution,
            BottomMarginResolution::AutoZero
        );
        assert_eq!(got.bottom().used_margin_bottom.to_bits(), 0);
        assert_eq!(got.bottom().margin_bottom, 20.);
    });
}
#[test]
fn bottom_overflow_preserves_reusable_predecessor_and_typed_error_sources() {
    use std::error::Error;
    const MAX: &str = "340282346638528859811704183484516925440px";
    for (suffix, stage) in [
        (
            format!("padding-bottom:{MAX};margin-bottom:-{MAX};"),
            BlockBottomStage::PaddingBottom,
        ),
        (
            format!("margin-bottom:{MAX};"),
            BlockBottomStage::MarginBottom,
        ),
    ] {
        fixture(&format!("{CSS} #p {{height:{MAX};{suffix}}}"), |p, d| {
            let facts = evaluate_clearance_box_facts(p, d, fact_limits()).unwrap();
            let prior = build_captured_clearance_with_parent_height(&facts, outer(&facts)).unwrap();
            assert_eq!(prior.height().content_bottom, f32::MAX);
            let cause = BlockBottomError::Unrepresentable { stage };
            let error =
                build_captured_clearance_with_parent_bottom(&facts, outer(&facts)).unwrap_err();
            assert_eq!(error, ParentBottomError::Bottom(cause));
            assert_eq!(
                error.source().unwrap().downcast_ref::<BlockBottomError>(),
                Some(&cause)
            );
            assert!(build_captured_clearance_with_parent_height(&facts, outer(&facts)).is_ok());
        });
    }
}
#[test]
fn previous_errors_remain_wrapped_before_bottom_work() {
    use std::error::Error;
    for suffix in [
        "#p {overflow:visible;}",
        "#b2 {margin-bottom:-1px;}",
        "#b2 {margin-top:6px;}",
    ] {
        fixture(&format!("{CSS} {suffix}"), |p, d| {
            let facts = evaluate_clearance_box_facts(p, d, fact_limits()).unwrap();
            let prior =
                build_captured_clearance_with_parent_height(&facts, outer(&facts)).unwrap_err();
            let got =
                build_captured_clearance_with_parent_bottom(&facts, outer(&facts)).unwrap_err();
            assert_eq!(got, ParentBottomError::Parent(prior));
            assert_eq!(
                got.source().unwrap().downcast_ref::<ParentHeightError>(),
                Some(&prior)
            );
        });
    }
    fixture(CSS, |p, d| {
        let facts = evaluate_clearance_box_facts(p, d, fact_limits()).unwrap();
        fixture(CSS, |other, _| {
            let mut geometry = outer(&facts);
            geometry.outer = &other.nodes()[0];
            geometry.content_top = f32::NAN;
            assert_eq!(
                build_captured_clearance_with_parent_bottom(&facts, geometry).unwrap_err(),
                ParentBottomError::Parent(ParentHeightError::Parent(
                    ParentClearanceError::OuterGeometrySource
                ))
            );
        });
    });
    support::fixture(CSS, |p, d| {
        let facts = evaluate_clearance_box_facts(p, d, fact_limits()).unwrap();
        assert_eq!(
            build_captured_clearance_with_parent_bottom(&facts, outer(&facts)).unwrap_err(),
            ParentBottomError::Parent(ParentHeightError::Parent(ParentClearanceError::Captured(
                CapturedClearanceError::Shape
            )))
        );
    });
}

#[test]
fn solid_border_is_rejected_by_predecessor_fact_producer() {
    fixture(&format!("{CSS} #p {{border-style:solid;}}"), |p, d| {
        assert!(matches!(
            evaluate_clearance_box_facts(p, d, fact_limits()),
            Err(ClearanceBoxFactsError::Unsupported {
                reason: BoxFactUnsupported::BorderStyle,
                ..
            })
        ));
    });
}
