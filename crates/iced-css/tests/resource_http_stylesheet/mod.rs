use super::*;

fn stylesheet_worker_limits() -> iced_css::resources::http_stylesheet::HttpStylesheetLimits {
    use iced_css::resources::{
        content_decoding::ContentDecodeLimits, http_stylesheet::HttpStylesheetLimits,
    };
    use iced_css_core::{
        resources::{content_codings::ContentCodingLimits, http_metadata::ContentTypeLimits},
        selectors::SelectorLimits,
        stylesheet::*,
        syntax::{
            RecoveryLimits, StringLimits, decode::DecodeLimits,
            encoding_selection::SelectionLimits, unicode_intake::IntakeLimits,
        },
        values::{NumericLimits, color::ColorLimits},
    };
    let a = nz(100000);
    HttpStylesheetLimits {
        header_fields: nz(32),
        transport_charset_bytes: nz(1024),
        header_reference_bytes: nz(1024),
        metadata_bytes: nz(1024),
        content_type: ContentTypeLimits {
            parameters: nz(10),
            retained_bytes: nz(4096),
            work: a,
        },
        codings: ContentCodingLimits {
            fields: nz(10),
            bytes: nz(4096),
            members: nz(10),
            record_bytes: nz(4096),
            work: a,
        },
        content: ContentDecodeLimits {
            input_bytes: nz(4096),
            output: BodyLimits {
                retained_bytes: nz(4096),
                work: a,
            },
            calls: nz(100),
            stages: nz(10),
            members: nz(10),
        },
        intake: StylesheetIntakeLimits {
            intake: IntakeLimits {
                selection: SelectionLimits {
                    registry_work: a,
                    prefix_work: a,
                },
                decode: DecodeLimits {
                    entries: a,
                    retained_bytes: a,
                    work: a,
                },
                charset_work: a,
            },
            sheet: StylesheetLimits {
                statements: a,
                record_bytes: a,
                work: a,
                rules: RuleAssemblyLimits {
                    recovery: RecoveryLimits {
                        work: a,
                        retained_bytes: a,
                        nesting: a,
                    },
                    selectors: SelectorLimits {
                        work: a,
                        retained_bytes: a,
                        entries: a,
                    },
                    values: ColorLimits {
                        syntax: StringLimits {
                            work: a,
                            retained_bytes: a,
                        },
                        numeric: NumericLimits { work: a, digits: a },
                    },
                    entries: a,
                    record_bytes: a,
                    work: a,
                    name_bytes: a,
                },
            },
        },
    }
}

mod acquisition;
mod admission;
mod representation;
mod worker;
