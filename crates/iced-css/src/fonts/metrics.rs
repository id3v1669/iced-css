//! Bounded extraction of unadjusted SFNT design-table metrics.
//! Success is not complete font validation, glyph coverage or variable-instance
//! metric qualification. No font bytes are copied and no heap allocation is used.
use std::num::NonZeroUsize;
use ttf_parser::{FaceParsingError, RawFace};
#[derive(Debug, Clone, Copy)]
pub struct FontMetricLimits {
    pub bytes: NonZeroUsize,
    pub tables: NonZeroUsize,
    /// One step per directory record plus five prepaid header/table parse steps.
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesignMetricSource {
    Os2,
    Hhea,
}
/// Raw signed decoration fields. Nonpositive thickness is not certified usable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DesignLineMetrics {
    pub position: i32,
    pub thickness: i32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DesignUnderlineMetrics {
    /// Original post format; Apple format4 geometry needs separate interpretation.
    pub format: u32,
    pub line: DesignLineMetrics,
}
fn design_line(line: ttf_parser::LineMetrics) -> DesignLineMetrics {
    DesignLineMetrics {
        position: i32::from(line.position),
        thickness: i32::from(line.thickness),
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DesignMetrics {
    /// Validated by head parser to be in 16..=16384.
    pub units_per_em: u16,
    pub source: DesignMetricSource,
    pub ascent: i32,
    /// Negated signed table descender, widened before negation.
    pub descent: i32,
    pub line_gap: i32,
    /// Raw field presence/value. Zero is not certified as usable measured x-height.
    pub x_height_field: Option<i32>,
    pub subscript_lowering: Option<i32>,
    pub superscript_raise: Option<i32>,
    pub underline: Option<DesignUnderlineMetrics>,
    pub strikeout: Option<DesignLineMetrics>,
    /// fvar or MVAR records present; contents/variation effects are not validated.
    pub variation_tables_present: bool,
}
#[derive(Debug)]
pub enum FontMetricError {
    Bytes,
    Tables,
    Work,
    Directory(FaceParsingError),
    DirectoryOrder,
    TableRange([u8; 4]),
    TableAlignment([u8; 4]),
    DuplicateTable([u8; 4]),
    MissingHead,
    InvalidHead,
    InvalidOs2,
    MissingHhea,
    InvalidHhea,
    InvalidPost,
}
impl std::fmt::Display for FontMetricError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SFNT design metrics: {self:?}")
    }
}
impl std::error::Error for FontMetricError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Directory(error) => Some(error),
            _ => None,
        }
    }
}
fn assign<'a>(
    slot: &mut Option<&'a [u8]>,
    data: &'a [u8],
    tag: [u8; 4],
) -> Result<(), FontMetricError> {
    if slot.is_some() {
        return Err(FontMetricError::DuplicateTable(tag));
    }
    *slot = Some(data);
    Ok(())
}
/// Read the explicitly selected SFNT face, preserving raw design-unit values.
/// CSS's recommended OS/2 metrics take precedence without requiring fsSelection's
/// optional USE_TYPO_METRICS flag. Only absent OS/2 falls back to hhea.
///
/// # Errors
/// Rejects limits, malformed directory/ranges, ambiguous required tables, absent
/// required metrics, or malformed used metric tables. Unrelated table contents,
/// checksums, glyphs and variation adjustments remain a separate admission stage.
pub fn read_sfnt_design_metrics(
    data: &[u8],
    face_index: u32,
    limits: FontMetricLimits,
) -> Result<DesignMetrics, FontMetricError> {
    if data.len() > limits.bytes.get() {
        return Err(FontMetricError::Bytes);
    }
    let remaining = limits
        .work
        .get()
        .checked_sub(5)
        .ok_or(FontMetricError::Work)?;
    let face = RawFace::parse(data, face_index).map_err(FontMetricError::Directory)?;
    let count = usize::from(face.table_records.len());
    if count > limits.tables.get() {
        return Err(FontMetricError::Tables);
    }
    if count > remaining {
        return Err(FontMetricError::Work);
    }
    let (mut head, mut hhea, mut os2, mut post) = (None, None, None, None);
    let mut last = None;
    let mut variation_tables_present = false;
    for record in face.table_records {
        if last.is_some_and(|old| old > record.tag) {
            return Err(FontMetricError::DirectoryOrder);
        }
        last = Some(record.tag);
        let tag = record.tag.0.to_be_bytes();
        let start = usize::try_from(record.offset).map_err(|_| FontMetricError::TableRange(tag))?;
        let length =
            usize::try_from(record.length).map_err(|_| FontMetricError::TableRange(tag))?;
        let end = start
            .checked_add(length)
            .ok_or(FontMetricError::TableRange(tag))?;
        let table = data
            .get(start..end)
            .ok_or(FontMetricError::TableRange(tag))?;
        if !start.is_multiple_of(4) {
            return Err(FontMetricError::TableAlignment(tag));
        }
        match &tag {
            b"head" => assign(&mut head, table, tag)?,
            b"hhea" => assign(&mut hhea, table, tag)?,
            b"OS/2" => assign(&mut os2, table, tag)?,
            b"post" => assign(&mut post, table, tag)?,
            b"fvar" | b"MVAR" => variation_tables_present = true,
            _ => {}
        }
    }
    let head = head.ok_or(FontMetricError::MissingHead)?;
    if !head.starts_with(&[0, 1, 0, 0]) || head.get(12..16) != Some(&[0x5f, 0x0f, 0x3c, 0xf5][..]) {
        return Err(FontMetricError::InvalidHead);
    }
    let head = ttf_parser::head::Table::parse(head).ok_or(FontMetricError::InvalidHead)?;
    let underline = post
        .map(|data| {
            let table = ttf_parser::post::Table::parse(data).ok_or(FontMetricError::InvalidPost)?;
            let format = u32::from_be_bytes(
                data.get(..4)
                    .ok_or(FontMetricError::InvalidPost)?
                    .try_into()
                    .map_err(|_| FontMetricError::InvalidPost)?,
            );
            Ok::<_, FontMetricError>(DesignUnderlineMetrics {
                format,
                line: design_line(table.underline_metrics),
            })
        })
        .transpose()?;
    if let Some(os2) = os2 {
        let os2 = ttf_parser::os2::Table::parse(os2).ok_or(FontMetricError::InvalidOs2)?;
        Ok(DesignMetrics {
            units_per_em: head.units_per_em,
            source: DesignMetricSource::Os2,
            ascent: i32::from(os2.typographic_ascender()),
            descent: -i32::from(os2.typographic_descender()),
            line_gap: i32::from(os2.typographic_line_gap()),
            x_height_field: os2.x_height().map(i32::from),
            subscript_lowering: Some(i32::from(os2.subscript_metrics().y_offset)),
            superscript_raise: Some(i32::from(os2.superscript_metrics().y_offset)),
            underline,
            strikeout: Some(design_line(os2.strikeout_metrics())),
            variation_tables_present,
        })
    } else {
        let hhea = hhea.ok_or(FontMetricError::MissingHhea)?;
        if !hhea.starts_with(&[0, 1, 0, 0]) {
            return Err(FontMetricError::InvalidHhea);
        }
        let hhea = ttf_parser::hhea::Table::parse(hhea).ok_or(FontMetricError::InvalidHhea)?;
        Ok(DesignMetrics {
            units_per_em: head.units_per_em,
            source: DesignMetricSource::Hhea,
            ascent: i32::from(hhea.ascender),
            descent: -i32::from(hhea.descender),
            line_gap: i32::from(hhea.line_gap),
            x_height_field: None,
            subscript_lowering: None,
            superscript_raise: None,
            underline,
            strikeout: None,
            variation_tables_present,
        })
    }
}

