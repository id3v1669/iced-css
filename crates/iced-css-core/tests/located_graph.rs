use iced_css_core::{
    selectors::SelectorLimits,
    stylesheet::*,
    syntax::{RecoveryLimits, StringLimits},
    values::{NumericLimits, color::ColorLimits},
};
use std::num::{NonZeroU64, NonZeroUsize};
fn n() -> NonZeroUsize {
    NonZeroUsize::new(100000).unwrap()
}
fn limits() -> RuleAssemblyLimits {
    RuleAssemblyLimits {
        recovery: RecoveryLimits {
            work: n(),
            retained_bytes: n(),
            nesting: n(),
        },
        selectors: SelectorLimits {
            work: n(),
            retained_bytes: n(),
            entries: n(),
        },
        values: ColorLimits {
            syntax: StringLimits {
                work: n(),
                retained_bytes: n(),
            },
            numeric: NumericLimits {
                work: n(),
                digits: n(),
            },
        },
        entries: n(),
        record_bytes: n(),
        work: n(),
        name_bytes: n(),
    }
}
fn context() -> RuleContext {
    RuleContext {
        first_order: 10,
        first_declaration: NonZeroU64::new(20).unwrap(),
    }
}

fn sheet_limits() -> StylesheetLimits {
    StylesheetLimits {
        rules: limits(),
        statements: n(),
        record_bytes: n(),
        work: n(),
    }
}

use iced_css_core::syntax::{
    decode::DecodeLimits, encoding_selection::SelectionLimits, unicode_intake::IntakeLimits,
};
fn intake_limits() -> StylesheetIntakeLimits {
    StylesheetIntakeLimits {
        intake: IntakeLimits {
            selection: SelectionLimits {
                registry_work: n(),
                prefix_work: n(),
            },
            decode: DecodeLimits {
                entries: n(),
                retained_bytes: n(),
                work: n(),
            },
            charset_work: n(),
        },
        sheet: sheet_limits(),
    }
}

