use iced_css_core::syntax::{Closure, StringLimits, parse_import_prelude};
use std::num::NonZeroUsize;

fn limits() -> StringLimits {
    StringLimits {
        work: NonZeroUsize::new(100_000).unwrap(),
        retained_bytes: NonZeroUsize::new(4096).unwrap(),
    }
}

#[test]
fn strings_and_uri_tokens_supply_the_same_unresolved_location() {
    for input in [
        r#""../theme.css""#,
        "url(../theme.css)",
        r#"/**/URL('../theme.css')/**/all"#,
    ] {
        let parsed = parse_import_prelude(input, limits()).unwrap();
        assert_eq!(parsed.location.text, "../theme.css");
        assert!(parsed.applies_to_screen);
        assert_eq!(parsed.location.closure, Closure::Explicit);
        assert_eq!(
            parsed.location_span.end - parsed.location_span.start,
            parsed.location.consumed
        );
    }
    let parsed = parse_import_prelude(r#""a\20 b.css" print, screen"#, limits()).unwrap();
    assert_eq!(parsed.location.text, "a b.css");
    assert!(parsed.applies_to_screen);
    assert!(
        !parse_import_prelude("url(x) print", limits())
            .unwrap()
            .applies_to_screen
    );
}

#[test]
fn invalid_location_or_media_cannot_be_accepted_even_when_nonmatching() {
    for input in [
        "",
        "theme.css",
        "url('x' bad) print",
        "url(x) screen,3D",
        "url(x) screen,",
        "url(x);",
        "url(x) {}",
        r#""\0" print"#,
    ] {
        assert!(parse_import_prelude(input, limits()).is_err(), "{input}");
    }
}

#[test]
fn eof_location_closure_and_repairs_retain_source_coordinates() {
    for input in ["  'x", "  url('x", "  url(x"] {
        let parsed = parse_import_prelude(input, limits()).unwrap();
        assert_eq!(parsed.location.text, "x");
        assert_eq!(parsed.location.closure, Closure::EndOfInput);
        assert_eq!(parsed.location_span, 2..input.len());
    }
    let parsed = parse_import_prelude(r#"  "\110000" screen"#, limits()).unwrap();
    assert_eq!(parsed.location.text, "\u{fffd}");
    let repair = &parsed.location.repairs[0];
    assert_eq!(
        &r#"  "\110000" screen"#
            [parsed.location_span.start + repair.start..parsed.location_span.start + repair.end],
        r"\110000"
    );
}

#[test]
fn import_uses_one_work_budget_and_bounds_decoded_location_storage() {
    let mut small = limits();
    small.work = NonZeroUsize::new(40).unwrap();
    assert!(parse_import_prelude(&format!("url(x){}", " /**/".repeat(100)), small).is_err());
    small = limits();
    small.retained_bytes = NonZeroUsize::new(1).unwrap();
    assert!(parse_import_prelude("url(long-location) print", small).is_err());
}