use iced_css_core::values::{
    FiniteF32,
    font_metrics::{FontMetricScaleError, scale_font_units},
};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesignMetric {
    Ascent,
    Descent,
    LineGap,
    XHeight,
    Subscript,
    Superscript,
    UnderlinePosition,
    UnderlineThickness,
    StrikeoutPosition,
    StrikeoutThickness,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DesignMetricScaleError {
    pub metric: DesignMetric,
    pub error: FontMetricScaleError,
}
impl std::fmt::Display for DesignMetricScaleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "scaling {:?}: {}", self.metric, self.error)
    }
}
impl std::error::Error for DesignMetricScaleError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScaledLineMetrics {
    pub position: FiniteF32,
    pub thickness: FiniteF32,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScaledUnderlineMetrics {
    pub format: u32,
    pub line: ScaledLineMetrics,
}
/// Base design-table values scaled to CSS pixels. Variation adjustments, usable
/// x-height selection and the normal-line-height policy remain the font owner's.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScaledDesignMetrics {
    pub source: DesignMetricSource,
    pub ascent: FiniteF32,
    pub descent: FiniteF32,
    pub line_gap: FiniteF32,
    pub x_height_field: Option<FiniteF32>,
    pub subscript_lowering: Option<FiniteF32>,
    pub superscript_raise: Option<FiniteF32>,
    pub underline: Option<ScaledUnderlineMetrics>,
    pub strikeout: Option<ScaledLineMetrics>,
    pub variation_tables_present: bool,
}
impl DesignMetrics {
    /// Scale each metric exactly once, preserving missing fields and conversion
    /// flags. This does not fabricate fallback metrics or normalize negative data.
    ///
    /// # Errors
    /// Reports which field was being scaled and the numeric/context error. No
    /// partial scaled record is returned if a later metric cannot be represented.
    pub fn scale(self, font_size: f32) -> Result<ScaledDesignMetrics, DesignMetricScaleError> {
        let scale = |units, metric| {
            scale_font_units(units, self.units_per_em, font_size)
                .map_err(|error| DesignMetricScaleError { metric, error })
        };
        let line = |raw: DesignLineMetrics, position, thickness| {
            Ok::<_, DesignMetricScaleError>(ScaledLineMetrics {
                position: scale(raw.position, position)?,
                thickness: scale(raw.thickness, thickness)?,
            })
        };
        Ok(ScaledDesignMetrics {
            source: self.source,
            ascent: scale(self.ascent, DesignMetric::Ascent)?,
            descent: scale(self.descent, DesignMetric::Descent)?,
            line_gap: scale(self.line_gap, DesignMetric::LineGap)?,
            x_height_field: self
                .x_height_field
                .map(|v| scale(v, DesignMetric::XHeight))
                .transpose()?,
            subscript_lowering: self
                .subscript_lowering
                .map(|v| scale(v, DesignMetric::Subscript))
                .transpose()?,
            superscript_raise: self
                .superscript_raise
                .map(|v| scale(v, DesignMetric::Superscript))
                .transpose()?,
            underline: self
                .underline
                .map(|raw| {
                    Ok::<_, DesignMetricScaleError>(ScaledUnderlineMetrics {
                        format: raw.format,
                        line: line(
                            raw.line,
                            DesignMetric::UnderlinePosition,
                            DesignMetric::UnderlineThickness,
                        )?,
                    })
                })
                .transpose()?,
            strikeout: self
                .strikeout
                .map(|raw| {
                    line(
                        raw,
                        DesignMetric::StrikeoutPosition,
                        DesignMetric::StrikeoutThickness,
                    )
                })
                .transpose()?,
            variation_tables_present: self.variation_tables_present,
        })
    }
}
