use super::*;
use iced_css_core::formatting::bfc_height::{SignedNormalMarginEdge, SignedNormalMarginError};

#[test]
fn snv_explicit_height_does_not_publish_unused_signed_edges() {
    for explicit in [true, false] {
        let parent_height = if explicit {
            "height:1px;"
        } else {
            "height:auto;"
        };
        let css = format!(
            "#o,#p,#b1,#f,#b2 {{display:block;margin:0;padding:0;clear:none;}} \
            #o {{width:100px;}} #p {{overflow:hidden;padding-top:1px;padding-bottom:1px;{parent_height}}} \
            #b1 {{height:1px;}} #f {{float:left;width:10px;height:0px;}} \
            #b2 {{height:1px;margin-top:-340282346638528859811704183484516925440px; \
                margin-bottom:-340282346638528859811704183484516925440px;}}"
        );
        sibling_fixture(5, &css, |projection, display| {
            let facts =
                evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
            let result = build_captured_sibling_layout(
                &facts,
                ClearanceOuterContentGeometry {
                    outer: &projection.nodes()[0],
                    content_left: 0.,
                    content_width: 100.,
                    content_top: -1.,
                },
                layout_limits(),
            );
            if explicit {
                let layout = result.unwrap();
                assert_eq!(
                    layout.parent().height(),
                    BfcHeightPlacement {
                        used_height: 1.,
                        content_bottom: 1.,
                        auto_extent: None
                    }
                );
                let tail = &layout.children()[2];
                assert_eq!(
                    (tail.border().top, tail.border().bottom),
                    (-f32::MAX, -f32::MAX)
                );
                assert_eq!(tail.used_margin_bottom(), -f32::MAX);
            } else {
                let error = result.unwrap_err();
                assert!(matches!(
                    error,
                    SiblingLayoutError::NormalMargin {
                        source: 4,
                        error: SignedNormalMarginError::Unrepresentable {
                            edge: SignedNormalMarginEdge::Bottom
                        }
                    }
                ));
                assert!(std::error::Error::source(&error).is_some());
            }
        });
    }
}

#[test]
fn snv_source_order_and_later_profile_precede_parent_edges() {
    // B2 publishes [-MAX,-MAX]; its bottom margin edge overflows. B3 also
    // publishes [-MAX,-MAX], and its positive top margin overflows its top edge.
    // The earlier source's bottom must win over the later source's top.
    let max = "340282346638528859811704183484516925440";
    for invalid_later_height in [false, true] {
        let later_height = if invalid_later_height { "0" } else { "1" };
        let css = format!(
            "#o,#p,#b1,#f,#b2,#b3 {{display:block;margin:0;padding:0;clear:none;}} \
             #o {{width:100px;}} #p {{overflow:hidden;padding-top:1px;padding-bottom:1px;}} \
             #b1 {{height:1px;}} #f {{float:left;width:10px;height:0px;}} \
             #b2 {{height:1px;margin-top:-{max}px;margin-bottom:-{max}px;}} \
             #b3 {{height:{later_height}px;margin-top:{max}px;}}"
        );
        sibling_fixture(6, &css, |projection, display| {
            let facts =
                evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
            let error = build_captured_sibling_layout(
                &facts,
                ClearanceOuterContentGeometry {
                    outer: &projection.nodes()[0],
                    content_left: 0.,
                    content_width: 100.,
                    content_top: -1.,
                },
                layout_limits(),
            )
            .unwrap_err();
            if invalid_later_height {
                assert!(matches!(
                    error,
                    SiblingLayoutError::Core(ClearanceSequenceError::Profile(
                        ClearanceOwnerError::Unsupported {
                            source: 5,
                            reason: OwnerProfileReason::NonpositiveBlockHeight
                        }
                    ))
                ));
            } else {
                assert!(matches!(
                    error,
                    SiblingLayoutError::NormalMargin {
                        source: 4,
                        error: SignedNormalMarginError::Unrepresentable {
                            edge: SignedNormalMarginEdge::Bottom
                        }
                    }
                ));
                assert_eq!(
                    std::error::Error::source(&error)
                        .unwrap()
                        .downcast_ref::<SignedNormalMarginError>(),
                    Some(&SignedNormalMarginError::Unrepresentable {
                        edge: SignedNormalMarginEdge::Bottom
                    })
                );
            }
        });
    }
}

#[test]
fn snv_nonnegative_negative_zero_and_auto_keep_legacy_height_errors() {
    use iced_css_core::formatting::bfc_height::{BfcHeightError, BfcHeightStage};
    // A nonnegative last margin overflows only at the predecessor height
    // primitive. Signed edge evaluation would expose a different error type.
    let max = "340282346638528859811704183484516925440";
    for zero_margin in ["0px", "-0px", "auto"] {
        let css = format!(
            "#o,#p,#b1,#f,#b2 {{display:block;margin:0;padding:0;clear:none;}} \
             #o {{width:100px;}} #p {{overflow:hidden;padding-top:1px;padding-bottom:1px;}} \
             #b1 {{height:1px;margin-top:{zero_margin};}} #f {{float:left;width:10px;height:0px;}} \
             #b2 {{height:1px;margin-top:{max}px;margin-bottom:{max}px;}}"
        );
        sibling_fixture(5, &css, |projection, display| {
            let facts =
                evaluate_sibling_box_facts(projection, display, sibling_fact_limits()).unwrap();
            let error = build_captured_sibling_layout(
                &facts,
                ClearanceOuterContentGeometry {
                    outer: &projection.nodes()[0],
                    content_left: 0.,
                    content_width: 100.,
                    content_top: -1.,
                },
                layout_limits(),
            )
            .unwrap_err();
            assert!(matches!(
                error,
                SiblingLayoutError::Height(BfcHeightError::Unrepresentable {
                    stage: BfcHeightStage::NormalMarginBottom
                })
            ));
            assert_eq!(
                std::error::Error::source(&error)
                    .unwrap()
                    .downcast_ref::<BfcHeightError>(),
                Some(&BfcHeightError::Unrepresentable {
                    stage: BfcHeightStage::NormalMarginBottom
                })
            );
        });
    }
}
