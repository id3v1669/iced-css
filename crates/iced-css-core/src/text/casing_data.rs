//! Pinned Unicode17 base casing mappings and conditional metadata.
//! This is not string casing, locale matching, context evaluation or CSS
//! text-transform execution. In particular base lower mapping omits Final_Sigma.
mod tables;

pub const DATA_VERSION: (u8, u8, u8) = (17, 0, 0);
type MappingEntry = (char, &'static [char]);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseMapping {
    /// Preserve the queried scalar.
    Identity,
    /// Explicit replacement; an empty slice means deletion, not identity.
    Replacement(&'static [char]),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CasingLocale {
    Lithuanian,
    Turkish,
    Azeri,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CasingContext {
    FinalSigma,
    AfterSoftDotted,
    MoreAbove,
    BeforeDot,
    AfterI,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CasingCondition {
    Locale(CasingLocale),
    Context {
        context: CasingContext,
        negated: bool,
    },
}
/// One original SpecialCasing record. All conditions must hold in original-input
/// context; this data API does not evaluate them or select a winning record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConditionalCaseMapping {
    scalar: char,
    lower: &'static [char],
    upper: &'static [char],
    conditions: &'static [CasingCondition],
    expression: &'static str,
}
impl ConditionalCaseMapping {
    pub fn scalar(&self) -> char {
        self.scalar
    }
    pub fn lower(&self) -> &'static [char] {
        self.lower
    }
    pub fn upper(&self) -> &'static [char] {
        self.upper
    }
    pub fn conditions(&self) -> &'static [CasingCondition] {
        self.conditions
    }
    /// Exact trimmed source condition field, including original Not_ spelling.
    pub fn expression(&self) -> &'static str {
        self.expression
    }
}
fn mapping(scalar: char, entries: &'static [MappingEntry]) -> CaseMapping {
    match entries.binary_search_by_key(&scalar, |entry| entry.0) {
        Ok(index) => CaseMapping::Replacement(entries[index].1),
        Err(_) => CaseMapping::Identity,
    }
}
/// Full base lower mapping: simple mapping plus unconditional SpecialCasing.
/// Conditional overrides (including language-independent Final_Sigma) are separate.
pub fn base_lower_mapping(scalar: char) -> CaseMapping {
    mapping(scalar, tables::LOWER)
}
/// Full base upper mapping, without conditional overrides or titlecasing.
pub fn base_upper_mapping(scalar: char) -> CaseMapping {
    mapping(scalar, tables::UPPER)
}
/// All conditional candidates, in original file order within this scalar's group.
/// An empty result means no conditional row, not an empty replacement mapping.
pub fn conditional_mappings(scalar: char) -> &'static [ConditionalCaseMapping] {
    match tables::CONDITIONAL_GROUPS.binary_search_by_key(&scalar, |entry| entry.0) {
        Ok(index) => {
            let (_, start, end) = tables::CONDITIONAL_GROUPS[index];
            &tables::CONDITIONAL[start..end]
        }
        Err(_) => &[],
    }
}
fn member(scalar: char, ranges: &[(char, char)]) -> bool {
    let end = ranges.partition_point(|(start, _)| *start <= scalar);
    end.checked_sub(1)
        .is_some_and(|index| scalar <= ranges[index].1)
}
pub fn is_cased(scalar: char) -> bool {
    member(scalar, tables::CASED)
}
/// Independent of Cased: the properties overlap, notably at U+0345.
pub fn is_case_ignorable(scalar: char) -> bool {
    member(scalar, tables::CASE_IGNORABLE)
}
pub fn is_soft_dotted(scalar: char) -> bool {
    member(scalar, tables::SOFT_DOTTED)
}
/// Exact canonical combining class; all unlisted scalars have class zero.
pub fn canonical_combining_class(scalar: char) -> u8 {
    let end = tables::CCC.partition_point(|(start, _, _)| *start <= scalar);
    end.checked_sub(1)
        .filter(|&index| scalar <= tables::CCC[index].1)
        .map_or(0, |index| tables::CCC[index].2)
}
