//! Final precedence between supplied faces with identical CSS matching properties.
//!
//! This is a pairwise policy primitive, not font matching or catalogue validation.
//! No font bytes are inspected, retained or copied.
//!
//! Not yet integrated: the `iced-css` `BoxTree` widget does not call this module; face
//! selection and font metrics go through cosmic-text.

use std::cmp::Ordering;

/// Caller-supplied local identity and ordering facts for an eligible face.
///
/// Numeric labels are not allocated host handles. Zero and integer maxima are
/// ordinary values. Identities carry equality, never an implicit preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FacePrecedenceKey {
    /// One face of a successfully validated and published application registration.
    Application {
        /// Logical registration identity within one host, independent of bytes.
        registration: u64,
        /// Immutable unique request ordinal; larger means a later request.
        request_rank: u64,
        /// Actual collection face index; zero for an individual face.
        face_index: u32,
    },
    /// One face in a supplied immutable provider snapshot.
    System {
        /// Local face identity in that snapshot.
        face: u64,
        /// Unique position in the explicit provider sequence; smaller wins.
        /// Discovery/completion arrival order is not a substitute for this order.
        preference_order: u64,
    },
}

/// Preference between two supplied keys, without retaining either candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FacePreference {
    Left,
    Right,
    /// Consistent keys identify the same logical face. This does not certify
    /// font bytes, metadata, namespace provenance or native selection.
    SameFace,
}

/// Contradictory identity/order facts in the supplied pair; no winner exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FacePrecedenceError {
    RegistrationRankConflict,
    DuplicateRequestRank,
    SystemOrderConflict,
    DuplicateSystemOrder,
}

impl std::fmt::Display for FacePrecedenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::RegistrationRankConflict => "one font registration has conflicting request ranks",
            Self::DuplicateRequestRank => "distinct font registrations share a request rank",
            Self::SystemOrderConflict => {
                "one system font face has conflicting preference positions"
            }
            Self::DuplicateSystemOrder => "distinct system font faces share a preference position",
        })
    }
}

impl std::error::Error for FacePrecedenceError {}

/// Compare two eligible faces after identical CSS matching properties are established.
///
/// Application faces precede system faces. Across application registrations,
/// later request rank wins, independently of validation completion or native
/// forwarding acknowledgement. Within one registration, lower face index wins.
/// System faces follow the supplied stable provider sequence.
///
/// # Caller obligations
///
/// Both keys must belong to the same host/catalogue comparison namespace, and
/// system keys to the same provider snapshot and order interpretation. The caller
/// must establish eligibility and equality of family/style/variant/weight/size
/// matching properties, including its resolved size-matching policy. This
/// function receives no metadata and cannot verify these conditions. It must not
/// rank different matching groups or bypass missing-glyph retry and fallback.
/// Emergency fallback faces must not be represented as ordinary entries here.
///
/// Each registration must have one unique immutable request rank. Each system
/// identity must have one unique preference position. Only this pair is checked:
/// comparing every candidate with a winning key does not validate conflicting
/// losing keys, a whole group, cross-host collisions or stale snapshot input.
/// Equal font bytes do not merge distinct registrations. A successful result
/// establishes only preference between the supplied keys, not native selection.
///
/// # Errors
///
/// Rejects conflicting ranks for the same registration, a rank shared by
/// different registrations, conflicting positions for the same system face,
/// or a position shared by different system faces. No fallback winner is chosen.
///
/// # Resources
///
/// Two fixed-size inputs and one fixed-size result; no allocation, loop,
/// recursion, external callback or ordering arithmetic. Work and storage are
/// constant. A future candidate traversal must account for its own visits and
/// group validation; this function supplies no aggregate resource guarantee.
pub fn compare_equal_property_face_keys(
    left: FacePrecedenceKey,
    right: FacePrecedenceKey,
) -> Result<FacePreference, FacePrecedenceError> {
    use FacePrecedenceError as E;
    use FacePrecedenceKey as K;
    use FacePreference as P;

    match (left, right) {
        (K::Application { .. }, K::System { .. }) => Ok(P::Left),
        (K::System { .. }, K::Application { .. }) => Ok(P::Right),
        (
            K::Application {
                registration: left_id,
                request_rank: left_rank,
                face_index: left_index,
            },
            K::Application {
                registration: right_id,
                request_rank: right_rank,
                face_index: right_index,
            },
        ) => {
            let same_registration = left_id == right_id;
            let same_rank = left_rank == right_rank;
            if same_registration && !same_rank {
                return Err(E::RegistrationRankConflict);
            }
            if !same_registration && same_rank {
                return Err(E::DuplicateRequestRank);
            }
            if !same_registration {
                return Ok(if left_rank > right_rank {
                    P::Left
                } else {
                    P::Right
                });
            }
            Ok(match left_index.cmp(&right_index) {
                Ordering::Less => P::Left,
                Ordering::Equal => P::SameFace,
                Ordering::Greater => P::Right,
            })
        }
        (
            K::System {
                face: left_id,
                preference_order: left_order,
            },
            K::System {
                face: right_id,
                preference_order: right_order,
            },
        ) => {
            let same_face = left_id == right_id;
            let same_order = left_order == right_order;
            if same_face && !same_order {
                return Err(E::SystemOrderConflict);
            }
            if !same_face && same_order {
                return Err(E::DuplicateSystemOrder);
            }
            if same_face {
                return Ok(P::SameFace);
            }
            Ok(if left_order < right_order {
                P::Left
            } else {
                P::Right
            })
        }
    }
}
