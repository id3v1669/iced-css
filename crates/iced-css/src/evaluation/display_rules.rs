//! Bounded, immutable UA/user/author rule admission with original provenance.
use iced_css_core::{
    cascade::{
        CascadeKey,
        rule::{
            PositionedRuleOffer, RuleBatchError, RuleBatchLimits, RuleContext, RuleOrigin,
            RuleTarget, cascade_positioned_rules, matched_declarations,
        },
        sheets::{AuthorSheets, SheetError, SheetId},
        ua_display::UaDisplayCatalog,
        winner_table::PropertyWinners,
    },
    selectors::{PseudoElement, SelectorSource, SimpleSelector},
    stylesheet::owned_graph::OwnedStylesheetGraph,
    values::{
        color::ColorLimits,
        custom::{self, CustomValues},
        property::{ParsedProperty, PropertyResult, SpecifiedValue, parse_value},
        registry::PropertyId,
    },
};
use std::{collections::HashMap, num::NonZeroUsize};

/// Offers by what the rightmost compound of a selector needs of an element: an id, a class
/// or a type. A selector needing none, such as `*` or `:hover`, can match anything.
#[derive(Default)]
struct RuleIndex<'a> {
    by_id: HashMap<&'a str, Vec<usize>>,
    by_class: HashMap<&'a str, Vec<usize>>,
    by_type: HashMap<&'a str, Vec<usize>>,
    any: Vec<usize>,
}

