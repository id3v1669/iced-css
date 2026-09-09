//! Bounded, immutable UA/user/author rule admission with original provenance.
use iced_css_core::{
    cascade::{
        rule::{PositionedRuleOffer, RuleContext, RuleOrigin},
        sheets::{AuthorSheets, SheetError, SheetId},
        ua_display::UaDisplayCatalog,
    },
    stylesheet::owned_graph::OwnedStylesheetGraph,
};
use std::num::NonZeroUsize;

/// Application bindings cannot introduce an additional UA origin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphOrigin {
    User,
    Author(SheetId),
}
impl GraphOrigin {
    fn rule_origin(self) -> RuleOrigin {
        match self {
            Self::User => RuleOrigin::User,
            Self::Author(id) => RuleOrigin::Author(id),
        }
    }
}

/// A graph and its root origin, including graphs with no accepted occurrences.
#[derive(Clone, Copy)]
pub struct GraphBinding<'a> {
    pub graph: &'a OwnedStylesheetGraph,
    pub origin: GraphOrigin,
}

#[derive(Debug, Clone, Copy)]
pub struct BundleLimits {
    pub graphs: NonZeroUsize,
    /// Includes the mandatory eight UA rules and excluded author occurrences.
    pub rules: NonZeroUsize,
    /// Actual offer-vector capacity only; borrowed storage stays with its owners.
    pub retained_bytes: NonZeroUsize,
    /// Admission + two graph traversals + earlier author-binding scans + two
    /// rule traversals: 1 + 2*g + P + 2*r, including all eight UA rules.
    pub work: NonZeroUsize,
    /// Sum of the exact controls' stored allowance for every author binding.
    pub membership_work: NonZeroUsize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BundleError {
    Graphs,
    Rules,
    Declarations,
    Bytes,
    Work,
    MembershipWork,
    Allocation,
    OriginOrder {
        binding: usize,
    },
    DuplicateAuthor {
        binding: usize,
        sheet: SheetId,
    },
    Membership {
        binding: usize,
        error: SheetError,
    },
    /// None identifies a synthetic UA rule; Some identifies a graph binding.
    Order {
        binding: Option<usize>,
        rule: usize,
    },
    Identity {
        binding: Option<usize>,
        rule: usize,
    },
    EmptyRule {
        binding: Option<usize>,
        rule: usize,
    },
}
impl std::fmt::Display for BundleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "display rule bundle: {self:?}")
    }
}
impl std::error::Error for BundleError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Membership { error, .. } => Some(error),
            _ => None,
        }
    }
}

