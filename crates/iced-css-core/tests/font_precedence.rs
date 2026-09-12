//! Supplied equal-property keys, not font metadata or catalogue lifecycle tests.
use iced_css_core::text::font_precedence::{
    FacePrecedenceError as E, FacePrecedenceKey as K, FacePreference as P,
    compare_equal_property_face_keys as compare,
};

const fn app(registration: u64, request_rank: u64, face_index: u32) -> K {
    K::Application {
        registration,
        request_rank,
        face_index,
    }
}

const fn sys(face: u64, preference_order: u64) -> K {
    K::System {
        face,
        preference_order,
    }
}

// Independent policy oracle labels. Identity deliberately opposes request rank
// and provider order, so ordering the opaque labels cannot pass these fixtures.
const A0: K = app(99, 10, 0);
const A3: K = app(99, 10, 3);
const B0: K = app(1, 20, 0);
const B9: K = app(1, 20, 9);
const S0: K = sys(77, 0);
const S7: K = sys(3, 7);
const Z0: K = app(0, 0, 0);
const MMAX: K = app(u64::MAX, u64::MAX, u32::MAX);
const M0: K = app(u64::MAX, u64::MAX, 0);
const SMAX: K = sys(u64::MAX, u64::MAX);

fn reversed(result: Result<P, E>) -> Result<P, E> {
    result.map(|p| match p {
        P::Left => P::Right,
        P::Right => P::Left,
        P::SameFace => P::SameFace,
    })
}

#[test]
fn approved_pair_policy_and_conflicting_keys() {
    let cases = [
        (app(1, 1, u32::MAX), sys(1, 0), Ok(P::Left)),
        (sys(1, 0), app(1, 1, 0), Ok(P::Right)),
        (
            app(99, 10, 0),
            app(99, 11, 3),
            Err(E::RegistrationRankConflict),
        ),
        (
            app(99, 10, 0),
            app(99, 11, 0),
            Err(E::RegistrationRankConflict),
        ),
        (app(99, 10, 0), app(1, 10, 0), Err(E::DuplicateRequestRank)),
        (app(99, 10, 0), app(1, 10, 9), Err(E::DuplicateRequestRank)),
        (sys(77, 0), sys(77, 1), Err(E::SystemOrderConflict)),
        (sys(77, 0), sys(3, 0), Err(E::DuplicateSystemOrder)),
        (A0, B9, Ok(P::Right)),
        (B9, B0, Ok(P::Right)),
        (A0, A0, Ok(P::SameFace)),
        (S0, S0, Ok(P::SameFace)),
        (Z0, MMAX, Ok(P::Right)),
        (S0, SMAX, Ok(P::Left)),
        (M0, MMAX, Ok(P::Left)),
    ];
    for (left, right, expected) in cases {
        assert_eq!(compare(left, right), expected, "{left:?} / {right:?}");
        assert_eq!(compare(right, left), reversed(expected));
    }
}

// These literal orders were authored independently of the production algorithm.
const GROUPS: &[&[K]] = &[
    &[B0, B9, A0, A3, S0, S7],
    &[B9, A0],
    &[B0, A0],
    &[M0, MMAX, Z0],
    &[S0, S7, SMAX],
    &[Z0, S0],
];

// Test-only selection over a supplied valid group. It does not validate a
// catalogue, and the empty-set behavior is not part of the pairwise API.
fn winner(keys: impl IntoIterator<Item = K>) -> Option<K> {
    keys.into_iter().fold(None, |best, key| match best {
        None => Some(key),
        Some(best) => match compare(best, key).unwrap() {
            P::Left | P::SameFace => Some(best),
            P::Right => Some(key),
        },
    })
}

fn visit_subsets(order: &[K], prefix: &mut Vec<usize>, visits: &mut usize) {
    // The expected winner is the first present entry of the literal oracle
    // order, never an ordering recomputed from the keys' numeric fields.
    let expected = prefix.iter().min().map(|&i| order[i]);
    let actual = winner(prefix.iter().map(|&i| order[i]));
    assert_eq!(actual, expected, "visitation {prefix:?}");
    *visits += 1;
    for i in 0..order.len() {
        if !prefix.contains(&i) {
            prefix.push(i);
            visit_subsets(order, prefix, visits);
            prefix.pop();
        }
    }
}

#[test]
fn independent_groups_cover_all_pairs_and_subset_permutations() {
    let mut pairs = 0;
    let mut visits = 0;
    for order in GROUPS {
        for (i, &left) in order.iter().enumerate() {
            for (j, &right) in order.iter().enumerate() {
                let expected = match i.cmp(&j) {
                    std::cmp::Ordering::Less => P::Left,
                    std::cmp::Ordering::Equal => P::SameFace,
                    std::cmp::Ordering::Greater => P::Right,
                };
                assert_eq!(compare(left, right), Ok(expected));
                pairs += 1;
            }
        }
        visit_subsets(order, &mut Vec::new(), &mut visits);
    }
    assert_eq!(pairs, 66);
    assert_eq!(visits, 2004);
}

#[test]
fn supplied_eligible_snapshots_preserve_request_order() {
    // Only the supplied sets are exercised. The observations in the oracle
    // explain these sets; no registration/validation/removal lifecycle runs.
    let snapshots: [(&[K], &[K]); 7] = [
        (&[A0, A3, S0], &[A0, A3, S0]),
        (&[A0, A3, B0, B9, S0], &[B0, B9, A0, A3, S0]),
        (&[B9, A3, S0, B0, A0], &[B0, B9, A0, A3, S0]),
        (&[S0, A3, A0], &[A0, A3, S0]),
        (&[A0, S0, A3], &[A0, A3, S0]),
        (&[S7, S0], &[S0, S7]),
        (&[A0, B0], &[B0, A0]),
    ];
    for (eligible, order) in snapshots {
        // Remove successive preferred keys from the supplied set, checking the
        // entire expected order rather than only its first member.
        for next in 0..=order.len() {
            let remaining = eligible
                .iter()
                .copied()
                .filter(|key| !order[..next].contains(key));
            assert_eq!(winner(remaining), order.get(next).copied());
        }
    }
}

#[test]
fn named_errors_are_regular_errors() {
    let rows = [
        (
            E::RegistrationRankConflict,
            "one font registration has conflicting request ranks",
        ),
        (
            E::DuplicateRequestRank,
            "distinct font registrations share a request rank",
        ),
        (
            E::SystemOrderConflict,
            "one system font face has conflicting preference positions",
        ),
        (
            E::DuplicateSystemOrder,
            "distinct system font faces share a preference position",
        ),
    ];
    for (error, expected) in rows {
        assert_eq!(error.to_string(), expected);
        assert!(std::error::Error::source(&error).is_none());
    }
}
