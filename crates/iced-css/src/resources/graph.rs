//! Acquire fresh graph inputs and assemble them under one existing worker lease.
use super::worker::{FileReader, SubmitError, WorkerFailure};
use crate::host::stylesheet::graph::{
    CycleLocatedGraphInputs, GraphCompletion, GraphRequest, LocatedGraphInputs,
};
use iced_css_core::stylesheet::{
    located_graph::{BaseLimits, LocatedStylesheetGraph},
    owned_graph::OwnedLoadedSheet,
};
use std::future::Future;

#[derive(Debug)]
pub enum GraphWorkerError<E> {
    Acquisition(E),
    Submit(SubmitError),
    Worker(WorkerFailure),
}
impl<E: std::fmt::Display> std::fmt::Display for GraphWorkerError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Acquisition(error) => error.fmt(f),
            Self::Submit(error) => error.fmt(f),
            Self::Worker(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for GraphWorkerError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Acquisition(error) => error,
            Self::Submit(error) => error,
            Self::Worker(error) => error,
        })
    }
}
impl FileReader {
    /// The supplied acquisition callback executes on a resource worker, then its
    /// fresh results are assembled under the request's immutable import policy.
    /// Caller authorization, URI resolution and per-resource acquisition limits
    /// remain callback obligations. No previous graph is passed to the callback.
    ///
    /// Admission rejection does not invoke the callback. Submission and worker
    /// failures become paired completions for ordinary owner failure handling.
    /// Cancellation/panic handling follows the existing resource-worker contract.
    /// Delivered application Messages have storage outside the worker lease.
    pub fn load_stylesheet_graph<E: Send + 'static, F>(
        &self,
        request: GraphRequest,
        acquire: F,
    ) -> impl Future<Output = GraphCompletion<GraphWorkerError<E>>> + Send + 'static + use<E, F>
    where
        F: FnOnce() -> Result<Vec<OwnedLoadedSheet>, E> + Send + 'static,
    {
        self.graph_job(
            request,
            move |request| request.assemble(acquire().map_err(GraphWorkerError::Acquisition)),
            |request, error| request.assemble(Err(error)),
        )
    }

    /// Acquire and assemble source plus effective-base metadata on one worker.
    /// The callback supplies fresh inputs and obeys its acquisition limits; this
    /// method does not infer bases or provide automatic URI transport routing.
    pub fn load_located_stylesheet_graph<E: Send + 'static, F>(
        &self,
        request: GraphRequest,
        limits: BaseLimits,
        acquire: F,
    ) -> impl Future<Output = GraphCompletion<GraphWorkerError<E>, LocatedStylesheetGraph>>
    + Send
    + 'static
    + use<E, F>
    where
        F: FnOnce() -> Result<LocatedGraphInputs, E> + Send + 'static,
    {
        self.graph_job(
            request,
            move |request| {
                request.assemble_located(acquire().map_err(GraphWorkerError::Acquisition), limits)
            },
            move |request, error| request.assemble_located(Err(error), limits),
        )
    }

    /// Carry explicit cycle identities with fresh located inputs under one worker
    /// lease. The immutable cycle table has its own capacity allowance; source,
    /// acquisition and graph budgets remain separate. Identities and refresh
    /// authorization are supplied by the caller, not inferred from URI spelling.
    pub fn load_located_stylesheet_graph_with_cycles<E: Send + 'static, F>(
        &self,
        request: GraphRequest,
        limits: BaseLimits,
        acquire: F,
    ) -> impl Future<Output = GraphCompletion<GraphWorkerError<E>, LocatedStylesheetGraph>>
    + Send
    + 'static
    + use<E, F>
    where
        F: FnOnce() -> Result<CycleLocatedGraphInputs, E> + Send + 'static,
    {
        self.graph_job(
            request,
            move |request| assemble_cycle_inputs(request, acquire(), limits),
            move |request, error| request.assemble_located(Err(error), limits),
        )
    }

    /// Keep a callback's owned report paired with graph assembly, including an
    /// acquisition/assembly failure. Submission or worker failure has no report.
    /// A report is evidence only; the graph owner still decides eligibility.
    pub fn load_located_stylesheet_graph_with_report<E: Send + 'static, Report: Send + 'static, F>(
        &self,
        request: GraphRequest,
        limits: BaseLimits,
        acquire: F,
    ) -> impl Future<
        Output = (
            GraphCompletion<GraphWorkerError<E>, LocatedStylesheetGraph>,
            Option<Report>,
        ),
    >
    + Send
    + 'static
    + use<E, Report, F>
    where
        F: FnOnce() -> (Result<CycleLocatedGraphInputs, E>, Report) + Send + 'static,
    {
        self.graph_job(
            request,
            move |request| {
                let (inputs, report) = acquire();
                (assemble_cycle_inputs(request, inputs, limits), Some(report))
            },
            move |request, error| (request.assemble_located(Err(error), limits), None),
        )
    }

    fn graph_job<E: Send + 'static, Output: Send + 'static, F, R>(
        &self,
        request: GraphRequest,
        build: F,
        failed: R,
    ) -> impl Future<Output = Output> + Send + 'static + use<E, Output, F, R>
    where
        F: FnOnce(GraphRequest) -> Output + Send + 'static,
        R: FnOnce(GraphRequest, GraphWorkerError<E>) -> Output + Send + 'static,
    {
        let worker_request = request.clone();
        let work = self.submit(move || build(worker_request));
        async move {
            match work {
                Ok(work) => match work.await {
                    Ok(completion) => completion,
                    Err(error) => failed(request, GraphWorkerError::Worker(error)),
                },
                Err(error) => failed(request, GraphWorkerError::Submit(error)),
            }
        }
    }
}

fn assemble_cycle_inputs<E>(
    request: GraphRequest,
    inputs: Result<CycleLocatedGraphInputs, E>,
    limits: BaseLimits,
) -> GraphCompletion<GraphWorkerError<E>, LocatedStylesheetGraph> {
    match inputs {
        Ok(inputs) => {
            request.assemble_located_with_cycles(Ok(inputs.graph), inputs.cycles.as_slice(), limits)
        }
        Err(error) => request.assemble_located(Err(GraphWorkerError::Acquisition(error)), limits),
    }
}
