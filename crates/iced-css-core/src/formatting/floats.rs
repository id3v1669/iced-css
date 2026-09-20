//! CSS 2.2 §9.5.1 float placement and the exclusion bands line layout consumes.
//!
//! Already-placed floats arrive as a slice in source placement order and the caller
//! owns their storage, so one block formatting context's list never reaches another.
//! Clearance, the positioned solvers and line-box interaction are separate.
use std::num::NonZeroUsize;

/// The side a float is anchored to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatSide {
    Left,
    Right,
}
/// A placed float's margin box within one block formatting context.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlacedFloat {
    pub side: FloatSide,
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
}
/// A horizontal interval; `left <= right` for every admitted value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Band {
    pub left: f32,
    pub right: f32,
}
impl Band {
    #[must_use]
    pub fn width(self) -> f32 {
        self.right - self.left
    }
}
/// A float to place.
#[derive(Debug, Clone, Copy)]
pub struct FloatRequest {
    pub side: FloatSide,
    /// Margin-box width, already resolved by the shrink-to-fit category where CSS
    /// requires it.
    pub width: f32,
    /// Margin-box height.
    pub height: f32,
    /// The containing block's top edge; §9.5.1 rule 4 keeps the float at or below it.
    pub containing_top: f32,
    /// Where the float would sit with no earlier float in the way. It carries the rules
    /// this module does not own: §9.5.1 rule 5's collapsing-margin parent, rule 7's
    /// earlier line boxes, and the earlier-*block* half of rule 6.
    pub hypothetical_top: f32,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FloatPlacement {
    pub placed: PlacedFloat,
    /// Bands examined before the float fit. Evidence that the search progressed;
    /// never a retry allowance standing in for layout behaviour.
    pub bands: usize,
    /// The float did not fit in any band and was placed flush to its containing
    /// block edge, which is §9.5.1 rule 8's "already as far left as possible".
    pub overflows: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatError {
    InvalidInput(&'static str),
    /// The band budget was exhausted. Candidates are bounded by the float count plus
    /// one, so a budget of `floats.len() + 1` can never hit this; it is a resource
    /// guard rather than a layout outcome.
    Work,
}
impl std::fmt::Display for FloatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "float placement: {self:?}")
    }
}
impl std::error::Error for FloatError {}
fn finite(value: f32, field: &'static str) -> Result<(), FloatError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(FloatError::InvalidInput(field))
    }
}
fn validate(floats: &[PlacedFloat], containing: Band) -> Result<(), FloatError> {
    finite(containing.left, "containing_left")?;
    finite(containing.right, "containing_right")?;
    if containing.left > containing.right {
        return Err(FloatError::InvalidInput("containing_order"));
    }
    for float in floats {
        finite(float.left, "float_left")?;
        finite(float.right, "float_right")?;
        finite(float.top, "float_top")?;
        finite(float.bottom, "float_bottom")?;
        if float.left > float.right {
            return Err(FloatError::InvalidInput("float_horizontal_order"));
        }
        if float.top > float.bottom {
            return Err(FloatError::InvalidInput("float_vertical_order"));
        }
    }
    Ok(())
}
/// Narrows the containing interval by every float occupying `y`. The result may be
/// inverted, which means the floats overlap and nothing at all fits, not even a
/// zero-width box; callers that need a reportable interval clamp it themselves.
fn narrow(floats: &[PlacedFloat], containing: Band, y: f32) -> (f32, f32) {
    let mut left = containing.left;
    let mut right = containing.right;
    // A float occupies `y` when its own vertical span covers it. Narrowing by the right
    // edges of left floats and the left edges of right floats also discharges §9.5.1
    // rule 3, which forbids a left float's right edge from passing the left edge of a
    // right float next to it.
    for float in floats
        .iter()
        .filter(|float| float.top <= y && float.bottom > y)
    {
        match float.side {
            FloatSide::Left => left = left.max(float.right),
            FloatSide::Right => right = right.min(float.left),
        }
    }
    (left, right)
}
/// The interval still available at `y`, narrowed by every float whose own vertical span
/// covers `y`. An interval closed by opposing floats is reported empty, never inverted.
///
/// Line layout may call this at any position, including above a float that starts
/// lower, so the test is a genuine vertical overlap rather than the weaker "ends below
/// `y`". Inside [`place_float`] the two coincide, because §9.5.1 rule 6 has already
/// raised the candidate to or below every earlier float's top.
///
/// # Errors
/// Returns [`FloatError::InvalidInput`] for a nonfinite or inverted input.
pub fn exclusion_band(
    floats: &[PlacedFloat],
    containing: Band,
    y: f32,
) -> Result<Band, FloatError> {
    validate(floats, containing)?;
    finite(y, "y")?;
    let (left, right) = narrow(floats, containing, y);
    Ok(Band {
        left,
        right: right.max(left),
    })
}
/// The next position below `y` at which the available interval can change, which is
/// the smallest bottom edge strictly below `y`.
///
/// # Errors
/// Returns [`FloatError::InvalidInput`] for a nonfinite or inverted input.
pub fn next_band_edge(floats: &[PlacedFloat], y: f32) -> Result<Option<f32>, FloatError> {
    finite(y, "y")?;
    let mut next: Option<f32> = None;
    for float in floats {
        finite(float.bottom, "float_bottom")?;
        if float.bottom > y {
            next = Some(next.map_or(float.bottom, |current: f32| current.min(float.bottom)));
        }
    }
    Ok(next)
}
/// Places one float under §9.5.1.
///
/// The requested top is first raised to the containing block's top (rule 4) and to the
/// outer top of every earlier float (rule 6). Bands are then examined downward: the
/// float is placed flush to its own side as soon as its margin-box width fits (rules 9
/// and 10), and otherwise the search advances to the next band edge. Candidates strictly
/// increase and after the first are drawn from the finite set of float bottoms, so at
/// most `floats.len() + 1` are examined and the search always terminates. Two candidates
/// may still expose the same interval; what is guaranteed is progress, not distinctness.
/// With no band left the float is placed flush to its containing block edge and the
/// result records the overflow, rather than being scaled.
///
/// # Errors
/// Returns [`FloatError::InvalidInput`] for a nonfinite or negative measurement and
/// [`FloatError::Work`] when the band budget is exhausted.
pub fn place_float(
    floats: &[PlacedFloat],
    containing: Band,
    request: FloatRequest,
    work: NonZeroUsize,
) -> Result<FloatPlacement, FloatError> {
    validate(floats, containing)?;
    finite(request.width, "width")?;
    finite(request.height, "height")?;
    finite(request.hypothetical_top, "hypothetical_top")?;
    finite(request.containing_top, "containing_top")?;
    if request.width < 0.0 {
        return Err(FloatError::InvalidInput("width"));
    }
    if request.height < 0.0 {
        return Err(FloatError::InvalidInput("height"));
    }
    // Rule 4 and rule 6: no higher than the containing block's top, and no higher than
    // the outer top of any float generated earlier in the source document. Rule 6 also
    // covers earlier *blocks*, whose tops reach this solver through `hypothetical_top`
    // exactly as rules 5 and 7 do.
    let mut y = request.hypothetical_top.max(request.containing_top);
    for float in floats {
        y = y.max(float.top);
    }
    let mut bands = 0;
    loop {
        bands += 1;
        if bands > work.get() {
            return Err(FloatError::Work);
        }
        let (available_left, available_right) = narrow(floats, containing, y);
        // An inverted interval means opposing floats overlap here, so nothing fits,
        // not even a zero-width box; clamping first would place one inside a float.
        if available_right >= available_left && request.width <= available_right - available_left {
            let (left, right) = match request.side {
                FloatSide::Left => (available_left, available_left + request.width),
                FloatSide::Right => (available_right - request.width, available_right),
            };
            return Ok(FloatPlacement {
                placed: PlacedFloat {
                    side: request.side,
                    left,
                    right,
                    top: y,
                    bottom: y + request.height,
                },
                bands,
                overflows: false,
            });
        }
        match next_band_edge(floats, y)? {
            Some(next) => y = next,
            None => {
                // Rule 8 is what forced every descent above: while another same-side
                // float is beside it, an oversized float would have its outer edge past
                // the containing block and is forbidden there. No float reaches this
                // band, so rule 8's precondition is unmet and nothing forbids the box
                // sticking out; rule 1 still pins its own-side edge. It keeps its
                // computed width rather than being reduced.
                let (left, right) = match request.side {
                    FloatSide::Left => (containing.left, containing.left + request.width),
                    FloatSide::Right => (containing.right - request.width, containing.right),
                };
                return Ok(FloatPlacement {
                    placed: PlacedFloat {
                        side: request.side,
                        left,
                        right,
                        top: y,
                        bottom: y + request.height,
                    },
                    bands,
                    overflows: true,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn nz(value: usize) -> NonZeroUsize {
        NonZeroUsize::new(value).unwrap()
    }
    fn containing() -> Band {
        Band {
            left: 0.0,
            right: 100.0,
        }
    }
    fn request(side: FloatSide, width: f32) -> FloatRequest {
        FloatRequest {
            side,
            width,
            height: 10.0,
            containing_top: 0.0,
            hypothetical_top: 0.0,
        }
    }
    #[test]
    fn an_empty_context_places_each_side_flush_to_its_own_edge() {
        for (side, left, right) in [
            (FloatSide::Left, 0.0, 30.0),
            (FloatSide::Right, 70.0, 100.0),
        ] {
            let placement = place_float(&[], containing(), request(side, 30.0), nz(4)).unwrap();
            assert_eq!(placement.placed.left, left);
            assert_eq!(placement.placed.right, right);
            assert_eq!(placement.placed.top, 0.0);
            assert_eq!((placement.bands, placement.overflows), (1, false));
        }
    }
    #[test]
    fn a_second_float_sits_beside_the_first_while_it_fits() {
        let first = place_float(&[], containing(), request(FloatSide::Left, 30.0), nz(4)).unwrap();
        let second = place_float(
            &[first.placed],
            containing(),
            request(FloatSide::Left, 30.0),
            nz(4),
        )
        .unwrap();
        assert_eq!(second.placed.left, 30.0);
        assert_eq!(second.placed.right, 60.0);
        assert_eq!(
            second.placed.top, 0.0,
            "rule 9 keeps it as high as possible"
        );
        assert_eq!(second.bands, 1);
    }
    #[test]
    fn a_float_that_does_not_fit_beside_earlier_ones_drops_to_the_next_band() {
        // Two earlier left floats ending at different depths; the third fits only
        // below the shallower one.
        let floats = [
            PlacedFloat {
                side: FloatSide::Left,
                left: 0.0,
                right: 60.0,
                top: 0.0,
                bottom: 10.0,
            },
            PlacedFloat {
                side: FloatSide::Right,
                left: 80.0,
                right: 100.0,
                top: 0.0,
                bottom: 30.0,
            },
        ];
        let placement =
            place_float(&floats, containing(), request(FloatSide::Left, 50.0), nz(8)).unwrap();
        // At y = 0 only 60..80 is free; at y = 10 the left float is past, leaving 0..80.
        assert_eq!(placement.placed.top, 10.0);
        assert_eq!(placement.placed.left, 0.0);
        assert_eq!(placement.placed.right, 50.0);
        assert_eq!((placement.bands, placement.overflows), (2, false));
    }
    #[test]
    fn rule_four_keeps_a_float_no_higher_than_its_containing_block() {
        let mut above = request(FloatSide::Left, 10.0);
        above.containing_top = 15.0;
        above.hypothetical_top = 5.0;
        let placement = place_float(&[], containing(), above, nz(4)).unwrap();
        assert_eq!(placement.placed.top, 15.0);
        assert_eq!(placement.placed.bottom, 25.0);
    }
    #[test]
    fn rule_six_keeps_a_float_no_higher_than_an_earlier_one() {
        let floats = [PlacedFloat {
            side: FloatSide::Left,
            left: 0.0,
            right: 10.0,
            top: 25.0,
            bottom: 35.0,
        }];
        let placement = place_float(
            &floats,
            containing(),
            request(FloatSide::Right, 10.0),
            nz(4),
        )
        .unwrap();
        assert_eq!(
            placement.placed.top, 25.0,
            "not higher than the earlier float's outer top"
        );
    }
    #[test]
    fn an_oversized_float_overflows_its_containing_block_instead_of_being_reduced() {
        let placement =
            place_float(&[], containing(), request(FloatSide::Left, 140.0), nz(4)).unwrap();
        assert!(placement.overflows);
        assert_eq!(placement.placed.left, 0.0);
        assert_eq!(placement.placed.right, 140.0, "kept at its computed width");
        assert_eq!(placement.bands, 1);
    }
    #[test]
    fn an_oversized_float_first_descends_past_every_earlier_float() {
        let floats = [PlacedFloat {
            side: FloatSide::Left,
            left: 0.0,
            right: 20.0,
            top: 0.0,
            bottom: 10.0,
        }];
        let placement = place_float(
            &floats,
            containing(),
            request(FloatSide::Left, 140.0),
            nz(8),
        )
        .unwrap();
        assert_eq!(
            placement.placed.top, 10.0,
            "descends to the last band first"
        );
        assert!(placement.overflows);
        assert_eq!(placement.bands, 2);
    }
    #[test]
    fn the_band_narrows_from_both_sides_and_never_inverts() {
        let floats = [
            PlacedFloat {
                side: FloatSide::Left,
                left: 0.0,
                right: 70.0,
                top: 0.0,
                bottom: 10.0,
            },
            PlacedFloat {
                side: FloatSide::Right,
                left: 40.0,
                right: 100.0,
                top: 0.0,
                bottom: 10.0,
            },
        ];
        let band = exclusion_band(&floats, containing(), 0.0).unwrap();
        assert_eq!(band.left, 70.0);
        assert_eq!(
            band.right, 70.0,
            "an over-excluded band is empty, not inverted"
        );
        assert_eq!(band.width(), 0.0);
        assert_eq!(
            exclusion_band(&floats, containing(), 10.0).unwrap(),
            containing()
        );
    }
    #[test]
    fn a_band_ignores_a_float_that_begins_below_it() {
        // Line layout queries arbitrary positions, so a float starting lower must not
        // narrow a band above it. Inside place_float rule 6 makes this case unreachable.
        let later = [PlacedFloat {
            side: FloatSide::Left,
            left: 0.0,
            right: 40.0,
            top: 50.0,
            bottom: 60.0,
        }];
        assert_eq!(
            exclusion_band(&later, containing(), 0.0).unwrap(),
            containing()
        );
        assert_eq!(
            exclusion_band(&later, containing(), 50.0).unwrap(),
            Band {
                left: 40.0,
                right: 100.0
            }
        );
    }
    #[test]
    fn overlapping_opposing_floats_admit_nothing_not_even_a_zero_width_box() {
        // The interval here is inverted, not merely empty: placing a zero-width float
        // at the clamped edge would put it inside the right float, breaking rule 3.
        let overlapping = [
            PlacedFloat {
                side: FloatSide::Left,
                left: 0.0,
                right: 70.0,
                top: 0.0,
                bottom: 10.0,
            },
            PlacedFloat {
                side: FloatSide::Right,
                left: 40.0,
                right: 100.0,
                top: 0.0,
                bottom: 10.0,
            },
        ];
        let placement = place_float(
            &overlapping,
            containing(),
            request(FloatSide::Left, 0.0),
            nz(4),
        )
        .unwrap();
        assert_eq!(
            placement.placed.top, 10.0,
            "it must descend past the overlap"
        );
        assert_eq!(placement.placed.left, 0.0);
        assert_eq!(placement.bands, 2);
        // A band closed exactly, without inversion, still admits a zero-width box.
        let touching = [
            PlacedFloat {
                side: FloatSide::Left,
                left: 0.0,
                right: 50.0,
                top: 0.0,
                bottom: 10.0,
            },
            PlacedFloat {
                side: FloatSide::Right,
                left: 50.0,
                right: 100.0,
                top: 0.0,
                bottom: 10.0,
            },
        ];
        let touched = place_float(
            &touching,
            containing(),
            request(FloatSide::Left, 0.0),
            nz(4),
        )
        .unwrap();
        assert_eq!((touched.placed.top, touched.placed.left), (0.0, 50.0));
        assert_eq!(touched.bands, 1);
    }
    #[test]
    fn a_zero_height_float_occupies_no_band_and_adds_no_edge() {
        let flat = [PlacedFloat {
            side: FloatSide::Left,
            left: 0.0,
            right: 90.0,
            top: 0.0,
            bottom: 0.0,
        }];
        assert_eq!(
            exclusion_band(&flat, containing(), 0.0).unwrap(),
            containing()
        );
        assert_eq!(next_band_edge(&flat, 0.0).unwrap(), None);
        let placement =
            place_float(&flat, containing(), request(FloatSide::Left, 30.0), nz(4)).unwrap();
        assert_eq!((placement.placed.top, placement.placed.left), (0.0, 0.0));
        assert_eq!(placement.bands, 1);
    }
    #[test]
    fn band_edges_are_the_float_bottoms_strictly_below_the_position() {
        let floats = [
            PlacedFloat {
                side: FloatSide::Left,
                left: 0.0,
                right: 10.0,
                top: 0.0,
                bottom: 10.0,
            },
            PlacedFloat {
                side: FloatSide::Right,
                left: 90.0,
                right: 100.0,
                top: 0.0,
                bottom: 30.0,
            },
        ];
        assert_eq!(next_band_edge(&floats, 0.0).unwrap(), Some(10.0));
        assert_eq!(next_band_edge(&floats, 10.0).unwrap(), Some(30.0));
        assert_eq!(next_band_edge(&floats, 30.0).unwrap(), None);
    }
    #[test]
    fn the_search_advances_through_candidates_and_reports_its_progress() {
        // Three nested left floats force three descents before the fourth fits.
        let floats = [
            PlacedFloat {
                side: FloatSide::Left,
                left: 0.0,
                right: 90.0,
                top: 0.0,
                bottom: 10.0,
            },
            PlacedFloat {
                side: FloatSide::Left,
                left: 0.0,
                right: 80.0,
                top: 0.0,
                bottom: 20.0,
            },
            PlacedFloat {
                side: FloatSide::Left,
                left: 0.0,
                right: 40.0,
                top: 0.0,
                bottom: 30.0,
            },
        ];
        let placement =
            place_float(&floats, containing(), request(FloatSide::Left, 50.0), nz(8)).unwrap();
        assert_eq!(placement.placed.top, 20.0);
        assert_eq!(placement.placed.left, 40.0);
        assert_eq!(placement.bands, 3, "one band per candidate top examined");
        // The budget is a resource guard; two bands are not enough to reach it.
        assert_eq!(
            place_float(&floats, containing(), request(FloatSide::Left, 50.0), nz(2)),
            Err(FloatError::Work)
        );
    }
    #[test]
    fn invalid_measurements_are_rejected_rather_than_normalised() {
        assert_eq!(
            place_float(&[], containing(), request(FloatSide::Left, -1.0), nz(4)),
            Err(FloatError::InvalidInput("width"))
        );
        assert_eq!(
            place_float(&[], containing(), request(FloatSide::Left, f32::NAN), nz(4)),
            Err(FloatError::InvalidInput("width"))
        );
        let mut oversized = request(FloatSide::Left, 10.0);
        oversized.height = -1.0;
        assert_eq!(
            place_float(&[], containing(), oversized, nz(4)),
            Err(FloatError::InvalidInput("height"))
        );
        assert_eq!(
            exclusion_band(
                &[],
                Band {
                    left: 10.0,
                    right: 0.0
                },
                0.0
            ),
            Err(FloatError::InvalidInput("containing_order"))
        );
        assert_eq!(
            exclusion_band(
                &[PlacedFloat {
                    side: FloatSide::Left,
                    left: 0.0,
                    right: 10.0,
                    top: 20.0,
                    bottom: 5.0,
                }],
                containing(),
                0.0
            ),
            Err(FloatError::InvalidInput("float_vertical_order"))
        );
    }
}
