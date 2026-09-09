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
    stylesheet::{imports::*, owned_graph::*},
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

use iced_css_core::stylesheet::{collector::*, located_graph::*, requests::ImportRequestLimits};
fn cycle(value: u64) -> ImportCycleId {
    ImportCycleId::new(NonZeroU64::new(value).unwrap())
}
fn collector_limits() -> CollectorLimits {
    CollectorLimits {
        entries: n(),
        record_bytes: n(),
        work: n(),
        requests: ImportRequestLimits {
            entries: n(),
            retained_bytes: n(),
            work: n(),
        },
        tracking: graph_limits().expansion.tracking,
        cycle_bytes: n(),
    }
}
fn root() -> CollectorRoot {
    CollectorRoot {
        target: ImportTarget {
            source: id(1),
            cycle: cycle(100),
        },
        input: AcquiredStylesheet {
            candidate: resource(
                1,
                "@import 'a.css'; @import 'ignored.css' print; @import 'c.css'; r{color:red}",
                vec![],
            )
            .candidate,
            effective_base: "https://assets.invalid/base/root.css".into(),
        },
    }
}
#[derive(Default)]
struct Provider {
    identify_error: bool,
    acquire_error: bool,
    conflict: bool,
    identified: Vec<(u64, usize)>,
    acquired: Vec<u64>,
}
fn assert_referring_base(probe: &ImportProbe<'_>) {
    let expected = match probe.parent.get() {
        1 => "https://assets.invalid/base/root.css",
        2 => "https://assets.invalid/redirect/a.css",
        3 => "https://assets.invalid/base/c.css",
        other => panic!("unexpected test parent {other}"),
    };
    assert_eq!(probe.effective_base, expected);
}
impl ImportProvider for Provider {
    type Error = &'static str;
    fn identify(&mut self, probe: ImportProbe<'_>) -> Result<ImportTarget, Self::Error> {
        assert_referring_base(&probe);
        assert_eq!(probe.encoding.referring, Some(b"UTF-8".as_slice()));
        assert_eq!(probe.encoding.transport, None);
        assert_eq!(probe.encoding.linking, None);
        self.identified
            .push((probe.parent.get(), probe.request.entry));
        if self.identify_error {
            return Err("identify failed");
        }
        let value = match probe.request.acquisition_uri() {
            "https://assets.invalid/base/a.css" => 2,
            "https://assets.invalid/base/c.css" => 3,
            _ => return Err("unexpected acquisition URI"),
        };
        Ok(ImportTarget {
            source: id(if self.conflict && value == 3 {
                2
            } else {
                value
            }),
            cycle: cycle(value * 100),
        })
    }
    fn acquire(
        &mut self,
        target: ImportTarget,
        probe: ImportProbe<'_>,
    ) -> Result<Option<AcquiredStylesheet>, Self::Error> {
        assert_referring_base(&probe);
        self.acquired.push(target.source.get());
        if self.acquire_error {
            return Err("acquire failed");
        }
        let (css, base) = match target.source.get() {
            2 => (
                "@import '../base/c.css'; a{width:1px}",
                "https://assets.invalid/redirect/a.css",
            ),
            3 => (
                "@import 'a.css'; c{height:2px}",
                "https://assets.invalid/base/c.css",
            ),
            _ => return Err("unexpected source"),
        };
        Ok(Some(AcquiredStylesheet {
            candidate: resource(target.source.get(), css, vec![]).candidate,
            effective_base: base.into(),
        }))
    }
}
#[test]
fn collector_acquires_once_and_revisits_contexts_in_depth_first_order() {
    let mut provider = Provider::default();
    let result = collect_imports(root(), &mut provider, collector_limits()).unwrap();
    assert_eq!(provider.acquired, [2, 3]);
    assert_eq!(provider.identified, [(1, 0), (2, 0), (3, 0), (1, 2)]);
    assert_eq!(result.resources.len(), 3);
    let graph = OwnedStylesheetGraph::new_with_cycles(
        result.resources,
        result.cycles.as_slice(),
        0,
        context(),
        graph_limits(),
    )
    .unwrap();
    let graph = LocatedStylesheetGraph::new(
        graph,
        result.bases,
        BaseLimits {
            entries: n(),
            retained_bytes: n(),
            work: n(),
        },
    )
    .unwrap();
    assert_eq!(
        graph
            .graph()
            .rules()
            .map(|rule| rule.source.get())
            .collect::<Vec<_>>(),
        [3, 2, 2, 3, 1]
    );
    assert_eq!(graph.graph().occurrences(), 5);
    assert_eq!(
        graph
            .graph()
            .cuts()
            .iter()
            .filter(|cut| cut.reason == ImportCutReason::Cycle)
            .count(),
        2
    );
    assert_eq!(
        graph.effective_base(id(2), n()).unwrap(),
        Some("https://assets.invalid/redirect/a.css")
    );
}

