use super::*;
use iced::futures::executor::block_on;
use iced_css::resources::http_stylesheet::HttpStylesheetSubmitError;
use iced_css::{
    host::stylesheet::graph::RuntimeLocatedStylesheetGraph,
    resources::{http_graph::*, worker::FileReader},
};
use iced_css_core::stylesheet::identities::*;
use std::sync::{Arc, Mutex};
#[path = "../fixtures_support/http_graph.rs"]
mod fixture;
use fixture::{Server, nz, transport};
fn settings() -> HttpGraphLimits {
    fixture::settings(
        intake_limits(),
        graph_limits().expansion.tracking,
        meta_limits(),
    )
}
fn identities() -> Arc<Mutex<ImportIdentityRegistry>> {
    Arc::new(Mutex::new(
        ImportIdentityRegistry::new(IdentityLimits {
            entries: n(),
            bytes: n(),
            work: n(),
        })
        .unwrap(),
    ))
}
fn new_owner() -> RuntimeLocatedStylesheetGraph {
    RuntimeLocatedStylesheetGraph::new(
        PublicationLimits {
            requests: NonZeroU64::new(100).unwrap(),
            revisions: NonZeroU64::new(100).unwrap(),
        },
        n(),
    )
}
#[test]
fn http_import_graph_resolves_redirect_cycles_and_preserves_atomic_replacements() {
    let server = Server::new();
    let transport = transport();
    let reader = FileReader::new(nz(1));
    let ids = identities();
    let mut owner = new_owner();
    let request = owner.request(0, context(), graph_limits()).unwrap();
    let work = reader.load_http_stylesheet_graph(
        request,
        transport.clone(),
        ids.clone(),
        context(),
        server.source("/start#part"),
        settings(),
    );
    fn owned_send<T: Send + 'static>(_: &T) {}
    owned_send(&work);
    let (completion, report) = block_on(work);
    let report = report.unwrap();
    assert!(matches!(
        owner.finish(completion).unwrap(),
        SheetCompletion::Published(_)
    ));
    assert_eq!(report.entries().len(), 3);
    assert_eq!(report.entries()[2].canonical_entry(), 1);
    assert_eq!(
        report.entries()[0]
            .representation()
            .unwrap()
            .transport_charset(),
        Some(b"UTF-16LE".as_slice())
    );
    assert_eq!(server.count("/start"), 1);
    assert_eq!(server.count("/nested/root.css"), 1);
    assert_eq!(server.count("/nested/child.css"), 2);
    assert_eq!(server.count("/alias.css"), 1);
    let active = owner.active().unwrap();
    assert_eq!(active.graph().resources().len(), 2);
    assert_eq!(active.graph().rules().count(), 3);
    let root = active.graph().resources()[0].resource;
    assert_eq!(
        active.effective_base(root, n()).unwrap(),
        Some(format!("{}/nested/root.css", server.base).as_str())
    );
    assert!(
        active
            .graph()
            .cuts()
            .iter()
            .all(|cut| cut.reason == ImportCutReason::Cycle)
    );
    let revision = owner.active_revision();
    {
        let mut state = server.state.lock().unwrap();
        state.missing = true;
        state.blue = true;
    }
    let request = owner.request(0, context(), graph_limits()).unwrap();
    let (completion, report) = block_on(reader.load_http_stylesheet_graph(
        request,
        transport.clone(),
        ids.clone(),
        context(),
        server.source("/start"),
        settings(),
    ));
    let report = report.unwrap();
    assert!(report.entries().iter().any(|entry| entry.error().is_some()));
    let expected_new_root: Vec<u8> = "@import 'child.css'; @import '../alias.css'; r{color:blue}"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    assert_eq!(
        report.entries()[0]
            .representation()
            .unwrap()
            .content_decoded_body()
            .as_slice(),
        expected_new_root
    );
    assert!(report.charged_bytes() <= settings().attempts.retained_bytes.get());
    assert!(matches!(
        owner.finish(completion).unwrap(),
        SheetCompletion::Failed(_)
    ));
    assert_eq!(owner.active_revision(), revision);
    assert_eq!(
        owner.active().unwrap().graph().resources()[0]
            .candidate
            .source()
            .text(),
        "@import 'child.css'; @import '../alias.css'; r{color:red}"
    );
    let mut first = new_owner();
    let request = first.request(0, context(), graph_limits()).unwrap();
    let (completion, _) = block_on(reader.load_http_stylesheet_graph(
        request,
        transport.clone(),
        identities(),
        context(),
        server.source("/start"),
        settings(),
    ));
    assert!(matches!(
        first.finish(completion).unwrap(),
        SheetCompletion::Published(_)
    ));
    assert_eq!(first.active().unwrap().graph().rules().count(), 1);
    server.state.lock().unwrap().empty = true;
    let request = owner.request(0, context(), graph_limits()).unwrap();
    let (completion, _) = block_on(reader.load_http_stylesheet_graph(
        request,
        transport,
        ids,
        context(),
        server.source("/start"),
        settings(),
    ));
    assert!(matches!(
        owner.finish(completion).unwrap(),
        SheetCompletion::Published(_)
    ));
    assert_eq!(owner.active().unwrap().graph().rules().count(), 0);
    assert_eq!(reader.in_flight(), 0);
}
#[test]
fn http_graph_shares_acquisition_across_referring_encodings() {
    let server = Server::new();
    let reader = FileReader::new(nz(1));
    let mut owner = new_owner();
    let request = owner.request(0, context(), graph_limits()).unwrap();
    let (completion, report) = block_on(reader.load_http_stylesheet_graph(
        request,
        transport(),
        identities(),
        context(),
        server.source("/contexts.css"),
        settings(),
    ));
    assert!(matches!(
        owner.finish(completion).unwrap(),
        SheetCompletion::Published(_)
    ));
    assert_eq!(report.unwrap().entries().len(), 4);
    assert_eq!(server.count("/shared.css"), 1);
    let active = owner.active().unwrap();
    let target = format!("{}/shared.css", server.base);
    let shared: Vec<_> = active
        .graph()
        .resources()
        .iter()
        .filter(|entry| {
            active.effective_base(entry.resource, n()).unwrap() == Some(target.as_str())
        })
        .map(|entry| entry.candidate.source().text())
        .collect();
    assert_eq!(shared, ["筲潣潬㩲敲絤⨯꧃⼪", "r{color:red}/*é*/"]);
}
#[test]
fn http_graph_rejects_inconsistent_final_content_and_preacquisition_limits() {
    let server = Server::new();
    server.state.lock().unwrap().inconsistent = true;
    let reader = FileReader::new(nz(1));
    let mut owner = new_owner();
    let transport = transport();
    let request = owner.request(0, context(), graph_limits()).unwrap();
    let (completion, report) = block_on(reader.load_http_stylesheet_graph(
        request,
        transport.clone(),
        identities(),
        context(),
        server.source("/start"),
        settings(),
    ));
    assert_eq!(report.unwrap().entries().len(), 3);
    assert!(matches!(
        acquisition_failure(&mut owner, completion),
        HttpGraphError::Imports(
            iced_css_core::stylesheet::collector::CollectionError::Provider {
                error: HttpImportError::Attempt(HttpAttemptError::ConflictingRepresentation),
                ..
            }
        )
    ));
    assert!(owner.active().is_none());
    let before = server.requests.lock().unwrap().len();
    let mut limited = settings();
    limited.attempts.retained_bytes = nz(1);
    let request = owner.request(0, context(), graph_limits()).unwrap();
    let (completion, report) = block_on(reader.load_http_stylesheet_graph(
        request,
        transport.clone(),
        identities(),
        context(),
        server.source("/start"),
        limited,
    ));
    assert_eq!(report.unwrap().entries().len(), 0);
    assert!(matches!(
        acquisition_failure(&mut owner, completion),
        HttpGraphError::Root(HttpImportError::Attempt(HttpAttemptError::Bytes))
    ));
    assert_eq!(server.requests.lock().unwrap().len(), before);
    let mut limited = settings();
    limited.attempts.entries = nz(1);
    let request = owner.request(0, context(), graph_limits()).unwrap();
    let (completion, report) = block_on(reader.load_http_stylesheet_graph(
        request,
        transport,
        identities(),
        context(),
        server.source("/start"),
        limited,
    ));
    assert_eq!(report.unwrap().entries().len(), 1);
    assert!(matches!(
        acquisition_failure(&mut owner, completion),
        HttpGraphError::Imports(
            iced_css_core::stylesheet::collector::CollectionError::Provider {
                error: HttpImportError::Attempt(HttpAttemptError::Entries),
                ..
            }
        )
    ));
    assert_eq!(server.requests.lock().unwrap().len(), before + 2);
}

