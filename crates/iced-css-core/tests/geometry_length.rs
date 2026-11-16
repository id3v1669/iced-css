use iced_css_core::values::{LengthTarget, NumericLimits, geometry::*};
use std::num::NonZeroUsize;
fn limits() -> NumericLimits {
    NumericLimits {
        digits: NonZeroUsize::new(1000).unwrap(),
        work: NonZeroUsize::new(1000000).unwrap(),
    }
}
fn evaluate(css: &str, context: LengthContext) -> Result<LengthEvaluation, LengthEvaluationError> {
    evaluate_length(
        &LengthTarget::parse(css, limits()).unwrap(),
        context,
        limits(),
    )
}
fn number(css: &str, context: LengthContext) -> f32 {
    let LengthEvaluation::Value(value) = evaluate(css, context).unwrap() else {
        panic!()
    };
    value.value
}

#[test]
fn evaluates_only_the_explicitly_required_context_and_preserves_auto() {
    let missing = LengthContext::default();
    assert_eq!(evaluate("auto", missing).unwrap(), LengthEvaluation::Auto);
    assert_eq!(number("-1in", missing), -96.0);
    let context = LengthContext {
        font_size: Some(20.0),
        x_height: Some(8.0),
        percentage_basis: Some(200.0),
        root_font_size: None,
    };
    assert_eq!(number("-1em", context), -20.0);
    assert_eq!(number("2ex", context), 16.0);
    assert_eq!(number("25%", context), 50.0);
    assert_eq!(
        number(
            "25%",
            LengthContext {
                percentage_basis: Some(-200.0),
                ..missing
            }
        ),
        -50.0
    );
    for (css, kind) in [
        ("1em", LengthBasis::FontSize),
        ("1ex", LengthBasis::XHeight),
        ("1%", LengthBasis::Percentage),
    ] {
        assert_eq!(
            evaluate(css, missing),
            Err(LengthEvaluationError::MissingContext(kind))
        );
    }
    assert_eq!(
        number(
            "1px",
            LengthContext {
                font_size: Some(f32::NAN),
                ..missing
            }
        ),
        1.0
    );
    assert!(matches!(
        evaluate(
            "1em",
            LengthContext {
                font_size: Some(-1.0),
                ..missing
            }
        ),
        Err(LengthEvaluationError::InvalidFontContext)
    ));
}

#[test]
fn resolved_negative_and_percentage_margins_feed_the_block_equation() {
    use iced_css_core::formatting::block::*;
    let context = LengthContext {
        font_size: Some(20.0),
        x_height: None,
        percentage_basis: Some(200.0),
        root_font_size: None,
    };
    let input = BlockWidthInput {
        containing_width: 200.0,
        direction: Direction::Ltr,
        width: AutoLength::Auto,
        margin_left: AutoLength::Value(number("-1em", context)),
        margin_right: AutoLength::Value(number("25%", context)),
        border_left: 0.0,
        border_right: 0.0,
        padding_left: 0.0,
        padding_right: 0.0,
        min_width: 0.0,
        max_width: None,
    };
    let result = solve_block_width(input).unwrap();
    assert_eq!(
        (result.width, result.margin_left, result.margin_right),
        (170.0, -20.0, 50.0)
    );
}