#[test]
fn collector_rejects_provider_errors_identity_conflicts_and_early_budget_failures() {
    let mut provider = Provider {
        identify_error: true,
        ..Default::default()
    };
    assert!(
        matches!(collect_imports(root(),&mut provider,collector_limits()),Err(CollectionError::Provider{source,entry:0,error:"identify failed"}) if source==id(1))
    );
    assert!(provider.acquired.is_empty());
    let mut provider = Provider {
        acquire_error: true,
        ..Default::default()
    };
    assert!(
        matches!(collect_imports(root(),&mut provider,collector_limits()),Err(CollectionError::Provider{source,entry:0,error:"acquire failed"}) if source==id(1))
    );
    let mut provider = Provider {
        conflict: true,
        ..Default::default()
    };
    assert!(
        matches!(collect_imports(root(),&mut provider,collector_limits()),Err(CollectionError::Build(CollectorBuildError::CycleIdentity{source})) if source==id(2))
    );
    assert_eq!(provider.acquired, [2]);
    for kind in 0..4 {
        let mut limits = collector_limits();
        match kind {
            0 => limits.entries = NonZeroUsize::MIN,
            1 => limits.record_bytes = NonZeroUsize::MIN,
            2 => limits.work = NonZeroUsize::MIN,
            _ => limits.cycle_bytes = NonZeroUsize::MIN,
        }
        let mut provider = Provider::default();
        assert!(matches!(
            collect_imports(root(), &mut provider, limits),
            Err(CollectionError::Build(CollectorBuildError::Budget(_)))
        ));
        assert!(
            provider.identified.is_empty(),
            "budget kind {kind} called the provider"
        );
    }
    let mut exact = collector_limits();
    exact.cycle_bytes = NonZeroUsize::new(3 * size_of::<ImportCycleId>()).unwrap();
    assert!(collect_imports(root(), &mut Provider::default(), exact).is_ok());
    let mut input = root();
    input.input.effective_base = "relative".into();
    let mut provider = Provider::default();
    assert!(
        matches!(collect_imports(input,&mut provider,collector_limits()),Err(CollectionError::Build(CollectorBuildError::Requests{source,..})) if source==id(1))
    );
    assert!(provider.identified.is_empty());
}