fn acquisition_failure(
    owner: &mut RuntimeLocatedStylesheetGraph,
    completion: HttpGraphCompletion,
) -> HttpGraphError {
    use iced_css::{host::stylesheet::graph::GraphLoadError, resources::graph::GraphWorkerError};
    match owner.finish(completion).unwrap() {
        SheetCompletion::Failed(GraphLoadError::Acquisition(GraphWorkerError::Acquisition(
            error,
        ))) => error,
        other => panic!("unexpected completion: {other:?}"),
    }
}
#[test]
fn http_graph_submission_input_and_stale_boundaries_preserve_request_ownership() {
    use iced_css::{
        host::stylesheet::graph::GraphLoadError,
        resources::{
            graph::GraphWorkerError,
            worker::{BackgroundReadLimits, SubmitError},
        },
    };
    let server = Server::new();
    let transport = transport();
    let reader = FileReader::new(nz(1));
    let mut owner = new_owner();
    let held = reader
        .read_file(
            std::path::PathBuf::from(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/content_decoding/red.gz"
            )),
            BackgroundReadLimits {
                path_bytes: n(),
                read: settings().acquisition.http.read,
            },
        )
        .unwrap();
    let request = owner.request(0, context(), graph_limits()).unwrap();
    let (completion, report) = block_on(reader.load_http_stylesheet_graph(
        request,
        transport.clone(),
        identities(),
        context(),
        server.source("/start"),
        settings(),
    ));
    assert!(report.is_none());
    assert!(matches!(
        owner.finish(completion).unwrap(),
        SheetCompletion::Failed(GraphLoadError::Acquisition(GraphWorkerError::Submit(
            SubmitError::Limit
        )))
    ));
    assert!(server.requests.lock().unwrap().is_empty());
    block_on(held).unwrap();
    let mut source = server.source("/start");
    let mut label = Vec::with_capacity(2048);
    label.extend_from_slice(b"UTF-8");
    source.encoding.linking = Some(label);
    let request = owner.request(0, context(), graph_limits()).unwrap();
    let (completion, report) = block_on(reader.load_http_stylesheet_graph(
        request,
        transport.clone(),
        identities(),
        context(),
        source,
        settings(),
    ));
    assert!(report.is_none());
    assert!(matches!(
        acquisition_failure(&mut owner, completion),
        HttpGraphError::Input(HttpStylesheetSubmitError::Metadata)
    ));
    assert!(server.requests.lock().unwrap().is_empty());
    assert_eq!(reader.in_flight(), 0);
    let mut limited = settings();
    limited.attempts.work = nz(1);
    let request = owner.request(0, context(), graph_limits()).unwrap();
    let (completion, report) = block_on(reader.load_http_stylesheet_graph(
        request,
        transport.clone(),
        identities(),
        context(),
        server.source("/start"),
        limited,
    ));
    assert!(report.unwrap().entries().is_empty());
    assert!(matches!(
        acquisition_failure(&mut owner, completion),
        HttpGraphError::Root(HttpImportError::Attempt(HttpAttemptError::Work))
    ));
    assert!(server.requests.lock().unwrap().is_empty());
    let old = owner.request(0, context(), graph_limits()).unwrap();
    let old = reader.load_http_stylesheet_graph(
        old,
        transport.clone(),
        identities(),
        context(),
        server.source("/start"),
        settings(),
    );
    let newest = owner.request(0, context(), graph_limits()).unwrap();
    let mut limited = settings();
    limited.acquisition.http.uri.bytes = nz(1);
    let (completion, report) = block_on(reader.load_http_stylesheet_graph(
        newest,
        transport,
        identities(),
        context(),
        server.source("/start"),
        limited,
    ));
    assert!(report.is_none());
    assert!(matches!(
        acquisition_failure(&mut owner, completion),
        HttpGraphError::Input(HttpStylesheetSubmitError::UriBytes)
    ));
    let (completion, report) = block_on(old);
    assert_eq!(report.unwrap().entries().len(), 3);
    assert!(matches!(
        owner.finish(completion).unwrap(),
        SheetCompletion::Ignored
    ));
    assert!(owner.active().is_none());
    assert_eq!(reader.in_flight(), 0);
}

