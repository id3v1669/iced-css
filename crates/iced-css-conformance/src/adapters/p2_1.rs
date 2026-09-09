use iced_css_core::{
    selectors::SelectorLimits,
    stylesheet::{
        RejectionReason, RuleAssembly, RuleAssemblyLimits, RuleContext, SheetEntryKind,
        SheetRejectionReason, StylesheetLimits, parse_style_rule_prefix, parse_stylesheet,
    },
    syntax::{RecoveryLimits, StringLimits, TokenKind, Tokens},
    values::{NumericLimits, color::ColorLimits},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::num::{NonZeroU64, NonZeroUsize};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Limits {
    work: NonZeroUsize,
    digits: NonZeroUsize,
    bytes: NonZeroUsize,
    nesting: NonZeroUsize,
    selector_entries: NonZeroUsize,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    scenario: String,
    css: String,
    limits: Limits,
}
/// Controlled decoded-rule and decoded-sheet scenarios using product assemblers.
/// Byte intake, import acquisition and normative oracle admission remain separate.
pub fn run(input: &Value) -> crate::CaseResult {
    let input: Input = serde_json::from_value(input.clone()).map_err(|error| error.to_string())?;
    if !matches!(
        input.scenario.as_str(),
        "declaration_recovery" | "stylesheet"
    ) {
        return Err("unknown syntax scenario".into());
    }
    let limits = input.limits;
    let rule_limits = RuleAssemblyLimits {
        recovery: RecoveryLimits {
            work: limits.work,
            nesting: limits.nesting,
            retained_bytes: limits.bytes,
        },
        selectors: SelectorLimits {
            work: limits.work,
            retained_bytes: limits.bytes,
            entries: limits.selector_entries,
        },
        values: ColorLimits {
            syntax: StringLimits {
                work: limits.work,
                retained_bytes: limits.bytes,
            },
            numeric: NumericLimits {
                work: limits.work,
                digits: limits.digits,
            },
        },
        entries: limits.selector_entries,
        record_bytes: limits.bytes,
        work: limits.work,
        name_bytes: limits.bytes,
    };
    let context = RuleContext {
        first_order: 0,
        first_declaration: NonZeroU64::MIN,
    };
    if input.scenario == "stylesheet" {
        return stylesheet(
            &input.css,
            context,
            StylesheetLimits {
                rules: rule_limits,
                statements: limits.selector_entries,
                record_bytes: limits.bytes,
                work: limits.work,
            },
        );
    }
    let result = parse_style_rule_prefix(&input.css, context, rule_limits)
        .map_err(|error| error.to_string())?;
    let RuleAssembly::Accepted(rule) = result else {
        return Err("invalid selector list".into());
    };
    for token in Tokens::new(&input.css[rule.consumed..], limits.work) {
        let token = token.map_err(|error| error.to_string())?;
        if !matches!(token.kind, TokenKind::Space | TokenKind::Comment) {
            return Err("scenario requires exactly one qualified rule".into());
        }
    }
    let accepted: Vec<_> = rule
        .declarations
        .iter()
        .map(|d| d.parsed.property.descriptor().name)
        .collect();
    let mut discarded = Vec::new();
    for rejected in rule.rejected {
        match rejected.reason {
            RejectionReason::UnknownProperty | RejectionReason::InvalidValue => {
                discarded.push(rejected.name.expect("decoded declaration name"))
            }
            RejectionReason::AtRule => {}
            RejectionReason::InapplicableProperty => {
                return Err("unexpected page-context rejection in a style rule".into());
            }
            RejectionReason::MalformedDeclaration => {
                return Err(
                    "scenario cannot label a malformed declaration without a decoded property name"
                        .into(),
                );
            }
        }
    }
    Ok(json!({"accepted_properties":accepted,"discarded_properties":discarded}))
}

fn stylesheet(css: &str, context: RuleContext, limits: StylesheetLimits) -> crate::CaseResult {
    let sheet = parse_stylesheet(css, context, limits).map_err(|error| error.to_string())?;
    let entry_kinds: Vec<_> = sheet
        .entries
        .iter()
        .map(|entry| match &entry.kind {
            SheetEntryKind::Style(_) => "style",
            SheetEntryKind::Page(_) => "page",
            SheetEntryKind::Media(_) => "media",
            SheetEntryKind::Import(_) => "import",
        })
        .collect();
    let properties: Vec<_> = sheet
        .local_screen_rules()
        .flat_map(|rule| {
            rule.declarations
                .iter()
                .map(|declaration| declaration.parsed.property.descriptor().name)
        })
        .collect();
    let imports:Vec<_>=sheet.entries.iter().filter_map(|entry|match &entry.kind {
        SheetEntryKind::Import(import)=>Some(json!({"location":import.location.text,"applies_to_screen":import.applies_to_screen})), _=>None,
    }).collect();
    let rejections: Vec<_> = sheet
        .rejected
        .iter()
        .map(|rejection| match rejection.reason {
            SheetRejectionReason::InvalidRule => "invalid_rule",
            SheetRejectionReason::UnknownAtRule => "unknown_at_rule",
            SheetRejectionReason::Charset => "charset",
            SheetRejectionReason::InvalidImport => "invalid_import",
            SheetRejectionReason::LateImport => "late_import",
            SheetRejectionReason::InvalidMedia => "invalid_media",
            SheetRejectionReason::NestedAtRule => "nested_at_rule",
        })
        .collect();
    Ok(
        json!({"entry_kinds":entry_kinds,"local_screen_properties":properties,"imports":imports,"rejections":rejections}),
    )
}
