use super::*;
use ClearanceBoxFactsError as E;
const NAMES: [&str; 21] = [
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
    "content",
];
const RECIPES: [&str; 21] = [
    "auto", "auto", "0", "0", "none", "none", "0", "0", "0", "0", "0", "0", "0", "0", "none",
    "none", "none", "none", "none", "visible", "normal",
];
pub(super) fn property_ids() -> Result<[PropertyId; 21], E> {
    let first = PropertyId::lookup(NAMES[0]).ok_or(E::PropertyRegistry { name: NAMES[0] })?;
    let mut ids = [first; 21];
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
pub(super) fn initial_row() -> ClearanceBoxFact {
    ClearanceBoxFact {
        width: BoxLengthFact::Auto {
            origin: BoxFactOrigin::Initial,
        },
        height: BoxLengthFact::Auto {
            origin: BoxFactOrigin::Initial,
        },
        margin: [BoxLengthFact::Absolute(ZERO); 4],
        padding: [ZERO; 4],
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
        before: PseudoAbsence {
            origin: BoxFactOrigin::Initial,
        },
        after: PseudoAbsence {
            origin: BoxFactOrigin::Initial,
        },
    }
}
fn inherited(v: BoxLengthFact, origin: BoxFactOrigin) -> BoxLengthFact {
    match v {
        BoxLengthFact::Absolute(v) => BoxLengthFact::Absolute(AbsoluteBoxFact { origin, ..v }),
        BoxLengthFact::Auto { .. } => BoxLengthFact::Auto { origin },
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
    fn length(
        &self,
        value: &SpecifiedValue,
        property: PropertyId,
        index: usize,
        calls: &mut usize,
    ) -> Result<BoxLengthFact, E> {
        let SpecifiedValue::Length(target) = value else {
            return Err(self.invalid(property));
        };
        match target {
            LengthTarget::Relative(..) => {
                return Err(self.unsupported(property, BoxFactUnsupported::RelativeLength));
            }
            LengthTarget::Percentage(..) => {
                return Err(self.unsupported(property, BoxFactUnsupported::PercentageLength));
            }
            LengthTarget::Auto => {
                return if index < 2 || (6..10).contains(&index) {
                    Ok(BoxLengthFact::Auto {
                        origin: BoxFactOrigin::Specified,
                    })
                } else {
                    Err(self.unsupported(property, BoxFactUnsupported::AutoForProperty))
                };
            }
            LengthTarget::Absolute(..) => {}
        }
        if !(6..10).contains(&index) && target.is_negative() {
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
}
pub(super) fn element(
    winners: &PropertyWinners<'_>,
    ids: &[PropertyId; 21],
    original: usize,
    parent: Option<(usize, ClearanceBoxFact)>,
    numeric: NumericLimits,
    calls: &mut usize,
) -> Result<ClearanceBoxFact, E> {
    let resolver = Resolver { original, numeric };
    let mut row = initial_row();
    for (index, &property) in ids[..20].iter().enumerate() {
        let defaulted = winners
            .defaulted(property, parent.is_some())
            .map_err(|error| E::Defaulting {
                original,
                property,
                error,
            })?;
        match defaulted {
            DefaultedValueRef::Initial(_) => recipe(property, index)?,
            DefaultedValueRef::ParentComputed => {
                let (original_parent, p) = parent.ok_or_else(|| resolver.invalid(property))?;
                let origin = BoxFactOrigin::Inherited { original_parent };
                match index {
                    0 => row.width = inherited(p.width, origin),
                    1 => row.height = inherited(p.height, origin),
                    // A successful parent's constraints are already computed zero/none.
                    2..=5 => {}
                    6..=9 => row.margin[index - 6] = inherited(p.margin[index - 6], origin),
                    10..=13 => {
                        row.padding[index - 10] = AbsoluteBoxFact {
                            origin,
                            ..p.padding[index - 10]
                        }
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
                    _ => return Err(resolver.invalid(property)),
                }
            }
            DefaultedValueRef::Compute(value) => {
                if index < 14 {
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
                    let length = resolver.length(value, property, index, calls)?;
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
                            let BoxLengthFact::Absolute(v) = length else {
                                return Err(resolver.invalid(property));
                            };
                            row.padding[index - 10] = v;
                        }
                        _ => return Err(resolver.invalid(property)),
                    }
                } else if index < 18 {
                    let style = BorderStyle::from_resolved(value)
                        .map_err(|_| resolver.invalid(property))?;
                    if !matches!(style, BorderStyle::None | BorderStyle::Hidden) {
                        return Err(resolver.unsupported(property, BoxFactUnsupported::BorderStyle));
                    }
                    row.border_style[index - 14] = KeywordBoxFact {
                        value: style,
                        origin: BoxFactOrigin::Specified,
                    };
                } else {
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
                }
            }
        }
    }
    Ok(row)
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
            recipe(property, 20)?;
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