#[derive(Default)]
struct ContextProvider {
    identified: Vec<(u64, usize)>,
    acquired: Vec<u64>,
}
impl ImportProvider for ContextProvider {
    type Error = &'static str;
    fn identify(&mut self, probe: ImportProbe<'_>) -> Result<ImportTarget, Self::Error> {
        self.identified
            .push((probe.parent.get(), probe.request.entry));
        match probe.request.acquisition_uri() {
            "https://assets.invalid/base/a.css" => Ok(ImportTarget {
                source: id(
                    if probe.encoding.referring == Some(b"UTF-16LE".as_slice()) {
                        4
                    } else {
                        2
                    },
                ),
                cycle: cycle(200),
            }),
            "https://assets.invalid/base/c.css" => Ok(ImportTarget {
                source: id(3),
                cycle: cycle(300),
            }),
            _ => Err("unexpected URI"),
        }
    }
    fn acquire(
        &mut self,
        target: ImportTarget,
        probe: ImportProbe<'_>,
    ) -> Result<Option<AcquiredStylesheet>, Self::Error> {
        self.acquired.push(target.source.get());
        if target.source == id(4) {
            assert_eq!(probe.encoding.referring, Some(b"UTF-16LE".as_slice()));
            assert!(
                self.identified.contains(&(1, 1)),
                "acquired while the first a URI was active"
            );
            return Ok(None);
        }
        let (bytes, encoding, base) = match target.source.get() {
            2 => (
                b"@import '../base/c.css'; a{width:1px}".to_vec(),
                probe.encoding,
                "https://assets.invalid/redirect/a.css",
            ),
            3 => (
                "@import 'a.css'; c{height:2px}"
                    .encode_utf16()
                    .flat_map(u16::to_le_bytes)
                    .collect(),
                EncodingMetadata {
                    transport: Some(b"UTF-16LE"),
                    ..probe.encoding
                },
                "https://assets.invalid/base/c.css",
            ),
            _ => return Err("unexpected source"),
        };
        let candidate =
            parse_separate_unicode_stylesheet(&bytes, encoding, context(), intake_limits())
                .unwrap();
        Ok(Some(AcquiredStylesheet {
            candidate,
            effective_base: base.into(),
        }))
    }
}
#[test]
fn collector_defers_a_distinct_charset_context_then_remembers_unavailability() {
    let mut input = root();
    input.input.candidate = resource(
        1,
        "@import 'a.css'; @import 'c.css'; @import 'c.css'; r{color:red}",
        vec![],
    )
    .candidate;
    let mut provider = ContextProvider::default();
    let result = collect_imports(input, &mut provider, collector_limits()).unwrap();
    assert_eq!(provider.acquired, [2, 3, 4]);
    assert_eq!(result.resources.len(), 3);
    assert_eq!(
        result.resources[2].candidate.source().referring_label(),
        b"UTF-16LE"
    );
    let graph = OwnedStylesheetGraph::new_with_cycles(
        result.resources,
        result.cycles.as_slice(),
        0,
        context(),
        graph_limits(),
    )
    .unwrap();
    assert_eq!(
        graph
            .rules()
            .map(|rule| rule.source.get())
            .collect::<Vec<_>>(),
        [3, 2, 3, 3, 1]
    );
    assert_eq!(
        graph
            .cuts()
            .iter()
            .map(|cut| cut.reason)
            .collect::<Vec<_>>(),
        [
            ImportCutReason::Cycle,
            ImportCutReason::Unavailable,
            ImportCutReason::Unavailable
        ]
    );
}

#[test]
fn an_empty_root_needs_no_provider_calls_and_remains_an_assembled_stylesheet() {
    let mut input = root();
    input.input.candidate = resource(1, "", vec![]).candidate;
    let mut provider = Provider {
        identify_error: true,
        acquire_error: true,
        ..Default::default()
    };
    let mut limits = collector_limits();
    limits.entries = NonZeroUsize::MIN;
    limits.cycle_bytes = NonZeroUsize::new(size_of::<ImportCycleId>()).unwrap();
    let result = collect_imports(input, &mut provider, limits).unwrap();
    assert!(provider.identified.is_empty());
    assert!(provider.acquired.is_empty());
    let mut expansion = graph_limits();
    expansion.expansion.unavailable = UnavailableImportPolicy::Fail;
    let graph = OwnedStylesheetGraph::new_with_cycles(
        result.resources,
        result.cycles.as_slice(),
        0,
        context(),
        expansion,
    )
    .unwrap();
    assert_eq!(graph.root(), id(1));
    assert_eq!(graph.occurrences(), 1);
    assert_eq!(graph.rules().count(), 0);
}
