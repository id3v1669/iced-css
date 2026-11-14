//! Immutable, all-face preparation of a bounded SFNT resource.
//!
//! Success establishes directory ranges and cached design metrics, not complete
//! font validity, CSS classification, glyph coverage, registration or rendering.
use super::metrics::{DesignMetrics, FontMetricError, FontMetricLimits, read_sfnt_design_metrics};
use std::collections::TryReserveError;
use std::error::Error;
use std::fmt;
use std::mem::size_of;
use std::num::NonZeroUsize;
use std::ops::Range;

#[derive(Debug, Clone, Copy)]
pub struct SfntResourceLimits {
    pub bytes: NonZeroUsize,
    pub faces: NonZeroUsize,
    pub tables_per_face: NonZeroUsize,
    pub tables: NonZeroUsize,
    /// Successful backing capacities; excludes allocator metadata and stack values.
    pub owned_bytes: NonZeroUsize,
    /// Logical work: one container unit, seven per face and two per table record.
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SfntContainer {
    Single,
    CollectionV1,
    CollectionV2,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SfntSignatureRecord {
    pub tag: [u8; 4],
    pub offset: u32,
    pub length: u32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SfntResourceUsage {
    pub owned_bytes: usize,
    pub work: usize,
    pub tables: usize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AmbiguousSfntTable {
    pub face_index: u32,
    pub tag: [u8; 4],
    pub first_record_index: usize,
    pub record_count: usize,
}
impl fmt::Display for AmbiguousSfntTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "SFNT face {}: tag {:?} has {} records starting at record {}",
            self.face_index, self.tag, self.record_count, self.first_record_index
        )
    }
}
impl Error for AmbiguousSfntTable {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SfntResourceLimit {
    Bytes,
    Faces,
    TablesPerFace,
    Tables,
    OwnedBytes,
    Work,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SfntDescriptorStorage {
    Faces,
    Tables,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SfntUnsupported {
    LegacySfnt(u32),
    CollectionVersion(u32),
    Signature(u32),
    NormalizationRequired([u8; 4]),
    DirectoryBeforeCollectionHeader,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SfntMalformed {
    Identification,
    CollectionHeader,
    EmptyCollection,
    FaceHeader,
    DirectoryRange,
    NestedCollection,
    DirectoryOrder,
    TableRange,
    TableAlignment,
    SignatureFields,
    SignatureRange,
    SignatureAlignment,
    SignaturePlacement,
}
#[derive(Debug)]
pub enum SfntResourceErrorKind {
    Limit(SfntResourceLimit),
    Addressability,
    Allocation {
        storage: SfntDescriptorStorage,
        source: TryReserveError,
    },
    Unsupported(SfntUnsupported),
    Malformed(SfntMalformed),
    Metrics(FontMetricError),
}
impl fmt::Display for SfntResourceErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Limit(limit) => write!(f, "resource limit {limit:?}"),
            Self::Addressability => f.write_str("resource is not addressable"),
            Self::Allocation { storage, source } => {
                write!(f, "{storage:?} reservation failed: {source}")
            }
            Self::Unsupported(kind) => write!(f, "unsupported preparation profile: {kind:?}"),
            Self::Malformed(kind) => write!(f, "malformed structure: {kind:?}"),
            Self::Metrics(error) => fmt::Display::fmt(error, f),
        }
    }
}
impl Error for SfntResourceErrorKind {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Allocation { source, .. } => Some(source),
            Self::Metrics(source) => Some(source),
            _ => None,
        }
    }
}
#[derive(Debug)]
pub struct SfntResourceError {
    pub face_index: Option<u32>,
    pub record_index: Option<usize>,
    pub kind: SfntResourceErrorKind,
}
impl fmt::Display for SfntResourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SFNT resource")?;
        if let Some(face) = self.face_index {
            write!(f, " face {face}")?;
        }
        if let Some(record) = self.record_index {
            write!(f, " record {record}")?;
        }
        write!(f, ": {}", self.kind)
    }
}
impl Error for SfntResourceError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.kind.source()
    }
}