#[test]
fn http_runtime_rejects_passive_refresh_and_preserves_reports_on_failed_reload() {
    use iced_css::host::stylesheet::http_graph::*;
    use iced_css_core::resources::{publication::PublicationStatus, triggers::*};
    for mode in [RuntimeMode::Manual, RuntimeMode::Auto] {
        let server = Server::new();
        let mut runtime = RuntimeHttpStylesheetGraph::new(
            FileReader::new(nz(1)),
            transport(),
            mode,
            context(),
            PublicationLimits {
                requests: NonZeroU64::new(30).unwrap(),
                revisions: NonZeroU64::new(30).unwrap(),
            },
            n(),
            IdentityLimits {
                entries: n(),
                bytes: n(),
                work: n(),
            },
        )
        .unwrap();
        let limits = HttpRuntimeLimits {
            acquisition: settings(),
            graph: graph_limits(),
        };
        assert!(matches!(
            runtime.retry(RequestReason::ExplicitReload),
            Err(HttpGraphRetryError::MissingConfiguration)
        ));
        for reason in [
            RequestReason::LocalChange,
            RequestReason::EmbeddedNotification,
            RequestReason::Reevaluation,
            RequestReason::CacheExpiry,
            RequestReason::CacheEviction,
        ] {
            assert!(matches!(
                runtime.request(reason, server.source("/start"), limits),
                Err(RequestError::Unauthorized)
            ));
        }
        assert_eq!(runtime.status(), PublicationStatus::Unrequested);
        assert!(server.requests.lock().unwrap().is_empty());
        let pending = runtime
            .request(
                RequestReason::FirstAcquisition,
                server.source("/start"),
                limits,
            )
            .unwrap();
        let pointer = runtime.configuration().unwrap().source().uri.as_ptr();
        assert!(matches!(
            runtime.retry(RequestReason::LocalChange),
            Err(HttpGraphRetryError::Request(RequestError::Unauthorized))
        ));
        assert!(matches!(
            runtime.request(
                RequestReason::FirstAcquisition,
                server.source("/start"),
                limits
            ),
            Err(RequestError::Unauthorized)
        ));
        let completion = block_on(pending);
        assert!(runtime.is_current(&completion));
        assert_eq!(
            runtime
                .pending_dependencies(&completion)
                .unwrap()
                .entries()
                .len(),
            3
        );
        assert!(matches!(
            runtime.finish(completion).unwrap(),
            SheetCompletion::Published(_)
        ));
        let revision = runtime.active_revision();
        let old_body = runtime.active_dependencies().unwrap().entries()[0]
            .representation()
            .unwrap()
            .content_decoded_body()
            .as_slice()
            .to_vec();
        {
            let mut state = server.state.lock().unwrap();
            state.missing = true;
            state.blue = true;
        }
        let completion = block_on(runtime.retry(RequestReason::ExplicitReload).unwrap());
        assert_eq!(
            runtime.configuration().unwrap().source().uri.as_ptr(),
            pointer
        );
        assert!(matches!(
            runtime.finish(completion).unwrap(),
            SheetCompletion::Failed(_)
        ));
        assert_eq!(runtime.active_revision(), revision);
        assert_eq!(
            runtime.active_dependencies().unwrap().entries()[0]
                .representation()
                .unwrap()
                .content_decoded_body()
                .as_slice(),
            old_body
        );
        assert_ne!(
            runtime.candidate_dependencies().unwrap().entries()[0]
                .representation()
                .unwrap()
                .content_decoded_body()
                .as_slice(),
            old_body
        );
        let mut invalid = server.source("/start");
        invalid.uri.reserve(8192);
        let completion = block_on(
            runtime
                .request(RequestReason::ExplicitReload, invalid, limits)
                .unwrap(),
        );
        assert!(matches!(
            runtime.finish(completion).unwrap(),
            SheetCompletion::Failed(_)
        ));
        assert!(runtime.candidate_dependencies().is_none());
        assert_eq!(
            runtime.configuration().unwrap().source().uri.as_ptr(),
            pointer
        );
        assert_eq!(runtime.active_revision(), revision);
        server.state.lock().unwrap().empty = true;
        let completion = block_on(runtime.retry(RequestReason::ExplicitReload).unwrap());
        assert!(matches!(
            runtime.finish(completion).unwrap(),
            SheetCompletion::Published(_)
        ));
        assert_eq!(runtime.active().unwrap().graph().rules().count(), 0);
        assert!(runtime.candidate_dependencies().is_none());
        assert_eq!(runtime.active_dependencies().unwrap().entries().len(), 1);
    }
}

