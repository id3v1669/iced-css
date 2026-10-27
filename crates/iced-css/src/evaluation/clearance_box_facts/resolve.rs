use super::*;
use ClearanceBoxFactsError as E;
const NAMES: [&str; 29] = [
    "width",
    "height",
    "min-width",
    "min-height",
    "max-width",
    "max-height",
    "margin-top",
    "margin-right",
    "margin-bottom",
    "margin-left",
    "padding-top",
    "padding-right",
    "padding-bottom",
    "padding-left",
    "border-top-style",
    "border-right-style",
    "border-bottom-style",
    "border-left-style",
    "clear",
    "overflow",
    "border-top-width",
    "border-right-width",
    "border-bottom-width",
    "border-left-width",
    "content",
    "top",
    "right",
    "bottom",
    "left",
];
const RECIPES: [&str; 29] = [
    "auto", "auto", "0", "0", "none", "none", "0", "0", "0", "0", "0", "0", "0", "0", "none",
    "none", "none", "none", "none", "visible", "medium", "medium", "medium", "medium", "normal",
    "auto", "auto", "auto", "auto",
];
pub(super) fn property_ids() -> Result<[PropertyId; 29], E> {
    let first = PropertyId::lookup(NAMES[0]).ok_or(E::PropertyRegistry { name: NAMES[0] })?;
    let mut ids = [first; 29];
    for (i, name) in NAMES.into_iter().enumerate() {
        ids[i] = PropertyId::lookup(name).ok_or(E::PropertyRegistry { name })?;
        if ids[i].descriptor().shorthand {
            return Err(E::InitialRecipe { property: ids[i] });
        }
    }
    Ok(ids)
}
fn recipe(property: PropertyId, index: usize) -> Result<(), E> {
    if property.descriptor().initial == RECIPES[index] {
        Ok(())
    } else {
        Err(E::InitialRecipe { property })
    }
}
const ZERO: AbsoluteBoxFact = AbsoluteBoxFact {
    pixels: 0.,
    rounded_to_zero: false,
    origin: BoxFactOrigin::Initial,
};
/// The initial `max-width`/`max-height` (MMX); every Legacy row keeps it.
const NO_MAX: BoxMaxFact = BoxMaxFact::None {
    origin: BoxFactOrigin::Initial,
};
/// Every Legacy width fact, and every Sequence width before its longhand is resolved.
const ZERO_WIDTH: BorderWidthFact = BorderWidthFact {
    pixels: 0.,
    event: None,
    origin: BoxFactOrigin::Initial,
};
pub(super) fn initial_row() -> ClearanceBoxFact {
    ClearanceBoxFact {
        width: BoxLengthFact::Auto {
            origin: BoxFactOrigin::Initial,
        },
        height: BoxLengthFact::Auto {
            origin: BoxFactOrigin::Initial,
        },
        min_width: LengthPercentageBoxFact::Absolute(ZERO),
        min_height: LengthPercentageBoxFact::Absolute(ZERO),
        max_width: NO_MAX,
        max_height: NO_MAX,
        margin: [BoxLengthFact::Absolute(ZERO); 4],
        padding: [LengthPercentageBoxFact::Absolute(ZERO); 4],
        clear: KeywordBoxFact {
            value: ClearSide::None,
            origin: BoxFactOrigin::Initial,
        },
        overflow: KeywordBoxFact {
            value: BoxOverflow::Visible,
            origin: BoxFactOrigin::Initial,
        },
        border_style: [KeywordBoxFact {
            value: BorderStyle::None,
            origin: BoxFactOrigin::Initial,
        }; 4],
        border_width: [ZERO_WIDTH; 4],
        before: PseudoAbsence {
            origin: BoxFactOrigin::Initial,
        },
        after: PseudoAbsence {
            origin: BoxFactOrigin::Initial,
        },
        offset: None,
    }
}
fn inherited(v: BoxLengthFact, origin: BoxFactOrigin) -> BoxLengthFact {
    match v {
        BoxLengthFact::Absolute(v) => BoxLengthFact::Absolute(AbsoluteBoxFact { origin, ..v }),
        BoxLengthFact::Auto { .. } => BoxLengthFact::Auto { origin },
        BoxLengthFact::Percentage(v) => {
            BoxLengthFact::Percentage(PercentageBoxFact { origin, ..v })
        }
    }
}
/// MMX: an inherited `max-*` fact keeps the parent's value with the new origin.
fn inherited_max(v: BoxMaxFact, origin: BoxFactOrigin) -> BoxMaxFact {
    match v {
        BoxMaxFact::Absolute(v) => BoxMaxFact::Absolute(AbsoluteBoxFact { origin, ..v }),
        BoxMaxFact::None { .. } => BoxMaxFact::None { origin },
        BoxMaxFact::Percentage(v) => BoxMaxFact::Percentage(PercentageBoxFact { origin, ..v }),
    }
}
/// PPM: an inherited padding or `min-width` (PHT: or `min-height`) fact keeps the
/// parent's value, absolute or percentage, with the new origin.
fn inherited_length_percentage(
    v: LengthPercentageBoxFact,
    origin: BoxFactOrigin,
) -> LengthPercentageBoxFact {
    match v {
        LengthPercentageBoxFact::Absolute(v) => {
            LengthPercentageBoxFact::Absolute(AbsoluteBoxFact { origin, ..v })
        }
        LengthPercentageBoxFact::Percentage(v) => {
            LengthPercentageBoxFact::Percentage(PercentageBoxFact { origin, ..v })
        }
    }
}
struct Resolver {
    original: usize,
    numeric: NumericLimits,
}
impl Resolver {
    fn invalid(&self, property: PropertyId) -> E {
        E::InvalidValue {
            original: self.original,
            property,
        }
    }
    fn unsupported(&self, property: PropertyId, reason: BoxFactUnsupported) -> E {
        E::Unsupported {
            original: self.original,
            property,
            target: BoxFactTarget::Element,
            reason,
        }
    }
    /// One length longhand: `auto` is admitted only when `auto` is set (width, height,
    /// margins and the REL offsets), a negative absolute length only when `signed` is
    /// set (margins and offsets); an absolute length is one counted conversion. A
    /// percentage is kept exactly, without conversion, only when `percentage` is set
    /// (PCT: Sequence `width`, margins, `right`, `left`; PPM: Sequence paddings,
    /// `min-width`, `max-width`; PHT: Sequence `height`, `min-height`, `max-height`; PTB:
    /// `top`, `bottom` of a relative Sequence row): a negative one only when `signed` is
    /// set, and a value outside the bounded representation is `PercentagePrecision`;
    /// otherwise it is `PercentageLength`.
    fn length(
        &self,
        value: &SpecifiedValue,
        property: PropertyId,
        auto: bool,
        signed: bool,
        percentage: bool,
        calls: &mut usize,
    ) -> Result<BoxLengthFact, E> {
        let SpecifiedValue::Length(target) = value else {
            return Err(self.invalid(property));
        };
        match target {
            LengthTarget::Relative(..) => {
                return Err(self.unsupported(property, BoxFactUnsupported::RelativeLength));
            }
            LengthTarget::Percentage(value) if percentage => {
                if !signed && value.is_negative() {
                    return Err(self.invalid(property));
                }
                let value = value.compact().ok_or_else(|| {
                    self.unsupported(property, BoxFactUnsupported::PercentagePrecision)
                })?;
                return Ok(BoxLengthFact::Percentage(PercentageBoxFact {
                    value,
                    origin: BoxFactOrigin::Specified,
                }));
            }
            LengthTarget::Percentage(..) => {
                return Err(self.unsupported(property, BoxFactUnsupported::PercentageLength));
            }
            LengthTarget::Auto => {
                return if auto {
                    Ok(BoxLengthFact::Auto {
                        origin: BoxFactOrigin::Specified,
                    })
                } else {
                    Err(self.unsupported(property, BoxFactUnsupported::AutoForProperty))
                };
            }
            LengthTarget::Absolute(..) => {}
        }
        if !signed && target.is_negative() {
            return Err(self.invalid(property));
        }
        *calls += 1;
        let evaluated =
            evaluate_length(target, LengthContext::default(), self.numeric).map_err(|error| {
                E::Length {
                    original: self.original,
                    property,
                    error,
                }
            })?;
        let LengthEvaluation::Value(v) = evaluated else {
            return Err(self.invalid(property));
        };
        Ok(BoxLengthFact::Absolute(AbsoluteBoxFact {
            pixels: if v.value == 0. { 0. } else { v.value },
            rounded_to_zero: v.rounded_to_zero,
            origin: BoxFactOrigin::Specified,
        }))
    }
    /// One `border-*-width` side in Sequence mode (CSS2.2 8.5.1; architecture 34).
    /// The side's own computed style is tested first: a `none` or `hidden` side is +0
    /// with no event, no `RelativeLength` and no conversion, whatever its value.
    /// `RelativeLength` is raised before any conversion; an absolute length is one
    /// counted conversion to the nearest non-negative f32, ties to even, saturating.
    fn border_width(
        &self,
        value: &SpecifiedValue,
        property: PropertyId,
        style: BorderStyle,
        calls: &mut usize,
    ) -> Result<BorderWidthFact, E> {
        let specified = |pixels: f32, event: Option<RangeEvent>| BorderWidthFact {
            pixels,
            event,
            origin: BoxFactOrigin::Specified,
        };
        if suppressed(style) {
            return Ok(specified(0., None));
        }
        match value {
            SpecifiedValue::Keyword(k) => match k.as_str() {
                "thin" => Ok(specified(1., None)),
                "medium" => Ok(specified(3., None)),
                "thick" => Ok(specified(5., None)),
                _ => Err(self.invalid(property)),
            },
            SpecifiedValue::Length(target) => match target {
                LengthTarget::Relative(..) => {
                    Err(self.unsupported(property, BoxFactUnsupported::RelativeLength))
                }
                LengthTarget::Absolute(_) if target.is_negative() => Err(self.invalid(property)),
                LengthTarget::Absolute(length) => {
                    *calls += 1;
                    let converted =
                        length
                            .to_nonnegative_f32(self.numeric)
                            .map_err(|error| E::Length {
                                original: self.original,
                                property,
                                error: LengthEvaluationError::Conversion(error),
                            })?;
                    // Zero is canonical +0.
                    let pixels = if converted.value == 0. {
                        0.
                    } else {
                        converted.value
                    };
                    Ok(specified(pixels, converted.event))
                }
                LengthTarget::Percentage(..) | LengthTarget::Auto => Err(self.invalid(property)),
            },
            _ => Err(self.invalid(property)),
        }
    }
}
/// CSS2.2 8.5.1: a `none` or `hidden` side style forces a zero computed width.
fn suppressed(style: BorderStyle) -> bool {
    matches!(style, BorderStyle::None | BorderStyle::Hidden)
}
pub(super) fn element(
    winners: &PropertyWinners<'_>,
    ids: &[PropertyId; 29],
    original: usize,
    parent: Option<(usize, ClearanceBoxFact)>,
    numeric: NumericLimits,
    mode: BoxFactMode,
    calls: &mut usize,
) -> Result<ClearanceBoxFact, E> {
    let resolver = Resolver { original, numeric };
    let mut row = initial_row();
    // Legacy resolves indices 0-19 only and never computes a width. Sequence also
    // resolves the widths 20-23, each after its own side's style (14-17).
    let resolved = match mode {
        BoxFactMode::Legacy => 20,
        BoxFactMode::Sequence => 24,
    };
    for (index, &property) in ids[..resolved].iter().enumerate() {
        let defaulted = winners
            .defaulted(property, parent.is_some())
            .map_err(|error| E::Defaulting {
                original,
                property,
                error,
            })?;
        match defaulted {
            DefaultedValueRef::Initial(_) => {
                recipe(property, index)?;
                // Sequence only (Legacy stops at 19): initial `medium` is 3 on a visible
                // side; a suppressed side keeps the initial +0.
                if index >= 20 && !suppressed(row.border_style[index - 20].value) {
                    row.border_width[index - 20].pixels = 3.;
                }
            }
            DefaultedValueRef::ParentComputed => {
                let (original_parent, p) = parent.ok_or_else(|| resolver.invalid(property))?;
                let origin = BoxFactOrigin::Inherited { original_parent };
                match index {
                    0 => row.width = inherited(p.width, origin),
                    1 => row.height = inherited(p.height, origin),
                    // MMX: a Sequence row copies the parent row's computed constraint.
                    2..=5 if mode == BoxFactMode::Sequence => match index {
                        2 => row.min_width = inherited_length_percentage(p.min_width, origin),
                        3 => row.min_height = inherited_length_percentage(p.min_height, origin),
                        4 => row.max_width = inherited_max(p.max_width, origin),
                        _ => row.max_height = inherited_max(p.max_height, origin),
                    },
                    // Legacy: a successful parent's constraints are already computed zero/none.
                    2..=5 => {}
                    6..=9 => row.margin[index - 6] = inherited(p.margin[index - 6], origin),
                    10..=13 => {
                        row.padding[index - 10] =
                            inherited_length_percentage(p.padding[index - 10], origin)
                    }
                    14..=17 => {
                        row.border_style[index - 14] = KeywordBoxFact {
                            origin,
                            ..p.border_style[index - 14]
                        }
                    }
                    18 => row.clear = KeywordBoxFact { origin, ..p.clear },
                    19 => {
                        row.overflow = KeywordBoxFact {
                            origin,
                            ..p.overflow
                        }
                    }
                    // Sequence only. The own style forces zero before `inherit`;
                    // otherwise the parent's computed fact is copied, pixels and event
                    // (an inherited event describes the original conversion).
                    20..=23 => {
                        let side = index - 20;
                        row.border_width[side] = if suppressed(row.border_style[side].value) {
                            BorderWidthFact {
                                pixels: 0.,
                                event: None,
                                origin,
                            }
                        } else {
                            BorderWidthFact {
                                origin,
                                ..p.border_width[side]
                            }
                        };
                    }
                    _ => return Err(resolver.invalid(property)),
                }
            }
            DefaultedValueRef::Compute(value) => {
                if index < 14 {
                    // MMX (CSS2.2 10.4, 10.7): a Sequence row stores its computed
                    // constraint: `none` for a max, else one counted conversion of an
                    // absolute nonnegative length; PPM keeps a `min-width` or `max-width`
                    // and PHT a `min-height` or `max-height` percentage exactly (`em`/`ex`,
                    // `auto` and negative values keep their refusals). Legacy keeps the
                    // refusals below.
                    if mode == BoxFactMode::Sequence && (2..6).contains(&index) {
                        if index >= 4
                            && matches!(value,SpecifiedValue::Keyword(k) if k.as_str()=="none")
                        {
                            let none = BoxMaxFact::None {
                                origin: BoxFactOrigin::Specified,
                            };
                            if index == 4 {
                                row.max_width = none;
                            } else {
                                row.max_height = none;
                            }
                            continue;
                        }
                        let percentage = true;
                        let length =
                            resolver.length(value, property, false, false, percentage, calls)?;
                        match (index, length) {
                            (2, BoxLengthFact::Absolute(v)) => {
                                row.min_width = LengthPercentageBoxFact::Absolute(v)
                            }
                            (2, BoxLengthFact::Percentage(v)) => {
                                row.min_width = LengthPercentageBoxFact::Percentage(v)
                            }
                            (3, BoxLengthFact::Absolute(v)) => {
                                row.min_height = LengthPercentageBoxFact::Absolute(v)
                            }
                            (3, BoxLengthFact::Percentage(v)) => {
                                row.min_height = LengthPercentageBoxFact::Percentage(v)
                            }
                            (4, BoxLengthFact::Absolute(v)) => {
                                row.max_width = BoxMaxFact::Absolute(v)
                            }
                            (4, BoxLengthFact::Percentage(v)) => {
                                row.max_width = BoxMaxFact::Percentage(v)
                            }
                            (5, BoxLengthFact::Absolute(v)) => {
                                row.max_height = BoxMaxFact::Absolute(v)
                            }
                            (5, BoxLengthFact::Percentage(v)) => {
                                row.max_height = BoxMaxFact::Percentage(v)
                            }
                            _ => return Err(resolver.invalid(property)),
                        }
                        continue;
                    }
                    if (4..6).contains(&index) {
                        if matches!(value,SpecifiedValue::Keyword(k) if k.as_str()=="none") {
                            continue;
                        }
                        // Every non-none absolute max is outside the profile; do not
                        // convert an operand whose numeric value cannot admit it.
                        let SpecifiedValue::Length(target) = value else {
                            return Err(resolver.invalid(property));
                        };
                        let reason = match target {
                            LengthTarget::Relative(..) => BoxFactUnsupported::RelativeLength,
                            LengthTarget::Percentage(..) => BoxFactUnsupported::PercentageLength,
                            LengthTarget::Auto => BoxFactUnsupported::AutoForProperty,
                            LengthTarget::Absolute(..) => {
                                if target.is_negative() {
                                    return Err(resolver.invalid(property));
                                }
                                BoxFactUnsupported::NondefaultMinMax
                            }
                        };
                        return Err(resolver.unsupported(property, reason));
                    }
                    let auto = index < 2 || (6..10).contains(&index);
                    let signed = (6..10).contains(&index);
                    // PCT: Sequence keeps `width` and margin percentages (CSS2.2 10.2, 8.3),
                    // PPM its padding percentages (8.4) and PHT its `height` percentage
                    // (10.5).
                    let percentage = mode == BoxFactMode::Sequence
                        && (index < 2 || signed || (10..14).contains(&index));
                    let length =
                        resolver.length(value, property, auto, signed, percentage, calls)?;
                    match index {
                        0 => row.width = length,
                        1 => row.height = length,
                        2..=3 => {
                            if !matches!(length,BoxLengthFact::Absolute(v) if v.pixels==0.) {
                                return Err(resolver
                                    .unsupported(property, BoxFactUnsupported::NondefaultMinMax));
                            }
                        }
                        6..=9 => row.margin[index - 6] = length,
                        10..=13 => {
                            row.padding[index - 10] = match length {
                                BoxLengthFact::Absolute(v) => LengthPercentageBoxFact::Absolute(v),
                                BoxLengthFact::Percentage(v) => {
                                    LengthPercentageBoxFact::Percentage(v)
                                }
                                BoxLengthFact::Auto { .. } => {
                                    return Err(resolver.invalid(property));
                                }
                            };
                        }
                        _ => return Err(resolver.invalid(property)),
                    }
                } else if index < 18 {
                    let style = BorderStyle::from_resolved(value)
                        .map_err(|_| resolver.invalid(property))?;
                    // Legacy keeps its accepted set. Sequence admits every CSS2.2 style,
                    // which then matters only through the computed width.
                    if mode == BoxFactMode::Legacy && !suppressed(style) {
                        return Err(resolver.unsupported(property, BoxFactUnsupported::BorderStyle));
                    }
                    row.border_style[index - 14] = KeywordBoxFact {
                        value: style,
                        origin: BoxFactOrigin::Specified,
                    };
                } else if index < 20 {
                    let SpecifiedValue::Keyword(k) = value else {
                        return Err(resolver.invalid(property));
                    };
                    if index == 18 {
                        row.clear = KeywordBoxFact {
                            value: match k.as_str() {
                                "none" => ClearSide::None,
                                "left" => ClearSide::Left,
                                "right" => ClearSide::Right,
                                "both" => ClearSide::Both,
                                _ => return Err(resolver.invalid(property)),
                            },
                            origin: BoxFactOrigin::Specified,
                        };
                    } else {
                        row.overflow = KeywordBoxFact {
                            value: match k.as_str() {
                                "visible" => BoxOverflow::Visible,
                                "hidden" => BoxOverflow::Hidden,
                                "scroll" => BoxOverflow::Scroll,
                                "auto" => BoxOverflow::Auto,
                                _ => return Err(resolver.invalid(property)),
                            },
                            origin: BoxFactOrigin::Specified,
                        };
                    }
                } else {
                    // Sequence only: indices 20-23, after their own side's style.
                    let side = index - 20;
                    row.border_width[side] = resolver.border_width(
                        value,
                        property,
                        row.border_style[side].value,
                        calls,
                    )?;
                }
            }
        }
    }
    Ok(row)
}
/// REL (CSS2.2 9.3.2, 9.4.3): the computed `top`, `right`, `bottom` and `left` (25-28)
/// of a relatively positioned Sequence row, in index order: `auto`, or an absolute
/// length converted like a signed margin (one counted conversion), a percentage kept
/// exactly (PCT: `right`, `left`; PTB: `top`, `bottom`), or, for `inherit`, the parent
/// row's computed offset (`auto`, an absolute length or a percentage).
pub(super) fn offsets(
    winners: &PropertyWinners<'_>,
    ids: &[PropertyId; 29],
    original: usize,
    parent: Option<(usize, ClearanceBoxFact)>,
    numeric: NumericLimits,
    calls: &mut usize,
) -> Result<[BoxLengthFact; 4], E> {
    let resolver = Resolver { original, numeric };
    let mut offset = [BoxLengthFact::Auto {
        origin: BoxFactOrigin::Initial,
    }; 4];
    for (side, &property) in ids[25..].iter().enumerate() {
        let defaulted = winners
            .defaulted(property, parent.is_some())
            .map_err(|error| E::Defaulting {
                original,
                property,
                error,
            })?;
        offset[side] = match defaulted {
            DefaultedValueRef::Initial(_) => {
                recipe(property, 25 + side)?;
                BoxLengthFact::Auto {
                    origin: BoxFactOrigin::Initial,
                }
            }
            // Q-4 (a): `inherit` copies the parent row's resolved offset, which exists
            // only when the parent row is itself relatively positioned.
            DefaultedValueRef::ParentComputed => {
                let (original_parent, p) = parent.ok_or_else(|| resolver.invalid(property))?;
                let parent_offset = p.offset.ok_or_else(|| {
                    resolver.unsupported(property, BoxFactUnsupported::InheritedOffset)
                })?;
                inherited(
                    parent_offset[side],
                    BoxFactOrigin::Inherited { original_parent },
                )
            }
            // PCT: `right` and `left` keep a percentage of the containing block's width,
            // PTB `top` and `bottom` one of its height (CSS2.2 9.3.2).
            DefaultedValueRef::Compute(value) => {
                resolver.length(value, property, true, true, true, calls)?
            }
        };
    }
    Ok(offset)
}
pub(super) fn pseudo(
    winners: &PropertyWinners<'_>,
    property: PropertyId,
    original: usize,
    target: BoxFactTarget,
) -> Result<PseudoAbsence, E> {
    // Every pseudo has its originating element, including pseudos on the root.
    let origin = match winners
        .defaulted(property, true)
        .map_err(|error| E::Defaulting {
            original,
            property,
            error,
        })? {
        DefaultedValueRef::Initial(_) => {
            recipe(property, 24)?;
            BoxFactOrigin::Initial
        }
        DefaultedValueRef::ParentComputed => BoxFactOrigin::Inherited {
            original_parent: original,
        },
        DefaultedValueRef::Compute(_) => {
            let candidate = winners
                .candidate(property)
                .map_err(|_| E::InvalidValue { original, property })?
                .ok_or(E::InvalidValue { original, property })?;
            let content_target = match target {
                BoxFactTarget::Before => ContentTarget::Before,
                BoxFactTarget::After => ContentTarget::After,
                BoxFactTarget::Element => return Err(E::InvalidValue { original, property }),
            };
            match select_content(candidate.value, content_target).map_err(|error| E::Content {
                original,
                target,
                error,
            })? {
                ContentSelection::Suppressed => {}
                ContentSelection::Items(_) => {
                    return Err(E::Unsupported {
                        original,
                        property,
                        target,
                        reason: BoxFactUnsupported::GeneratedContent,
                    });
                }
                ContentSelection::Normal => return Err(E::InvalidValue { original, property }),
            }
            BoxFactOrigin::Specified
        }
    };
    Ok(PseudoAbsence { origin })
}
