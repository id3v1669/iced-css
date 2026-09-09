//! Selected iced source-language UA display policy (architecture section 47).
//!
//! These are normal typed selector rules. Orders 0..7 and declaration IDs 1..8
//! identify synthetic policy declarations, not application file locations.
use super::{
    Specificity,
    rule::{PositionedRuleOffer, RuleContext, RuleDeclaration, RuleOffer, RuleOrigin},
};
use crate::{
    selectors::{Compound, Selector, SimpleSelector},
    syntax::{Declaration, SyntaxError, decode_identifier},
    values::{
        color::ColorLimits,
        property::{PropertyError, PropertyResult, SpecifiedValue, parse_property},
    },
};
use std::{
    mem::size_of,
    num::{NonZeroU64, NonZeroUsize},
};

const GROUPS: &[(&[&str], &str)] = &[
    (
        &[
            "container",
            "row",
            "column",
            "grid",
            "stack",
            "keyed_column",
            "scrollable",
            "pin",
            "mouse_area",
            "sensor",
            "themer",
            "float",
            "tooltip",
            "hover",
            "opaque",
            "responsive",
            "lazy",
            "component",
            "markdown",
            "iced",
            "pane_grid",
            "pane_content",
            "pane_title",
        ],
        "block",
    ),
    (&["text", "rich_text"], "inline"),
    (
        &[
            "button",
            "checkbox",
            "radio",
            "toggler",
            "text_input",
            "text_editor",
            "slider",
            "vertical_slider",
            "pick_list",
            "combo_box",
            "progress_bar",
            "space",
            "rule",
            "image",
            "image_viewer",
            "svg",
            "canvas",
            "qr_code",
            "shader",
            "pane_controls",
        ],
        "inline-block",
    ),
    (&["table"], "table"),
    (&["table_header_group"], "table-header-group"),
    (&["table_row_group"], "table-row-group"),
    (&["table_row"], "table-row"),
    (&["table_cell", "table_header_cell"], "table-cell"),
];

#[derive(Debug, Clone, Copy)]
pub struct UaDisplayLimits {
    pub rules: NonZeroUsize,
    /// Cumulative actual retained vector/string capacities, excluding stack
    /// headers and independently bounded per-group parser scratch.
    pub retained_bytes: NonZeroUsize,
    /// Frozen construction schedule: 2 + 5*8 + 7*51 + 409 = 808 units.
    /// Each reserve/account is one unit; visits and record construction one each;
    /// name copying costs its byte length. Fixed parser work is separate.
    pub work: NonZeroUsize,
    /// Independent limits for each fixed name decode and each keyword parse.
    /// Effective syntax bytes are capped at isize::MAX so larger caller
    /// allowances do not trigger the decoder's allowance-size rejection.
    pub values: ColorLimits,
}
#[derive(Debug, PartialEq, Eq)]
pub enum UaDisplayError {
    Rules,
    Bytes,
    Work,
    Allocation,
    Identity,
    Syntax(SyntaxError),
    Property(PropertyError),
    /// The fixed policy declaration did not parse as its normal display keyword.
    InvalidDisplay,
}
impl std::fmt::Display for UaDisplayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "UA display catalog: {self:?}")
    }
}
impl std::error::Error for UaDisplayError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Syntax(error) => Some(error),
            Self::Property(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Debug)]
struct OwnedRule {
    selectors: Vec<Selector>,
    declarations: Vec<RuleDeclaration>,
    context: RuleContext,
}

/// Owns immutable rules whose offers and cascade winners borrow this catalog.
///
/// ```compile_fail
/// use iced_css_core::cascade::ua_display::{UaDisplayCatalog, UaDisplayLimits};
/// fn cannot_escape(limits: UaDisplayLimits) {
///     let offer = {
///         let catalog = UaDisplayCatalog::new(limits).unwrap();
///         catalog.offers().next().unwrap()
///     };
///     let _ = offer.rule.declarations;
/// }
/// ```
#[derive(Debug)]
pub struct UaDisplayCatalog {
    rules: Vec<OwnedRule>,
    next_context: RuleContext,
    retained_bytes: usize,
}
impl UaDisplayCatalog {
    /// Build all eight groups atomically with fallible, single-reservation storage.
    /// Unknown/custom types receive no universal fallback. The CSS cascade and
    /// computed display conversion remain responsible for the effective display.
    ///
    /// # Errors
    /// Returns a bound/allocation error or a fixed-value parsing error. Any
    /// failure drops the entire private candidate; no partial catalog escapes.
    pub fn new(limits: UaDisplayLimits) -> Result<Self, UaDisplayError> {
        if GROUPS.len() > limits.rules.get() {
            return Err(UaDisplayError::Rules);
        }
        let mut budget = Budget {
            bytes: 0,
            limit: limits.retained_bytes.get(),
            work: limits.work.get(),
        };
        budget.charge(1)?;
        // The existing decoder rejects an allowance above isize::MAX. Cap the
        // effective bound, never the actual fixed input or resulting allocation.
        let mut values = limits.values;
        values.syntax.retained_bytes = values.syntax.retained_bytes.min(
            NonZeroUsize::new(isize::MAX as usize).expect("platform allocation bound is positive"),
        );
        let mut rules = budget.vector(GROUPS.len())?;
        let mut context = RuleContext {
            first_order: 0,
            first_declaration: NonZeroU64::MIN,
        };
        for &(names, keyword) in GROUPS {
            budget.charge(1)?;
            let mut selectors = budget.vector(names.len())?;
            for &name in names {
                budget.charge(1)?;
                let mut owned_name = budget.string(name.len())?;
                budget.charge(name.len())?;
                owned_name.push_str(name);
                let mut tests = budget.vector(1)?;
                budget.charge(1)?;
                tests.push(SimpleSelector::Type(owned_name));
                let mut compounds = budget.vector(1)?;
                budget.charge(1)?;
                compounds.push(Compound { tests });
                budget.charge(1)?;
                selectors.push(Selector {
                    compounds,
                    combinators: Vec::new(),
                    pseudo: None,
                    specificity: Specificity::selector(0, 0, 1),
                });
            }
            let mut declarations = budget.vector(1)?;
            // Decoder and parser scratch each use their existing independent
            // syntax/value bounds and are released at the end of this group.
            let declaration = Declaration {
                name: decode_identifier("display", values.syntax)
                    .map_err(UaDisplayError::Syntax)?,
                value: keyword,
                important: false,
                span: 0..keyword.len(),
                value_span: 0..keyword.len(),
                implicit_close: false,
                recovery_only: false,
            };
            let PropertyResult::Parsed(parsed) =
                parse_property(&declaration, values).map_err(UaDisplayError::Property)?
            else {
                return Err(UaDisplayError::InvalidDisplay);
            };
            if parsed.property.descriptor().name != "display"
                || parsed.important
                || !matches!(parsed.value, SpecifiedValue::Keyword(value) if value.as_str() == keyword)
            {
                return Err(UaDisplayError::InvalidDisplay);
            }
            budget.charge(1)?;
            declarations.push(RuleDeclaration {
                parsed,
                order: context.first_order,
                declaration: context.first_declaration,
            });
            budget.charge(1)?;
            rules.push(OwnedRule {
                selectors,
                declarations,
                context,
            });
            context = context.offset(1).ok_or(UaDisplayError::Identity)?;
        }
        Ok(Self {
            rules,
            next_context: context,
            retained_bytes: budget.bytes,
        })
    }

