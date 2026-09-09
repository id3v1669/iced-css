use super::PropertyError;
use crate::syntax::{SyntaxError, SyntaxFault, SyntaxResource};
use std::num::NonZeroUsize;
pub(super) struct Storage {
    pub(super) used: usize,
    pub(super) limit: usize,
}
pub(super) fn resource(offset: usize, resource: SyntaxResource) -> PropertyError {
    PropertyError::Syntax(SyntaxError {
        offset,
        fault: SyntaxFault::Resource(resource),
    })
}
impl Storage {
    pub(super) fn remaining(&self, offset: usize) -> Result<NonZeroUsize, PropertyError> {
        self.limit
            .checked_sub(self.used)
            .and_then(NonZeroUsize::new)
            .ok_or_else(|| resource(offset, SyntaxResource::Bytes))
    }
    pub(super) fn reserve<T>(
        &mut self,
        items: &mut Vec<T>,
        offset: usize,
        work: &mut usize,
    ) -> Result<(), PropertyError> {
        if items.len() < items.capacity() {
            return Ok(());
        }
        let available = (self.limit - self.used) / size_of::<T>() + items.capacity();
        let needed = items
            .len()
            .checked_add(1)
            .filter(|n| *n <= available)
            .ok_or_else(|| resource(offset, SyntaxResource::Bytes))?;
        let target = needed.max(items.capacity().saturating_mul(2).min(available));
        *work = work
            .checked_sub(items.len() * size_of::<T>())
            .ok_or_else(|| resource(offset, SyntaxResource::Work))?;
        let mut next = Vec::new();
        next.try_reserve_exact(target)
            .map_err(|_| resource(offset, SyntaxResource::Allocation))?;
        if next.capacity() > available {
            return Err(resource(offset, SyntaxResource::Bytes));
        }
        self.used += (next.capacity() - items.capacity()) * size_of::<T>();
        next.append(items);
        *items = next;
        Ok(())
    }
}
