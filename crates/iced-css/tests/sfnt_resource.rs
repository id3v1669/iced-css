//! Public tests for `prepare_sfnt_resource`.
//! Fixtures are deliberately incomplete fonts; success is structural/metric preparation.
use iced_css::fonts::metrics::{
    DesignMetricSource, FontMetricError, FontMetricLimits, read_sfnt_design_metrics,
};
use iced_css::fonts::sfnt_resource::{
    AmbiguousSfntTable, PreparedSfntResource, SfntContainer, SfntMalformed, SfntResourceError,
    SfntResourceErrorKind, SfntResourceLimit, SfntResourceLimits, SfntTable, SfntUnsupported,
    prepare_sfnt_resource,
};
use std::num::NonZeroUsize;
macro_rules! fixture {
    ($name:literal) => {
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/sfnt_resource/",
            $name
        ))
        .as_slice()
    };
}
fn nz(n: usize) -> NonZeroUsize {
    NonZeroUsize::new(n).expect("positive test limit")
}
fn limits() -> SfntResourceLimits {
    SfntResourceLimits {
        bytes: nz(usize::MAX),
        faces: nz(usize::MAX),
        tables_per_face: nz(usize::MAX),
        tables: nz(usize::MAX),
        owned_bytes: nz(usize::MAX),
        work: nz(usize::MAX),
    }
}
fn ready(bytes: &[u8]) -> PreparedSfntResource {
    prepare_sfnt_resource(bytes.to_vec(), limits()).expect("proposed preparation success")
}
fn error(bytes: Vec<u8>, cap: SfntResourceLimits) -> SfntResourceError {
    match prepare_sfnt_resource(bytes, cap) {
        Err(e) => e,
        Ok(_) => panic!("expected no completed owner"),
    }
}
fn context(e: &SfntResourceError, face: Option<u32>, record: Option<usize>) {
    assert_eq!(e.face_index, face);
    assert_eq!(e.record_index, record);
}
fn malformed(bytes: &[u8], kind: SfntMalformed, face: Option<u32>, record: Option<usize>) {
    let e = error(bytes.to_vec(), limits());
    context(&e, face, record);
    match e.kind {
        SfntResourceErrorKind::Malformed(actual) => assert_eq!(
            std::mem::discriminant(&actual),
            std::mem::discriminant(&kind)
        ),
        other => panic!("expected malformed {kind:?}, got {other:?}"),
    }
}
fn unsupported(bytes: &[u8], kind: SfntUnsupported, face: Option<u32>) {
    let e = error(bytes.to_vec(), limits());
    context(&e, face, None);
    match (e.kind, kind) {
        (
            SfntResourceErrorKind::Unsupported(SfntUnsupported::LegacySfnt(a)),
            SfntUnsupported::LegacySfnt(b),
        )
        | (
            SfntResourceErrorKind::Unsupported(SfntUnsupported::CollectionVersion(a)),
            SfntUnsupported::CollectionVersion(b),
        )
        | (
            SfntResourceErrorKind::Unsupported(SfntUnsupported::Signature(a)),
            SfntUnsupported::Signature(b),
        ) => assert_eq!(a, b),
        (
            SfntResourceErrorKind::Unsupported(SfntUnsupported::NormalizationRequired(a)),
            SfntUnsupported::NormalizationRequired(b),
        ) => assert_eq!(a, b),
        (
            SfntResourceErrorKind::Unsupported(SfntUnsupported::DirectoryBeforeCollectionHeader),
            SfntUnsupported::DirectoryBeforeCollectionHeader,
        ) => {}
        (a, b) => panic!("expected unsupported {b:?}, got {a:?}"),
    }
}
fn limit_error(
    bytes: Vec<u8>,
    cap: SfntResourceLimits,
    kind: SfntResourceLimit,
    face: Option<u32>,
) {
    let e = error(bytes, cap);
    context(&e, face, None);
    match e.kind {
        SfntResourceErrorKind::Limit(actual) => assert_eq!(
            std::mem::discriminant(&actual),
            std::mem::discriminant(&kind)
        ),
        other => panic!("expected limit {kind:?}, got {other:?}"),
    }
}
fn ambiguity(result: Result<Option<SfntTable<'_>>, AmbiguousSfntTable>) -> AmbiguousSfntTable {
    match result {
        Err(e) => e,
        Ok(_) => panic!("expected ambiguous table records"),
    }
}
fn put16(bytes: &mut [u8], at: usize, value: u16) {
    bytes[at..at + 2].copy_from_slice(&value.to_be_bytes());
}
fn put32(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_be_bytes());
}
fn signed(bytes: &mut [u8], at: usize, value: i16) {
    bytes[at..at + 2].copy_from_slice(&value.to_be_bytes());
}
fn a_metrics(owner: &PreparedSfntResource, index: u32, ascent: i32, descent: i32, gap: i32) {
    let face = || owner.face(index).unwrap();
    let m = face().metrics();
    assert_eq!(
        (m.units_per_em, m.source, m.ascent, m.descent, m.line_gap),
        (1000, DesignMetricSource::Hhea, ascent, descent, gap)
    );
    assert_eq!(
        (m.x_height_field, m.subscript_lowering, m.superscript_raise),
        (None, None, None)
    );
    assert!(m.underline.is_none() && m.strikeout.is_none());
    assert!(!m.variation_tables_present);
}
fn actual_metrics_join(owner: &PreparedSfntResource) {
    for index in 0..owner.face_count() {
        let f = || owner.face(index).unwrap();
        let direct = read_sfnt_design_metrics(
            owner.bytes(),
            index,
            FontMetricLimits {
                bytes: nz(owner.bytes().len()),
                tables: nz(f().table_count().max(1)),
                work: nz(5 + f().table_count()),
            },
        )
        .expect("accepted metrics prerequisite");
        let cached = f().metrics();
        assert_eq!(cached.units_per_em, direct.units_per_em);
        assert_eq!(cached.source, direct.source);
        assert_eq!(cached.ascent, direct.ascent);
        assert_eq!(cached.descent, direct.descent);
        assert_eq!(cached.line_gap, direct.line_gap);
        assert_eq!(cached.x_height_field, direct.x_height_field);
        assert_eq!(cached.subscript_lowering, direct.subscript_lowering);
        assert_eq!(cached.superscript_raise, direct.superscript_raise);
        assert_eq!(cached.underline, direct.underline);
        assert_eq!(cached.strikeout, direct.strikeout);
        assert_eq!(
            cached.variation_tables_present,
            direct.variation_tables_present
        );
    }
}
#[test]
fn immutable_single_owner_preserves_bytes_indices_tables_and_real_metrics() {
    for input in [fixture!("single.ttf"), fixture!("single-otto.otf")] {
        let caller = input.to_vec();
        let original_pointer = caller.as_ptr();
        let owner = prepare_sfnt_resource(caller, limits()).unwrap();
        let owner = Box::new(owner); // Move before taking borrowed views.
        assert_eq!(owner.bytes(), input);
        assert_eq!(owner.bytes().as_ptr(), original_pointer);
        assert_eq!(owner.container(), SfntContainer::Single);
        assert_eq!(owner.face_count(), 1);
        assert!(owner.signature_record().is_none());
        let face = || owner.face(0).unwrap();
        assert!(std::ptr::eq(face().resource(), owner.as_ref()));
        assert_eq!(
            (
                face().index(),
                face().directory_offset(),
                face().table_count()
            ),
            (0, 0, 2)
        );
        assert_eq!(
            face().sfnt_version(),
            u32::from_be_bytes(input[..4].try_into().unwrap())
        );
        for (i, tag, offset, length) in [(0, *b"head", 44, 54), (1, *b"hhea", 100, 36)] {
            let t = || face().table(i).unwrap();
            assert_eq!(
                (
                    t().record_index(),
                    t().tag(),
                    t().offset(),
                    t().length(),
                    t().checksum()
                ),
                (i, tag, offset, length, 0)
            );
            assert_eq!(t().face().index(), 0);
            assert!(std::ptr::eq(t().face().resource(), owner.as_ref()));
            assert_eq!(
                t().bytes(),
                &owner.bytes()[offset as usize..(offset + length) as usize]
            );
            assert_eq!(face().unique_table(tag).unwrap().unwrap().record_index(), i);
        }
        assert!(owner.face(1).is_none() && owner.face(u32::MAX).is_none());
        assert!(face().table(2).is_none() && face().table(usize::MAX).is_none());
        assert!(face().unique_table(*b"name").unwrap().is_none());
        a_metrics(&owner, 0, 731, 219, 43);
        actual_metrics_join(&owner);
        assert_eq!((owner.usage().work, owner.usage().tables), (12, 2));
    }
}
#[test]
fn collection_inventory_shares_global_ranges_without_losing_face_identity() {
    for (input, container, dirs) in [
        (
            fixture!("shared-v1.ttc"),
            SfntContainer::CollectionV1,
            [32, 76],
        ),
        (
            fixture!("shared-v2-null-dsig.ttc"),
            SfntContainer::CollectionV2,
            [32, 76],
        ),
        (
            fixture!("unaligned-directories-aligned-tables.ttc"),
            SfntContainer::CollectionV1,
            [33, 77],
        ),
    ] {
        let owner = ready(input);
        assert_eq!(owner.bytes(), input);
        assert_eq!(owner.container(), container);
        assert_eq!(owner.face_count(), 2);
        assert_eq!(owner.usage().tables, 4);
        assert_eq!(owner.usage().work, 23);
        for (i, offset) in dirs.into_iter().enumerate() {
            assert_eq!(owner.face(i as u32).unwrap().directory_offset(), offset);
        }
        let first = || {
            owner
                .face(0)
                .unwrap()
                .unique_table(*b"head")
                .unwrap()
                .unwrap()
        };
        let second = || {
            owner
                .face(1)
                .unwrap()
                .unique_table(*b"head")
                .unwrap()
                .unwrap()
        };
        assert_eq!(first().offset(), second().offset());
        assert_eq!(first().bytes().as_ptr(), second().bytes().as_ptr());
        assert_eq!(second().face().index(), 1);
        a_metrics(&owner, 0, 731, 219, 43);
        a_metrics(&owner, 1, 887, 303, -17);
        actual_metrics_join(&owner);
    }
}
#[test]
fn original_indices_survive_unsorted_repeated_offsets_and_mixed_member_words() {
    let mut b = fixture!("shared-v1.ttc").to_vec();
    put32(&mut b, 12, 76);
    put32(&mut b, 16, 32);
    b[76..80].copy_from_slice(b"OTTO");
    let owner = ready(&b);
    assert_eq!(
        owner.face(0).unwrap().sfnt_version(),
        u32::from_be_bytes(*b"OTTO")
    );
    a_metrics(&owner, 0, 887, 303, -17);
    a_metrics(&owner, 1, 731, 219, 43);
    actual_metrics_join(&owner);
    put32(&mut b, 16, 76);
    let owner = ready(&b);
    assert_eq!(owner.face_count(), 2);
    assert_eq!((owner.usage().tables, owner.usage().work), (4, 23));
    assert_eq!(
        owner.face(0).unwrap().directory_offset(),
        owner.face(1).unwrap().directory_offset()
    );
    a_metrics(&owner, 0, 887, 303, -17);
    a_metrics(&owner, 1, 887, 303, -17);
    actual_metrics_join(&owner);
}
#[test]
fn equal_custom_tags_preserve_record_identity_and_report_complete_ambiguity() {
    let owner = ready(fixture!("custom-duplicate.ttf"));
    let f = || owner.face(0).unwrap();
    let repeated = ambiguity(f().unique_table(*b"ZZZZ"));
    assert_eq!(
        (
            repeated.face_index,
            repeated.tag,
            repeated.first_record_index,
            repeated.record_count
        ),
        (0, *b"ZZZZ", 0, 2)
    );
    assert_eq!(f().table(0).unwrap().bytes(), b"ABCD");
    assert_eq!(f().table(1).unwrap().bytes(), b"WXYZ");
    assert_eq!(
        f().unique_table(*b"hhea").unwrap().unwrap().record_index(),
        3
    );
    assert!(
        f().unique_table(*b"AAAA").unwrap().is_none()
            && f().unique_table(*b"zzzz").unwrap().is_none()
    );
    let mut b = fixture!("custom-duplicate.ttf").to_vec();
    put32(&mut b, 36, 76);
    let owner = ready(&b);
    let f = || owner.face(0).unwrap();
    assert_eq!(f().table(0).unwrap().bytes(), f().table(1).unwrap().bytes());
    assert_ne!(
        f().table(0).unwrap().record_index(),
        f().table(1).unwrap().record_index()
    );
    assert_eq!(ambiguity(f().unique_table(*b"ZZZZ")).record_count, 2);
}
#[test]
fn equal_tag_ambiguity_at_directory_end_preserves_full_range() {
    let mut checksum_input = fixture!("custom-duplicate.ttf").to_vec();
    put32(&mut checksum_input, 16, 0x12345678);
    let checksum_owner = ready(&checksum_input);
    assert_eq!(
        checksum_owner.face(0).unwrap().table(0).unwrap().checksum(),
        0x12345678
    );
    let b = metric_sfnt(vec![
        (*b"head", head_body()),
        (*b"hhea", hhea_body()),
        (*b"zzzz", b"ABCD".to_vec()),
        (*b"zzzz", b"WXYZ".to_vec()),
    ]);
    let owner = ready(&b);
    let f = || owner.face(0).unwrap();
    let repeated = ambiguity(f().unique_table(*b"zzzz"));
    assert_eq!((repeated.first_record_index, repeated.record_count), (2, 2));
    assert_eq!(f().table(2).unwrap().bytes(), b"ABCD");
    assert_eq!(f().table(3).unwrap().bytes(), b"WXYZ");
    assert!(f().unique_table([255; 4]).unwrap().is_none());
}
#[test]
fn all_frozen_structural_failures_have_original_context_and_no_completed_owner() {
    malformed(
        fixture!("malformed-later-face.ttc"),
        SfntMalformed::TableRange,
        Some(1),
        Some(1),
    );
    malformed(
        fixture!("truncated-sfnt-header.bin"),
        SfntMalformed::FaceHeader,
        Some(0),
        None,
    );
    malformed(
        fixture!("truncated-sfnt-records.bin"),
        SfntMalformed::DirectoryRange,
        Some(0),
        None,
    );
    malformed(
        fixture!("truncated-ttc-offsets.bin"),
        SfntMalformed::CollectionHeader,
        None,
        None,
    );
    malformed(
        fixture!("truncated-v2-dsig.bin"),
        SfntMalformed::CollectionHeader,
        None,
        None,
    );
    malformed(
        fixture!("truncated-table.bin"),
        SfntMalformed::TableRange,
        Some(0),
        Some(1),
    );
    malformed(
        fixture!("v2-dsig-incoherent.ttc"),
        SfntMalformed::SignatureFields,
        None,
        None,
    );
    malformed(
        fixture!("v2-dsig-out-of-bounds.ttc"),
        SfntMalformed::SignatureRange,
        None,
        None,
    );
    malformed(
        fixture!("misaligned-table.ttf"),
        SfntMalformed::TableAlignment,
        Some(0),
        Some(0),
    );
    let e = error(fixture!("consumed-duplicate.ttf").to_vec(), limits());
    context(&e, Some(0), None);
    assert!(
        matches!(e.kind,SfntResourceErrorKind::Metrics(FontMetricError::DuplicateTable(tag)) if tag==*b"head")
    );
    assert!(std::error::Error::source(&e).is_some());
}
#[test]
fn unsupported_signatures_preserve_raw_words_and_normalization_distinction() {
    unsupported(
        fixture!("apple-true.ttf"),
        SfntUnsupported::LegacySfnt(u32::from_be_bytes(*b"true")),
        None,
    );
    unsupported(
        fixture!("apple-typ1.ttf"),
        SfntUnsupported::LegacySfnt(u32::from_be_bytes(*b"typ1")),
        None,
    );
    unsupported(
        fixture!("woff.bin"),
        SfntUnsupported::NormalizationRequired(*b"wOFF"),
        None,
    );
    unsupported(
        fixture!("woff2.bin"),
        SfntUnsupported::NormalizationRequired(*b"wOF2"),
        None,
    );
    unsupported(
        fixture!("unknown-magic.bin"),
        SfntUnsupported::Signature(u32::from_be_bytes(*b"ABCD")),
        None,
    );
    unsupported(
        fixture!("future-ttc-version.ttc"),
        SfntUnsupported::CollectionVersion(0x30000),
        None,
    );
    unsupported(
        fixture!("misaligned-face-offset.ttc"),
        SfntUnsupported::Signature(0x01000000),
        Some(1),
    );
    let mut b = fixture!("shared-v1.ttc").to_vec();
    b[76..80].copy_from_slice(b"wOFF");
    unsupported(
        &b,
        SfntUnsupported::Signature(u32::from_be_bytes(*b"wOFF")),
        Some(1),
    );
    b[76..80].copy_from_slice(b"true");
    unsupported(
        &b,
        SfntUnsupported::LegacySfnt(u32::from_be_bytes(*b"true")),
        Some(1),
    );
    b[76..80].copy_from_slice(b"typ1");
    unsupported(
        &b,
        SfntUnsupported::LegacySfnt(u32::from_be_bytes(*b"typ1")),
        Some(1),
    );
    b[76..80].copy_from_slice(b"ABCD");
    unsupported(
        &b,
        SfntUnsupported::Signature(u32::from_be_bytes(*b"ABCD")),
        Some(1),
    );
    b[76..80].copy_from_slice(b"ttcf");
    malformed(&b, SfntMalformed::NestedCollection, Some(1), None);
    let mut future = fixture!("future-ttc-version.ttc")[..8].to_vec();
    unsupported(&future, SfntUnsupported::CollectionVersion(0x30000), None);
    future.truncate(3);
    malformed(&future, SfntMalformed::Identification, None, None);
}
#[test]
fn signature_envelope_retains_opaque_bytes_and_trailing_data() {
    for tail in [&b""[..], &b"uninterpreted trailer"[..]] {
        let mut b = fixture!("shared-v2-dsig.ttc").to_vec();
        b.extend_from_slice(tail);
        let owner = ready(&b);
        assert_eq!(owner.bytes(), b.as_slice());
        let s = owner.signature_record().unwrap();
        assert_eq!((s.tag, s.offset, s.length), (*b"DSIG", 248, 8));
        assert_eq!(
            &owner.bytes()[s.offset as usize..(s.offset + s.length) as usize],
            &[0, 0, 0, 1, 0, 0, 0, 0]
        );
        actual_metrics_join(&owner);
    }
    assert!(
        ready(fixture!("shared-v2-null-dsig.ttc"))
            .signature_record()
            .is_none()
    );
    let mut b = fixture!("shared-v2-dsig.ttc").to_vec();
    b[20..24].copy_from_slice(b"ABCD");
    malformed(&b, SfntMalformed::SignatureFields, None, None);
    let mut b = fixture!("shared-v2-dsig.ttc").to_vec();
    put32(&mut b, 24, 0);
    malformed(&b, SfntMalformed::SignatureFields, None, None);
    let mut b = fixture!("shared-v2-dsig.ttc").to_vec();
    put32(&mut b, 28, 249);
    put32(&mut b, 24, 4);
    malformed(&b, SfntMalformed::SignatureAlignment, None, None);
    let mut b = fixture!("shared-v2-dsig.ttc").to_vec();
    put32(&mut b, 28, 28);
    malformed(&b, SfntMalformed::SignaturePlacement, None, None);
    let mut b = fixture!("shared-v2-dsig.ttc").to_vec();
    put32(&mut b, 28, 80);
    malformed(&b, SfntMalformed::SignaturePlacement, Some(1), None);
    let mut b = fixture!("shared-v2-dsig.ttc").to_vec();
    put32(&mut b, 28, 200);
    malformed(&b, SfntMalformed::SignaturePlacement, Some(0), Some(1));
    // A member record aliases the collection envelope. Placement fails before missing head.
    let mut b = fixture!("shared-v2-dsig.ttc").to_vec();
    b[44..48].copy_from_slice(b"DSIG");
    put32(&mut b, 52, 248);
    put32(&mut b, 56, 8);
    malformed(&b, SfntMalformed::SignaturePlacement, Some(0), Some(0));
}
#[test]
fn empty_zero_table_and_hostile_headers_are_bounded_without_large_allocation() {
    for length in 0..4 {
        malformed(
            &fixture!("single.ttf")[..length],
            SfntMalformed::Identification,
            None,
            None,
        );
    }
    malformed(b"\0\x01\0", SfntMalformed::Identification, None, None);
    let mut empty = fixture!("shared-v1.ttc")[..12].to_vec();
    put32(&mut empty, 8, 0);
    malformed(&empty, SfntMalformed::EmptyCollection, None, None);
    let mut zero = fixture!("single.ttf")[..12].to_vec();
    put16(&mut zero, 4, 0);
    let e = error(zero, limits());
    context(&e, Some(0), None);
    assert!(matches!(
        e.kind,
        SfntResourceErrorKind::Metrics(FontMetricError::MissingHead)
    ));
    let mut b = vec![0; 32];
    b[..4].copy_from_slice(b"ttcf");
    put32(&mut b, 4, 0x10000);
    put32(&mut b, 8, 2);
    put32(&mut b, 12, 16);
    put32(&mut b, 16, 0x10000);
    unsupported(
        &b,
        SfntUnsupported::DirectoryBeforeCollectionHeader,
        Some(0),
    );
    let mut huge = fixture!("shared-v1.ttc")[..12].to_vec();
    put32(&mut huge, 8, u32::MAX);
    let mut count_cap = limits();
    count_cap.faces = nz(100);
    limit_error(huge.clone(), count_cap, SfntResourceLimit::Faces, None);
    let mut cap = limits();
    cap.faces = nz(usize::MAX);
    let e = error(huge, cap);
    // Actual parser span 12 + 4*u32::MAX exceeds u32 on both32/64-bit hosts.
    context(&e, None, None);
    assert!(matches!(e.kind, SfntResourceErrorKind::Addressability));
    let mut huge = fixture!("single.ttf")[..12].to_vec();
    put16(&mut huge, 4, u16::MAX);
    let mut cap = limits();
    cap.tables_per_face = nz(usize::MAX);
    let e = error(huge, cap);
    context(&e, Some(0), None);
    assert!(matches!(
        e.kind,
        SfntResourceErrorKind::Malformed(SfntMalformed::DirectoryRange)
    ));
}
#[test]
fn custom_zero_length_eof_interleaving_and_descending_order_have_distinct_results() {
    let mut b = fixture!("custom-duplicate.ttf").to_vec();
    put32(&mut b, 20, 176);
    put32(&mut b, 24, 0);
    let owner = ready(&b);
    assert!(owner.face(0).unwrap().table(0).unwrap().bytes().is_empty());
    // Both custom ranges encompass head bytes; metric bodies remain intact.
    let mut b = fixture!("custom-duplicate.ttf").to_vec();
    put32(&mut b, 20, 84);
    put32(&mut b, 24, 56);
    put32(&mut b, 36, 88);
    put32(&mut b, 40, 52);
    let owner = ready(&b);
    a_metrics(&owner, 0, 731, 219, 43);
    assert_eq!(
        owner.face(0).unwrap().table(0).unwrap().bytes(),
        &b[84..140]
    );
    // An unknown table may reference the directory itself under this bounded profile.
    put32(&mut b, 20, 0);
    put32(&mut b, 24, 12);
    assert_eq!(
        ready(&b).face(0).unwrap().table(0).unwrap().bytes(),
        &b[..12]
    );
    let mut b = fixture!("single.ttf").to_vec();
    b[12..16].copy_from_slice(b"zzzz");
    malformed(&b, SfntMalformed::DirectoryOrder, Some(0), Some(1));
}
#[test]
fn exact_and_one_less_length_face_table_and_work_limits_are_measured() {
    let mut single_cap = limits();
    single_cap.work = nz(12);
    let single = prepare_sfnt_resource(fixture!("single.ttf").to_vec(), single_cap).unwrap();
    assert_eq!(single.usage().work, 12);
    single_cap.work = nz(11);
    limit_error(
        fixture!("single.ttf").to_vec(),
        single_cap,
        SfntResourceLimit::Work,
        None,
    );
    let input = fixture!("shared-v1.ttc");
    let mut exact = limits();
    exact.bytes = nz(input.len());
    exact.faces = nz(2);
    exact.tables_per_face = nz(2);
    exact.tables = nz(4);
    exact.work = nz(23);
    let owner = prepare_sfnt_resource(input.to_vec(), exact).unwrap();
    assert_eq!((owner.usage().tables, owner.usage().work), (4, 23));
    assert!(owner.usage().owned_bytes <= exact.owned_bytes.get());
    let mut cap = exact;
    cap.bytes = nz(input.len() - 1);
    limit_error(input.to_vec(), cap, SfntResourceLimit::Bytes, None);
    let mut cap = exact;
    cap.faces = nz(1);
    limit_error(input.to_vec(), cap, SfntResourceLimit::Faces, None);
    let mut cap = exact;
    cap.tables_per_face = nz(1);
    limit_error(
        input.to_vec(),
        cap,
        SfntResourceLimit::TablesPerFace,
        Some(0),
    );
    let mut cap = exact;
    cap.tables = nz(3);
    limit_error(input.to_vec(), cap, SfntResourceLimit::Tables, Some(1));
    let mut cap = exact;
    cap.work = nz(22);
    limit_error(input.to_vec(), cap, SfntResourceLimit::Work, None);
    let largest = nz(usize::MAX);
    let cap = SfntResourceLimits {
        bytes: largest,
        faces: largest,
        tables_per_face: largest,
        tables: largest,
        owned_bytes: largest,
        work: largest,
    };
    let owner = prepare_sfnt_resource(fixture!("single.ttf").to_vec(), cap).unwrap();
    assert_eq!(owner.usage().work, 12);
}
#[test]
fn moved_input_capacity_is_measured_and_not_confused_with_length() {
    let input = fixture!("single.ttf");
    let mut bytes = Vec::with_capacity(4096);
    bytes.extend_from_slice(input);
    let capacity = bytes.capacity();
    assert!(capacity > bytes.len());
    let mut cap = limits();
    cap.owned_bytes = nz(capacity - 1);
    limit_error(bytes, cap, SfntResourceLimit::OwnedBytes, None);
    let mut bytes = Vec::with_capacity(4096);
    bytes.extend_from_slice(input);
    let capacity = bytes.capacity();
    let pointer = bytes.as_ptr();
    let owner = prepare_sfnt_resource(bytes, limits()).unwrap();
    assert_eq!(owner.bytes().as_ptr(), pointer);
    assert!(owner.usage().owned_bytes > capacity);
    assert!(owner.usage().owned_bytes <= limits().owned_bytes.get());
}
#[test]
fn reported_capacity_bounds_remain_sound_when_separate_allocations_vary() {
    let input = fixture!("single.ttf");
    let first = ready(input);
    let observed = first.usage().owned_bytes;
    assert!(observed > input.len());
    for ceiling in [observed, observed - 1] {
        let mut cap = limits();
        cap.owned_bytes = nz(ceiling);
        match prepare_sfnt_resource(input.to_vec(), cap) {
            Ok(owner) => assert!(owner.usage().owned_bytes <= ceiling),
            Err(e) => {
                context(&e, None, None);
                assert!(matches!(
                    e.kind,
                    SfntResourceErrorKind::Limit(SfntResourceLimit::OwnedBytes)
                ));
            }
        }
    }
    // This is a portability/soundness check, not proof of a same-allocation exact-B threshold.
    // Root must independently recompute private capacities and compare B / B-1 internally.
}
#[test]
fn combined_defects_freeze_phase_order_and_original_error_context() {
    let mut cap = limits();
    cap.bytes = nz(1);
    cap.owned_bytes = nz(1);
    cap.work = nz(1);
    limit_error(
        fixture!("single.ttf").to_vec(),
        cap,
        SfntResourceLimit::Bytes,
        None,
    );
    let mut b = Vec::with_capacity(4096);
    b.extend_from_slice(b"ABCD");
    let mut cap = limits();
    cap.owned_bytes = nz(b.capacity() - 1);
    cap.work = nz(1);
    limit_error(b, cap, SfntResourceLimit::OwnedBytes, None);
    let mut cap = limits();
    cap.work = nz(1);
    limit_error(
        fixture!("truncated-sfnt-header.bin").to_vec(),
        cap,
        SfntResourceLimit::Work,
        Some(0),
    );
    // Invalid head body in face0, but later directory header fails first inventory.
    let mut b = fixture!("shared-v1.ttc").to_vec();
    b[132] = 0;
    put32(&mut b, 16, 248);
    malformed(&b, SfntMalformed::FaceHeader, Some(1), None);
    // Earlier metrics fail before later record traversal.
    let mut b = fixture!("malformed-later-face.ttc").to_vec();
    b[132] = 0;
    let e = error(b, limits());
    context(&e, Some(0), None);
    assert!(matches!(
        e.kind,
        SfntResourceErrorKind::Metrics(FontMetricError::InvalidHead)
    ));
    // Descending second tag wins over its hostile range.
    let mut b = fixture!("single.ttf").to_vec();
    b[28..32].copy_from_slice(b"AAAA");
    put32(&mut b, 36, u32::MAX);
    malformed(&b, SfntMalformed::DirectoryOrder, Some(0), Some(1));
    // A range that is both out-of-bounds and unaligned reports range first.
    let mut b = fixture!("single.ttf").to_vec();
    put32(&mut b, 20, u32::MAX);
    malformed(&b, SfntMalformed::TableRange, Some(0), Some(0));
    // Enough inventory work, insufficient remaining work, and insufficient descriptor memory.
    let mut cap = limits();
    cap.work = nz(2);
    let b = fixture!("single.ttf").to_vec();
    cap.owned_bytes = nz(b.capacity());
    limit_error(b, cap, SfntResourceLimit::Work, None);
}
// Literal auxiliary metric bodies; offsets from the retained field-layout references.
fn metric_sfnt(mut records: Vec<([u8; 4], Vec<u8>)>) -> Vec<u8> {
    records.sort_by_key(|r| r.0);
    let mut b = vec![0; 12 + 16 * records.len()];
    put32(&mut b, 0, 0x10000);
    put16(&mut b, 4, records.len().try_into().unwrap());
    for (i, (tag, body)) in records.into_iter().enumerate() {
        let at = 12 + 16 * i;
        let offset = b.len();
        b[at..at + 4].copy_from_slice(&tag);
        put32(&mut b, at + 8, offset.try_into().unwrap());
        put32(&mut b, at + 12, body.len().try_into().unwrap());
        b.extend(body);
        while !b.len().is_multiple_of(4) {
            b.push(0);
        }
    }
    b
}
fn head_body() -> Vec<u8> {
    fixture!("single.ttf")[44..98].to_vec()
}
fn hhea_body() -> Vec<u8> {
    fixture!("single.ttf")[100..136].to_vec()
}
fn os2_body() -> Vec<u8> {
    let mut b = vec![0; 96];
    put16(&mut b, 0, 2);
    signed(&mut b, 16, 151);
    signed(&mut b, 24, 349);
    signed(&mut b, 26, -21);
    signed(&mut b, 28, 301);
    signed(&mut b, 68, 811);
    signed(&mut b, 70, -223);
    signed(&mut b, 72, -19);
    signed(&mut b, 86, 0);
    b
}
fn post_body() -> Vec<u8> {
    let mut b = vec![0; 32];
    put32(&mut b, 0, 0x30000);
    signed(&mut b, 8, -79);
    signed(&mut b, 10, 51);
    b
}
#[test]
fn metric_join_preserves_os2_preference_raw_signed_fields_and_variation_presence() {
    let b = metric_sfnt(vec![
        (*b"head", head_body()),
        (*b"hhea", hhea_body()),
        (*b"OS/2", os2_body()),
        (*b"post", post_body()),
        (*b"fvar", vec![]),
        (*b"fvar", vec![]),
        (*b"MVAR", vec![]),
    ]);
    let owner = ready(&b);
    actual_metrics_join(&owner);
    let f = || owner.face(0).unwrap();
    let m = f().metrics();
    assert_eq!(
        (m.source, m.ascent, m.descent, m.line_gap),
        (DesignMetricSource::Os2, 811, 223, -19)
    );
    assert_eq!(
        (m.x_height_field, m.subscript_lowering, m.superscript_raise),
        (Some(0), Some(151), Some(349))
    );
    assert!(m.variation_tables_present);
    let underline = m.underline.unwrap();
    assert_eq!(
        (
            underline.format,
            underline.line.position,
            underline.line.thickness
        ),
        (0x30000, -79, 51)
    );
    let strike = m.strikeout.unwrap();
    assert_eq!((strike.position, strike.thickness), (301, -21));
    assert_eq!(ambiguity(f().unique_table(*b"fvar")).record_count, 2);
    let b = metric_sfnt(vec![
        (*b"head", head_body()),
        (*b"hhea", hhea_body()),
        (*b"OS/2", vec![0; 2]),
    ]);
    let e = error(b, limits());
    context(&e, Some(0), None);
    assert!(matches!(
        e.kind,
        SfntResourceErrorKind::Metrics(FontMetricError::InvalidOs2)
    ));
    let b = metric_sfnt(vec![
        (*b"head", head_body()),
        (*b"hhea", hhea_body()),
        (*b"post", vec![0; 31]),
    ]);
    let e = error(b, limits());
    context(&e, Some(0), None);
    assert!(matches!(
        e.kind,
        SfntResourceErrorKind::Metrics(FontMetricError::InvalidPost)
    ));
}
#[test]
fn all_consumed_metric_duplicates_preserve_nested_reader_error() {
    for (tag, body) in [
        (*b"hhea", hhea_body()),
        (*b"OS/2", os2_body()),
        (*b"post", post_body()),
    ] {
        let mut tables = vec![
            (*b"head", head_body()),
            (*b"OS/2", os2_body()),
            (*b"hhea", hhea_body()),
            (*b"post", post_body()),
        ];
        tables.push((tag, body));
        let e = error(metric_sfnt(tables), limits());
        context(&e, Some(0), None);
        assert!(
            matches!(e.kind,SfntResourceErrorKind::Metrics(FontMetricError::DuplicateTable(actual)) if actual==tag)
        );
        assert!(std::error::Error::source(&e).is_some());
    }
}
#[test]
fn retained_fira_exact_bytes_join_metrics_without_rendering_claim() {
    let input = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/sfnt_resource/FiraSans-Regular.ttf"
    ))
    .as_slice();
    // Byte SHA-256 is checked by the sealed external input verification, not by a new Rust dependency.
    assert_eq!(input.len(), 440984);
    let owner = ready(input);
    assert_eq!(owner.bytes(), input);
    assert_eq!(owner.face_count(), 1);
    assert_eq!(owner.face(0).unwrap().table_count(), 17);
    assert_eq!(owner.usage().work, 42);
    actual_metrics_join(&owner);
    let m = owner.face(0).unwrap().metrics();
    assert_eq!(
        (m.units_per_em, m.source, m.ascent, m.descent, m.line_gap),
        (1000, DesignMetricSource::Os2, 935, 265, 0)
    );
    assert_eq!(
        (m.x_height_field, m.subscript_lowering, m.superscript_raise),
        (Some(527), Some(75), Some(350))
    );
    let u = m.underline.unwrap();
    assert_eq!(
        (u.format, u.line.position, u.line.thickness),
        (0x20000, -75, 50)
    );
    let s = m.strikeout.unwrap();
    assert_eq!((s.position, s.thickness), (316, 50));
    assert!(!m.variation_tables_present);
}
