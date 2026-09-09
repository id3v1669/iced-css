//! Synthetic metric-table structures, not complete renderable font fixtures.
use iced_css::fonts::metrics::{
    DesignMetricSource, FontMetricError, FontMetricLimits, read_sfnt_design_metrics,
};
use std::num::NonZeroUsize;
fn nz(n: usize) -> NonZeroUsize {
    NonZeroUsize::new(n).unwrap()
}
fn limits() -> FontMetricLimits {
    FontMetricLimits {
        bytes: nz(100000),
        tables: nz(100),
        work: nz(1000),
    }
}
fn word(data: &mut [u8], offset: usize, value: i16) {
    data[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
}
fn head() -> Vec<u8> {
    let mut data = vec![0; 54];
    data[..4].copy_from_slice(&0x00010000u32.to_be_bytes());
    data[12..16].copy_from_slice(&0x5f0f3cf5u32.to_be_bytes());
    word(&mut data, 18, 1000);
    data
}
fn hhea() -> Vec<u8> {
    let mut data = vec![0; 36];
    data[..4].copy_from_slice(&0x00010000u32.to_be_bytes());
    word(&mut data, 4, 900);
    word(&mut data, 6, -250);
    word(&mut data, 8, 50);
    data
}
fn os2(version: u16) -> Vec<u8> {
    let mut data = vec![0; if version == 0 { 78 } else { 96 }];
    data[..2].copy_from_slice(&version.to_be_bytes());
    word(&mut data, 16, 150);
    word(&mut data, 24, 350);
    word(&mut data, 68, 800);
    word(&mut data, 70, -200);
    word(&mut data, 72, 100);
    if version >= 2 {
        word(&mut data, 86, 450);
    }
    data
}
fn sfnt(mut tables: Vec<([u8; 4], Vec<u8>)>) -> Vec<u8> {
    tables.sort_by_key(|t| t.0);
    let mut data = vec![0; 12 + 16 * tables.len()];
    data[..4].copy_from_slice(&0x00010000u32.to_be_bytes());
    data[4..6].copy_from_slice(&(tables.len() as u16).to_be_bytes());
    for (i, (tag, body)) in tables.into_iter().enumerate() {
        let offset = data.len();
        let entry = 12 + i * 16;
        data[entry..entry + 4].copy_from_slice(&tag);
        data[entry + 8..entry + 12].copy_from_slice(&(offset as u32).to_be_bytes());
        data[entry + 12..entry + 16].copy_from_slice(&(body.len() as u32).to_be_bytes());
        data.extend(body);
        while !data.len().is_multiple_of(4) {
            data.push(0);
        }
    }
    data
}
fn fixture() -> Vec<u8> {
    sfnt(vec![
        (*b"head", head()),
        (*b"hhea", hhea()),
        (*b"OS/2", os2(2)),
    ])
}
#[test]
fn css_typographic_metrics_win_without_the_optional_use_typo_flag() {
    let m = read_sfnt_design_metrics(&fixture(), 0, limits()).unwrap();
    assert_eq!(m.source, DesignMetricSource::Os2);
    assert_eq!(
        (m.units_per_em, m.ascent, m.descent, m.line_gap),
        (1000, 800, 200, 100)
    );
    assert_eq!(
        (m.x_height_field, m.subscript_lowering, m.superscript_raise),
        (Some(450), Some(150), Some(350))
    );
    assert!(!m.variation_tables_present);
}
#[test]
fn absence_and_zero_are_preserved_and_minimum_descender_does_not_overflow() {
    let data = sfnt(vec![(*b"head", head()), (*b"hhea", hhea())]);
    let m = read_sfnt_design_metrics(&data, 0, limits()).unwrap();
    assert_eq!(m.source, DesignMetricSource::Hhea);
    assert_eq!((m.ascent, m.descent, m.line_gap), (900, 250, 50));
    assert_eq!(
        (m.x_height_field, m.subscript_lowering, m.superscript_raise),
        (None, None, None)
    );
    let mut table = os2(0);
    word(&mut table, 70, i16::MIN);
    word(&mut table, 72, -20);
    let data = sfnt(vec![
        (*b"head", head()),
        (*b"OS/2", table),
        (*b"fvar", vec![0; 16]),
    ]);
    let m = read_sfnt_design_metrics(&data, 0, limits()).unwrap();
    assert_eq!(
        (m.descent, m.line_gap, m.x_height_field),
        (32768, -20, None)
    );
    assert!(m.variation_tables_present);
    let mut table = os2(2);
    word(&mut table, 86, 0);
    let data = sfnt(vec![(*b"head", head()), (*b"OS/2", table)]);
    assert_eq!(
        read_sfnt_design_metrics(&data, 0, limits())
            .unwrap()
            .x_height_field,
        Some(0)
    );
}
#[test]
fn collections_use_absolute_table_offsets_and_the_requested_face_index() {
    let first = fixture();
    let second = sfnt(vec![(*b"head", head()), (*b"hhea", hhea())]);
    let mut data = vec![0; 20];
    data[..4].copy_from_slice(b"ttcf");
    data[4..8].copy_from_slice(&0x00010000u32.to_be_bytes());
    data[8..12].copy_from_slice(&2u32.to_be_bytes());
    for (i, mut face) in [first, second].into_iter().enumerate() {
        let start = data.len();
        data[12 + i * 4..16 + i * 4].copy_from_slice(&(start as u32).to_be_bytes());
        let count = u16::from_be_bytes(face[4..6].try_into().unwrap()) as usize;
        for j in 0..count {
            let at = 12 + j * 16 + 8;
            let old = u32::from_be_bytes(face[at..at + 4].try_into().unwrap());
            face[at..at + 4].copy_from_slice(&(old + start as u32).to_be_bytes());
        }
        data.extend(face);
    }
    assert_eq!(
        read_sfnt_design_metrics(&data, 0, limits()).unwrap().ascent,
        800
    );
    assert_eq!(
        read_sfnt_design_metrics(&data, 1, limits()).unwrap().ascent,
        900
    );
    assert!(matches!(
        read_sfnt_design_metrics(&data, 2, limits()),
        Err(FontMetricError::Directory(_))
    ));
    assert!(matches!(
        read_sfnt_design_metrics(&fixture(), 1, limits()),
        Err(FontMetricError::Directory(_))
    ));
}
#[test]
fn malformed_used_tables_and_directory_ranges_are_not_silent_fallbacks() {
    let bad = sfnt(vec![
        (*b"head", head()),
        (*b"hhea", hhea()),
        (*b"OS/2", vec![0; 2]),
    ]);
    assert!(matches!(
        read_sfnt_design_metrics(&bad, 0, limits()),
        Err(FontMetricError::InvalidOs2)
    ));
    let mut bad = fixture();
    bad[24..28].copy_from_slice(&u32::MAX.to_be_bytes());
    assert!(matches!(
        read_sfnt_design_metrics(&bad, 0, limits()),
        Err(FontMetricError::TableRange(_))
    ));
    let bad = sfnt(vec![(*b"head", head()), (*b"head", head())]);
    assert!(matches!(
        read_sfnt_design_metrics(&bad, 0, limits()),
        Err(FontMetricError::DuplicateTable(_))
    ));
    let mut bad = fixture();
    bad[12..16].copy_from_slice(b"zzzz");
    assert!(matches!(
        read_sfnt_design_metrics(&bad, 0, limits()),
        Err(FontMetricError::DirectoryOrder)
    ));
    let mut bad_head = head();
    bad_head[12] = 0;
    assert!(matches!(
        read_sfnt_design_metrics(
            &sfnt(vec![(*b"head", bad_head), (*b"hhea", hhea())]),
            0,
            limits()
        ),
        Err(FontMetricError::InvalidHead)
    ));
    let bad = sfnt(vec![
        (*b"ZZZZ", vec![0; 4]),
        (*b"ZZZZ", vec![0; 4]),
        (*b"head", head()),
        (*b"hhea", hhea()),
    ]);
    assert_eq!(
        read_sfnt_design_metrics(&bad, 0, limits()).unwrap().ascent,
        900
    );
}
#[test]
fn input_byte_record_and_work_budgets_are_checked() {
    let data = fixture();
    let mut bound = limits();
    bound.bytes = nz(1);
    assert!(matches!(
        read_sfnt_design_metrics(&data, 0, bound),
        Err(FontMetricError::Bytes)
    ));
    bound = limits();
    bound.tables = nz(1);
    assert!(matches!(
        read_sfnt_design_metrics(&data, 0, bound),
        Err(FontMetricError::Tables)
    ));
    bound = limits();
    bound.work = nz(1);
    assert!(matches!(
        read_sfnt_design_metrics(&data, 0, bound),
        Err(FontMetricError::Work)
    ));
    bound = limits();
    bound.work = nz(6);
    assert!(matches!(
        read_sfnt_design_metrics(&data, 0, bound),
        Err(FontMetricError::Work)
    ));
    assert!(matches!(
        read_sfnt_design_metrics(b"", 0, limits()),
        Err(FontMetricError::Directory(_))
    ));
}

#[test]
fn missing_headers_invalid_versions_units_and_alignment_have_explicit_errors() {
    let data = sfnt(vec![(*b"hhea", hhea())]);
    assert!(matches!(
        read_sfnt_design_metrics(&data, 0, limits()),
        Err(FontMetricError::MissingHead)
    ));
    let data = sfnt(vec![(*b"head", head())]);
    assert!(matches!(
        read_sfnt_design_metrics(&data, 0, limits()),
        Err(FontMetricError::MissingHhea)
    ));
    let mut table = head();
    table[1] = 2;
    let data = sfnt(vec![(*b"head", table), (*b"hhea", hhea())]);
    assert!(matches!(
        read_sfnt_design_metrics(&data, 0, limits()),
        Err(FontMetricError::InvalidHead)
    ));
    let mut table = head();
    word(&mut table, 18, 0);
    let data = sfnt(vec![(*b"head", table), (*b"hhea", hhea())]);
    assert!(matches!(
        read_sfnt_design_metrics(&data, 0, limits()),
        Err(FontMetricError::InvalidHead)
    ));
    let mut table = hhea();
    table[1] = 2;
    let data = sfnt(vec![(*b"head", head()), (*b"hhea", table)]);
    assert!(matches!(
        read_sfnt_design_metrics(&data, 0, limits()),
        Err(FontMetricError::InvalidHhea)
    ));
    let data = sfnt(vec![(*b"head", head()), (*b"hhea", vec![0; 2])]);
    assert!(matches!(
        read_sfnt_design_metrics(&data, 0, limits()),
        Err(FontMetricError::InvalidHhea)
    ));
    let mut data = fixture();
    let offset = u32::from_be_bytes(data[20..24].try_into().unwrap());
    data[20..24].copy_from_slice(&(offset + 1).to_be_bytes());
    assert!(matches!(
        read_sfnt_design_metrics(&data, 0, limits()),
        Err(FontMetricError::TableAlignment(_))
    ));
}

#[test]
fn scaled_design_metrics_feed_line_geometry_and_keep_absent_fields() {
    use iced_css_core::formatting::line_geometry::{
        InlineBounds, LineAlignment, LineGeometryLimits, LineParticipant, layout_line_geometry,
    };
    let design = read_sfnt_design_metrics(&fixture(), 0, limits()).unwrap();
    let scaled = design.scale(10.0).unwrap();
    assert_eq!(
        (
            scaled.ascent.value,
            scaled.descent.value,
            scaled.line_gap.value
        ),
        (8.0, 2.0, 1.0)
    );
    assert_eq!(scaled.x_height_field.unwrap().value, 4.5);
    assert_eq!(scaled.subscript_lowering.unwrap().value, 1.5);
    assert_eq!(scaled.superscript_raise.unwrap().value, 3.5);
    assert_eq!(scaled.source, DesignMetricSource::Os2);
    let strut = InlineBounds::font(scaled.ascent.value, scaled.descent.value, 10.0).unwrap();
    let child = InlineBounds::font(4.0, 1.0, 5.0).unwrap();
    let line = layout_line_geometry(
        strut,
        &[LineParticipant {
            bounds: child,
            alignment: LineAlignment::Baseline {
                raise: scaled.superscript_raise.unwrap().value,
            },
        }],
        LineGeometryLimits {
            participants: nz(10),
            retained_bytes: nz(1000),
            work: nz(1000),
        },
    )
    .unwrap();
    assert_eq!((line.height, line.baseline), (10.0, 8.0));
    let p = line.placements[0];
    assert_eq!((p.top, p.bottom, p.origin), (0.5, 5.5, 4.5));
    let data = sfnt(vec![(*b"head", head()), (*b"hhea", hhea())]);
    let scaled = read_sfnt_design_metrics(&data, 0, limits())
        .unwrap()
        .scale(10.0)
        .unwrap();
    assert!(scaled.x_height_field.is_none());
    assert!(scaled.subscript_lowering.is_none());
    assert!(scaled.superscript_raise.is_none());
}

#[test]
fn scaling_keeps_conversion_flags_and_reports_late_field_failures() {
    use iced_css::fonts::metrics::DesignMetric;
    use iced_css_core::values::font_metrics::FontMetricScaleError;
    let design = read_sfnt_design_metrics(&fixture(), 0, limits()).unwrap();
    let tiny = design.scale(f32::from_bits(1)).unwrap();
    assert!(tiny.descent.rounded_to_zero);
    assert!(tiny.subscript_lowering.unwrap().rounded_to_zero);
    assert_eq!(tiny.descent.value.to_bits(), 0);
    let mut font_head = head();
    word(&mut font_head, 18, 16);
    let mut table = os2(2);
    word(&mut table, 68, 0);
    word(&mut table, 70, 0);
    let data = sfnt(vec![(*b"head", font_head), (*b"OS/2", table)]);
    let design = read_sfnt_design_metrics(&data, 0, limits()).unwrap();
    let error = design.scale(f32::MAX).unwrap_err();
    assert_eq!(error.metric, DesignMetric::LineGap);
    assert_eq!(error.error, FontMetricScaleError::Unrepresentable);
}

fn post(format: u32, position: i16, thickness: i16) -> Vec<u8> {
    let mut data = vec![0; if format == 0x00020000 { 34 } else { 32 }];
    data[..4].copy_from_slice(&format.to_be_bytes());
    word(&mut data, 8, position);
    word(&mut data, 10, thickness);
    data
}
#[test]
fn decoration_metrics_preserve_raw_signed_fields_and_post_format() {
    for format in [0x00010000, 0x00020000, 0x00025000, 0x00030000, 0x00040000] {
        let mut table = os2(2);
        word(&mut table, 26, -20);
        word(&mut table, 28, 300);
        let data = sfnt(vec![
            (*b"head", head()),
            (*b"OS/2", table),
            (*b"post", post(format, -100, 50)),
        ]);
        let metrics = read_sfnt_design_metrics(&data, 0, limits()).unwrap();
        let underline = metrics.underline.unwrap();
        assert_eq!(
            (
                underline.format,
                underline.line.position,
                underline.line.thickness
            ),
            (format, -100, 50)
        );
        let strikeout = metrics.strikeout.unwrap();
        assert_eq!((strikeout.position, strikeout.thickness), (300, -20));
        let scaled = metrics.scale(20.0).unwrap();
        let underline = scaled.underline.unwrap();
        assert_eq!(
            (
                underline.format,
                underline.line.position.value,
                underline.line.thickness.value
            ),
            (format, -2.0, 1.0)
        );
        let strikeout = scaled.strikeout.unwrap();
        assert_eq!(
            (strikeout.position.value, strikeout.thickness.value),
            (6.0, -0.4)
        );
    }
}
#[test]
fn missing_decoration_tables_do_not_synthesize_metrics() {
    let data = sfnt(vec![(*b"head", head()), (*b"hhea", hhea())]);
    let metrics = read_sfnt_design_metrics(&data, 0, limits()).unwrap();
    assert!(metrics.underline.is_none());
    assert!(metrics.strikeout.is_none());
    let scaled = metrics.scale(20.0).unwrap();
    assert!(scaled.underline.is_none());
    assert!(scaled.strikeout.is_none());
    let data = sfnt(vec![
        (*b"head", head()),
        (*b"hhea", hhea()),
        (*b"post", post(0x00030000, i16::MIN, i16::MAX)),
    ]);
    let metrics = read_sfnt_design_metrics(&data, 0, limits()).unwrap();
    assert!(metrics.strikeout.is_none());
    let line = metrics.underline.unwrap().line;
    assert_eq!((line.position, line.thickness), (-32768, 32767));
}
#[test]
fn malformed_duplicate_and_unadmitted_post_tables_fail() {
    for table in [
        vec![0; 31],
        post(0x00050000, 0, 0),
        post(0x00020000, 0, 0)[..32].to_vec(),
    ] {
        let data = sfnt(vec![
            (*b"head", head()),
            (*b"hhea", hhea()),
            (*b"post", table),
        ]);
        assert!(matches!(
            read_sfnt_design_metrics(&data, 0, limits()),
            Err(FontMetricError::InvalidPost)
        ));
    }
    let data = sfnt(vec![
        (*b"head", head()),
        (*b"hhea", hhea()),
        (*b"post", post(0x00030000, 0, 0)),
        (*b"post", post(0x00030000, 0, 0)),
    ]);
    assert!(
        matches!(read_sfnt_design_metrics(&data,0,limits()),Err(FontMetricError::DuplicateTable(tag)) if tag==*b"post")
    );
    let data = sfnt(vec![
        (*b"head", head()),
        (*b"hhea", hhea()),
        (*b"post", post(0x00030000, 0, 0)),
    ]);
    let mut cap = limits();
    cap.work = nz(7);
    assert!(matches!(
        read_sfnt_design_metrics(&data, 0, cap),
        Err(FontMetricError::Work)
    ));
    cap.work = nz(8);
    assert!(read_sfnt_design_metrics(&data, 0, cap).is_ok());
}
#[test]
fn decoration_scaling_reports_the_late_field_and_keeps_underflow_flags() {
    use iced_css::fonts::metrics::DesignMetric;
    let mut h = hhea();
    word(&mut h, 4, 0);
    word(&mut h, 6, 0);
    word(&mut h, 8, 0);
    let mut font_head = head();
    word(&mut font_head, 18, 16);
    let data = sfnt(vec![
        (*b"head", font_head),
        (*b"hhea", h),
        (*b"post", post(0x00030000, 0, 1000)),
    ]);
    let metrics = read_sfnt_design_metrics(&data, 0, limits()).unwrap();
    assert_eq!(
        metrics.scale(f32::MAX).unwrap_err().metric,
        DesignMetric::UnderlineThickness
    );
    let mut strike_design = metrics;
    strike_design.underline = None;
    for (position, thickness, expected) in [
        (1000, 0, DesignMetric::StrikeoutPosition),
        (0, 1000, DesignMetric::StrikeoutThickness),
    ] {
        strike_design.strikeout = Some(iced_css::fonts::metrics::DesignLineMetrics {
            position,
            thickness,
        });
        assert_eq!(strike_design.scale(f32::MAX).unwrap_err().metric, expected);
    }
    let data = sfnt(vec![
        (*b"head", head()),
        (*b"hhea", hhea()),
        (*b"post", post(0x00030000, -1, 1)),
    ]);
    let metrics = read_sfnt_design_metrics(&data, 0, limits())
        .unwrap()
        .scale(f32::from_bits(1))
        .unwrap();
    let line = metrics.underline.unwrap().line;
    assert!(line.position.rounded_to_zero);
    assert!(line.thickness.rounded_to_zero);
    assert_eq!(
        (
            line.position.value.to_bits(),
            line.thickness.value.to_bits()
        ),
        (0, 0)
    );
}