/// Complete admission retains the exact catalog, controls, and graph bindings.
/// All borrowed dependencies, including empty graphs, must remain live.
///
/// ```compile_fail
/// use iced_css::evaluation::display_rules::{DisplayRuleBundle, BundleLimits, GraphBinding};
/// use iced_css_core::cascade::{sheets::AuthorSheets, ua_display::{UaDisplayCatalog, UaDisplayLimits}};
/// fn catalog_cannot_escape(bindings: &[GraphBinding<'_>], sheets: &AuthorSheets, ua_limits: UaDisplayLimits, limits: BundleLimits) {
///     let bundle = {
///         let catalog = UaDisplayCatalog::new(ua_limits).unwrap();
///         DisplayRuleBundle::new(&catalog, bindings, sheets, limits).unwrap()
///     };
///     let _ = bundle.rule_count();
/// }
/// ```
///
/// ```compile_fail
/// use iced_css::evaluation::display_rules::{DisplayRuleBundle, BundleLimits, GraphBinding, GraphOrigin};
/// use iced_css_core::{cascade::{sheets::AuthorSheets, ua_display::UaDisplayCatalog}, stylesheet::owned_graph::OwnedStylesheetGraph};
/// fn graph_cannot_escape(catalog: &UaDisplayCatalog, sheets: &AuthorSheets, graph: OwnedStylesheetGraph, limits: BundleLimits) {
///     let bindings;
///     let bundle = {
///         let owner = graph;
///         bindings = [GraphBinding { graph: &owner, origin: GraphOrigin::User }];
///         DisplayRuleBundle::new(catalog, &bindings, sheets, limits).unwrap()
///     };
///     let _ = bundle.bindings();
/// }
/// ```
///
/// ```compile_fail
/// use iced_css::evaluation::display_rules::{DisplayRuleBundle, BundleLimits, GraphBinding};
/// use iced_css_core::cascade::{sheets::AuthorSheets, ua_display::UaDisplayCatalog};
/// fn bindings_cannot_escape(catalog: &UaDisplayCatalog, sheets: &AuthorSheets, limits: BundleLimits) {
///     let bundle = {
///         let bindings: Vec<GraphBinding<'_>> = Vec::new();
///         DisplayRuleBundle::new(catalog, &bindings, sheets, limits).unwrap()
///     };
///     let _ = bundle.bindings();
/// }
/// ```
///
/// ```compile_fail
/// use iced_css::evaluation::display_rules::{DisplayRuleBundle, BundleLimits, GraphBinding};
/// use iced_css_core::cascade::{sheets::AuthorSheets, ua_display::UaDisplayCatalog};
/// fn controls_cannot_change(catalog: &UaDisplayCatalog, bindings: &[GraphBinding<'_>], sheets: &mut AuthorSheets, limits: BundleLimits) {
///     let bundle = DisplayRuleBundle::new(catalog, bindings, sheets, limits).unwrap();
///     sheets.set_author_enabled(false).unwrap();
///     let _ = bundle.sheets();
/// }
/// ```
pub struct DisplayRuleBundle<'a> {
    catalog: &'a UaDisplayCatalog,
    bindings: &'a [GraphBinding<'a>],
    sheets: &'a AuthorSheets,
    offers: Vec<PositionedRuleOffer<'a>>,
    declarations: usize,
    author_rules: usize,
    retained_bytes: usize,
    work: usize,
    membership_work: usize,
}
impl<'a> DisplayRuleBundle<'a> {
    /// Count and validate original ranges, reserve once, then emit in source order.
    /// Users must precede authors. Author IDs identify registered roots, not order.
    ///
    /// # Errors
    /// Any limit, original-range collision, unknown/duplicate root, membership
    /// failure or allocation failure rejects the entire private candidate.
    pub fn new(
        catalog: &'a UaDisplayCatalog,
        bindings: &'a [GraphBinding<'a>],
        sheets: &'a AuthorSheets,
        limits: BundleLimits,
    ) -> Result<Self, BundleError> {
        let mut remaining = limits.work.get();
        charge(&mut remaining)?;
        if bindings.len() > limits.graphs.get() {
            return Err(BundleError::Graphs);
        }
        let mut counts = Counts {
            rules: 0,
            declarations: 0,
            author_rules: 0,
            last: None,
        };
        for (rule, offer) in catalog.offers().enumerate() {
            charge(&mut remaining)?;
            counts.accept(offer, None, rule, limits.rules.get())?;
        }
        let mut author_seen = false;
        let mut membership_work = 0usize;
        for (index, binding) in bindings.iter().enumerate() {
            charge(&mut remaining)?;
            match binding.origin {
                GraphOrigin::User if author_seen => {
                    return Err(BundleError::OriginOrder { binding: index });
                }
                GraphOrigin::User => {}
                GraphOrigin::Author(id) => {
                    author_seen = true;
                    for prior in &bindings[..index] {
                        charge(&mut remaining)?;
                        if prior.origin == GraphOrigin::Author(id) {
                            return Err(BundleError::DuplicateAuthor {
                                binding: index,
                                sheet: id,
                            });
                        }
                    }
                    membership_work = membership_work
                        .checked_add(sheets.work_limit().get())
                        .filter(|total| *total <= limits.membership_work.get())
                        .ok_or(BundleError::MembershipWork)?;
                    // Registration is checked even for empty or disabled roots.
                    sheets
                        .applies(id)
                        .map_err(|error| BundleError::Membership {
                            binding: index,
                            error,
                        })?;
                }
            }
            for (rule, offer) in binding
                .graph
                .offers(binding.origin.rule_origin())
                .enumerate()
            {
                charge(&mut remaining)?;
                counts.accept(offer, Some(index), rule, limits.rules.get())?;
            }
        }
        offer_bytes(counts.rules, limits.retained_bytes.get())?;
        let mut offers = Vec::new();
        offers
            .try_reserve_exact(counts.rules)
            .map_err(|_| BundleError::Allocation)?;
        let retained_bytes = offer_bytes(offers.capacity(), limits.retained_bytes.get())?;
        // Owners and the concrete immutable traversals are unchanged since count.
        for offer in catalog.offers() {
            charge(&mut remaining)?;
            offers.push(offer);
        }
        for binding in bindings {
            charge(&mut remaining)?;
            for offer in binding.graph.offers(binding.origin.rule_origin()) {
                charge(&mut remaining)?;
                offers.push(offer);
            }
        }
        debug_assert_eq!(offers.len(), counts.rules);
        Ok(Self {
            catalog,
            bindings,
            sheets,
            offers,
            declarations: counts.declarations,
            author_rules: counts.author_rules,
            retained_bytes,
            work: limits.work.get() - remaining,
            membership_work,
        })
    }
    pub fn offers(&self) -> &[PositionedRuleOffer<'a>] {
        &self.offers
    }
    pub fn catalog(&self) -> &'a UaDisplayCatalog {
        self.catalog
    }
    pub fn bindings(&self) -> &'a [GraphBinding<'a>] {
        self.bindings
    }
    pub fn sheets(&self) -> &'a AuthorSheets {
        self.sheets
    }
    pub fn rule_count(&self) -> usize {
        self.offers.len()
    }
    pub fn declaration_count(&self) -> usize {
        self.declarations
    }
    pub fn author_rule_count(&self) -> usize {
        self.author_rules
    }
    pub fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }
    pub fn work(&self) -> usize {
        self.work
    }
    pub fn membership_work(&self) -> usize {
        self.membership_work
    }
}

