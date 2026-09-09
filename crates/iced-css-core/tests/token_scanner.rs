use iced_css_core::syntax::*;
use std::num::NonZeroUsize;
fn scan(input: &str) -> Token<'_> {
    scan_token(input, NonZeroUsize::new(100000).unwrap())
        .unwrap()
        .unwrap()
}
#[test]
fn retained_uri_boundaries_preserve_following_declarations() {
    use TokenKind::*;
    for (input, kind, len) in [
        (r"url(a)", Uri, 6),
        (r"url(a\\)", Uri, 8),
        (r"url(a\\);color:red;}", Uri, 8),
        (r"url(a\)b);color:red", Uri, 9),
        ("url(a b);color:red", BadUri, 6),
        ("url(\"a\n);color:red", BadUri, 6),
        ("url(a\\\n);color:red", BadUri, 5),
        ("URL(a)", Uri, 6),
        (r"\75rl(a)", Uri, 8),
    ] {
        let actual = scan(input);
        assert_eq!((actual.kind, actual.raw.len()), (kind, len), "{input:?}");
    }
}
#[test]
fn logical_eof_closure_does_not_erase_invalidity() {
    use TokenKind::*;
    for (input, kind, implicit) in [
        ("'unfinished", String, true),
        ("'a\\", String, true),
        ("'bad\nrest", BadString, false),
        ("url(a", Uri, true),
        ("url('a", Uri, true),
        ("url('a' ", Uri, true),
        ("url(a b", BadUri, false),
        ("url(a\\", BadUri, false),
        ("/*\\", Comment, true),
        ("/*x*/", Comment, false),
        ("\\", Delim, false),
    ] {
        let actual = scan(input);
        assert_eq!(
            (actual.kind, actual.implicit_close),
            (kind, implicit),
            "{input:?}"
        );
    }
}
#[test]
fn chapter_four_tokens_and_identifier_boundaries() {
    use TokenKind::*;
    for (input, kind, raw) in [
        ("red-->", Ident, "red--"),
        ("-->", Cdc, "-->"),
        ("<!--x", Cdo, "<!--"),
        ("@media ", AtKeyword, "@media"),
        ("#123;", Hash, "#123"),
        ("f(", Function, "f("),
        ("~=", Includes, "~="),
        ("|=", DashMatch, "|="),
        (" \r\n\t\u{c}x", Space, " \r\n\t\u{c}"),
        ("\u{7f}", Delim, "\u{7f}"),
        ("\u{80}", Ident, "\u{80}"),
        ("\0", Ident, "\0"),
        ("1e2px", Dimension, "1e2px"),
        ("1e2", Number, "1e2"),
        (".5%", Percentage, ".5%"),
        ("--x", Delim, "-"),
        ("'x';y", String, "'x'"),
    ] {
        let actual = scan(input);
        assert_eq!((actual.kind, actual.raw), (kind, raw), "{input:?}");
    }
    for (input, kind) in [
        (":", Colon),
        (";", Semicolon),
        ("{", OpenBrace),
        ("}", CloseBrace),
        ("(", OpenParen),
        (")", CloseParen),
        ("[", OpenBracket),
        ("]", CloseBracket),
    ] {
        assert_eq!(scan(input).kind, kind);
    }
}
#[test]
fn unicode_ranges_follow_the_lexical_six_digit_boundaries() {
    for (input, raw) in [
        ("u+???????", "u+??????"),
        ("U+1234567", "U+123456"),
        ("u+ab??x", "u+ab??"),
        ("u+123-456xyz", "u+123-456"),
        ("u+123-", "u+123"),
    ] {
        let actual = scan(input);
        assert_eq!((actual.kind, actual.raw), (TokenKind::UnicodeRange, raw));
    }
    assert_eq!(scan("u+").kind, TokenKind::Ident);
}
#[test]
fn comments_and_strings_hide_internal_delimiters() {
    for input in ["/*;{}*/after", "'a;{b}';after", "url(a;b);after"] {
        let value = scan(input);
        assert!(matches!(
            value.kind,
            TokenKind::Comment | TokenKind::String | TokenKind::Uri
        ));
        assert!(input[value.raw.len()..].ends_with("after"));
    }
    assert!(scan_token("", NonZeroUsize::MIN).unwrap().is_none());
    assert!(matches!(
        scan_token("long_identifier", NonZeroUsize::MIN),
        Err(SyntaxError {
            fault: SyntaxFault::Resource(SyntaxResource::Work),
            ..
        })
    ));
}

#[test]
fn token_stream_preserves_all_bytes_and_fuses_after_global_resource_error() {
    let input = "é; url(a\\\\); 'x' /**/";
    let tokens: Vec<_> = Tokens::new(input, NonZeroUsize::new(10000).unwrap())
        .map(Result::unwrap)
        .collect();
    assert_eq!(
        tokens.iter().map(|token| token.raw).collect::<String>(),
        input
    );
    let input = format!("é; {}", "a".repeat(1000));
    let mut tokens = Tokens::new(&input, NonZeroUsize::new(80).unwrap());
    assert_eq!(tokens.next().unwrap().unwrap().raw, "é");
    assert_eq!(tokens.next().unwrap().unwrap().raw, ";");
    assert_eq!(tokens.next().unwrap().unwrap().raw, " ");
    let error = tokens.next().unwrap().unwrap_err();
    assert!(error.offset >= 4);
    assert!(error.offset <= input.len());
    assert_eq!(tokens.consumed(), 4);
    assert!(tokens.next().is_none());
    assert!(tokens.next().is_none());
}