struct FaceDescriptor {
    directory_offset: u32,
    sfnt_version: u32,
    table_start: usize,
    table_count: usize,
    metrics: DesignMetrics,
}
struct TableDescriptor {
    tag: [u8; 4],
    checksum: u32,
    offset: u32,
    length: u32,
}
/// Exact moved bytes and cached descriptors, published only after all faces pass.
#[must_use]
pub struct PreparedSfntResource {
    bytes: Vec<u8>,
    faces: Vec<FaceDescriptor>,
    tables: Vec<TableDescriptor>,
    container: SfntContainer,
    signature: Option<SfntSignatureRecord>,
    face_count: u32,
    usage: SfntResourceUsage,
}
impl fmt::Debug for PreparedSfntResource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreparedSfntResource")
            .field("byte_length", &self.bytes.len())
            .field("container", &self.container)
            .field("face_count", &self.face_count)
            .field("signature", &self.signature)
            .field("usage", &self.usage)
            .finish_non_exhaustive()
    }
}
#[derive(Clone, Copy, Debug)]
pub struct SfntFace<'a> {
    owner: &'a PreparedSfntResource,
    index: u32,
    descriptor_index: usize,
}
#[derive(Clone, Copy, Debug)]
pub struct SfntTable<'a> {
    face: SfntFace<'a>,
    record_index: usize,
}
impl PreparedSfntResource {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn container(&self) -> SfntContainer {
        self.container
    }
    pub fn signature_record(&self) -> Option<SfntSignatureRecord> {
        self.signature
    }
    pub fn face_count(&self) -> u32 {
        self.face_count
    }
    pub fn face(&self, index: u32) -> Option<SfntFace<'_>> {
        let descriptor_index = usize::try_from(index).ok()?;
        self.faces.get(descriptor_index)?;
        Some(SfntFace {
            owner: self,
            index,
            descriptor_index,
        })
    }
    pub fn usage(&self) -> SfntResourceUsage {
        self.usage
    }
}
impl<'a> SfntFace<'a> {
    fn descriptor(self) -> &'a FaceDescriptor {
        &self.owner.faces[self.descriptor_index]
    }
    pub fn resource(self) -> &'a PreparedSfntResource {
        self.owner
    }
    pub fn index(self) -> u32 {
        self.index
    }
    pub fn directory_offset(self) -> u32 {
        self.descriptor().directory_offset
    }
    pub fn sfnt_version(self) -> u32 {
        self.descriptor().sfnt_version
    }
    pub fn metrics(self) -> &'a DesignMetrics {
        &self.descriptor().metrics
    }
    pub fn table_count(self) -> usize {
        self.descriptor().table_count
    }
    pub fn table(self, record_index: usize) -> Option<SfntTable<'a>> {
        (record_index < self.table_count()).then_some(SfntTable {
            face: self,
            record_index,
        })
    }
    /// Find a tag without choosing arbitrarily among equal directory records.
    ///
    /// # Errors
    /// Returns the original equal-tag range when the tag occurs more than once.
    pub fn unique_table(self, tag: [u8; 4]) -> Result<Option<SfntTable<'a>>, AmbiguousSfntTable> {
        let face = self.descriptor();
        let tables = &self.owner.tables[face.table_start..face.table_start + face.table_count];
        let first = tables.partition_point(|table| table.tag < tag);
        let end = tables.partition_point(|table| table.tag <= tag);
        match end - first {
            0 => Ok(None),
            1 => Ok(self.table(first)),
            record_count => Err(AmbiguousSfntTable {
                face_index: self.index,
                tag,
                first_record_index: first,
                record_count,
            }),
        }
    }
}
impl<'a> SfntTable<'a> {
    fn descriptor(self) -> &'a TableDescriptor {
        &self.face.owner.tables[self.face.descriptor().table_start + self.record_index]
    }
    pub fn face(self) -> SfntFace<'a> {
        self.face
    }
    pub fn record_index(self) -> usize {
        self.record_index
    }
    pub fn tag(self) -> [u8; 4] {
        self.descriptor().tag
    }
    pub fn checksum(self) -> u32 {
        self.descriptor().checksum
    }
    pub fn offset(self) -> u32 {
        self.descriptor().offset
    }
    pub fn length(self) -> u32 {
        self.descriptor().length
    }
    pub fn bytes(self) -> &'a [u8] {
        // Construction checked both conversions and the complete resource range.
        let start = usize::try_from(self.offset()).expect("prepared table offset");
        let length = usize::try_from(self.length()).expect("prepared table length");
        &self.face.owner.bytes[start..start + length]
    }
}