fn charge(work: &mut usize) -> Result<(), BundleError> {
    *work = work.checked_sub(1).ok_or(BundleError::Work)?;
    Ok(())
}
fn offer_bytes(capacity: usize, limit: usize) -> Result<usize, BundleError> {
    capacity
        .checked_mul(size_of::<PositionedRuleOffer<'_>>())
        .filter(|bytes| *bytes <= limit && *bytes <= isize::MAX as usize)
        .ok_or(BundleError::Bytes)
}
struct Counts {
    rules: usize,
    declarations: usize,
    author_rules: usize,
    last: Option<(u64, u64)>,
}
impl Counts {
    fn accept(
        &mut self,
        offer: PositionedRuleOffer<'_>,
        binding: Option<usize>,
        rule: usize,
        limit: usize,
    ) -> Result<(), BundleError> {
        self.rules = self
            .rules
            .checked_add(1)
            .filter(|count| *count <= limit)
            .ok_or(BundleError::Rules)?;
        let length = offer.rule.declarations.len();
        let last = range_end(offer.context, length, binding, rule)?;
        if let Some((order, identity)) = self.last
            && (offer.context.first_order <= order
                || offer.context.first_declaration.get() <= identity)
        {
            return Err(BundleError::Order { binding, rule });
        }
        self.last = Some(last);
        self.declarations = self
            .declarations
            .checked_add(length)
            .ok_or(BundleError::Declarations)?;
        if matches!(offer.rule.origin, RuleOrigin::Author(_)) {
            self.author_rules = self.author_rules.checked_add(1).ok_or(BundleError::Rules)?;
        }
        Ok(())
    }
}
fn range_end(
    context: RuleContext,
    length: usize,
    binding: Option<usize>,
    rule: usize,
) -> Result<(u64, u64), BundleError> {
    let offset = length
        .checked_sub(1)
        .ok_or(BundleError::EmptyRule { binding, rule })?;
    let offset = u64::try_from(offset).map_err(|_| BundleError::Identity { binding, rule })?;
    let order = context
        .first_order
        .checked_add(offset)
        .ok_or(BundleError::Identity { binding, rule })?;
    let identity = context
        .first_declaration
        .get()
        .checked_add(offset)
        .ok_or(BundleError::Identity { binding, rule })?;
    Ok((order, identity))
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced_css_core::{
        cascade::{sheets::SheetLimits, ua_display::UaDisplayLimits},
        syntax::StringLimits,
        values::{NumericLimits, color::ColorLimits},
    };
    use std::num::NonZeroU64;

    #[test]
    fn bytes_measure_actual_private_vector_capacity() {
        let bound = NonZeroUsize::new(100_000).unwrap();
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
        let sheets = AuthorSheets::new(
            vec![],
            vec![],
            true,
            None,
            SheetLimits {
                entries: bound,
                retained_bytes: bound,
                work: bound,
            },
        )
        .unwrap();
        let bundle = DisplayRuleBundle::new(
            &catalog,
            &[],
            &sheets,
            BundleLimits {
                graphs: bound,
                rules: bound,
                retained_bytes: bound,
                work: bound,
                membership_work: bound,
            },
        )
        .unwrap();
        let measured = bundle.offers.capacity() * size_of::<PositionedRuleOffer<'_>>();
        assert_eq!(bundle.retained_bytes(), measured);
        assert!(bundle.offers.capacity() >= bundle.rule_count());
        eprintln!("actual bundle offer capacity bytes: {measured}");
    }

    #[test]
    fn range_defense_rejects_empty_and_overflowing_records() {
        let context = |order, id| RuleContext {
            first_order: order,
            first_declaration: NonZeroU64::new(id).unwrap(),
        };
        assert_eq!(
            range_end(context(8, 9), 0, Some(3), 2),
            Err(BundleError::EmptyRule {
                binding: Some(3),
                rule: 2
            })
        );
        for start in [context(u64::MAX, 9), context(8, u64::MAX)] {
            assert_eq!(
                range_end(start, 2, Some(3), 2),
                Err(BundleError::Identity {
                    binding: Some(3),
                    rule: 2
                })
            );
        }
        assert_eq!(
            range_end(context(u64::MAX - 1, u64::MAX - 1), 2, None, 0),
            Ok((u64::MAX, u64::MAX))
        );
        assert_eq!(offer_bytes(usize::MAX, usize::MAX), Err(BundleError::Bytes));
        let above_isize = isize::MAX as usize / size_of::<PositionedRuleOffer<'_>>() + 1;
        assert_eq!(
            offer_bytes(above_isize, usize::MAX),
            Err(BundleError::Bytes)
        );
    }
}
