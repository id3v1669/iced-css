use iced_css_core::resources::{
    http_admission::{
        GetResponseKind, StylesheetMediaError, StylesheetMediaPolicy, admit_stylesheet_media,
        classify_get_response,
    },
    http_metadata::{ContentTypeLimits, parse_content_type_fields},
};
use std::num::NonZeroUsize;
#[test]
fn get_response_classification_preserves_absent_partial_and_validation_distinctions() {
    use GetResponseKind::*;
    for (status, expected) in [
        (0, Invalid),
        (99, Invalid),
        (100, Informational),
        (199, Informational),
        (200, Complete),
        (201, Created),
        (202, Accepted),
        (203, Transformed),
        (204, NoContent),
        (205, ResetContent),
        (206, PartialContent),
        (207, UnrecognizedSuccess),
        (299, UnrecognizedSuccess),
        (300, Redirection),
        (304, NotModified),
        (399, Redirection),
        (400, ClientError),
        (471, ClientError),
        (499, ClientError),
        (500, ServerError),
        (599, ServerError),
        (600, Invalid),
        (u16::MAX, Invalid),
    ] {
        assert_eq!(classify_get_response(status), expected, "status {status}");
        assert_eq!(
            expected.has_complete_representation(),
            matches!(status, 200 | 203 | 207 | 299)
        );
    }
}
#[test]
fn stylesheet_media_admission_requires_explicit_absence_policy_without_sniffing() {
    let n = NonZeroUsize::new(10000).unwrap();
    let limits = ContentTypeLimits {
        parameters: n,
        retained_bytes: n,
        work: n,
    };
    for policy in [
        StylesheetMediaPolicy::RequireCss,
        StylesheetMediaPolicy::AllowAbsent,
    ] {
        let css = parse_content_type_fields(&[b"TeXt/CsS; charset=\"UTF-8\""], limits)
            .unwrap()
            .unwrap();
        assert_eq!(admit_stylesheet_media(Some(&css), policy), Ok(()));
        assert_eq!(css.charset(), Some(b"UTF-8".as_slice()));
        let other = parse_content_type_fields(&[b"text/plain"], limits)
            .unwrap()
            .unwrap();
        assert_eq!(
            admit_stylesheet_media(Some(&other), policy),
            Err(StylesheetMediaError::Other)
        );
    }
    assert_eq!(
        admit_stylesheet_media(None, StylesheetMediaPolicy::RequireCss),
        Err(StylesheetMediaError::Missing)
    );
    assert_eq!(
        admit_stylesheet_media(None, StylesheetMediaPolicy::AllowAbsent),
        Ok(())
    );
}
