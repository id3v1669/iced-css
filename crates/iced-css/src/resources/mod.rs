//! Resource acquisition primitives. Request authorization, canonical identities,
//! background scheduling and generation publication belong to the resource host.
pub mod local;

pub mod worker;

mod budget;
pub mod http;
pub mod http_headers;

pub mod embedded;

pub mod watch;

pub mod watch_sessions;

#[cfg(unix)]
pub mod file_path;

#[cfg(unix)]
pub mod file_uri_read;

#[cfg(unix)]
pub mod file_identity;

pub mod graph;
pub mod stylesheet;

#[cfg(unix)]
pub mod file_imports;

pub mod content_decoding;

#[cfg(unix)]
pub mod file_dependencies;

#[cfg(unix)]
pub mod watch_paths;

pub mod watch_work;

pub mod http_stylesheet;

pub mod http_graph;