#[derive(Clone, Copy)]
struct Context {
    face: Option<u32>,
    record: Option<usize>,
}
const GLOBAL: Context = Context {
    face: None,
    record: None,
};
impl Context {
    fn face(face: u32) -> Self {
        Self {
            face: Some(face),
            record: None,
        }
    }
    fn record(self, record: usize) -> Self {
        Self {
            record: Some(record),
            ..self
        }
    }
    fn error(self, kind: SfntResourceErrorKind) -> SfntResourceError {
        SfntResourceError {
            face_index: self.face,
            record_index: self.record,
            kind,
        }
    }
    fn malformed(self, kind: SfntMalformed) -> SfntResourceError {
        self.error(SfntResourceErrorKind::Malformed(kind))
    }
    fn unsupported(self, kind: SfntUnsupported) -> SfntResourceError {
        self.error(SfntResourceErrorKind::Unsupported(kind))
    }
    fn addressability(self) -> SfntResourceError {
        self.error(SfntResourceErrorKind::Addressability)
    }
}
fn check_limit(
    actual: usize,
    limit: usize,
    kind: SfntResourceLimit,
    context: Context,
) -> Result<usize, SfntResourceError> {
    if actual > limit {
        Err(context.error(SfntResourceErrorKind::Limit(kind)))
    } else {
        Ok(actual)
    }
}
struct Work {
    remaining: usize,
    initial: usize,
}
impl Work {
    fn charge(&mut self, amount: usize, context: Context) -> Result<(), SfntResourceError> {
        self.remaining = self
            .remaining
            .checked_sub(amount)
            .ok_or_else(|| context.error(SfntResourceErrorKind::Limit(SfntResourceLimit::Work)))?;
        Ok(())
    }
    fn require(&self, amount: usize) -> Result<(), SfntResourceError> {
        check_limit(amount, self.remaining, SfntResourceLimit::Work, GLOBAL).map(|_| ())
    }
    fn used(&self) -> usize {
        self.initial - self.remaining
    }
}
fn u32_at(data: &[u8], start: usize) -> u32 {
    u32::from_be_bytes(
        data[start..start + 4]
            .try_into()
            .expect("checked fixed field"),
    )
}
fn u16_at(data: &[u8], start: usize) -> u16 {
    u16::from_be_bytes(
        data[start..start + 2]
            .try_into()
            .expect("checked fixed field"),
    )
}
/// Parser array endpoints have an additional u32 integration bound, before the
/// input-length check. Payload endpoints below do not inherit that restriction.
fn parser_end(
    start: u64,
    count: u64,
    stride: u64,
    context: Context,
) -> Result<usize, SfntResourceError> {
    let end = count
        .checked_mul(stride)
        .and_then(|n| start.checked_add(n))
        .filter(|&n| n <= u64::from(u32::MAX))
        .ok_or_else(|| context.addressability())?;
    usize::try_from(end).map_err(|_| context.addressability())
}
fn parser_range(
    data: &[u8],
    start: u64,
    length: u64,
    context: Context,
    kind: SfntMalformed,
) -> Result<Range<usize>, SfntResourceError> {
    let end = parser_end(start, length, 1, context)?;
    if end > data.len() {
        return Err(context.malformed(kind));
    }
    let start = usize::try_from(start).map_err(|_| context.addressability())?;
    Ok(start..end)
}
fn payload_range(
    data: &[u8],
    offset: u32,
    length: u32,
    context: Context,
    kind: SfntMalformed,
) -> Result<Range<usize>, SfntResourceError> {
    let start = u64::from(offset);
    let end = start + u64::from(length);
    if end > data.len() as u64 {
        return Err(context.malformed(kind));
    }
    let start = usize::try_from(start).map_err(|_| context.addressability())?;
    let end = usize::try_from(end).map_err(|_| context.addressability())?;
    Ok(start..end)
}
fn supported_word(word: u32, top_level: bool, context: Context) -> Result<(), SfntResourceError> {
    match word {
        0x0001_0000 | 0x4f54_544f => Ok(()),
        0x7472_7565 | 0x7479_7031 => Err(context.unsupported(SfntUnsupported::LegacySfnt(word))),
        0x774f_4646 | 0x774f_4632 if top_level => {
            Err(context.unsupported(SfntUnsupported::NormalizationRequired(word.to_be_bytes())))
        }
        0x7474_6366 => Err(context.malformed(SfntMalformed::NestedCollection)),
        _ => Err(context.unsupported(SfntUnsupported::Signature(word))),
    }
}
struct ContainerHeader {
    kind: SfntContainer,
    faces: u32,
    end: usize,
    signature: Option<SfntSignatureRecord>,
}
impl ContainerHeader {
    fn face_offset(&self, data: &[u8], index: u32) -> u32 {
        if self.kind == SfntContainer::Single {
            0
        } else {
            // The complete array and actual count were checked before inventory.
            let entry = usize::try_from(12 + 4 * u64::from(index)).expect("prepared offset entry");
            u32_at(data, entry)
        }
    }
}
fn container_header(
    data: &[u8],
    limits: SfntResourceLimits,
) -> Result<ContainerHeader, SfntResourceError> {
    if data.len() < 4 {
        return Err(GLOBAL.malformed(SfntMalformed::Identification));
    }
    let word = u32_at(data, 0);
    if word != 0x7474_6366 {
        supported_word(word, true, GLOBAL)?;
        check_limit(1, limits.faces.get(), SfntResourceLimit::Faces, GLOBAL)?;
        return Ok(ContainerHeader {
            kind: SfntContainer::Single,
            faces: 1,
            end: 0,
            signature: None,
        });
    }
    if data.len() < 8 {
        return Err(GLOBAL.malformed(SfntMalformed::CollectionHeader));
    }
    let kind = match u32_at(data, 4) {
        0x0001_0000 => SfntContainer::CollectionV1,
        0x0002_0000 => SfntContainer::CollectionV2,
        version => return Err(GLOBAL.unsupported(SfntUnsupported::CollectionVersion(version))),
    };
    if data.len() < 12 {
        return Err(GLOBAL.malformed(SfntMalformed::CollectionHeader));
    }
    let faces = u32_at(data, 8);
    if faces == 0 {
        return Err(GLOBAL.malformed(SfntMalformed::EmptyCollection));
    }
    let count = usize::try_from(faces).map_err(|_| GLOBAL.addressability())?;
    check_limit(count, limits.faces.get(), SfntResourceLimit::Faces, GLOBAL)?;
    let array_end = parser_end(12, u64::from(faces), 4, GLOBAL)?;
    let end = if kind == SfntContainer::CollectionV2 {
        parser_end(array_end as u64, 12, 1, GLOBAL)?
    } else {
        array_end
    };
    if end > data.len() {
        return Err(GLOBAL.malformed(SfntMalformed::CollectionHeader));
    }
    let mut signature = None;
    if kind == SfntContainer::CollectionV2 {
        let tag = u32_at(data, array_end);
        let length = u32_at(data, array_end + 4);
        let offset = u32_at(data, array_end + 8);
        if tag != 0 || length != 0 || offset != 0 {
            if tag != u32::from_be_bytes(*b"DSIG") || length == 0 {
                return Err(GLOBAL.malformed(SfntMalformed::SignatureFields));
            }
            let range = payload_range(data, offset, length, GLOBAL, SfntMalformed::SignatureRange)?;
            if !offset.is_multiple_of(4) {
                return Err(GLOBAL.malformed(SfntMalformed::SignatureAlignment));
            }
            if range.start < end {
                return Err(GLOBAL.malformed(SfntMalformed::SignaturePlacement));
            }
            signature = Some(SfntSignatureRecord {
                tag: tag.to_be_bytes(),
                offset,
                length,
            });
        }
    }
    Ok(ContainerHeader {
        kind,
        faces,
        end,
        signature,
    })
}
struct FaceHeader {
    offset: u32,
    word: u32,
    table_count: usize,
    directory: Range<usize>,
}
fn face_header(
    data: &[u8],
    container: &ContainerHeader,
    index: u32,
    limits: SfntResourceLimits,
) -> Result<FaceHeader, SfntResourceError> {
    let context = Context::face(index);
    let offset = container.face_offset(data, index);
    let header = parser_range(
        data,
        u64::from(offset),
        12,
        context,
        SfntMalformed::FaceHeader,
    )?;
    let word = u32_at(data, header.start);
    supported_word(word, false, context)?;
    if header.start < container.end {
        return Err(context.unsupported(SfntUnsupported::DirectoryBeforeCollectionHeader));
    }
    let table_count = usize::from(u16_at(data, header.start + 4));
    check_limit(
        table_count,
        limits.tables_per_face.get(),
        SfntResourceLimit::TablesPerFace,
        context,
    )?;
    let end = parser_end(header.end as u64, table_count as u64, 16, context)?;
    if end > data.len() {
        return Err(context.malformed(SfntMalformed::DirectoryRange));
    }
    Ok(FaceHeader {
        offset,
        word,
        table_count,
        directory: header.end..end,
    })
}
fn allocation_bytes<T>(capacity: usize) -> Result<usize, SfntResourceError> {
    capacity
        .checked_mul(size_of::<T>())
        .filter(|&n| n <= isize::MAX as usize)
        .ok_or_else(|| GLOBAL.addressability())
}
fn retained_bytes(
    bytes: usize,
    faces: usize,
    tables: usize,
    limit: usize,
) -> Result<usize, SfntResourceError> {
    let total = bytes
        .checked_add(faces)
        .and_then(|n| n.checked_add(tables))
        .ok_or_else(|| GLOBAL.addressability())?;
    check_limit(total, limit, SfntResourceLimit::OwnedBytes, GLOBAL)
}
fn reservation_error(storage: SfntDescriptorStorage, source: TryReserveError) -> SfntResourceError {
    GLOBAL.error(SfntResourceErrorKind::Allocation { storage, source })
}
fn reserve_descriptors(
    byte_capacity: usize,
    face_count: usize,
    table_count: usize,
    limit: usize,
) -> Result<(Vec<FaceDescriptor>, Vec<TableDescriptor>, usize), SfntResourceError> {
    let face_floor = allocation_bytes::<FaceDescriptor>(face_count)?;
    let table_floor = allocation_bytes::<TableDescriptor>(table_count)?;
    retained_bytes(byte_capacity, face_floor, table_floor, limit)?;
    let mut faces = Vec::new();
    faces
        .try_reserve_exact(face_count)
        .map_err(|e| reservation_error(SfntDescriptorStorage::Faces, e))?;
    let face_bytes = allocation_bytes::<FaceDescriptor>(faces.capacity())?;
    retained_bytes(byte_capacity, face_bytes, table_floor, limit)?;
    let mut tables = Vec::new();
    tables
        .try_reserve_exact(table_count)
        .map_err(|e| reservation_error(SfntDescriptorStorage::Tables, e))?;
    let owned_bytes = retained_bytes(
        byte_capacity,
        face_bytes,
        allocation_bytes::<TableDescriptor>(tables.capacity())?,
        limit,
    )?;
    Ok((faces, tables, owned_bytes))
}
fn remaining_work(faces: usize, tables: usize) -> Result<usize, SfntResourceError> {
    faces
        .checked_mul(6)
        .and_then(|n| tables.checked_mul(2).and_then(|t| n.checked_add(t)))
        .ok_or_else(|| GLOBAL.addressability())
}
fn read_record(
    data: &[u8],
    at: usize,
    prior: Option<[u8; 4]>,
    signature: Option<SfntSignatureRecord>,
    context: Context,
) -> Result<TableDescriptor, SfntResourceError> {
    let tag = u32_at(data, at).to_be_bytes();
    if prior.is_some_and(|old| old > tag) {
        return Err(context.malformed(SfntMalformed::DirectoryOrder));
    }
    let checksum = u32_at(data, at + 4);
    let offset = u32_at(data, at + 8);
    let length = u32_at(data, at + 12);
    let range = payload_range(data, offset, length, context, SfntMalformed::TableRange)?;
    if !offset.is_multiple_of(4) {
        return Err(context.malformed(SfntMalformed::TableAlignment));
    }
    if signature.is_some_and(|s| range.end as u64 > u64::from(s.offset)) {
        return Err(context.malformed(SfntMalformed::SignaturePlacement));
    }
    Ok(TableDescriptor {
        tag,
        checksum,
        offset,
        length,
    })
}

