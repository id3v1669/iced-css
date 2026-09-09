use iced_css_core::source::sibling_matches::*;
use std::num::{NonZeroU64, NonZeroUsize};
fn limits() -> MatchLimits {
    MatchLimits {
        entries: NonZeroUsize::new(100).unwrap(),
        retained_bytes: NonZeroUsize::new(4096).unwrap(),
        work: NonZeroUsize::new(10000).unwrap(),
    }
}
fn keyed(s: &str) -> SiblingKey<'_> {
    SiblingKey::Explicit(s.as_bytes())
}
#[test]
fn reorder_insert_remove_and_static_paths_have_distinct_identities() {
    let site = NonZeroU64::new(1).unwrap();
    let old = [
        keyed("a"),
        keyed("b"),
        SiblingKey::Static {
            site,
            role_path: &[0, 1],
        },
    ];
    let new = [
        keyed("c"),
        keyed("b"),
        SiblingKey::Static {
            site,
            role_path: &[0, 1],
        },
        keyed("a"),
        SiblingKey::Static {
            site,
            role_path: &[1, 0],
        },
    ];
    let plan = match_siblings(&old, &new, limits()).unwrap();
    assert_eq!(
        plan.previous_indices(),
        &[None, Some(1), Some(2), Some(0), None]
    );
    assert!(plan.retained_bytes() >= 5 * std::mem::size_of::<Option<usize>>());
    assert_eq!(
        match_siblings(&old, &[], limits())
            .unwrap()
            .previous_indices(),
        &[]
    );
    assert_eq!(
        match_siblings(&[], &new, limits())
            .unwrap()
            .previous_indices(),
        &[None; 5]
    );
}
#[test]
fn duplicates_in_both_generations_are_errors_with_located_indices() {
    let dup = [keyed("a"), keyed("b"), keyed("a")];
    assert_eq!(
        match_siblings(&dup, &[], limits()).unwrap_err(),
        MatchError::Duplicate {
            generation: Generation::Previous,
            first: 0,
            second: 2
        }
    );
    assert_eq!(
        match_siblings(&[], &dup, limits()).unwrap_err(),
        MatchError::Duplicate {
            generation: Generation::Incoming,
            first: 0,
            second: 2
        }
    );
    let same = SiblingKey::Static {
        site: NonZeroU64::new(2).unwrap(),
        role_path: &[7],
    };
    assert!(matches!(
        match_siblings(&[], &[same, same], limits()),
        Err(MatchError::Duplicate { .. })
    ));
    // An explicit empty application key remains a valid identity.
    assert_eq!(
        match_siblings(&[keyed("")], &[keyed("")], limits())
            .unwrap()
            .previous_indices(),
        &[Some(0)]
    );
}
#[test]
fn limits_cover_entry_count_allocation_and_long_key_comparisons() {
    let old = [keyed("aaaaaaaa")];
    let new = [keyed("aaaaaaab")];
    let mut tiny = limits();
    tiny.entries = NonZeroUsize::new(1).unwrap();
    assert_eq!(
        match_siblings(&old, &new, tiny).unwrap_err(),
        MatchError::Entries
    );
    tiny = limits();
    tiny.retained_bytes = NonZeroUsize::new(1).unwrap();
    assert_eq!(
        match_siblings(&old, &new, tiny).unwrap_err(),
        MatchError::Bytes
    );
    tiny = limits();
    tiny.work = NonZeroUsize::new(4).unwrap();
    assert_eq!(
        match_siblings(&old, &new, tiny).unwrap_err(),
        MatchError::Work
    );
}
#[test]
fn each_partial_permutation_preserves_only_equal_sibling_keys() {
    let keys = [keyed("a"), keyed("b"), keyed("c"), keyed("d")];
    let previous = &keys[..3];
    let mut cases = 0;
    for mask in 0usize..256 {
        let indices = [mask % 4, mask / 4 % 4, mask / 16 % 4, mask / 64 % 4];
        let incoming: Vec<_> = indices.iter().map(|i| keys[*i]).collect();
        let unique = (0..4).all(|i| (0..i).all(|j| indices[i] != indices[j]));
        let result = match_siblings(previous, &incoming, limits());
        if unique {
            let expected: Vec<_> = indices.iter().map(|i| (*i < 3).then_some(*i)).collect();
            assert_eq!(result.unwrap().previous_indices(), expected);
        } else {
            assert!(matches!(
                result,
                Err(MatchError::Duplicate {
                    generation: Generation::Incoming,
                    ..
                })
            ));
        }
        cases += 1;
    }
    assert_eq!(cases, 256);
}
