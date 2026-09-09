use super::Metadata;
use std::num::NonZeroUsize;

/// Bounds for the resulting canonical attribute map and one write operation.
#[derive(Debug, Clone, Copy)]
pub struct AttributeLimits {
    pub entries: NonZeroUsize,
    pub retained_bytes: NonZeroUsize,
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttributeError {
    Entries,
    Bytes,
    Work,
    Allocation,
    InvalidClassToken,
}
impl std::fmt::Display for AttributeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "source attribute write: {self:?}")
    }
}
impl std::error::Error for AttributeError {}

fn charge(work: &mut usize, amount: usize) -> Result<(), AttributeError> {
    *work = work.checked_sub(amount).ok_or(AttributeError::Work)?;
    Ok(())
}
fn bytes(capacity: usize, strings: usize) -> Result<usize, AttributeError> {
    capacity
        .checked_mul(size_of::<(String, String)>())
        .and_then(|n| n.checked_add(strings))
        .ok_or(AttributeError::Bytes)
}
fn reserve_string(text: &str) -> Result<String, AttributeError> {
    let mut result = String::new();
    result
        .try_reserve_exact(text.len())
        .map_err(|_| AttributeError::Allocation)?;
    result.push_str(text);
    Ok(result)
}

impl Metadata {
    /// Number of exact-name attribute slots, including present empty values.
    pub fn attribute_count(&self) -> usize {
        self.attributes.len()
    }
    /// Owned heap capacities, excluding the inline Metadata value. Saturates if
    /// a hypothetical sum cannot fit; bounded writes separately reject overflow.
    pub fn retained_bytes(&self) -> usize {
        self.attributes.iter().fold(
            self.attributes
                .capacity()
                .saturating_mul(size_of::<(String, String)>()),
            |sum, (key, value)| {
                sum.saturating_add(key.capacity())
                    .saturating_add(value.capacity())
            },
        )
    }

    /// Prepare all allocations before changing the map. `false` means unchanged.
    /// Existing strings and vector capacity remain untouched on any error.
    pub fn try_set_attribute(
        &mut self,
        name: &str,
        value: &str,
        limits: AttributeLimits,
    ) -> Result<bool, AttributeError> {
        self.write_attribute(name, value, limits, &mut limits.work.get())
    }

    /// Serialize validated tokens into the same slot used by raw class writes.
    pub fn try_set_classes(
        &mut self,
        classes: &[&str],
        limits: AttributeLimits,
    ) -> Result<bool, AttributeError> {
        let mut work = limits.work.get();
        let mut length = classes.len().saturating_sub(1);
        for class in classes {
            charge(&mut work, 1)?;
            charge(&mut work, class.len())?;
            if class.is_empty()
                || class
                    .bytes()
                    .any(|b| matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 12))
            {
                return Err(AttributeError::InvalidClassToken);
            }
            length = length
                .checked_add(class.len())
                .ok_or(AttributeError::Bytes)?;
        }
        if length > limits.retained_bytes.get() {
            return Err(AttributeError::Bytes);
        }
        charge(&mut work, length)?;
        let mut value = String::new();
        value
            .try_reserve_exact(length)
            .map_err(|_| AttributeError::Allocation)?;
        if value.capacity() > limits.retained_bytes.get() {
            return Err(AttributeError::Bytes);
        }
        for (index, class) in classes.iter().enumerate() {
            if index != 0 {
                value.push(' ');
            }
            value.push_str(class);
        }
        self.write_attribute("class", &value, limits, &mut work)
    }

    fn write_attribute(
        &mut self,
        name: &str,
        value: &str,
        limits: AttributeLimits,
        work: &mut usize,
    ) -> Result<bool, AttributeError> {
        let mut position = None;
        for (index, (key, _)) in self.attributes.iter().enumerate() {
            charge(work, 1)?;
            charge(
                work,
                key.len()
                    .min(name.len())
                    .checked_add(1)
                    .ok_or(AttributeError::Work)?,
            )?;
            if key == name {
                position = Some(index);
                break;
            }
        }
        let count = self
            .attributes
            .len()
            .checked_add(usize::from(position.is_none()))
            .ok_or(AttributeError::Entries)?;
        if count > limits.entries.get() {
            return Err(AttributeError::Entries);
        }
        if let Some(index) = position {
            let old = &self.attributes[index].1;
            charge(
                work,
                old.len()
                    .min(value.len())
                    .checked_add(1)
                    .ok_or(AttributeError::Work)?,
            )?;
            if old == value {
                charge(work, self.attributes.len())?;
                if self.retained_bytes() > limits.retained_bytes.get() {
                    return Err(AttributeError::Bytes);
                }
                return Ok(false);
            }
        }
        let mut kept_bytes = 0usize;
        for (index, (key, value)) in self.attributes.iter().enumerate() {
            charge(work, 2)?; // capacity accounting and later ownership movement
            if Some(index) != position {
                kept_bytes = kept_bytes
                    .checked_add(key.capacity())
                    .and_then(|n| n.checked_add(value.capacity()))
                    .ok_or(AttributeError::Bytes)?;
            }
        }
        let required = kept_bytes
            .checked_add(name.len())
            .and_then(|n| n.checked_add(value.len()))
            .ok_or(AttributeError::Bytes)?;
        if bytes(count, required)? > limits.retained_bytes.get() {
            return Err(AttributeError::Bytes);
        }
        charge(work, name.len())?;
        charge(work, value.len())?;
        charge(work, 1)?;
        let name = reserve_string(name)?;
        let value = reserve_string(value)?;
        let actual_strings = kept_bytes
            .checked_add(name.capacity())
            .and_then(|n| n.checked_add(value.capacity()))
            .ok_or(AttributeError::Bytes)?;
        let mut next = Vec::new();
        next.try_reserve_exact(count)
            .map_err(|_| AttributeError::Allocation)?;
        if bytes(next.capacity(), actual_strings)? > limits.retained_bytes.get() {
            return Err(AttributeError::Bytes);
        }
        // No allocation, fallible operation, or user callback after this boundary.
        for (index, pair) in std::mem::take(&mut self.attributes).into_iter().enumerate() {
            if Some(index) != position {
                next.push(pair);
            }
        }
        next.push((name, value));
        self.attributes = next;
        Ok(true)
    }
}