/// Move a complete resource into immutable storage after every face is prepared.
///
/// # Errors
/// Rejects configured limits, addressability/reservation failures, unsupported
/// layouts, malformed directory/envelope ranges and errors from the design
/// metrics reader. Failure consumes and drops the input; no partial owner is
/// returned. Observed vector capacities are bounded after reservation, which is
/// not a strict transient or process-wide allocation ceiling.
pub fn prepare_sfnt_resource(
    bytes: Vec<u8>,
    limits: SfntResourceLimits,
) -> Result<PreparedSfntResource, SfntResourceError> {
    check_limit(
        bytes.len(),
        limits.bytes.get(),
        SfntResourceLimit::Bytes,
        GLOBAL,
    )?;
    check_limit(
        bytes.capacity(),
        limits.owned_bytes.get(),
        SfntResourceLimit::OwnedBytes,
        GLOBAL,
    )?;
    let mut work = Work {
        remaining: limits.work.get(),
        initial: limits.work.get(),
    };
    work.charge(1, GLOBAL)?;
    let container = container_header(&bytes, limits)?;
    let face_count = usize::try_from(container.faces).map_err(|_| GLOBAL.addressability())?;
    let mut table_count = 0_usize;
    for index in 0..container.faces {
        let context = Context::face(index);
        work.charge(1, context)?;
        let header = face_header(&bytes, &container, index, limits)?;
        table_count = table_count
            .checked_add(header.table_count)
            .ok_or_else(|| context.addressability())?;
        check_limit(
            table_count,
            limits.tables.get(),
            SfntResourceLimit::Tables,
            context,
        )?;
        if container
            .signature
            .is_some_and(|s| header.directory.end as u64 > u64::from(s.offset))
        {
            return Err(context.malformed(SfntMalformed::SignaturePlacement));
        }
    }
    work.require(remaining_work(face_count, table_count)?)?;
    let (mut faces, mut tables, owned_bytes) = reserve_descriptors(
        bytes.capacity(),
        face_count,
        table_count,
        limits.owned_bytes.get(),
    )?;
    for index in 0..container.faces {
        let context = Context::face(index);
        work.charge(1, context)?;
        let header = face_header(&bytes, &container, index, limits)?;
        let table_start = tables.len();
        let mut prior = None;
        for (record_index, at) in (header.directory.start..header.directory.end)
            .step_by(16)
            .enumerate()
        {
            let context = context.record(record_index);
            work.charge(1, context)?;
            let record = read_record(&bytes, at, prior, container.signature, context)?;
            prior = Some(record.tag);
            // Counts were inventoried from these immutable bytes and reserved once.
            if tables.len() >= table_count || tables.len() >= tables.capacity() {
                return Err(context.addressability());
            }
            tables.push(record);
        }
        let metric_work = 5_usize
            .checked_add(header.table_count)
            .ok_or_else(|| context.addressability())?;
        work.charge(metric_work, context)?;
        let metrics = read_sfnt_design_metrics(
            &bytes,
            index,
            FontMetricLimits {
                bytes: NonZeroUsize::new(bytes.len()).expect("prepared nonempty resource"),
                tables: NonZeroUsize::new(header.table_count.max(1))
                    .expect("positive table allowance"),
                work: NonZeroUsize::new(metric_work).expect("five metric units"),
            },
        )
        .map_err(|e| context.error(SfntResourceErrorKind::Metrics(e)))?;
        if faces.len() >= face_count || faces.len() >= faces.capacity() {
            return Err(context.addressability());
        }
        faces.push(FaceDescriptor {
            directory_offset: header.offset,
            sfnt_version: header.word,
            table_start,
            table_count: header.table_count,
            metrics,
        });
    }
    Ok(PreparedSfntResource {
        bytes,
        faces,
        tables,
        container: container.kind,
        signature: container.signature,
        face_count: container.faces,
        usage: SfntResourceUsage {
            owned_bytes,
            work: work.used(),
            tables: table_count,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn limits() -> SfntResourceLimits {
        let max = NonZeroUsize::new(usize::MAX).unwrap();
        SfntResourceLimits {
            bytes: max,
            faces: max,
            tables_per_face: max,
            tables: max,
            owned_bytes: max,
            work: max,
        }
    }
    #[test]
    fn retained_budget_uses_the_same_owners_actual_three_capacities() {
        let bytes = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/sfnt_resource/single.ttf"
        ))
        .to_vec();
        let owner = prepare_sfnt_resource(bytes, limits()).unwrap();
        // Independent arithmetic on the one successful construction, no reallocation.
        let byte_capacity = owner.bytes.capacity();
        let face_capacity_bytes = owner.faces.capacity() * size_of::<FaceDescriptor>();
        let table_capacity_bytes = owner.tables.capacity() * size_of::<TableDescriptor>();
        let total = byte_capacity + face_capacity_bytes + table_capacity_bytes;
        assert_eq!(owner.usage().owned_bytes, total);
        assert_eq!(
            retained_bytes(
                byte_capacity,
                face_capacity_bytes,
                table_capacity_bytes,
                total
            )
            .unwrap(),
            total
        );
        let error = retained_bytes(
            byte_capacity,
            face_capacity_bytes,
            table_capacity_bytes,
            total - 1,
        )
        .unwrap_err();
        assert!(matches!(
            error.kind,
            SfntResourceErrorKind::Limit(SfntResourceLimit::OwnedBytes)
        ));
        assert_eq!((error.face_index, error.record_index), (None, None));
    }
    #[test]
    fn checked_arithmetic_rejects_actual_spans_and_allocation_sizes() {
        let end = u64::from(u32::MAX);
        assert_eq!(parser_end(end - 16, 1, 16, GLOBAL).unwrap() as u64, end);
        for result in [
            parser_end(end - 15, 1, 16, GLOBAL),
            parser_end(u64::MAX, 1, 1, GLOBAL),
            parser_end(0, u64::MAX, 2, GLOBAL),
            allocation_bytes::<FaceDescriptor>(usize::MAX),
            allocation_bytes::<u8>(isize::MAX as usize + 1),
            retained_bytes(usize::MAX, 1, 0, usize::MAX),
            remaining_work(usize::MAX, 0),
            remaining_work(0, usize::MAX),
        ] {
            let error = result.unwrap_err();
            assert!(matches!(error.kind, SfntResourceErrorKind::Addressability));
            assert_eq!((error.face_index, error.record_index), (None, None));
        }
        assert_eq!(
            allocation_bytes::<u8>(isize::MAX as usize).unwrap(),
            isize::MAX as usize
        );
        assert_eq!(remaining_work(2, 4).unwrap(), 20);
        assert_eq!(retained_bytes(0, 0, 0, 1).unwrap(), 0);
        let malformed = payload_range(
            &[],
            u32::MAX,
            u32::MAX,
            Context::face(7).record(2),
            SfntMalformed::TableRange,
        )
        .unwrap_err();
        assert!(matches!(
            malformed.kind,
            SfntResourceErrorKind::Malformed(SfntMalformed::TableRange)
        ));
        assert_eq!(
            (malformed.face_index, malformed.record_index),
            (Some(7), Some(2))
        );
    }
    #[test]
    fn deterministic_capacity_overflow_preserves_reservation_error_sources() {
        for storage in [SfntDescriptorStorage::Faces, SfntDescriptorStorage::Tables] {
            // This impossible capacity fails deterministically, without physical OOM.
            // It qualifies conversion, not an allocator failure after valid preflight.
            let source = Vec::<u8>::new().try_reserve_exact(usize::MAX).unwrap_err();
            let error = reservation_error(storage, source);
            let nested = error.source().unwrap();
            assert!(nested.downcast_ref::<TryReserveError>().is_some());
            assert_eq!((error.face_index, error.record_index), (None, None));
            match &error.kind {
                SfntResourceErrorKind::Allocation {
                    storage: actual,
                    source,
                } => {
                    assert_eq!(*actual, storage);
                    assert!(std::ptr::eq(
                        nested.downcast_ref::<TryReserveError>().unwrap(),
                        source
                    ));
                }
                other => panic!("unexpected conversion: {other:?}"),
            }
            assert!(error.to_string().contains("reservation failed"));
        }
    }
}
