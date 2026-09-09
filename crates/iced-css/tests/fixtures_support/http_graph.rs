use iced::futures::executor::block_on;
use iced_css::resources::{http::*, http_graph::*, http_stylesheet::*};
use iced_css_core::{
    resources::{
        body::BodyLimits, content_codings::ContentCodingLimits,
        http_admission::StylesheetMediaPolicy, http_metadata::ContentTypeLimits, uri::UriLimits,
    },
    stylesheet::{
        StylesheetIntakeLimits, collector::CollectorLimits, located_graph::BaseLimits,
        requests::ImportRequestLimits, uri_aliases::ImportUriAliasLimits,
    },
    syntax::imports::ImportLimits,
};
use std::{
    io::{Read, Write},
    net::TcpListener,
    num::NonZeroUsize,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};
fn n() -> NonZeroUsize {
    nz(100000)
}
pub fn nz(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap()
}
#[derive(Default)]
pub struct State {
    pub missing: bool,
    pub blue: bool,
    pub empty: bool,
    pub inconsistent: bool,
    pub warning: bool,
}
pub struct Server {
    pub base: String,
    pub state: Arc<Mutex<State>>,
    pub requests: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}
impl Server {
    pub fn new() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let state = Arc::new(Mutex::new(State::default()));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let (state2, requests2, stop2) = (state.clone(), requests.clone(), stop.clone());
        let worker = thread::spawn(move || {
            while !stop2.load(Ordering::SeqCst) {
                let (mut socket, _) = match listener.accept() {
                    Ok(v) => v,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(1));
                        continue;
                    }
                    Err(e) => panic!("{e}"),
                };
                socket
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                socket
                    .set_write_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut request = Vec::new();
                let mut bytes = [0; 512];
                while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                    let count = socket.read(&mut bytes).unwrap();
                    assert!(count > 0);
                    request.extend_from_slice(&bytes[..count]);
                    assert!(request.len() < 8192);
                }
                let path = std::str::from_utf8(&request)
                    .unwrap()
                    .split_whitespace()
                    .nth(1)
                    .unwrap()
                    .to_owned();
                let count = {
                    let mut requests = requests2.lock().unwrap();
                    requests.push(path.clone());
                    requests.iter().filter(|p| **p == path).count()
                };
                let state = state2.lock().unwrap();
                let mut redirect = None;
                let mut status = "200 OK";
                let mut charset = None;
                let utf16 = |text: &str| {
                    text.encode_utf16()
                        .flat_map(u16::to_le_bytes)
                        .collect::<Vec<_>>()
                };
                let body = match path.as_str() {
                    "/start" => {
                        redirect = Some("/nested/root.css");
                        Vec::new()
                    }
                    "/alias.css" => {
                        redirect = Some("/nested/child.css");
                        Vec::new()
                    }
                    "/nested/root.css" => {
                        charset = Some("UTF-16LE");
                        utf16(if state.empty {
                            ""
                        } else if state.warning {
                            "@import 'child.css'; r{color:nope;color:red}"
                        } else if state.blue {
                            "@import 'child.css'; @import '../alias.css'; r{color:blue}"
                        } else {
                            "@import 'child.css'; @import '../alias.css'; r{color:red}"
                        })
                    }
                    "/missing.css" => {
                        status = "404 Not Found";
                        Vec::new()
                    }
                    "/nested/child.css" if state.missing => {
                        status = "404 Not Found";
                        Vec::new()
                    }
                    "/nested/child.css" => utf16(if state.inconsistent && count == 2 {
                        "@import '../start'; c{color:green}"
                    } else {
                        "@import '../start'; c{color:blue}"
                    }),
                    "/contexts.css" => {
                        b"@import 'p16.css'; @import 'p8.css'; r{color:red}".to_vec()
                    }
                    "/p16.css" => {
                        charset = Some("UTF-16LE");
                        utf16("@import 'shared.css'; a{color:red}")
                    }
                    "/p8.css" => b"@import 'shared.css'; b{color:blue}".to_vec(),
                    "/shared.css" => b"r{color:red}/*\xc3\xa9*/".to_vec(),
                    _ => panic!("unexpected route {path}"),
                };
                let head = if let Some(location) = redirect {
                    format!(
                        "HTTP/1.1 307 Temporary Redirect\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    )
                } else {
                    let label =
                        charset.map_or(String::new(), |label| format!("; charset=\"{label}\""));
                    format!(
                        "HTTP/1.1 {status}\r\nContent-Type: text/css{label}\r\nCache-Control: no-store\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    )
                };
                socket.write_all(head.as_bytes()).unwrap();
                socket.write_all(&body).unwrap();
            }
        });
        Self {
            base,
            state,
            requests,
            stop,
            thread: Some(worker),
        }
    }
    pub fn source(&self, path: &str) -> HttpStylesheetSource {
        HttpStylesheetSource {
            uri: format!("{}{path}", self.base),
            encoding: Default::default(),
        }
    }
    pub fn count(&self, path: &str) -> usize {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .filter(|p| p.as_str() == path)
            .count()
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(worker) = self.thread.take() {
            worker.join().unwrap();
        }
    }
}
pub fn transport() -> Arc<DirectHttpTransport> {
    Arc::new(
        block_on(DirectHttpTransport::new(HttpSettings {
            operations: nz(4),
            timeout: Duration::from_secs(5),
            connect_timeout: Duration::from_secs(5),
            additional_roots: vec![],
            root_entries: nz(1),
            root_bytes: nz(8192),
        }))
        .unwrap(),
    )
}
pub fn settings(
    intake: StylesheetIntakeLimits,
    tracking: ImportLimits,
    bases: BaseLimits,
) -> HttpGraphLimits {
    HttpGraphLimits {
        acquisition: HttpStylesheetReadLimits {
            http: HttpLimits {
                uri: UriLimits {
                    bytes: nz(4096),
                    work: n(),
                },
                header_fields: nz(32),
                header_bytes: nz(8192),
                header_work: n(),
                read: iced_css::resources::local::ReadBodyLimits {
                    body: BodyLimits {
                        retained_bytes: nz(4096),
                        work: n(),
                    },
                    reads: nz(100),
                },
            },
            redirects: RedirectLimits {
                hops: nz(8),
                retained_bytes: nz(8192),
                work: n(),
            },
            stylesheet: HttpStylesheetLimits {
                transport_charset_bytes: nz(1024),
                header_fields: nz(32),
                header_reference_bytes: nz(1024),
                metadata_bytes: nz(1024),
                content_type: ContentTypeLimits {
                    parameters: nz(10),
                    retained_bytes: nz(4096),
                    work: n(),
                },
                codings: ContentCodingLimits {
                    fields: nz(10),
                    bytes: nz(4096),
                    members: nz(10),
                    record_bytes: nz(4096),
                    work: n(),
                },
                content: iced_css::resources::content_decoding::ContentDecodeLimits {
                    input_bytes: nz(4096),
                    output: BodyLimits {
                        retained_bytes: nz(4096),
                        work: n(),
                    },
                    calls: nz(100),
                    stages: nz(10),
                    members: nz(10),
                },
                intake,
            },
            media: StylesheetMediaPolicy::RequireCss,
        },
        attempts: HttpAttemptLimits {
            entries: nz(32),
            retained_bytes: nz(1_000_000),
            work: nz(1_000_000),
        },
        aliases: ImportUriAliasLimits {
            entries: nz(100),
            bytes: n(),
            work: n(),
        },
        collector: CollectorLimits {
            entries: n(),
            record_bytes: n(),
            work: n(),
            cycle_bytes: n(),
            requests: ImportRequestLimits {
                entries: n(),
                retained_bytes: n(),
                work: n(),
            },
            tracking,
        },
        bases,
    }
}