impl<'a> RuleIndex<'a> {
    fn new(offers: &[PositionedRuleOffer<'a>]) -> Self {
        let mut index = Self::default();
        for (at, offer) in offers.iter().enumerate() {
            for selector in offer.rule.selectors {
                let tests = selector
                    .compounds
                    .last()
                    .map_or(&[][..], |compound| compound.tests.as_slice());
                let find =
                    |want: fn(&'a SimpleSelector) -> Option<&'a str>| tests.iter().find_map(want);
                let id = find(|test| match test {
                    SimpleSelector::Id(id) => Some(id.as_str()),
                    _ => None,
                });
                let class = find(|test| match test {
                    SimpleSelector::Class(class) => Some(class.as_str()),
                    _ => None,
                });
                let kind = find(|test| match test {
                    SimpleSelector::Type(name) => Some(name.as_str()),
                    _ => None,
                });
                let bucket = match (id, class, kind) {
                    (Some(id), ..) => index.by_id.entry(id).or_default(),
                    (_, Some(class), _) => index.by_class.entry(class).or_default(),
                    (.., Some(kind)) => index.by_type.entry(kind).or_default(),
                    _ => &mut index.any,
                };
                // One entry per offer in each bucket: its selectors may share one.
                if bucket.last() != Some(&at) {
                    bucket.push(at);
                }
            }
        }
        index
    }
}

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
    /// Actual offer-vector capacity only; borrowed storage stays with its owners. The
    /// selector index (`DisplayRuleBundle::offers_for`) is not counted.
    pub retained_bytes: NonZeroUsize,
    /// Admission + two graph traversals + earlier author-binding scans + two
    /// rule traversals: 1 + 2*g + P + 2*r, including all eight UA rules. Building the
    /// selector index, one pass over every selector, is not counted.
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
    index: RuleIndex<'a>,
    declarations: usize,
    author_rules: usize,
    retained_bytes: usize,
    work: usize,
    membership_work: usize,
    /// What each element's `var()` values parse to, once [`resolve_variables`] has run.
    variables: Option<&'a ResolvedVariables>,
    /// Keyframe declarations laid over each source node's own winners (CSS Animations 1).
    animated: Option<&'a HashMap<usize, Vec<&'a ParsedProperty>>>,
}

/// What a tree's values holding `var()` parse to: per element and pseudo-element, per
/// longhand. `None` was invalid at computed-value time.
#[derive(Debug, Default)]
pub struct ResolvedVariables {
    values: HashMap<(usize, u8, PropertyId), Option<Box<ParsedProperty>>>,
}

const TARGETS: [Option<PseudoElement>; 5] = [
    None,
    Some(PseudoElement::Before),
    Some(PseudoElement::After),
    Some(PseudoElement::FirstLine),
    Some(PseudoElement::FirstLetter),
];
fn target_key(pseudo: Option<PseudoElement>) -> u8 {
    TARGETS
        .iter()
        .position(|target| *target == pseudo)
        .expect("listed") as u8
}
fn holds_variables(parsed: &ParsedProperty) -> bool {
    match &parsed.value {
        SpecifiedValue::Custom { .. } | SpecifiedValue::Pending { .. } => true,
        SpecifiedValue::Expanded(leaves) => leaves.iter().any(holds_variables),
        _ => false,
    }
}

/// Compute every element's custom properties, top down, and parse each winning value that
/// holds `var()` with them substituted (CSS Custom Properties Level 1). `order` lists the
/// source's nodes, every parent before its children.
///
/// # Errors
/// The cascade's own limits, as the passes that follow would meet them.
pub fn resolve_variables<S: SelectorSource<Node = usize>>(
    bundle: &DisplayRuleBundle<'_>,
    source: &S,
    order: impl IntoIterator<Item = usize>,
    per_target: RuleBatchLimits,
    values: ColorLimits,
) -> Result<ResolvedVariables, RuleBatchError> {
    let mut resolved = ResolvedVariables::default();
    // Which pseudo-elements any rule with a custom property or a `var()` targets: the
    // others have their element's custom properties and nothing to parse.
    let targeted = TARGETS.map(|target| {
        target.is_none()
            || bundle.offers.iter().any(|offer| {
                offer
                    .rule
                    .selectors
                    .iter()
                    .any(|selector| selector.pseudo == target)
                    && offer
                        .rule
                        .declarations
                        .iter()
                        .any(|declaration| holds_variables(&declaration.parsed))
            })
    });
    let mut computed: HashMap<usize, std::rc::Rc<CustomValues>> = HashMap::new();
    let none = std::rc::Rc::new(CustomValues::new());
    for node in order {
        let inherited = source
            .parent(node)
            .and_then(|parent| computed.get(&parent).cloned())
            .unwrap_or_else(|| none.clone());
        let mut element = inherited.clone();
        for (target, _) in TARGETS.into_iter().zip(targeted).filter(|(_, on)| *on) {
            let rule_target = || RuleTarget {
                source,
                node,
                pseudo: target,
            };
            let offers = bundle.offers_for(source, node);
            // This target's own custom declarations, each name's winner.
            let mut own: Vec<(&str, &str, CascadeKey)> = Vec::new();
            matched_declarations(
                offers.iter().copied(),
                rule_target(),
                bundle.sheets,
                per_target,
                &mut |parsed, key| {
                    if let SpecifiedValue::Custom { name, raw } = &parsed.value {
                        match own.iter_mut().find(|(other, ..)| other == name) {
                            Some(current) if key >= current.2 => *current = (name, raw, key),
                            Some(_) => {}
                            None => own.push((name, raw, key)),
                        }
                    }
                },
            )?;
            let mut work = 1_000_000usize;
            // A pseudo-element inherits from its element.
            let base = if target.is_none() {
                &inherited
            } else {
                &element
            };
            let own: Vec<(&str, &str)> = own.iter().map(|(name, raw, _)| (*name, *raw)).collect();
            let environment = if own.is_empty() {
                base.clone()
            } else {
                std::rc::Rc::new(custom::compute(base, &own, &mut work))
            };
            if target.is_none() {
                element = environment.clone();
            }
            let mut winners = PropertyWinners::new();
            cascade_positioned_rules(
                &mut winners,
                offers,
                rule_target(),
                bundle.sheets,
                per_target,
            )?;
            // A shorthand's value, parsed once for all its longhands.
            let mut shorthands: Vec<(PropertyId, &str, Option<ParsedProperty>)> = Vec::new();
            for (property, pending) in winners.pending() {
                let SpecifiedValue::Pending { raw, shorthand } = &pending.value else {
                    continue;
                };
                let mut parse = |property: PropertyId| {
                    let lookup = |name: &str| environment.get(name).cloned().flatten();
                    let text = custom::substitute(raw, &lookup, &mut work)?;
                    match parse_value(property, &text, pending.important, values) {
                        Ok(PropertyResult::Parsed(parsed)) => Some(parsed),
                        _ => None,
                    }
                };
                let value = match shorthand {
                    None => parse(property),
                    Some(shorthand) => {
                        let found = shorthands
                            .iter()
                            .position(|(other, text, _)| other == shorthand && *text == raw);
                        let at = found.unwrap_or_else(|| {
                            let whole = parse(*shorthand);
                            shorthands.push((*shorthand, raw, whole));
                            shorthands.len() - 1
                        });
                        match shorthands[at].2.as_ref().map(|whole| &whole.value) {
                            Some(SpecifiedValue::Expanded(leaves)) => leaves
                                .iter()
                                .find(|leaf| leaf.property == property)
                                .cloned(),
                            _ => None,
                        }
                    }
                };
                resolved
                    .values
                    .insert((node, target_key(target), property), value.map(Box::new));
            }
        }
        computed.insert(node, element);
    }
    Ok(resolved)
}
impl<'a> DisplayRuleBundle<'a> {
    /// Whether any rule declares a custom property or holds a `var()`.
    pub fn uses_variables(&self) -> bool {
        self.offers.iter().any(|offer| {
            offer
                .rule
                .declarations
                .iter()
                .any(|declaration| holds_variables(&declaration.parsed))
        })
    }
    /// Use `variables`, from [`resolve_variables`] over these same rules and source.
    #[must_use]
    pub fn with_variables(mut self, variables: &'a ResolvedVariables) -> Self {
        self.variables = Some(variables);
        self
    }
    /// Lay `animated`, by source node, over each element's winners as an animation does.
    #[must_use]
    pub fn with_animated(mut self, animated: &'a HashMap<usize, Vec<&'a ParsedProperty>>) -> Self {
        self.animated = Some(animated);
        self
    }
    /// After a cascade for `node` and `pseudo`: put each winner that holds `var()` as it
    /// parses for that target, or empty its slot where it is invalid there (or where no
    /// variables were resolved), so the property is inherited or initial.
    pub fn resolve<'w>(
        &self,
        winners: &mut PropertyWinners<'w>,
        node: usize,
        pseudo: Option<PseudoElement>,
    ) where
        'a: 'w,
    {
        let pending: Vec<PropertyId> = winners.pending().map(|(property, _)| property).collect();
        for property in pending {
            let value = self.variables.and_then(|variables| {
                variables
                    .values
                    .get(&(node, target_key(pseudo), property))?
                    .as_deref()
            });
            winners.substitute(property, value);
        }
        // An element's own animations; its `:before` and `:after` have none here.
        if let (None, Some(values)) = (
            pseudo,
            self.animated.and_then(|animated| animated.get(&node)),
        ) {
            for value in values {
                let leaves = match &value.value {
                    SpecifiedValue::Expanded(leaves) => leaves.as_slice(),
                    _ => std::slice::from_ref(*value),
                };
                // A keyframe value holding `var()` is not substituted here: left out.
                for leaf in leaves
                    .iter()
                    .filter(|leaf| !matches!(leaf.value, SpecifiedValue::Pending { .. }))
                {
                    winners.animate(leaf);
                }
            }
        }
    }
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
        let index = RuleIndex::new(&offers);
        Ok(Self {
            catalog,
            bindings,
            sheets,
            offers,
            index,
            declarations: counts.declarations,
            author_rules: counts.author_rules,
            retained_bytes,
            work: limits.work.get() - remaining,
            membership_work,
            variables: None,
            animated: None,
        })
    }
    pub fn offers(&self) -> &[PositionedRuleOffer<'a>] {
        &self.offers
    }
    /// The offers that can match `node`, in source order: those whose selectors' last
    /// compound needs its id, one of its classes or its type, or nothing of these. The rest
    /// cannot match it, so the cascade need not try them. A node with no element facts
    /// gets them all.
    pub fn offers_for<S: SelectorSource>(
        &self,
        source: &S,
        node: S::Node,
    ) -> Vec<PositionedRuleOffer<'a>> {
        let Some(facts) = source.facts(node) else {
            return self.offers.clone();
        };
        let index = &self.index;
        let mut chosen: Vec<usize> = index.any.clone();
        if let Some(found) = index.by_type.get(facts.type_name) {
            chosen.extend(found);
        }
        if let Some(found) = facts.metadata.css_id().and_then(|id| index.by_id.get(id)) {
            chosen.extend(found);
        }
        if let Some(classes) = facts.metadata.attribute("class") {
            for class in classes.split([' ', '\t', '\n', '\r', '\u{C}']) {
                if let Some(found) = (!class.is_empty())
                    .then(|| index.by_class.get(class))
                    .flatten()
                {
                    chosen.extend(found);
                }
            }
        }
        chosen.sort_unstable();
        chosen.dedup();
        chosen.into_iter().map(|at| self.offers[at]).collect()
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
