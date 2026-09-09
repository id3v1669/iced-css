use iced_css_core::{
    selectors::SelectorLimits,
    stylesheet::*,
    syntax::{RecoveryLimits, StringLimits},
    values::{NumericLimits, color::ColorLimits},
};
use std::num::{NonZeroU64, NonZeroUsize};
fn n() -> NonZeroUsize {
    NonZeroUsize::new(100000).unwrap()
}
fn limits() -> RuleAssemblyLimits {
    RuleAssemblyLimits {
        recovery: RecoveryLimits {
            work: n(),
            retained_bytes: n(),
            nesting: n(),
        },
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
    }
}
fn context() -> RuleContext {
    RuleContext {
        first_order: 10,
        first_declaration: NonZeroU64::new(20).unwrap(),
    }
}

fn sheet_limits() -> StylesheetLimits {
    StylesheetLimits {
        rules: limits(),
        statements: n(),
        record_bytes: n(),
        work: n(),
    }
}
#[test]
fn typed_sheet_preserves_import_media_page_and_original_declaration_coordinates() {
    let css = "<!-- @charset \"UTF-8\"; @import \"theme.css\" screen; @media print {a{color:red} @import \"bad.css\"; b{width:2px}} @page:left{margin:1cm} .x{height:3px} -->";
    let sheet = parse_stylesheet(css, context(), sheet_limits()).unwrap();
    assert_eq!(sheet.entries.len(), 4);
    let SheetEntryKind::Import(import) = &sheet.entries[0].kind else {
        panic!()
    };
    assert_eq!(import.location.text, "theme.css");
    assert!(import.applies_to_screen);
    assert_eq!(&css[import.location_span.clone()], "\"theme.css\"");
    let SheetEntryKind::Media(media) = &sheet.entries[1].kind else {
        panic!()
    };
    assert!(!media.applies_to_screen);
    assert_eq!(media.rules.len(), 2);
    assert_eq!(&css[media.rules[1].locations[0].value.clone()], "2px");
    assert_eq!(media.rules[1].declarations[0].order, 11);
    assert!(matches!(sheet.entries[2].kind, SheetEntryKind::Page(_)));
    let SheetEntryKind::Style(style) = &sheet.entries[3].kind else {
        panic!()
    };
    assert_eq!(style.declarations[0].order, 13);
    assert_eq!(&css[style.locations[0].value.clone()], "3px");
    assert_eq!(sheet.next_context.unwrap().first_order, 14);
    assert_eq!(
        sheet.rejected.iter().map(|r| r.reason).collect::<Vec<_>>(),
        [
            SheetRejectionReason::Charset,
            SheetRejectionReason::NestedAtRule
        ]
    );
}
#[test]
fn only_nonignored_rules_close_the_initial_import_window() {
    let css = "@unknown{x{}} :bad{font-weight:+100} @media screen, 3D {} @page:bad{} @import \"a\"; .empty{} @import \"late\";";
    let sheet = parse_stylesheet(css, context(), sheet_limits()).unwrap();
    assert_eq!(sheet.entries.len(), 2);
    assert!(matches!(sheet.entries[0].kind, SheetEntryKind::Import(_)));
    assert!(matches!(sheet.entries[1].kind, SheetEntryKind::Style(_)));
    assert_eq!(
        sheet.rejected.last().unwrap().reason,
        SheetRejectionReason::LateImport
    );
    for rule in ["@media unknown {}", "@page{}", ".unmatched{}"] {
        let sheet = parse_stylesheet(
            &format!("{rule} @import \"late\";"),
            context(),
            sheet_limits(),
        )
        .unwrap();
        assert_eq!(sheet.entries.len(), 1);
        assert_eq!(sheet.rejected[0].reason, SheetRejectionReason::LateImport);
    }
}
#[test]
fn nested_at_rules_do_not_leak_children_and_invalid_imports_are_recovered() {
    let css = "@import x; @import \"x\" {} @import \"ok\" unknown; @media screen{@media screen{a{color:red}} @page{margin:0} b{color:blue}}";
    let sheet = parse_stylesheet(css, context(), sheet_limits()).unwrap();
    assert_eq!(sheet.entries.len(), 2);
    let SheetEntryKind::Import(import) = &sheet.entries[0].kind else {
        panic!()
    };
    assert!(!import.applies_to_screen);
    let SheetEntryKind::Media(media) = &sheet.entries[1].kind else {
        panic!()
    };
    assert!(media.applies_to_screen);
    assert_eq!(media.rules.len(), 1);
    assert_eq!(media.rules[0].declarations[0].order, 10);
    assert_eq!(sheet.rejected.len(), 4);
}
#[test]
fn bounded_candidate_errors_never_return_a_partial_sheet() {
    let mut bounded = sheet_limits();
    bounded.statements = NonZeroUsize::MIN;
    assert!(matches!(
        parse_stylesheet("@media screen{a{color:red}}", context(), bounded),
        Err(RuleAssemblyError::Entries)
    ));
    bounded = sheet_limits();
    bounded.record_bytes = NonZeroUsize::MIN;
    assert!(matches!(
        parse_stylesheet("a{}", context(), bounded),
        Err(RuleAssemblyError::Bytes)
    ));
    assert!(matches!(
        parse_stylesheet("a{color:red}b{font-weight:+100}", context(), sheet_limits()),
        Err(RuleAssemblyError::Property { .. })
    ));
    let sheet = parse_stylesheet("@media screen{a{color:red", context(), sheet_limits()).unwrap();
    let SheetEntryKind::Media(media) = &sheet.entries[0].kind else {
        panic!()
    };
    assert!(media.rules[0].implicit_close);
}

#[test]
fn local_screen_rules_preserve_order_without_import_or_page_leakage() {
    let sheet = parse_stylesheet("@import \"x\"; a{width:1px} @media print{a{height:9px}} @page{margin:0} @media screen{a{color:red} a{display:block}}",context(),sheet_limits()).unwrap();
    let properties: Vec<_> = sheet
        .local_screen_rules()
        .flat_map(|rule| {
            rule.declarations
                .iter()
                .map(|d| d.parsed.property.descriptor().name)
        })
        .collect();
    assert_eq!(properties, ["width", "color", "display"]);
}
