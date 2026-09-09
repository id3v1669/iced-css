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

use iced_css_core::{resources::uri::UriError, stylesheet::requests::*};
fn requests_limits() -> ImportRequestLimits {
    ImportRequestLimits {
        entries: n(),
        retained_bytes: n(),
        work: n(),
    }
}
#[test]
fn applicable_imports_use_effective_stylesheet_base_and_fragment_free_acquisition() {
    let css = "@import '../a.css?x=1#part' screen; @import 'é.css' print; @import '?v=2'; @import '../a.css?x=1#part';";
    let sheet = parse_stylesheet(css, context(), sheet_limits()).unwrap();
    let requests = resolve_import_requests(
        &sheet,
        "https://host/new/css/main.css?old#base",
        requests_limits(),
    )
    .unwrap();
    assert_eq!(
        requests.iter().map(|r| r.entry).collect::<Vec<_>>(),
        [0, 2, 3]
    );
    assert_eq!(
        requests[0].reference_uri(),
        "https://host/new/a.css?x=1#part"
    );
    assert_eq!(requests[0].acquisition_uri(), "https://host/new/a.css?x=1");
    assert_eq!(
        requests[1].acquisition_uri(),
        "https://host/new/css/main.css?v=2"
    );
    assert_eq!(requests[0].reference_uri(), requests[2].reference_uri());
    assert_eq!(
        &css[requests[0].location_span.clone()],
        "'../a.css?x=1#part'"
    );
}
#[test]
fn uri_limitations_remain_located_errors_and_inactive_imports_need_no_base() {
    let css = "@import 'é.css';";
    let sheet = parse_stylesheet(css, context(), sheet_limits()).unwrap();
    let Err(ImportRequestError::Uri {
        entry,
        location,
        error: UriError::NonAscii,
    }) = resolve_import_requests(&sheet, "https://host/base.css", requests_limits())
    else {
        panic!()
    };
    assert_eq!(entry, 0);
    assert_eq!(&css[location], "'é.css'");
    let sheet = parse_stylesheet("@import 'x' print;", context(), sheet_limits()).unwrap();
    assert!(
        resolve_import_requests(&sheet, "not-an-absolute-base", requests_limits())
            .unwrap()
            .is_empty()
    );
    let sheet = parse_stylesheet("@import 'x';", context(), sheet_limits()).unwrap();
    assert!(matches!(
        resolve_import_requests(&sheet, "relative", requests_limits()),
        Err(ImportRequestError::Uri {
            error: UriError::RelativeBase,
            ..
        })
    ));
}
#[test]
fn requests_share_work_and_capacity_limits_without_partial_success() {
    let one = parse_stylesheet("@import 'a';", context(), sheet_limits()).unwrap();
    let many = parse_stylesheet(&"@import 'a';".repeat(50), context(), sheet_limits()).unwrap();
    let mut limits = requests_limits();
    limits.work = NonZeroUsize::new(5000).unwrap();
    assert!(resolve_import_requests(&one, "https://host/a.css", limits).is_ok());
    assert!(resolve_import_requests(&many, "https://host/a.css", limits).is_err());
    limits = requests_limits();
    limits.entries = NonZeroUsize::MIN;
    assert!(matches!(
        resolve_import_requests(&many, "https://host/a.css", limits),
        Err(ImportRequestError::Budget(RuleAssemblyError::Entries))
    ));
    limits = requests_limits();
    limits.retained_bytes = NonZeroUsize::MIN;
    assert!(matches!(
        resolve_import_requests(&one, "https://host/a.css", limits),
        Err(ImportRequestError::Budget(RuleAssemblyError::Bytes))
    ));
}
