//! Resource lifecycle primitives. Transport and host publication are separate owners.
pub mod publication;
pub mod triggers;
pub mod uri;

pub mod body;

pub mod slot;

pub mod http_metadata;

pub mod entity_tag;

pub mod vary;

pub mod cache_control;
mod http_syntax;

pub mod age;

pub mod location;

pub mod cache_restrictions;

pub mod watch_ownership;

pub mod file_uri;

pub mod content_codings;

pub mod http_admission;

pub mod freshness;

pub mod http_date;

pub mod vary_request;

pub mod validation_selection;

pub mod connection;
pub mod field_names;

pub mod storage_fields;

pub mod header_update;