use iced_css_core::{
    resources::publication::PublicationLimits,
    stylesheet::{imports::*, owned_graph::*, publication::*},
    syntax::{
        encoding_selection::EncodingMetadata,
        imports::{ImportLimits, ResourceId},
    },
};
fn id(value: u64) -> ResourceId {
    ResourceId::new(NonZeroU64::new(value).unwrap())
}
fn graph_limits() -> OwnedGraphLimits {
    OwnedGraphLimits {
        record_bytes: n(),
        work: n(),
        expansion: LoadedGraphLimits {
            unavailable: UnavailableImportPolicy::Record,
            entries: n(),
            record_bytes: n(),
            work: n(),
            tracking: ImportLimits {
                entries: n(),
                retained_bytes: n(),
                work: n(),
                occurrences: NonZeroU64::new(100).unwrap(),
            },
        },
    }
}
fn resource(value: u64, css: &str, imports: Vec<ImportBinding>) -> OwnedLoadedSheet {
    OwnedLoadedSheet {
        resource: id(value),
        candidate: parse_separate_unicode_stylesheet(
            css.as_bytes(),
            EncodingMetadata {
                transport: None,
                linking: None,
                referring: None,
            },
            context(),
            intake_limits(),
        )
        .unwrap(),
        imports,
    }
}
fn link(entry: usize, sheet: usize) -> ImportBinding {
    ImportBinding {
        entry,
        resolution: ImportResolution::Sheet(sheet),
    }
}
use iced_css_core::{
    resources::uri::UriError,
    stylesheet::{located_graph::*, requests::ImportRequestLimits},
};
fn meta_limits() -> BaseLimits {
    BaseLimits {
        entries: n(),
        retained_bytes: n(),
        work: n(),
    }
}
fn graph() -> OwnedStylesheetGraph {
    OwnedStylesheetGraph::new(
        vec![
            resource(
                1,
                "@import \"../child.css#part\"; @import \"ignored\" print; r{color:red}",
                vec![link(0, 1)],
            ),
            resource(2, "a{margin:1px}", vec![]),
        ],
        0,
        context(),
        graph_limits(),
    )
    .unwrap()
}
fn bases() -> Vec<StylesheetBase> {
    vec![
        StylesheetBase {
            resource: id(2),
            effective_base: "https://cdn.example/assets/child.css?v=2".into(),
        },
        StylesheetBase {
            resource: id(1),
            effective_base: "https://cdn.example/assets/themes/root.css?v=1".into(),
        },
    ]
}
#[test]
fn effective_bases_follow_resources_and_import_resolution_after_graph_move() {
    let graph = LocatedStylesheetGraph::new(graph(), bases(), meta_limits()).unwrap();
    assert_eq!(
        graph.effective_base(id(2), n()).unwrap(),
        Some("https://cdn.example/assets/child.css?v=2")
    );
    let requests = graph
        .resolve_imports(
            id(1),
            n(),
            ImportRequestLimits {
                entries: n(),
                retained_bytes: n(),
                work: n(),
            },
        )
        .unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].reference_uri(),
        "https://cdn.example/assets/child.css#part"
    );
    assert_eq!(
        requests[0].acquisition_uri(),
        "https://cdn.example/assets/child.css"
    );
    assert!(matches!(
        graph.effective_base(id(1), NonZeroUsize::MIN),
        Err(LocatedGraphError::Budget(RuleAssemblyError::Work))
    ));
    assert!(matches!(
        graph.resolve_imports(
            id(1),
            n(),
            ImportRequestLimits {
                entries: n(),
                retained_bytes: n(),
                work: NonZeroUsize::MIN
            }
        ),
        Err(LocatedGraphError::Imports(_))
    ));
    let declaration = graph
        .find_declaration(NonZeroU64::new(20).unwrap(), n(), n())
        .unwrap()
        .unwrap();
    assert_eq!(
        declaration.declaration.parsed.property.descriptor().name,
        "margin"
    );
    assert_eq!(
        declaration.effective_base,
        "https://cdn.example/assets/child.css?v=2"
    );
    let moved = std::thread::spawn(move || graph).join().unwrap();
    assert_eq!(moved.graph().rules().count(), 2);
    assert!(
        moved
            .find_declaration(NonZeroU64::new(30).unwrap(), n(), n())
            .unwrap()
            .is_none()
    );
    assert!(moved.effective_base(id(99), n()).unwrap().is_none());
    assert!(matches!(
        moved.resolve_imports(
            id(99),
            n(),
            ImportRequestLimits {
                entries: n(),
                retained_bytes: n(),
                work: n()
            }
        ),
        Err(LocatedGraphError::UnknownResource(_))
    ));
}
#[test]
fn invalid_or_unbounded_metadata_never_forms_a_located_candidate() {
    assert!(matches!(
        LocatedStylesheetGraph::new(graph(), vec![], meta_limits()),
        Err(LocatedGraphError::MissingBase)
    ));
    let mut input = bases();
    input[1].resource = id(2);
    assert!(matches!(
        LocatedStylesheetGraph::new(graph(), input, meta_limits()),
        Err(LocatedGraphError::DuplicateResource(_))
    ));
    let mut input = bases();
    input[0].resource = id(99);
    assert!(matches!(
        LocatedStylesheetGraph::new(graph(), input, meta_limits()),
        Err(LocatedGraphError::UnknownResource(_))
    ));
    for uri in [
        "relative.css",
        "https://example.test/theme.css#fragment",
        "https://example.test/%XX",
    ] {
        let mut input = bases();
        input[0].effective_base = uri.into();
        assert!(LocatedStylesheetGraph::new(graph(), input, meta_limits()).is_err());
    }
    let mut oversized = bases();
    let mut uri = String::with_capacity(n().get() + 1);
    uri.push_str("https://example.test/theme.css");
    oversized[0].effective_base = uri;
    assert!(matches!(
        LocatedStylesheetGraph::new(graph(), oversized, meta_limits()),
        Err(LocatedGraphError::Budget(RuleAssemblyError::Bytes))
    ));
    let mut tiny = meta_limits();
    tiny.retained_bytes = NonZeroUsize::MIN;
    assert!(matches!(
        LocatedStylesheetGraph::new(graph(), bases(), tiny),
        Err(LocatedGraphError::Budget(RuleAssemblyError::Bytes))
    ));
    let mut tiny = meta_limits();
    tiny.work = NonZeroUsize::MIN;
    assert!(LocatedStylesheetGraph::new(graph(), bases(), tiny).is_err());
    let mut tiny = meta_limits();
    tiny.entries = NonZeroUsize::MIN;
    assert!(matches!(
        LocatedStylesheetGraph::new(graph(), bases(), tiny),
        Err(LocatedGraphError::Budget(RuleAssemblyError::Entries))
    ));
}
#[test]
fn failed_replacement_retains_graph_and_effective_bases_together() {
    let mut owner = CandidatePublication::<LocatedStylesheetGraph>::new(PublicationLimits {
        requests: NonZeroU64::new(10).unwrap(),
        revisions: NonZeroU64::new(10).unwrap(),
    });
    let first = owner.request().unwrap();
    let candidate = LocatedStylesheetGraph::new(graph(), bases(), meta_limits());
    assert!(matches!(
        owner.finish(first, candidate).unwrap(),
        SheetCompletion::Published(_)
    ));
    let revision = owner.active_revision();
    let next = owner.request().unwrap();
    let mut wrong = bases();
    wrong[0].effective_base = "relative.css".into();
    assert!(matches!(
        owner
            .finish(
                next,
                LocatedStylesheetGraph::new(graph(), wrong, meta_limits())
            )
            .unwrap(),
        SheetCompletion::Failed(LocatedGraphError::Uri {
            error: UriError::RelativeBase,
            ..
        })
    ));
    assert_eq!(owner.active_revision(), revision);
    assert_eq!(
        owner.active().unwrap().effective_base(id(2), n()).unwrap(),
        Some("https://cdn.example/assets/child.css?v=2")
    );
}