#[test]
fn http_runtime_stale_foreign_and_exhausted_requests_cannot_replace_configuration() {
    use iced_css::host::stylesheet::http_graph::*;
    use iced_css_core::resources::triggers::*;
    let server = Server::new();
    let transport = transport();
    let make = |requests| {
        RuntimeHttpStylesheetGraph::new(
            FileReader::new(nz(1)),
            Arc::clone(&transport),
            RuntimeMode::Auto,
            context(),
            PublicationLimits {
                requests: NonZeroU64::new(requests).unwrap(),
                revisions: NonZeroU64::new(30).unwrap(),
            },
            n(),
            IdentityLimits {
                entries: n(),
                bytes: n(),
                work: n(),
            },
        )
        .unwrap()
    };
    let limits = HttpRuntimeLimits {
        acquisition: settings(),
        graph: graph_limits(),
    };
    let mut owner = make(30);
    let mut foreign = make(1);
    let older = block_on(
        owner
            .request(
                RequestReason::FirstAcquisition,
                server.source("/start"),
                limits,
            )
            .unwrap(),
    );
    assert!(!foreign.is_current(&older));
    assert!(foreign.pending_dependencies(&older).is_none());
    let newer = block_on(
        owner
            .request(
                RequestReason::ExplicitReload,
                server.source("/missing.css"),
                limits,
            )
            .unwrap(),
    );
    let pointer = owner.configuration().unwrap().source().uri.as_ptr();
    assert!(!owner.is_current(&older));
    assert!(owner.pending_dependencies(&older).is_none());
    assert!(matches!(
        owner.finish(newer).unwrap(),
        SheetCompletion::Failed(_)
    ));
    assert!(matches!(
        owner.finish(older).unwrap(),
        SheetCompletion::Ignored
    ));
    assert!(owner.active().is_none());
    assert_eq!(
        owner.candidate_dependencies().unwrap().entries()[0].reference_uri(),
        server.source("/missing.css").uri
    );
    let retry = block_on(owner.retry(RequestReason::ExplicitReload).unwrap());
    assert_eq!(server.count("/missing.css"), 2);
    assert_eq!(
        owner.configuration().unwrap().source().uri.as_ptr(),
        pointer
    );
    assert!(matches!(
        foreign.finish(retry).unwrap(),
        SheetCompletion::Ignored
    ));
    assert!(foreign.active_dependencies().is_none());
    assert!(foreign.candidate_dependencies().is_none());
    let pending = foreign
        .request(
            RequestReason::FirstAcquisition,
            server.source("/start"),
            limits,
        )
        .unwrap();
    let pointer = foreign.configuration().unwrap().source().uri.as_ptr();
    assert!(matches!(
        foreign.request(
            RequestReason::ExplicitReload,
            server.source("/missing.css"),
            limits
        ),
        Err(RequestError::Exhausted(_))
    ));
    assert_eq!(
        foreign.configuration().unwrap().source().uri.as_ptr(),
        pointer
    );
    assert!(matches!(
        foreign.finish(block_on(pending)).unwrap(),
        SheetCompletion::Published(_)
    ));
    assert_eq!(server.count("/missing.css"), 2);
}

