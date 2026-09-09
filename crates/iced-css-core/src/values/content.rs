use super::{
    ColorLimits, Keyword, PropertyError, Token, TokenKind, decode_identifier_budget, keyword_data,
    owned::{Storage, resource},
    significant,
};
use crate::syntax::{SyntaxFault, SyntaxResource, decode_string_budget, decode_uri_budget};
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentItem {
    Text(String),
    Uri(String),
    Attribute(String),
    Quote(Keyword),
    Counter {
        name: String,
        separator: Option<String>,
        style: Keyword,
    },
}
struct Parser<'a, 'b> {
    input: &'a str,
    offset: usize,
    storage: Storage,
    work: &'b mut usize,
}
impl<'a> Parser<'a, '_> {
    fn next(&mut self) -> Result<Option<(usize, Token<'a>)>, PropertyError> {
        significant(self.input, &mut self.offset, self.work)
    }
    fn text(
        &mut self,
        (start, token): (usize, Token<'_>),
    ) -> Result<Option<String>, PropertyError> {
        if token.invalid_escape {
            return Ok(None);
        }
        let available = self.storage.remaining(start)?;
        let decoded = match token.kind {
            TokenKind::String => {
                decode_string_budget(token.raw, available, self.work).map(|v| v.text)
            }
            TokenKind::Uri => decode_uri_budget(token.raw, available, self.work).map(|v| v.text),
            TokenKind::Ident => {
                decode_identifier_budget(token.raw, available, self.work).map(|v| v.text)
            }
            TokenKind::Function => {
                decode_identifier_budget(&token.raw[..token.raw.len() - 1], available, self.work)
                    .map(|v| v.text)
            }
            _ => return Ok(None),
        };
        match decoded {
            Ok(value) => {
                self.storage.used += value.capacity();
                Ok(Some(value))
            }
            Err(mut error) if matches!(error.fault, SyntaxFault::Resource(_)) => {
                error.offset += start;
                Err(PropertyError::Syntax(error))
            }
            Err(_) => Ok(None),
        }
    }
    fn argument(&mut self, kind: TokenKind) -> Result<Option<String>, PropertyError> {
        let Some(token) = self.next()?.filter(|(_, token)| token.kind == kind) else {
            return Ok(None);
        };
        self.text(token)
    }
    fn function(
        &mut self,
        token: (usize, Token<'_>),
    ) -> Result<Option<ContentItem>, PropertyError> {
        let Some(function) = self.text(token)? else {
            return Ok(None);
        };
        let attr = function.eq_ignore_ascii_case("attr");
        let sequence = function.eq_ignore_ascii_case("counters");
        let single = function.eq_ignore_ascii_case("counter");
        self.storage.used -= function.capacity();
        drop(function);
        if !attr && !sequence && !single {
            return Ok(None);
        }
        let Some(name) = self.argument(TokenKind::Ident)? else {
            return Ok(None);
        };
        let mut next = self.next()?;
        if attr {
            return Ok(closed(next).then_some(ContentItem::Attribute(name)));
        }
        if ["none", "inherit", "initial"]
            .iter()
            .any(|key| name.eq_ignore_ascii_case(key))
        {
            return Ok(None);
        }
        let separator = if sequence {
            if !comma(next) {
                return Ok(None);
            }
            let Some(value) = self.argument(TokenKind::String)? else {
                return Ok(None);
            };
            next = self.next()?;
            Some(value)
        } else {
            None
        };
        let style = if comma(next) {
            let Some(value) = self.argument(TokenKind::Ident)? else {
                return Ok(None);
            };
            let styles = keyword_data::KEYWORDS
                .iter()
                .find(|(name, _)| *name == "list-style-type")
                .expect("registered list styles")
                .1;
            let Some(style) = styles.iter().find(|key| key.eq_ignore_ascii_case(&value)) else {
                return Ok(None);
            };
            self.storage.used -= value.capacity();
            drop(value);
            next = self.next()?;
            Keyword(style)
        } else {
            Keyword("decimal")
        };
        Ok(closed(next).then_some(ContentItem::Counter {
            name,
            separator,
            style,
        }))
    }
}
fn comma(token: Option<(usize, Token<'_>)>) -> bool {
    token.is_some_and(|(_, token)| token.kind == TokenKind::Delim && token.raw == ",")
}
fn closed(token: Option<(usize, Token<'_>)>) -> bool {
    token.is_none_or(|(_, token)| token.kind == TokenKind::CloseParen)
}
pub(super) fn parse(
    input: &str,
    limits: ColorLimits,
    work: &mut usize,
) -> Result<Option<Vec<ContentItem>>, PropertyError> {
    let mut parser = Parser {
        input,
        offset: 0,
        storage: Storage {
            used: 0,
            limit: limits.syntax.retained_bytes.get(),
        },
        work,
    };
    if parser.storage.limit > isize::MAX as usize {
        return Err(resource(0, SyntaxResource::Bytes));
    }
    let mut items = Vec::new();
    while let Some(token @ (start, raw)) = parser.next()? {
        parser.storage.reserve(&mut items, start, parser.work)?;
        let item = match raw.kind {
            TokenKind::String | TokenKind::Uri => {
                let Some(value) = parser.text(token)? else {
                    return Ok(None);
                };
                if raw.kind == TokenKind::String {
                    ContentItem::Text(value)
                } else {
                    ContentItem::Uri(value)
                }
            }
            TokenKind::Ident => {
                let Some(value) = parser.text(token)? else {
                    return Ok(None);
                };
                let Some(keyword) = [
                    "open-quote",
                    "close-quote",
                    "no-open-quote",
                    "no-close-quote",
                ]
                .iter()
                .find(|key| key.eq_ignore_ascii_case(&value)) else {
                    return Ok(None);
                };
                parser.storage.used -= value.capacity();
                drop(value);
                ContentItem::Quote(Keyword(keyword))
            }
            TokenKind::Function => {
                let Some(value) = parser.function(token)? else {
                    return Ok(None);
                };
                value
            }
            _ => return Ok(None),
        };
        items.push(item);
    }
    Ok((!items.is_empty()).then_some(items))
}
