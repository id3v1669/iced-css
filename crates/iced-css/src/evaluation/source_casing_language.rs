//! Private complete RFC5646 ABNF recognizer. Registry validity is not implied.
use super::{SourceCasingLanguageStatus as Status, SourceCasingProfile as Profile};
const GRANDFATHERED: [&str; 26] = [
    "en-GB-oed",
    "i-ami",
    "i-bnn",
    "i-default",
    "i-enochian",
    "i-hak",
    "i-klingon",
    "i-lux",
    "i-mingo",
    "i-navajo",
    "i-pwn",
    "i-tao",
    "i-tay",
    "i-tsu",
    "sgn-BE-FR",
    "sgn-BE-NL",
    "sgn-CH-DE",
    "art-lojban",
    "cel-gaulish",
    "no-bok",
    "no-nyn",
    "zh-guoyu",
    "zh-hakka",
    "zh-min",
    "zh-min-nan",
    "zh-xiang",
];
#[derive(Clone, Copy)]
struct Token {
    len: u8,
    alpha: bool,
    digit: bool,
    alnum: bool,
    first: u8,
}
impl Token {
    fn new() -> Self {
        Self {
            len: 0,
            alpha: true,
            digit: true,
            alnum: true,
            first: 0,
        }
    }
    fn push(&mut self, b: u8) {
        if self.len == 0 {
            self.first = b.to_ascii_lowercase();
        }
        self.len = self.len.saturating_add(1).min(9);
        self.alpha &= b.is_ascii_alphabetic();
        self.digit &= b.is_ascii_digit();
        self.alnum &= b.is_ascii_alphanumeric();
    }
    fn private(self) -> bool {
        (1..=8).contains(&self.len) && self.alnum
    }
    fn variant(self) -> bool {
        self.alnum
            && ((5..=8).contains(&self.len) || (self.len == 4 && self.first.is_ascii_digit()))
    }
}
#[derive(Clone, Copy)]
enum State {
    Primary,
    Extlang(u8),
    Script,
    Region,
    Variant,
    ExtensionNeed,
    ExtensionTail,
    PrivateNeed,
    PrivateTail,
    Invalid,
}
impl State {
    fn token(self, t: Token) -> Self {
        use State::*;
        if !t.private() {
            return Invalid;
        }
        match self {
            Primary => {
                if t.len == 1 && t.first == b'x' {
                    PrivateNeed
                } else if t.alpha && (2..=8).contains(&t.len) {
                    if t.len <= 3 { Extlang(0) } else { Script }
                } else {
                    Invalid
                }
            }
            Extlang(n) => {
                if n < 3 && t.len == 3 && t.alpha {
                    Extlang(n + 1)
                } else {
                    Script.token(t)
                }
            }
            Script => {
                if t.len == 4 && t.alpha {
                    Region
                } else {
                    Region.token(t)
                }
            }
            Region => {
                if (t.len == 2 && t.alpha) || (t.len == 3 && t.digit) {
                    Variant
                } else {
                    Variant.token(t)
                }
            }
            Variant => {
                if t.variant() {
                    Variant
                } else if t.len == 1 {
                    if t.first == b'x' {
                        PrivateNeed
                    } else {
                        ExtensionNeed
                    }
                } else {
                    Invalid
                }
            }
            ExtensionNeed => {
                if t.len >= 2 {
                    ExtensionTail
                } else {
                    Invalid
                }
            }
            ExtensionTail => {
                if t.len >= 2 {
                    ExtensionTail
                } else if t.first == b'x' {
                    PrivateNeed
                } else {
                    ExtensionNeed
                }
            }
            PrivateNeed | PrivateTail => PrivateTail,
            Invalid => Invalid,
        }
    }
    fn accepted(self) -> bool {
        !matches!(
            self,
            Self::Primary | Self::ExtensionNeed | Self::PrivateNeed | Self::Invalid
        )
    }
}
/// Caller prepays all bytes, finalization1, and fixed grandfathered comparisons218.
/// No malformed prefix stops the byte scan. Underscore/non-ASCII are not repaired.
pub(super) fn classify(tag: &str) -> (Status, Profile) {
    let mut state = State::Primary;
    let mut token = Token::new();
    let mut prefix = [0; 3];
    for (i, b) in tag.bytes().enumerate() {
        if i < 3 {
            prefix[i] = b.to_ascii_lowercase();
        }
        if b == b'-' {
            state = state.token(token);
            token = Token::new();
        } else {
            token.push(b);
        }
    }
    state = state.token(token);
    let mut grandfathered = false;
    for literal in GRANDFATHERED {
        let matched = tag.eq_ignore_ascii_case(literal);
        grandfathered |= matched;
    }
    if !state.accepted() && !grandfathered {
        return (Status::Malformed, Profile::Default);
    }
    let profile = if tag.len() == 2 || prefix[2] == b'-' {
        match &prefix[..2] {
            b"lt" => Profile::Lithuanian,
            b"tr" => Profile::Turkish,
            b"az" => Profile::Azeri,
            _ => Profile::Default,
        }
    } else {
        Profile::Default
    };
    (Status::WellFormed, profile)
}