#[test]
fn http_message_host_bounds_delivery_and_only_reloads_after_explicit_messages() {
    use iced_css::host::stylesheet::http_coordinator::*;
    use iced_css::host::stylesheet::http_graph::HttpRuntimeLimits;
    use iced_css::resources::watch_work::{WatchWorkError, WatchWorkLimits};
    use iced_css_core::resources::triggers::*;
    for mode in [RuntimeMode::Manual, RuntimeMode::Auto] {
        let server = Server::new();
        let config = HttpStylesheetHostConfig {
            mode,
            context: context(),
            publication: PublicationLimits {
                requests: NonZeroU64::new(30).unwrap(),
                revisions: NonZeroU64::new(30).unwrap(),
            },
            diagnostic_work: n(),
            identities: IdentityLimits {
                entries: n(),
                bytes: n(),
                work: n(),
            },
            runtime: HttpRuntimeLimits {
                acquisition: settings(),
                graph: graph_limits(),
            },
            work: WatchWorkLimits {
                slots: nz(1),
                retained_bytes: n(),
            },
        };
        let transport = transport();
        let make = || {
            HttpStylesheetHost::new(FileReader::new(nz(1)), Arc::clone(&transport), config).unwrap()
        };
        let mut host = make();
        let mut foreign = make();
        let reload = host.reload_message();
        let result = host.update(reload);
        assert!(result.work.is_none() && result.outcome.is_none());
        assert!(host.is_dirty() && !host.is_busy());
        assert_eq!(host.pending_work().unwrap(), 0);
        let work = host
            .request(RequestReason::FirstAcquisition, server.source("/start"))
            .unwrap();
        assert!(host.is_busy() && !host.is_dirty());
        assert!(matches!(
            host.request(RequestReason::ExplicitReload, server.source("/missing.css")),
            Err(HttpStylesheetRequestError::Work(WatchWorkError::Full))
        ));
        assert_eq!(
            host.configuration().unwrap().source().uri,
            server.source("/start").uri
        );
        let message = block_on(work);
        let _: iced::Element<'_, HttpStylesheetMessage> = iced::widget::button("Reload CSS")
            .on_press(host.reload_message())
            .into();
        let duplicate = message.clone();
        assert!(foreign.update(message.clone()).outcome.is_none());
        let result = host.update(message);
        assert!(matches!(
            result.outcome.unwrap().unwrap(),
            SheetCompletion::Published(_)
        ));
        assert!(result.work.is_none() && !host.is_busy());
        let revision = host.active_revision();
        assert!(host.update(duplicate).outcome.is_none());
        assert_eq!(host.active_revision(), revision);
        assert_eq!(host.pending_work().unwrap(), 0);
        let before = server.count("/start");
        let work = host
            .request(RequestReason::ExplicitReload, server.source("/start"))
            .unwrap();
        for _ in 0..5 {
            let reload = host.reload_message();
            let result = host.update(reload);
            assert!(result.outcome.is_none() && result.work.is_none());
        }
        assert!(host.is_dirty());
        let result = host.update(block_on(work));
        assert!(matches!(
            result.outcome.unwrap().unwrap(),
            SheetCompletion::Published(_)
        ));
        assert!(host.is_busy() && !host.is_dirty());
        assert_eq!(host.pending_work().unwrap(), 1);
        let result = host.update(block_on(result.work.unwrap()));
        assert!(matches!(
            result.outcome.unwrap().unwrap(),
            SheetCompletion::Published(_)
        ));
        assert!(result.work.is_none() && !host.is_busy());
        assert_eq!(server.count("/start") - before, 2);
        assert_eq!(host.pending_work().unwrap(), 0);
        let abandoned = host
            .request(RequestReason::ExplicitReload, server.source("/start"))
            .unwrap();
        let message = block_on(abandoned);
        let clone = message.clone();
        drop(message);
        assert!(host.is_busy());
        drop(clone);
        assert!(!host.is_busy());
        assert_eq!(host.pending_work().unwrap(), 0);
        let reload = host.reload_message();
        let result = host.update(reload);
        let completion = block_on(result.work.unwrap());
        assert!(matches!(
            host.update(completion).outcome.unwrap().unwrap(),
            SheetCompletion::Published(_)
        ));
        let work = host
            .request(RequestReason::ExplicitReload, server.source("/start"))
            .unwrap();
        let message = block_on(work);
        let clone = message.clone();
        drop(host);
        assert!(!message.is_live() && !clone.is_live());
        assert!(foreign.update(message).outcome.is_none());
    }
}

