//! Author membership control candidates. The host owns identity issuance, resource
//! acquisition and atomic generation publication; these controls do none of them.
use std::num::{NonZeroU64, NonZeroUsize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SheetId(NonZeroU64);
impl SheetId {
    pub const fn new(value: NonZeroU64) -> Self {
        Self(value)
    }
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SetId(NonZeroU64);
impl SetId {
    pub const fn new(value: NonZeroU64) -> Self {
        Self(value)
    }
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}
#[derive(Debug)]
pub enum Membership {
    Persistent,
    Alternate(Vec<SetId>),
}
#[derive(Debug)]
pub struct AuthorSheet {
    pub id: SheetId,
    pub enabled: bool,
    pub membership: Membership,
}
#[derive(Debug, Clone, Copy)]
pub struct SheetLimits {
    pub entries: NonZeroUsize,
    pub retained_bytes: NonZeroUsize,
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SheetError {
    DuplicateSheet,
    DuplicateSet,
    DuplicateMembership,
    UnknownSheet,
    UnknownSet,
    EmptyMembership,
    Entries,
    Bytes,
    Work,
}
impl std::fmt::Display for SheetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "author sheet membership: {self:?}")
    }
}
impl std::error::Error for SheetError {}

struct Work(usize);
impl Work {
    fn charge(&mut self) -> Result<(), SheetError> {
        self.0 = self.0.checked_sub(1).ok_or(SheetError::Work)?;
        Ok(())
    }
    fn contains<T: PartialEq>(&mut self, values: &[T], target: &T) -> Result<bool, SheetError> {
        for value in values {
            self.charge()?;
            if value == target {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

fn storage<T>(values: &Vec<T>, bytes: &mut usize) -> Result<(), SheetError> {
    let capacity = values
        .capacity()
        .checked_mul(size_of::<T>())
        .ok_or(SheetError::Bytes)?;
    *bytes = bytes.checked_sub(capacity).ok_or(SheetError::Bytes)?;
    Ok(())
}
fn effective(
    sheet: &AuthorSheet,
    enabled: bool,
    selected: Option<SetId>,
    work: &mut Work,
) -> Result<bool, SheetError> {
    work.charge()?;
    if !enabled {
        return Ok(false);
    }
    match &sheet.membership {
        Membership::Persistent => Ok(true),
        Membership::Alternate(sets) => match selected {
            Some(selected) => work.contains(sets, &selected),
            None => Ok(false),
        },
    }
}

pub struct AuthorSheets {
    sheets: Vec<AuthorSheet>,
    sets: Vec<SetId>,
    author_enabled: bool,
    selected: Option<SetId>,
    work: NonZeroUsize,
}
impl AuthorSheets {
    /// Moves and validates caller-owned vectors without allocating or cloning.
    /// Capacity limits cover all retained vectors, excluding fixed struct storage.
    pub fn new(
        sheets: Vec<AuthorSheet>,
        sets: Vec<SetId>,
        author_enabled: bool,
        selected: Option<SetId>,
        limits: SheetLimits,
    ) -> Result<Self, SheetError> {
        let mut entries = limits
            .entries
            .get()
            .checked_sub(sheets.len())
            .and_then(|left| left.checked_sub(sets.len()))
            .ok_or(SheetError::Entries)?;
        let mut bytes = limits.retained_bytes.get();
        storage(&sheets, &mut bytes)?;
        storage(&sets, &mut bytes)?;
        let mut work = Work(limits.work.get());
        for (index, set) in sets.iter().enumerate() {
            work.charge()?;
            if work.contains(&sets[..index], set)? {
                return Err(SheetError::DuplicateSet);
            }
        }
        if let Some(selected) = selected
            && !work.contains(&sets, &selected)?
        {
            return Err(SheetError::UnknownSet);
        }
        for (index, sheet) in sheets.iter().enumerate() {
            work.charge()?;
            for prior in &sheets[..index] {
                work.charge()?;
                if prior.id == sheet.id {
                    return Err(SheetError::DuplicateSheet);
                }
            }
            if let Membership::Alternate(memberships) = &sheet.membership {
                if memberships.is_empty() {
                    return Err(SheetError::EmptyMembership);
                }
                entries = entries
                    .checked_sub(memberships.len())
                    .ok_or(SheetError::Entries)?;
                storage(memberships, &mut bytes)?;
                for (index, membership) in memberships.iter().enumerate() {
                    work.charge()?;
                    if !work.contains(&sets, membership)? {
                        return Err(SheetError::UnknownSet);
                    }
                    if work.contains(&memberships[..index], membership)? {
                        return Err(SheetError::DuplicateMembership);
                    }
                }
            }
        }
        Ok(Self {
            sheets,
            sets,
            author_enabled,
            selected,
            work: limits.work,
        })
    }
    fn index(&self, id: SheetId, work: &mut Work) -> Result<usize, SheetError> {
        for (index, sheet) in self.sheets.iter().enumerate() {
            work.charge()?;
            if sheet.id == id {
                return Ok(index);
            }
        }
        Err(SheetError::UnknownSheet)
    }
    pub fn sheet_ids(&self) -> impl ExactSizeIterator<Item = SheetId> + '_ {
        self.sheets.iter().map(|sheet| sheet.id)
    }
    /// Stored per-call work allowance used by membership and control operations.
    pub fn work_limit(&self) -> NonZeroUsize {
        self.work
    }
    pub fn native_hints_apply(&self) -> bool {
        self.author_enabled
    }
    pub fn selected_set(&self) -> Option<SetId> {
        self.selected
    }
    pub fn sheet_enabled(&self, id: SheetId) -> Result<bool, SheetError> {
        let mut work = Work(self.work.get());
        Ok(self.sheets[self.index(id, &mut work)?].enabled)
    }
    pub fn applies(&self, id: SheetId) -> Result<bool, SheetError> {
        let mut work = Work(self.work.get());
        let sheet = &self.sheets[self.index(id, &mut work)?];
        effective(
            sheet,
            self.author_enabled && sheet.enabled,
            self.selected,
            &mut work,
        )
    }
    /// Reports effective change; an individually disabled sheet retains that state
    /// while globally disabled or outside the currently selected alternate set.
    pub fn set_sheet_enabled(&mut self, id: SheetId, enabled: bool) -> Result<bool, SheetError> {
        let mut work = Work(self.work.get());
        let index = self.index(id, &mut work)?;
        let sheet = &self.sheets[index];
        let old = effective(
            sheet,
            self.author_enabled && sheet.enabled,
            self.selected,
            &mut work,
        )?;
        let new = effective(
            sheet,
            self.author_enabled && enabled,
            self.selected,
            &mut work,
        )?;
        self.sheets[index].enabled = enabled;
        Ok(old != new)
    }
    /// Global enable also gates native author hints, even with no registered sheet.
    pub fn set_author_enabled(&mut self, enabled: bool) -> Result<bool, SheetError> {
        let mut work = Work(self.work.get());
        work.charge()?;
        let changed = self.author_enabled != enabled;
        self.author_enabled = enabled;
        Ok(changed)
    }
    pub fn select_set(&mut self, selected: Option<SetId>) -> Result<bool, SheetError> {
        let mut work = Work(self.work.get());
        if let Some(selected) = selected
            && !work.contains(&self.sets, &selected)?
        {
            return Err(SheetError::UnknownSet);
        }
        let mut changed = false;
        for sheet in &self.sheets {
            let enabled = self.author_enabled && sheet.enabled;
            let old = effective(sheet, enabled, self.selected, &mut work)?;
            let new = effective(sheet, enabled, selected, &mut work)?;
            changed |= old != new;
        }
        self.selected = selected;
        Ok(changed)
    }
}