    /// Borrow synthetic policy occurrences at normal UserAgent origin.
    pub fn offers(&self) -> impl ExactSizeIterator<Item = PositionedRuleOffer<'_>> {
        self.rules.iter().map(|rule| PositionedRuleOffer {
            rule: RuleOffer {
                selectors: &rule.selectors,
                declarations: &rule.declarations,
                origin: RuleOrigin::UserAgent,
            },
            context: rule.context,
        })
    }
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }
    /// Start subsequent loaded-graph positioning here (order 8, declaration ID 9).
    pub fn next_context(&self) -> RuleContext {
        self.next_context
    }
    pub fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }
}

struct Budget {
    bytes: usize,
    limit: usize,
    work: usize,
}
impl Budget {
    fn charge(&mut self, amount: usize) -> Result<(), UaDisplayError> {
        self.work = self.work.checked_sub(amount).ok_or(UaDisplayError::Work)?;
        Ok(())
    }
    fn total(&self, bytes: usize) -> Result<usize, UaDisplayError> {
        let total = self.bytes.checked_add(bytes).ok_or(UaDisplayError::Bytes)?;
        if total > self.limit || total > isize::MAX as usize {
            return Err(UaDisplayError::Bytes);
        }
        Ok(total)
    }
    fn vector<T>(&mut self, count: usize) -> Result<Vec<T>, UaDisplayError> {
        self.charge(1)?;
        self.total(
            count
                .checked_mul(size_of::<T>())
                .ok_or(UaDisplayError::Bytes)?,
        )?;
        let mut result = Vec::new();
        result
            .try_reserve_exact(count)
            .map_err(|_| UaDisplayError::Allocation)?;
        self.bytes = self.total(
            result
                .capacity()
                .checked_mul(size_of::<T>())
                .ok_or(UaDisplayError::Bytes)?,
        )?;
        Ok(result)
    }
    fn string(&mut self, count: usize) -> Result<String, UaDisplayError> {
        self.charge(1)?;
        self.total(count)?;
        let mut result = String::new();
        result
            .try_reserve_exact(count)
            .map_err(|_| UaDisplayError::Allocation)?;
        self.bytes = self.total(result.capacity())?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{syntax::StringLimits, values::NumericLimits};

    #[test]
    fn reported_bytes_include_actual_capacity_of_every_owned_layer() {
        let bound = NonZeroUsize::new(1_000_000).unwrap();
        let catalog = UaDisplayCatalog::new(UaDisplayLimits {
            rules: bound,
            retained_bytes: bound,
            work: bound,
            values: ColorLimits {
                syntax: StringLimits {
                    work: bound,
                    retained_bytes: bound,
                },
                numeric: NumericLimits {
                    work: bound,
                    digits: bound,
                },
            },
        })
        .unwrap();
        // Rewalk the actual owned allocations; do not reuse Budget accounting.
        let mut measured = catalog.rules.capacity() * size_of::<OwnedRule>();
        let mut names = 0;
        for rule in &catalog.rules {
            measured += rule.selectors.capacity() * size_of::<Selector>();
            measured += rule.declarations.capacity() * size_of::<RuleDeclaration>();
            for selector in &rule.selectors {
                measured += selector.compounds.capacity() * size_of::<Compound>();
                measured +=
                    selector.combinators.capacity() * size_of::<crate::selectors::Combinator>();
                for compound in &selector.compounds {
                    measured += compound.tests.capacity() * size_of::<SimpleSelector>();
                    for simple in &compound.tests {
                        let SimpleSelector::Type(name) = simple else {
                            panic!()
                        };
                        measured += name.capacity();
                        names += 1;
                    }
                }
            }
        }
        assert_eq!(names, 51);
        assert_eq!(catalog.retained_bytes(), measured);
        eprintln!("actual catalog capacity bytes: {measured}");
    }
}