#[test]
fn http_message_host_retires_abandoned_and_stale_work_without_remote_fallback() {
    use iced_css::host::stylesheet::{http_coordinator::*, http_graph::HttpRuntimeLimits};
    use iced_css::resources::watch_work::WatchWorkLimits;
    use iced_css_core::resources::triggers::*;
    let server = Server::new();
    let transport = transport();
    let config = HttpStylesheetHostConfig {
        mode: RuntimeMode::Auto,
        context: context(),
        publication: PublicationLimits {
            requests: NonZeroU64::new(30).unwrap(),
            revisions: NonZeroU64::new(30).unwrap(),
        },
        diagnostic_work: n(),
        identities: IdentityLimits {
            entries: n(),
            bytes: n(),
            work: n(),
        },
        runtime: HttpRuntimeLimits {
            acquisition: settings(),
            graph: graph_limits(),
        },
        work: WatchWorkLimits {
            slots: nz(2),
            retained_bytes: n(),
        },
    };
    let make = |config| {
        HttpStylesheetHost::new(FileReader::new(nz(1)), Arc::clone(&transport), config).unwrap()
    };
    let mut host = make(config);
    let first = host
        .request(RequestReason::FirstAcquisition, server.source("/start"))
        .unwrap();
    assert!(matches!(
        host.update(block_on(first)).outcome.unwrap().unwrap(),
        SheetCompletion::Published(_)
    ));
    let revision = host.active_revision();
    let old_body = host.active_dependencies().unwrap().entries()[0]
        .representation()
        .unwrap()
        .content_decoded_body()
        .as_slice()
        .to_vec();
    let older = block_on(
        host.request(RequestReason::ExplicitReload, server.source("/start"))
            .unwrap(),
    );
    let newest = block_on(
        host.request(RequestReason::ExplicitReload, server.source("/missing.css"))
            .unwrap(),
    );
    assert_eq!(host.pending_work().unwrap(), 2);
    drop(newest);
    assert!(!host.is_busy());
    assert_eq!(host.pending_work().unwrap(), 1);
    let reload = host.reload_message();
    let retry = host.update(reload).work.unwrap();
    assert!(host.update(older).outcome.is_none());
    assert!(host.is_busy());
    assert_eq!(host.pending_work().unwrap(), 1);
    assert!(matches!(
        host.update(block_on(retry)).outcome.unwrap().unwrap(),
        SheetCompletion::Failed(_)
    ));
    assert!(!host.is_busy());
    assert_eq!(host.active_revision(), revision);
    assert_eq!(
        host.active_dependencies().unwrap().entries()[0]
            .representation()
            .unwrap()
            .content_decoded_body()
            .as_slice(),
        old_body
    );
    assert_eq!(server.count("/missing.css"), 2);
    let mut once = make(HttpStylesheetHostConfig {
        publication: PublicationLimits {
            requests: NonZeroU64::MIN,
            revisions: NonZeroU64::MIN,
        },
        ..config
    });
    let first = once
        .request(RequestReason::FirstAcquisition, server.source("/start"))
        .unwrap();
    assert!(matches!(
        once.request(RequestReason::ExplicitReload, server.source("/missing.css")),
        Err(HttpStylesheetRequestError::Request(
            RequestError::Exhausted(_)
        ))
    ));
    assert!(once.is_busy());
    assert_eq!(once.pending_work().unwrap(), 1);
    assert!(matches!(
        once.update(block_on(first)).outcome.unwrap().unwrap(),
        SheetCompletion::Published(_)
    ));
    let reload = once.reload_message();
    let result = once.update(reload);
    assert!(result.work.is_none() && once.is_dirty() && !once.is_busy());
    assert_eq!(once.pending_work().unwrap(), 0);
    assert_eq!(server.count("/missing.css"), 2);
    let mut early = make(config);
    let work = early
        .request(RequestReason::FirstAcquisition, server.source("/start"))
        .unwrap();
    drop(early);
    assert!(!block_on(work).is_live());
    // A resource worker may still be finishing the authorized collection after Drop.
    let mut task_host = make(config);
    let work = task_host
        .request(RequestReason::FirstAcquisition, server.source("/start"))
        .unwrap();
    let task: iced::Task<HttpStylesheetMessage> = work.into_task();
    drop(task);
    assert!(!task_host.is_busy());
    assert_eq!(task_host.pending_work().unwrap(), 0);
}
