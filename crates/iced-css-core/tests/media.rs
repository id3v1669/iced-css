use iced_css_core::syntax::{MediaListContext, StringLimits, media_matches_screen};
use std::num::NonZeroUsize;

fn limits() -> StringLimits {
    StringLimits {
        work: NonZeroUsize::new(100_000).unwrap(),
        retained_bytes: NonZeroUsize::new(4096).unwrap(),
    }
}

#[test]
fn css22_lists_match_decoded_case_insensitive_screen_or_all() {
    for (input, expected) in [
        ("screen", true),
        ("ALL", true),
        ("print, SCREEN", true),
        (r"\73 creen", true),
        ("print, future", false),
        ("future", false),
        (
            "braille, embossed, handheld, projection, speech, tty, tv",
            false,
        ),
        ("/**/screen/**/,/**/all/**/", true),
        ("screen,screen", true),
        (r"screen, \33 D", true),
    ] {
        assert_eq!(
            media_matches_screen(input, MediaListContext::MediaRule, limits()).unwrap(),
            expected,
            "{input}"
        );
    }
}

#[test]
fn malformed_member_invalidates_a_previously_matching_list() {
    for input in [
        "screen,3D",
        "screen,",
        ",screen",
        "screen,,print",
        "screen print",
        "screen and (color)",
        "\"screen\"",
        "screen;",
        "screen, url(x)",
        "screen, <!--",
        r"screen, \0",
        "scr/**/een",
    ] {
        assert!(
            media_matches_screen(input, MediaListContext::MediaRule, limits()).is_err(),
            "{input}"
        );
    }
}

#[test]
fn absent_import_media_is_unconditional_but_media_rule_requires_a_list() {
    for input in ["", " /**/ \n"] {
        assert!(media_matches_screen(input, MediaListContext::Import, limits()).unwrap());
        assert!(media_matches_screen(input, MediaListContext::MediaRule, limits()).is_err());
    }
    assert!(!media_matches_screen("unknown", MediaListContext::Import, limits()).unwrap());
}

#[test]
fn work_and_decoded_storage_are_bounded_even_after_a_match() {
    let mut bounded = limits();
    bounded.work = NonZeroUsize::new(50).unwrap();
    assert!(
        media_matches_screen(
            &format!("screen{}", " /**/".repeat(100)),
            MediaListContext::Import,
            bounded
        )
        .is_err()
    );
    bounded = limits();
    bounded.retained_bytes = NonZeroUsize::new(1).unwrap();
    assert!(media_matches_screen("screen", MediaListContext::Import, bounded).is_err());
}
