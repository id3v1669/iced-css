use iced_css_core::syntax::{StatementContext, import_placement::*};
#[test]
fn only_a_non_ignored_other_statement_closes_the_window() {
    let mut placement = ImportPlacement::new(StatementContext::Stylesheet);
    for disposition in [
        StatementDisposition::Ignored,
        StatementDisposition::Charset,
        StatementDisposition::Import,
        StatementDisposition::Ignored,
        StatementDisposition::Import,
    ] {
        let decision = placement.observe(disposition);
        assert_eq!(
            decision,
            if disposition == StatementDisposition::Import {
                PlacementDecision::Allowed
            } else {
                PlacementDecision::NotImport
            }
        );
    }
    assert_eq!(
        placement.observe(StatementDisposition::Other),
        PlacementDecision::NotImport
    );
    assert_eq!(
        placement.observe(StatementDisposition::Import),
        PlacementDecision::Late
    );
    placement.observe(StatementDisposition::Charset);
    placement.observe(StatementDisposition::Ignored);
    assert_eq!(
        placement.observe(StatementDisposition::Import),
        PlacementDecision::Late
    );
}
#[test]
fn nested_imports_are_never_admitted_and_new_sources_have_their_own_window() {
    let mut nested = ImportPlacement::new(StatementContext::Block);
    assert_eq!(
        nested.observe(StatementDisposition::Import),
        PlacementDecision::Nested
    );
    nested.observe(StatementDisposition::Other);
    assert_eq!(
        nested.observe(StatementDisposition::Import),
        PlacementDecision::Nested
    );
    let mut first_source = ImportPlacement::new(StatementContext::Stylesheet);
    first_source.observe(StatementDisposition::Other);
    let mut imported_source = ImportPlacement::new(StatementContext::Stylesheet);
    assert_eq!(
        imported_source.observe(StatementDisposition::Import),
        PlacementDecision::Allowed
    );
    assert_eq!(
        first_source.observe(StatementDisposition::Import),
        PlacementDecision::Late
    );
}
