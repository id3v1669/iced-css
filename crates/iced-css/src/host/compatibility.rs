//! Read-only native reuse preflight after source-parent-local sibling matching.
use iced::advanced::widget::{Tree, tree::Tag};
use std::num::{NonZeroU32, NonZeroUsize};

/// Trusted registered state contract, independent of source keys and CSS IDs.
/// Equal declarations assert compatibility; they do not prove adapter behavior.
#[derive(Debug, Clone, Copy)]
pub struct NativeCompatibility<'a> {
    pub adapter_id: &'a str,
    pub schema_version: NonZeroU32,
    pub content_tag: Tag,
    /// Injective canonical encoding of the complete ordered role schema, supplied
    /// by the registry. Arbitrary concatenated role names are not sufficient.
    pub ordered_roles: &'a [u8],
    pub representation: &'a str,
}
#[derive(Debug, Clone, Copy)]
pub struct CompatibilityLimits {
    pub entries: NonZeroUsize,
    pub retained_bytes: NonZeroUsize,
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompatibilityError {
    Dimensions,
    Entries,
    Bytes,
    Work,
    Allocation,
    OutOfBounds { incoming: usize, previous: usize },
    Missing { incoming: usize, previous: usize },
    Duplicate { first: usize, second: usize },
    Tag { incoming: usize, previous: usize },
}
impl std::fmt::Display for CompatibilityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "native compatibility preflight: {self:?}")
    }
}
impl std::error::Error for CompatibilityError {}
/// A read-only candidate mapping. Native slot preparation must still validate
/// current state exclusively before moving Trees; this object owns no state.
#[derive(Debug)]
pub struct CompatibleNativeSlots {
    previous: Vec<Option<usize>>,
}
impl CompatibleNativeSlots {
    pub(crate) fn into_previous_indices(self) -> Vec<Option<usize>> {
        self.previous
    }
    pub fn previous_indices(&self) -> &[Option<usize>] {
        &self.previous
    }
    pub fn retained_bytes(&self) -> usize {
        self.previous.capacity() * size_of::<Option<usize>>()
    }
}
fn charge(work: &mut usize, amount: usize) -> Result<(), CompatibilityError> {
    *work = work.checked_sub(amount).ok_or(CompatibilityError::Work)?;
    Ok(())
}
fn bytes_equal(left: &[u8], right: &[u8], work: &mut usize) -> Result<bool, CompatibilityError> {
    charge(work, 1)?;
    if left.len() != right.len() {
        return Ok(false);
    }
    for (a, b) in left.iter().zip(right) {
        charge(work, 1)?;
        if a != b {
            return Ok(false);
        }
    }
    Ok(true)
}
fn compatible(
    a: NativeCompatibility<'_>,
    b: NativeCompatibility<'_>,
    work: &mut usize,
) -> Result<bool, CompatibilityError> {
    charge(work, 1)?;
    if a.schema_version != b.schema_version || a.content_tag != b.content_tag {
        return Ok(false);
    }
    Ok(
        bytes_equal(a.adapter_id.as_bytes(), b.adapter_id.as_bytes(), work)?
            && bytes_equal(a.ordered_roles, b.ordered_roles, work)?
            && bytes_equal(
                a.representation.as_bytes(),
                b.representation.as_bytes(),
                work,
            )?,
    )
}
/// Filter source matches using the full declared state contract without moving
/// state or invoking native callbacks. All inputs belong to one established
/// source-parent scope. The caller must retain the same descriptions/state until
/// the subsequent exclusive native-slot preparation and publication.
///
/// # Errors
/// Rejects malformed mappings, mismatched previous tags, bounds or allocation.
pub fn filter_native_compatibility(
    previous: &[Option<Tree>],
    previous_schema: &[NativeCompatibility<'_>],
    incoming_schema: &[NativeCompatibility<'_>],
    mapping: &[Option<usize>],
    limits: CompatibilityLimits,
) -> Result<CompatibleNativeSlots, CompatibilityError> {
    if previous.len() != previous_schema.len() || mapping.len() != incoming_schema.len() {
        return Err(CompatibilityError::Dimensions);
    }
    if previous
        .len()
        .checked_add(mapping.len())
        .ok_or(CompatibilityError::Entries)?
        > limits.entries.get()
    {
        return Err(CompatibilityError::Entries);
    }
    let requested = mapping
        .len()
        .checked_mul(size_of::<Option<usize>>())
        .ok_or(CompatibilityError::Bytes)?;
    if requested > limits.retained_bytes.get() {
        return Err(CompatibilityError::Bytes);
    }
    let mut work = limits.work.get();
    charge(&mut work, mapping.len())?;
    for (incoming, slot) in mapping.iter().enumerate() {
        charge(&mut work, 1)?;
        let Some(index) = slot else { continue };
        let tree = previous
            .get(*index)
            .ok_or(CompatibilityError::OutOfBounds {
                incoming,
                previous: *index,
            })?
            .as_ref()
            .ok_or(CompatibilityError::Missing {
                incoming,
                previous: *index,
            })?;
        if tree.tag != previous_schema[*index].content_tag {
            return Err(CompatibilityError::Tag {
                incoming,
                previous: *index,
            });
        }
        for (first, earlier) in mapping[..incoming].iter().enumerate() {
            charge(&mut work, 1)?;
            if earlier == slot {
                return Err(CompatibilityError::Duplicate {
                    first,
                    second: incoming,
                });
            }
        }
    }
    let mut output = Vec::new();
    output
        .try_reserve_exact(mapping.len())
        .map_err(|_| CompatibilityError::Allocation)?;
    if output
        .capacity()
        .checked_mul(size_of::<Option<usize>>())
        .ok_or(CompatibilityError::Bytes)?
        > limits.retained_bytes.get()
    {
        return Err(CompatibilityError::Bytes);
    }
    for (incoming, slot) in mapping.iter().enumerate() {
        let reuse = match slot {
            Some(index)
                if compatible(
                    previous_schema[*index],
                    incoming_schema[incoming],
                    &mut work,
                )? =>
            {
                Some(*index)
            }
            _ => None,
        };
        output.push(reuse);
    }
    Ok(CompatibleNativeSlots { previous: output })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompatiblePreparationError {
    Compatibility(CompatibilityError),
    Slots(super::reconcile::SlotError),
}
impl std::fmt::Display for CompatiblePreparationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Compatibility(error) => std::fmt::Display::fmt(error, f),
            Self::Slots(error) => std::fmt::Display::fmt(error, f),
        }
    }
}
impl std::error::Error for CompatiblePreparationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Compatibility(error) => error,
            Self::Slots(error) => error,
        })
    }
}
/// Validate compatibility and prepare movement under one exclusive state borrow.
/// The preparation owns the filtered mapping, so input descriptors and source
/// matches need not remain alive. No native state moves until apply.
/// Compatibility limits bound the mapping; slot limits bound the destination.
///
/// ```compile_fail,E0506
/// # use iced_css::host::{compatibility::*,reconcile::SlotLimits};
/// # use iced::advanced::widget::Tree;
/// fn mutate(previous:&mut [Option<Tree>], schema:&[NativeCompatibility<'_>],
///     mapping:&[Option<usize>], a:CompatibilityLimits, b:SlotLimits) {
///     let prepared=prepare_compatible_native_slots(previous,schema,schema,mapping,a,b).unwrap();
///     previous[0]=None;
///     let _=prepared.apply();
/// }
/// ```
///
/// # Errors
/// Returns either compatibility preflight or native slot preflight failures.
/// Both stages leave all previous Trees untouched on failure.
pub fn prepare_compatible_native_slots<'state>(
    previous: &'state mut [Option<Tree>],
    previous_schema: &[NativeCompatibility<'_>],
    incoming_schema: &[NativeCompatibility<'_>],
    mapping: &[Option<usize>],
    compatibility_limits: CompatibilityLimits,
    slot_limits: super::reconcile::SlotLimits,
) -> Result<super::reconcile::PreparedNativeSlots<'state, 'static>, CompatiblePreparationError> {
    let compatible = filter_native_compatibility(
        previous,
        previous_schema,
        incoming_schema,
        mapping,
        compatibility_limits,
    )
    .map_err(CompatiblePreparationError::Compatibility)?;
    super::reconcile::prepare_native_slots_owned(previous, compatible.previous, slot_limits)
        .map_err(CompatiblePreparationError::Slots)
}