#[test]
fn nested_import_requests_use_the_referring_resources_own_charset_and_base() {
    let child = "@import 'nested.css';";
    let bytes: Vec<u8> = child.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let child = OwnedLoadedSheet {
        resource: id(2),
        candidate: parse_separate_unicode_stylesheet(
            &bytes,
            EncodingMetadata {
                transport: Some(b"UTF-16LE"),
                ..Default::default()
            },
            context(),
            intake_limits(),
        )
        .unwrap(),
        imports: vec![link(0, 2)],
    };
    let owned = OwnedStylesheetGraph::new(
        vec![
            resource(1, "@import 'child.css';", vec![link(0, 1)]),
            child,
            resource(3, "a{color:red}", vec![]),
        ],
        0,
        context(),
        graph_limits(),
    )
    .unwrap();
    let mut metadata = bases();
    metadata.push(StylesheetBase {
        resource: id(3),
        effective_base: "https://cdn.example/assets/nested.css".into(),
    });
    let graph = LocatedStylesheetGraph::new(owned, metadata, meta_limits()).unwrap();
    let limits = ImportRequestLimits {
        entries: n(),
        retained_bytes: n(),
        work: n(),
    };
    let requests = graph.resolve_unicode_imports(id(2), n(), limits).unwrap();
    assert_eq!(
        requests.requests()[0].acquisition_uri(),
        "https://cdn.example/assets/nested.css"
    );
    assert_eq!(
        requests.encoding_metadata(None, None).referring,
        Some(b"UTF-16LE".as_slice())
    );
    assert_eq!(
        graph
            .resolve_unicode_imports(id(1), n(), limits)
            .unwrap()
            .encoding_metadata(None, None)
            .referring,
        Some(b"UTF-8".as_slice())
    );
    assert!(matches!(
        graph.resolve_unicode_imports(id(2), NonZeroUsize::MIN, limits),
        Err(LocatedGraphError::Budget(RuleAssemblyError::Work))
    ));
    assert!(
        matches!(graph.resolve_unicode_imports(id(99),n(),limits),Err(LocatedGraphError::UnknownResource(value)) if value==id(99))
    );
}
