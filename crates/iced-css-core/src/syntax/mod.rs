//! Source syntax primitives. Decoded Rust text is distinct from transport-byte decoding.
mod strings;
pub use strings::{
    Closure, DecodedString, Repair, RepairKind, StringLimits, SyntaxError, SyntaxFault,
    SyntaxResource, decode_string,
};
mod numeric;
pub use numeric::{NumericToken, NumericTokenKind, scan_numeric_prefix};
mod token;
pub use token::{Token, TokenKind, scan_token};
mod recover;
pub use recover::{Boundary, Recovered, RecoveryLimits, RecoveryMode, recover_prefix};
pub use strings::{DecodedIdentifier, decode_identifier};
mod declarations;
pub use declarations::{Declaration, DeclarationItem, DeclarationLimits, Declarations};

pub(crate) use strings::{decode_identifier_budget, decode_name_budget};
pub(crate) use token::scan_token_budget;
mod rule;
pub use rule::{QualifiedRule, RuleError, qualified_rule_prefix};
pub use token::Tokens;

pub(crate) use strings::decode_string_budget;
pub use strings::decode_uri;

pub(crate) use strings::decode_uri_budget;
mod media;
pub use media::{MediaError, MediaListContext, media_matches_screen};
mod import;
pub use import::{ImportError, ImportPrelude, parse_import_prelude};
mod statements;
pub use statements::{Statement, StatementContext, StatementKind, Statements};
pub mod charset;
pub mod decode;
pub mod encoding;
pub mod encoding_selection;
pub mod encoding_transport;
pub mod import_placement;
pub mod imports;
pub mod unicode_intake;
