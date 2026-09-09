use iced_css_core::formatting::quotes::*;
use std::num::NonZeroUsize;
fn pairs() -> [[String; 2]; 2] {
    [
        ["outer-open".into(), "outer-close".into()],
        ["inner-open".into(), "inner-close".into()],
    ]
}
#[test]
fn opening_and_closing_select_before_and_after_depth_changes() {
    let pairs = pairs();
    let quotes = QuotePairs::new(&pairs);
    let mut state = QuoteDepth::new(NonZeroUsize::new(10).unwrap());
    assert_eq!(state.apply(QuoteOperation::Close, quotes).unwrap(), None);
    for expected in ["outer-open", "inner-open", "inner-open"] {
        assert_eq!(
            state.apply(QuoteOperation::Open, quotes).unwrap(),
            Some(expected)
        );
    }
    for expected in ["inner-close", "inner-close", "outer-close"] {
        assert_eq!(
            state.apply(QuoteOperation::Close, quotes).unwrap(),
            Some(expected)
        );
    }
    assert_eq!(state.depth(), 0);
    assert_eq!(state.apply(QuoteOperation::NoClose, quotes).unwrap(), None);
    assert_eq!(state.depth(), 0);
}
#[test]
fn none_and_no_quote_operations_still_update_depth() {
    let pairs = pairs();
    let quotes = QuotePairs::new(&pairs);
    let mut state = QuoteDepth::new(NonZeroUsize::new(10).unwrap());
    assert_eq!(state.apply(QuoteOperation::NoOpen, quotes).unwrap(), None);
    assert_eq!(state.depth(), 1);
    assert_eq!(
        state
            .apply(QuoteOperation::Open, QuotePairs::none())
            .unwrap(),
        None
    );
    assert_eq!(state.depth(), 2);
    assert_eq!(
        state
            .apply(QuoteOperation::NoClose, QuotePairs::none())
            .unwrap(),
        None
    );
    assert_eq!(
        state.apply(QuoteOperation::Close, quotes).unwrap(),
        Some("outer-close")
    );
    assert_eq!(state.depth(), 0);
}
#[test]
fn depth_limits_reject_atomically_and_empty_marks_stay_distinct_from_none() {
    let pairs = [[String::new(), String::new()]];
    let quotes = QuotePairs::new(&pairs);
    let mut state = QuoteDepth::new(NonZeroUsize::MIN);
    assert_eq!(state.apply(QuoteOperation::Open, quotes).unwrap(), Some(""));
    for operation in [QuoteOperation::Open, QuoteOperation::NoOpen] {
        assert_eq!(state.apply(operation, quotes), Err(QuoteError::Depth));
        assert_eq!(state.depth(), 1);
    }
    assert_eq!(
        state.apply(QuoteOperation::Close, quotes).unwrap(),
        Some("")
    );
    let empty = QuotePairs::new(&[]);
    assert_eq!(state.apply(QuoteOperation::Open, empty).unwrap(), None);
    assert_eq!(state.depth(), 1);
}
