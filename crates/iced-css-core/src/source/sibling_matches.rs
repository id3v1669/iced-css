//! Preflight candidate matching within one caller-established source-parent scope.
//! A match does not establish adapter/native-state compatibility.
use std::num::{NonZeroU64, NonZeroUsize};

/// Explicit keys and generated static paths are disjoint identity spaces.
/// Explicit bytes must be an injective canonical key representation supplied by
/// the owner; static paths must not be synthesized from repeated-data positions.
#[derive(Debug, Clone, Copy)]
pub enum SiblingKey<'a> {
    Explicit(&'a [u8]),
    Static {
        site: NonZeroU64,
        role_path: &'a [u32],
    },
}
#[derive(Debug, Clone, Copy)]
pub struct MatchLimits {
    pub entries: NonZeroUsize,
    pub retained_bytes: NonZeroUsize,
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Generation {
    Previous,
    Incoming,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchError {
    Entries,
    Bytes,
    Work,
    Allocation,
    Duplicate {
        generation: Generation,
        first: usize,
        second: usize,
    },
}
impl std::fmt::Display for MatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "sibling matching preflight: {self:?}")
    }
}
impl std::error::Error for MatchError {}
#[derive(Debug)]
pub struct SiblingMatches {
    previous: Vec<Option<usize>>,
}
impl SiblingMatches {
    /// One candidate previous index per incoming sibling, in incoming order.
    pub fn previous_indices(&self) -> &[Option<usize>] {
        &self.previous
    }
    pub fn retained_bytes(&self) -> usize {
        self.previous.capacity() * std::mem::size_of::<Option<usize>>()
    }
}
fn charge(work: &mut usize, amount: usize) -> Result<(), MatchError> {
    *work = work.checked_sub(amount).ok_or(MatchError::Work)?;
    Ok(())
}
fn equal<T: PartialEq>(a: &[T], b: &[T], work: &mut usize) -> Result<bool, MatchError> {
    charge(work, 1)?;
    if a.len() != b.len() {
        return Ok(false);
    }
    for (a, b) in a.iter().zip(b) {
        charge(work, 1)?;
        if a != b {
            return Ok(false);
        }
    }
    Ok(true)
}
fn same(a: SiblingKey<'_>, b: SiblingKey<'_>, work: &mut usize) -> Result<bool, MatchError> {
    charge(work, 1)?;
    match (a, b) {
        (SiblingKey::Explicit(a), SiblingKey::Explicit(b)) => equal(a, b, work),
        (
            SiblingKey::Static {
                site: a,
                role_path: ap,
            },
            SiblingKey::Static {
                site: b,
                role_path: bp,
            },
        ) if a == b => equal(ap, bp, work),
        _ => Ok(false),
    }
}
fn unique(
    keys: &[SiblingKey<'_>],
    generation: Generation,
    work: &mut usize,
) -> Result<(), MatchError> {
    for (second, key) in keys.iter().enumerate() {
        charge(work, 1)?;
        for (first, earlier) in keys[..second].iter().enumerate() {
            if same(*earlier, *key, work)? {
                return Err(MatchError::Duplicate {
                    generation,
                    first,
                    second,
                });
            }
        }
    }
    Ok(())
}
/// Validate both identity sets before constructing a complete candidate plan.
/// The caller must use a single compatible parent scope and check each matched
/// child's full adapter compatibility before moving state or preserving identity.
pub fn match_siblings(
    previous: &[SiblingKey<'_>],
    incoming: &[SiblingKey<'_>],
    limits: MatchLimits,
) -> Result<SiblingMatches, MatchError> {
    let entries = previous
        .len()
        .checked_add(incoming.len())
        .ok_or(MatchError::Entries)?;
    if entries > limits.entries.get() {
        return Err(MatchError::Entries);
    }
    let bytes = incoming
        .len()
        .checked_mul(std::mem::size_of::<Option<usize>>())
        .ok_or(MatchError::Bytes)?;
    if bytes > limits.retained_bytes.get() {
        return Err(MatchError::Bytes);
    }
    let mut work = limits.work.get();
    unique(previous, Generation::Previous, &mut work)?;
    unique(incoming, Generation::Incoming, &mut work)?;
    let mut indices = Vec::new();
    indices
        .try_reserve_exact(incoming.len())
        .map_err(|_| MatchError::Allocation)?;
    let capacity_bytes = indices
        .capacity()
        .checked_mul(std::mem::size_of::<Option<usize>>())
        .ok_or(MatchError::Bytes)?;
    if capacity_bytes > limits.retained_bytes.get() {
        return Err(MatchError::Bytes);
    }
    for key in incoming {
        charge(&mut work, 1)?;
        let mut matched = None;
        for (index, old) in previous.iter().enumerate() {
            if same(*old, *key, &mut work)? {
                matched = Some(index);
                break;
            }
        }
        indices.push(matched);
    }
    Ok(SiblingMatches { previous: indices })
}
