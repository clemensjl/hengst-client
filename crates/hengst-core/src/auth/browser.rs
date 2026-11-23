//! Microsoft sign-in through the system browser: OAuth 2.0 authorization code flow with PKCE
//! (RFC 7636, `S256`) and a loopback redirect (RFC 8252). The caller opens
//! [`BrowserLogin::authorize_url`] in the browser; the core only listens on the loopback address.

use super::msa::{error_code, map_client_error, parse_tokens, MsaTokens};
use super::{AuthError, MicrosoftAuth, SCOPE};
use crate::secret::Secret;
use crate::{Error, Result};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use sha2::{Digest, Sha256};
use std::collections::VecDeque;
use std::fmt;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot};
use tokio::task::{AbortHandle, JoinSet};

/// How long [`MicrosoftAuth::finish_browser_login`] waits for the browser by default.
pub const BROWSER_LOGIN_TIMEOUT: Duration = Duration::from_secs(300);

/// Largest request head the listener reads (request line plus headers).
const MAX_REQUEST: usize = 8 * 1024;
/// Header lines allowed after the request line.
const MAX_HEADER_LINES: usize = 64;
/// Time a connection gets to send its request head.
const READ_TIMEOUT: Duration = Duration::from_secs(5);
/// Time a connection gets to take its answer.
const WRITE_TIMEOUT: Duration = Duration::from_secs(5);
/// Connection tasks alive at once; beyond that the oldest one still reading is closed.
const MAX_CONNECTIONS: usize = 32;
/// Pause before accepting again after an accept error that may repeat (resources exhausted or
/// unknown); per-connection errors cause none.
const ACCEPT_BACKOFF: Duration = Duration::from_millis(50);
/// Longest `error` code kept from an authorization response.
const MAX_ERROR_CODE: usize = 64;

/// Texts of the page the browser shows after the redirect, supplied (translated) by the caller.
/// The core HTML-escapes them.
#[derive(Clone, Debug)]
pub struct BrowserLoginPages {
    pub success_title: String,
    pub success_body: String,
    pub failure_title: String,
    pub failure_body: String,
}

/// Fresh OS-chosen ports tried before the sign-in gives up on finding one free on both
/// loopback addresses.
const BIND_ATTEMPTS: usize = 8;

/// The listening sockets of one sign-in, on the same port.
struct Listeners {
    v4: TcpListener,
    /// `None` only when the machine has no IPv6 loopback.
    v6: Option<TcpListener>,
}

/// A started browser sign-in. Owns the listening sockets: dropping it frees the port.
pub struct BrowserLogin {
    /// Shared only so tests can watch the sockets close through a `Weak`.
    listeners: Arc<Listeners>,
    port: u16,
    redirect_uri: String,
    authorize_url: String,
    verifier: Secret,
    state: String,
}

impl BrowserLogin {
    /// The address to open in the browser. It carries `state` and the PKCE challenge: hand it to
    /// the browser only, never to a log or the web view.
    pub fn authorize_url(&self) -> &str {
        &self.authorize_url
    }

    pub fn redirect_uri(&self) -> &str {
        &self.redirect_uri
    }

    #[cfg(test)]
    fn verifier_for_test(&self) -> &str {
        self.verifier.expose()
    }

    #[cfg(test)]
    fn state_for_test(&self) -> &str {
        &self.state
    }

    /// Dead once the listening sockets are closed.
    #[cfg(test)]
    fn listeners_probe(&self) -> std::sync::Weak<Listeners> {
        Arc::downgrade(&self.listeners)
    }
}

impl fmt::Debug for BrowserLogin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BrowserLogin")
            .field("redirect_uri", &self.redirect_uri)
            .finish_non_exhaustive()
    }
}

/// Base64url without padding of `len` bytes from the operating system's CSPRNG.
fn random_b64(len: usize) -> Result<String> {
    let mut bytes = vec![0u8; len];
    getrandom::fill(&mut bytes).map_err(|e| Error::Io(std::io::Error::other(e.to_string())))?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

/// PKCE `S256`: base64url(SHA-256(ASCII verifier)), RFC 7636 section 4.2.
fn challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

/// Comparison whose duration does not depend on where the first difference is.
fn same(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

/// The `error` of an authorization response comes from the URL: keep at most 64 characters of
/// `[A-Za-z0-9_.-]`.
fn sanitize_code(raw: &str) -> String {
    let code: String = raw
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
        .take(MAX_ERROR_CODE)
        .collect();
    if code.is_empty() {
        "unknown".into()
    } else {
        code
    }
}

fn escape_html(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

/// What the browser gets back.
enum Reply {
    BadRequest,
    NotFound,
    Page { title: String, body: String },
}

impl Reply {
    fn page(title: &str, body: &str) -> Reply {
        Reply::Page {
            title: title.to_string(),
            body: body.to_string(),
        }
    }

    fn bytes(&self) -> Vec<u8> {
        let (status, content_type, body) = match self {
            Reply::BadRequest => ("400 Bad Request", "text/plain", "Bad Request".to_string()),
            Reply::NotFound => ("404 Not Found", "text/plain", "Not Found".to_string()),
            Reply::Page { title, body } => {
                let (title, body) = (escape_html(title), escape_html(body));
                (
                    "200 OK",
                    "text/html",
                    format!(
                        "<!DOCTYPE html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n\
                         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
                         <title>{title}</title>\n</head>\n<body>\n<h1>{title}</h1>\n<p>{body}</p>\n\
                         </body>\n</html>\n"
                    ),
                )
            }
        };
        format!(
            "HTTP/1.1 {status}\r\n\
             Content-Type: {content_type}; charset=utf-8\r\n\
             Content-Length: {}\r\n\
             Cache-Control: no-store\r\n\
             Referrer-Policy: no-referrer\r\n\
             X-Content-Type-Options: nosniff\r\n\
             Content-Security-Policy: default-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'\r\n\
             Connection: close\r\n\r\n{body}",
            body.len()
        )
        .into_bytes()
    }
}

/// The parameters of a `GET /?...` request, handed from a connection task to the wait loop.
struct Callback {
    state: Option<String>,
    code: Option<Secret>,
    error: Option<String>,
    reply: oneshot::Sender<Reply>,
    /// Closed by the connection task once the answer is written (or the task ended).
    written: oneshot::Receiver<()>,
}

enum Request {
    /// Not an HTTP/1.x request line, a repeated parameter or too many header lines.
    Bad,
    /// Another method or path.
    NotFound,
    Callback {
        state: Option<String>,
        code: Option<Secret>,
        error: Option<String>,
    },
}

/// Exactly one `Host` header naming a loopback host with this listener's port. Anything else (a
/// DNS-rebinding page under a foreign name, a missing or repeated header) is refused, so no
/// other origin can read the listener's answers.
fn host_is_loopback(headers: &[&str], port: u16) -> bool {
    let mut hosts = headers.iter().filter_map(|h| {
        let (name, value) = h.split_once(':')?;
        name.eq_ignore_ascii_case("host").then_some(value)
    });
    let (Some(value), None) = (hosts.next(), hosts.next()) else {
        return false;
    };
    let Some((host, given_port)) = value.trim_matches([' ', '\t']).rsplit_once(':') else {
        return false;
    };
    given_port == port.to_string()
        && ["localhost", "127.0.0.1", "[::1]"]
            .iter()
            .any(|h| host.eq_ignore_ascii_case(h))
}

/// Classifies the request head (everything up to the empty line).
fn parse_request(head: &[u8], port: u16) -> Request {
    let Ok(head) = std::str::from_utf8(head) else {
        return Request::Bad;
    };
    let mut lines = head.split("\r\n").filter(|l| !l.is_empty());
    let Some(line) = lines.next() else {
        return Request::Bad;
    };
    let headers: Vec<&str> = lines.collect();
    if headers.len() > MAX_HEADER_LINES || !host_is_loopback(&headers, port) {
        return Request::Bad;
    }
    let mut parts = line.split(' ');
    let (Some(verb), Some(target), Some(version), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Request::Bad;
    };
    if !matches!(version, "HTTP/1.1" | "HTTP/1.0") || !target.starts_with('/') {
        return Request::Bad;
    }
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    if verb != "GET" || path != "/" {
        return Request::NotFound;
    }
    // The URL library decodes the query (percent escapes and `+`).
    let mut url = reqwest::Url::parse("http://localhost/").expect("constant URL");
    url.set_query(Some(query));
    let (mut state, mut code, mut error) = (None, None, None);
    for (key, value) in url.query_pairs() {
        let slot = match key.as_ref() {
            "state" => &mut state,
            "code" => &mut code,
            "error" => &mut error,
            _ => continue,
        };
        if slot.replace(value.into_owned()).is_some() {
            return Request::Bad;
        }
    }
    Request::Callback {
        state,
        code: code.map(Secret::new),
        error,
    }
}

/// Reads up to the empty line that ends the request head. `None` for a closed connection, a
/// read error or a head larger than [`MAX_REQUEST`].
async fn read_head(stream: &mut TcpStream) -> Option<Vec<u8>> {
    let mut head = Vec::with_capacity(1024);
    let mut chunk = [0u8; 1024];
    loop {
        let n = stream.read(&mut chunk).await.ok()?;
        if n == 0 {
            return None;
        }
        let searched_from = head.len().saturating_sub(3);
        head.extend_from_slice(&chunk[..n]);
        if let Some(end) = head[searched_from..]
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
        {
            head.truncate(searched_from + end);
            return Some(head);
        }
        if head.len() >= MAX_REQUEST {
            return None;
        }
    }
}

async fn write_reply(stream: &mut TcpStream, reply: &Reply) {
    let _ = tokio::time::timeout(WRITE_TIMEOUT, async {
        stream.write_all(&reply.bytes()).await?;
        stream.shutdown().await
    })
    .await;
}

/// One accepted connection: read and classify the request, answer what is not a callback
/// itself, hand callbacks to the wait loop and write its answer.
async fn serve(
    mut stream: TcpStream,
    port: u16,
    loop_tx: mpsc::Sender<Callback>,
    reading: Arc<AtomicBool>,
) {
    let head = tokio::time::timeout(READ_TIMEOUT, read_head(&mut stream)).await;
    reading.store(false, Ordering::Relaxed);
    let Ok(Some(head)) = head else {
        return;
    };
    let (state, code, error) = match parse_request(&head, port) {
        Request::Bad => return write_reply(&mut stream, &Reply::BadRequest).await,
        Request::NotFound => return write_reply(&mut stream, &Reply::NotFound).await,
        Request::Callback { state, code, error } => (state, code, error),
    };
    let (reply_tx, reply_rx) = oneshot::channel();
    let (_written, written) = oneshot::channel();
    let callback = Callback {
        state,
        code,
        error,
        reply: reply_tx,
        written,
    };
    if loop_tx.send(callback).await.is_err() {
        return;
    }
    if let Ok(reply) = reply_rx.await {
        write_reply(&mut stream, &reply).await;
    }
    // `_written` drops here and tells the loop the answer is out.
}

/// The connection tasks of one login. Dropping it aborts all of them.
struct Connections {
    tasks: JoinSet<()>,
    /// Tasks still reading their request, oldest first.
    reading: VecDeque<(AbortHandle, Arc<AtomicBool>)>,
}

impl Connections {
    fn new() -> Self {
        Connections {
            tasks: JoinSet::new(),
            reading: VecDeque::new(),
        }
    }

    /// Serves an accepted connection. An error that concerns only one connection is skipped at
    /// once; any other accept error (no file handles or buffers left, unknown) holds accepting
    /// back for [`ACCEPT_BACKOFF`] through `not_before`, so it cannot spin the loop.
    fn accepted(
        &mut self,
        conn: std::io::Result<TcpStream>,
        port: u16,
        loop_tx: &mpsc::Sender<Callback>,
        not_before: &mut Option<tokio::time::Instant>,
    ) {
        match conn {
            Ok(stream) => {
                *not_before = None;
                self.spawn(stream, port, loop_tx);
            }
            Err(e) if per_connection(&e) => {}
            Err(_) => *not_before = Some(tokio::time::Instant::now() + ACCEPT_BACKOFF),
        }
    }

    fn spawn(&mut self, stream: TcpStream, port: u16, loop_tx: &mpsc::Sender<Callback>) {
        while self.tasks.try_join_next().is_some() {}
        self.reading
            .retain(|(task, reading)| !task.is_finished() && reading.load(Ordering::Relaxed));
        if self.tasks.len() >= MAX_CONNECTIONS {
            if let Some((oldest, _)) = self.reading.pop_front() {
                oldest.abort();
            }
        }
        let reading = Arc::new(AtomicBool::new(true));
        let task = self
            .tasks
            .spawn(serve(stream, port, loop_tx.clone(), reading.clone()));
        self.reading.push_back((task, reading));
    }
}

/// Where the wait loop takes its connections from; tests script accept errors through it.
trait Acceptor: Send + Sync {
    /// The next connection. Cancel safe.
    fn accept(&self) -> impl std::future::Future<Output = std::io::Result<TcpStream>> + Send + '_;
}

impl Acceptor for Arc<Listeners> {
    fn accept(&self) -> impl std::future::Future<Output = std::io::Result<TcpStream>> + Send + '_ {
        accept(&self.v4, self.v6.as_ref())
    }
}

/// An accept error that concerns only the one connection (it was reset or aborted before the
/// accept): accept the next one at once.
fn per_connection(e: &std::io::Error) -> bool {
    // Windows reports WSAECONNRESET (10054) and WSAECONNABORTED (10053) as these kinds.
    use std::io::ErrorKind::{ConnectionAborted, ConnectionReset, Interrupted};
    matches!(e.kind(), ConnectionReset | ConnectionAborted | Interrupted)
}

/// The next connection, but not before `not_before`. The pause lives in this future only, so
/// the other branches of the wait loop keep being polled while it lasts.
async fn next_connection<A: Acceptor>(
    acceptor: &A,
    not_before: Option<tokio::time::Instant>,
) -> std::io::Result<TcpStream> {
    if let Some(at) = not_before {
        tokio::time::sleep_until(at).await;
    }
    acceptor.accept().await
}

/// The next connection on either loopback listener. Cancel safe.
async fn accept(v4: &TcpListener, v6: Option<&TcpListener>) -> std::io::Result<TcpStream> {
    match v6 {
        Some(v6) => tokio::select! {
            r = v4.accept() => r.map(|(s, _)| s),
            r = v6.accept() => r.map(|(s, _)| s),
        },
        None => v4.accept().await.map(|(s, _)| s),
    }
}

/// The IPv6 bind failed because the machine has no IPv6 loopback, not because the port is
/// taken.
fn ipv6_unavailable(e: &std::io::Error) -> bool {
    // WSAEAFNOSUPPORT (10047), WSAEADDRNOTAVAIL (10049); the latter is also `AddrNotAvailable`.
    e.kind() == std::io::ErrorKind::AddrNotAvailable
        || (cfg!(windows) && matches!(e.raw_os_error(), Some(10047 | 10049)))
}

/// Binds `127.0.0.1` on an OS-chosen port and `[::1]` on the same port (`bind_v6` is the
/// IPv6 step, replaceable in tests). The IPv4 and IPv6 port spaces are separate, so the port
/// may be taken on `[::1]`: then a browser resolving `localhost` to `::1` would reach another
/// program. Such a port is given up for a fresh one; only a machine without IPv6 loopback
/// continues on IPv4 alone.
async fn bind_loopback<F, Fut>(mut bind_v6: F) -> Result<(Listeners, u16)>
where
    F: FnMut(u16) -> Fut,
    Fut: std::future::Future<Output = std::io::Result<TcpListener>>,
{
    for _ in 0..BIND_ATTEMPTS {
        let v4 = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .map_err(|_| AuthError::LoopbackUnavailable)?;
        let port = v4
            .local_addr()
            .map_err(|_| AuthError::LoopbackUnavailable)?
            .port();
        match bind_v6(port).await {
            Ok(v6) => return Ok((Listeners { v4, v6: Some(v6) }, port)),
            Err(e) if ipv6_unavailable(&e) => return Ok((Listeners { v4, v6: None }, port)),
            // Taken (or any other failure): drop this IPv4 listener and try a fresh port.
            Err(_) => {}
        }
    }
    Err(AuthError::LoopbackUnavailable.into())
}

/// What the one accepted callback carried.
enum Outcome {
    Code(Secret),
    Error(String),
}

impl MicrosoftAuth {
    /// Binds the loopback listener and prepares the authorization request. The caller opens
    /// [`BrowserLogin::authorize_url`] in the system browser, then calls
    /// [`MicrosoftAuth::finish_browser_login`].
    ///
    /// The authorization URL contains `state` and the PKCE challenge. Do not log it, do not hand
    /// it to a web view, and do not put it where other users of the machine can read it (for
    /// example a command line of another process or a shared file); give it to the browser only.
    ///
    /// The success page the browser shows means only that the Microsoft step succeeded: the
    /// Xbox, XSTS and game-services steps of [`MicrosoftAuth::complete_login`] still follow and
    /// can fail.
    pub async fn start_browser_login(&self) -> Result<BrowserLogin> {
        let (listeners, port) =
            bind_loopback(|port| TcpListener::bind((Ipv6Addr::LOCALHOST, port))).await?;
        let verifier = Secret::new(random_b64(32)?);
        let state = random_b64(16)?;
        let redirect_uri = format!("http://localhost:{port}");
        let authorize_url = reqwest::Url::parse_with_params(
            &format!("{}/authorize", self.ep.msa),
            &[
                ("client_id", self.client_id.as_str()),
                ("response_type", "code"),
                ("redirect_uri", &redirect_uri),
                ("response_mode", "query"),
                ("scope", SCOPE),
                ("code_challenge", &challenge(verifier.expose())),
                ("code_challenge_method", "S256"),
                ("state", &state),
                ("prompt", "select_account"),
            ],
        )
        .map_err(|_| {
            Error::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "invalid Microsoft sign-in endpoint",
            ))
        })?
        .into();
        Ok(BrowserLogin {
            listeners: Arc::new(listeners),
            port,
            redirect_uri,
            authorize_url,
            verifier,
            state,
        })
    }

    /// Waits for the browser's redirect and exchanges its code for Microsoft tokens. Exactly one
    /// callback with the right `state` is accepted; everything else is answered and ignored.
    /// The listeners close when this returns or its future is dropped.
    pub async fn finish_browser_login(
        &self,
        login: BrowserLogin,
        pages: &BrowserLoginPages,
        timeout: Duration,
    ) -> Result<MsaTokens> {
        self.finish_with(login, pages, timeout, |listeners| listeners)
            .await
    }

    /// [`MicrosoftAuth::finish_browser_login`] with the connection source built by `acceptor`
    /// from the login's listeners.
    async fn finish_with<A: Acceptor>(
        &self,
        login: BrowserLogin,
        pages: &BrowserLoginPages,
        timeout: Duration,
        acceptor: impl FnOnce(Arc<Listeners>) -> A,
    ) -> Result<MsaTokens> {
        let BrowserLogin {
            listeners,
            port,
            redirect_uri,
            verifier,
            state,
            ..
        } = login;
        let acceptor = acceptor(listeners);
        let deadline = tokio::time::sleep(timeout);
        tokio::pin!(deadline);
        let (loop_tx, mut callbacks) = mpsc::channel::<Callback>(MAX_CONNECTIONS);
        let mut connections = Connections::new();
        // Set after an accept error that may repeat; see `Connections::accepted`.
        let mut not_before: Option<tokio::time::Instant> = None;

        let (outcome, reply, written) = loop {
            tokio::select! {
                _ = &mut deadline => return Err(AuthError::BrowserLoginTimedOut.into()),
                conn = next_connection(&acceptor, not_before) => {
                    connections.accepted(conn, port, &loop_tx, &mut not_before);
                }
                Some(cb) = callbacks.recv() => {
                    let valid = cb.state.as_deref().is_some_and(|s| same(s, &state));
                    let outcome = match (cb.error, cb.code) {
                        (Some(error), _) => Some(Outcome::Error(error)),
                        (None, Some(code)) => Some(Outcome::Code(code)),
                        (None, None) => None,
                    };
                    match outcome {
                        Some(outcome) if valid => break (outcome, cb.reply, cb.written),
                        _ => {
                            let _ = cb.reply.send(Reply::BadRequest);
                        }
                    }
                }
            }
        };

        let result = match outcome {
            Outcome::Error(error) if error == "access_denied" => {
                Err(AuthError::LoginDeclined.into())
            }
            Outcome::Error(error) => Err(AuthError::BrowserLoginFailed {
                code: sanitize_code(&error),
            }
            .into()),
            Outcome::Code(code) => {
                let exchange = self.exchange_code(&code, &verifier, &redirect_uri);
                tokio::pin!(exchange);
                loop {
                    tokio::select! {
                        r = &mut exchange => break r,
                        _ = &mut deadline => break Err(AuthError::BrowserLoginTimedOut.into()),
                        conn = next_connection(&acceptor, not_before) => {
                            connections.accepted(conn, port, &loop_tx, &mut not_before);
                        }
                        Some(cb) = callbacks.recv() => {
                            let _ = cb.reply.send(Reply::BadRequest);
                        }
                    }
                }
            }
        };
        let page = match result {
            Ok(_) => Reply::page(&pages.success_title, &pages.success_body),
            Err(_) => Reply::page(&pages.failure_title, &pages.failure_body),
        };
        if reply.send(page).is_ok() {
            // The connection task writes within its own timeout, then closes `written`.
            let _ = written.await;
        }
        result
    }

    async fn exchange_code(
        &self,
        code: &Secret,
        verifier: &Secret,
        redirect_uri: &str,
    ) -> Result<MsaTokens> {
        let url = format!("{}/token", self.ep.msa);
        let resp = self
            .post_form(
                &url,
                &[
                    ("grant_type", "authorization_code"),
                    ("client_id", &self.client_id),
                    ("code", code.expose()),
                    ("redirect_uri", redirect_uri),
                    ("code_verifier", verifier.expose()),
                    ("scope", SCOPE),
                ],
            )
            .await?;
        if resp.status().is_success() {
            return parse_tokens(resp, &url).await;
        }
        // An expired or reused code (`invalid_grant`) and the other OAuth errors end this sign-in
        // like an error in the redirect: only the sanitized code, never the description or body.
        let err = error_code(resp, &url).await?;
        let failed = AuthError::BrowserLoginFailed {
            code: sanitize_code(&err),
        };
        Err(map_client_error(err, failed.into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{AuthEndpoints, AuthError};
    use crate::Error;
    use serde_json::json;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;
    use wiremock::matchers::{body_string_contains, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn auth(server: &MockServer) -> MicrosoftAuth {
        MicrosoftAuth::new(
            "test-id".into(),
            AuthEndpoints {
                msa: server.uri(),
                ..AuthEndpoints::default()
            },
        )
    }

    fn pages() -> BrowserLoginPages {
        BrowserLoginPages {
            success_title: "SUCCESS-TITLE".into(),
            success_body: "SUCCESS-BODY".into(),
            failure_title: "FAILURE-TITLE".into(),
            failure_body: "FAILURE-BODY".into(),
        }
    }

    fn token_ok() -> ResponseTemplate {
        ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "at", "refresh_token": "rt", "expires_in": 3600
        }))
    }

    async fn auth_with_token_endpoint(calls: u64) -> (MicrosoftAuth, MockServer) {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(token_ok())
            .expect(calls)
            .mount(&server)
            .await;
        (auth(&server), server)
    }

    /// `/token` answers with an OAuth error and a description that must never surface.
    async fn auth_with_token_error(status: u16, error: &str) -> (MicrosoftAuth, MockServer) {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(
                ResponseTemplate::new(status)
                    .set_body_json(json!({"error": error, "error_description": "secret detail"})),
            )
            .expect(1)
            .mount(&server)
            .await;
        (auth(&server), server)
    }

    fn redirect_port(login: &BrowserLogin) -> u16 {
        login
            .redirect_uri()
            .strip_prefix("http://localhost:")
            .expect("redirect uri form")
            .parse()
            .unwrap()
    }

    /// The tests never go through a proxy configured on the machine.
    fn client() -> reqwest::Client {
        reqwest::Client::builder().no_proxy().build().unwrap()
    }

    async fn callback(port: u16, query: &str) -> reqwest::Response {
        client()
            .get(format!("http://127.0.0.1:{port}/?{query}"))
            .send()
            .await
            .unwrap()
    }

    /// Waits (up to 10 s) until the login's listening sockets are closed. Checks the login's own
    /// sockets instead of binding the port again: in a parallel run the OS may hand a just-freed
    /// port to another test at once.
    async fn assert_closed(probe: &std::sync::Weak<Listeners>) {
        tokio::time::timeout(Duration::from_secs(10), async {
            while probe.upgrade().is_some() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("the listening sockets are still open");
    }

    /// A token endpoint that holds its answer until the test releases it.
    struct Gate {
        auth: MicrosoftAuth,
        /// Fires once the exchange request has arrived.
        received: oneshot::Receiver<()>,
        release: oneshot::Sender<()>,
        /// Connections the endpoint accepted.
        calls: Arc<std::sync::atomic::AtomicUsize>,
    }

    async fn gated_token_endpoint(status: u16, body: serde_json::Value) -> Gate {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let msa = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let (received_tx, received) = oneshot::channel();
        let (release, released) = oneshot::channel::<()>();
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counted = calls.clone();
        tokio::spawn(async move {
            let (mut s, _) = listener.accept().await.unwrap();
            counted.fetch_add(1, Ordering::SeqCst);
            read_http_request(&mut s).await;
            let _ = received_tx.send(());
            let _ = released.await;
            let body = body.to_string();
            let answer = format!(
                "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            s.write_all(answer.as_bytes()).await.unwrap();
            let _ = s.shutdown().await;
            drop(s);
            while listener.accept().await.is_ok() {
                counted.fetch_add(1, Ordering::SeqCst);
            }
        });
        let auth = MicrosoftAuth::new(
            "test-id".into(),
            AuthEndpoints {
                msa,
                ..AuthEndpoints::default()
            },
        );
        Gate {
            auth,
            received,
            release,
            calls,
        }
    }

    /// Reads a request head and its `Content-Length` body.
    async fn read_http_request(s: &mut TcpStream) {
        let mut buf = Vec::new();
        let mut chunk = [0u8; 4096];
        let head_end = loop {
            let n = s.read(&mut chunk).await.unwrap();
            assert!(n > 0, "request ended early");
            buf.extend_from_slice(&chunk[..n]);
            if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                break i + 4;
            }
        };
        let head = String::from_utf8_lossy(&buf[..head_end]).to_ascii_lowercase();
        let length: usize = head
            .lines()
            .find_map(|l| l.strip_prefix("content-length:"))
            .map(|v| v.trim().parse().unwrap())
            .unwrap_or(0);
        while buf.len() < head_end + length {
            let n = s.read(&mut chunk).await.unwrap();
            assert!(n > 0, "body ended early");
            buf.extend_from_slice(&chunk[..n]);
        }
    }

    /// Writes raw bytes and reads the whole answer (empty when the server just closes).
    async fn raw(port: u16, request: &[u8]) -> String {
        let mut s = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        // A reset while writing is a valid way of refusing an oversized request.
        let _ = s.write_all(request).await;
        let mut out = Vec::new();
        let _ = s.read_to_end(&mut out).await;
        String::from_utf8_lossy(&out).into_owned()
    }

    fn b64(bytes: &[u8]) -> String {
        use base64::Engine;
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
    }

    #[test]
    fn s256_matches_the_rfc_7636_example() {
        // RFC 7636 appendix B.
        assert_eq!(
            challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[tokio::test]
    async fn verifier_and_state_are_fresh_random_base64url() {
        let (auth, _server) = auth_with_token_endpoint(0).await;
        let a = auth.start_browser_login().await.unwrap();
        let b = auth.start_browser_login().await.unwrap();
        let url_safe = |s: &str| {
            s.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        };
        for login in [&a, &b] {
            assert_eq!(login.verifier_for_test().len(), 43);
            assert!(url_safe(login.verifier_for_test()));
            assert_eq!(login.state_for_test().len(), 22);
            assert!(url_safe(login.state_for_test()));
        }
        assert_ne!(a.verifier_for_test(), b.verifier_for_test());
        assert_ne!(a.state_for_test(), b.state_for_test());
        assert_ne!(redirect_port(&a), redirect_port(&b));
    }

    #[tokio::test]
    async fn authorize_url_carries_pkce_and_consumers_scope() {
        let (auth, server) = auth_with_token_endpoint(0).await;
        let login = auth.start_browser_login().await.unwrap();
        let url = reqwest::Url::parse(login.authorize_url()).unwrap();
        let mock = reqwest::Url::parse(&server.uri()).unwrap();
        assert_eq!(url.host_str(), mock.host_str());
        assert_eq!(url.port(), mock.port());
        assert!(url.path().ends_with("/authorize"), "{}", url.path());
        let q: std::collections::HashMap<String, String> = url.query_pairs().into_owned().collect();
        assert_eq!(q.len(), 9, "{q:?}");
        assert_eq!(q["client_id"], "test-id");
        assert_eq!(q["response_type"], "code");
        assert_eq!(q["response_mode"], "query");
        assert_eq!(q["scope"], "XboxLive.signin offline_access");
        assert_eq!(q["code_challenge_method"], "S256");
        assert_eq!(q["prompt"], "select_account");
        assert_eq!(q["redirect_uri"], login.redirect_uri());
        assert!(q["redirect_uri"].starts_with("http://localhost:"));
        assert_eq!(
            q["code_challenge"],
            b64(&{
                use sha2::Digest;
                sha2::Sha256::digest(login.verifier_for_test().as_bytes())
            })
        );
        assert!(q["state"].len() >= 22);
        assert_eq!(q["state"], login.state_for_test());
        // The verifier itself never travels in the front channel.
        assert!(!login.authorize_url().contains(login.verifier_for_test()));
    }

    #[tokio::test]
    async fn redirect_with_code_exchanges_it_with_the_verifier() {
        let server = MockServer::start().await;
        let auth = auth(&server);
        let login = auth.start_browser_login().await.unwrap();
        let port = redirect_port(&login);
        let probe = login.listeners_probe();
        let state = login.state_for_test().to_string();
        Mock::given(method("POST"))
            .and(path("/token"))
            .and(body_string_contains("grant_type=authorization_code"))
            .and(body_string_contains("client_id=test-id"))
            .and(body_string_contains("code=abc"))
            .and(body_string_contains(format!(
                "code_verifier={}",
                login.verifier_for_test()
            )))
            .and(body_string_contains(format!(
                "redirect_uri=http%3A%2F%2Flocalhost%3A{port}&"
            )))
            .and(body_string_contains("scope=XboxLive.signin+offline_access"))
            .respond_with(token_ok())
            .expect(1)
            .mount(&server)
            .await;
        let finish = tokio::spawn(async move {
            auth.finish_browser_login(login, &pages(), Duration::from_secs(10))
                .await
        });
        let r = callback(port, &format!("code=abc&state={state}")).await;
        assert_eq!(r.status(), 200);
        let body = r.text().await.unwrap();
        assert!(body.contains("SUCCESS-TITLE") && body.contains("SUCCESS-BODY"));
        assert!(!body.contains("FAILURE-TITLE"));
        let tokens = finish.await.unwrap().unwrap();
        assert_eq!(tokens.access_token.expose(), "at");
        assert_eq!(tokens.refresh_token.expose(), "rt");
        assert_closed(&probe).await;
        let body =
            String::from_utf8(server.received_requests().await.unwrap()[0].body.clone()).unwrap();
        assert!(!body.contains("client_secret"), "{body}");
    }

    #[tokio::test]
    async fn responses_carry_the_hardening_headers() {
        let (auth, _server) = auth_with_token_endpoint(1).await;
        let login = auth.start_browser_login().await.unwrap();
        let port = redirect_port(&login);
        let state = login.state_for_test().to_string();
        let finish = tokio::spawn(async move {
            auth.finish_browser_login(login, &pages(), Duration::from_secs(10))
                .await
        });
        let not_found = client()
            .get(format!("http://127.0.0.1:{port}/favicon.ico"))
            .send()
            .await
            .unwrap();
        let page = callback(port, &format!("code=abc&state={state}")).await;
        for (r, content_type) in [
            (&not_found, "text/plain; charset=utf-8"),
            (&page, "text/html; charset=utf-8"),
        ] {
            let h = r.headers();
            assert_eq!(h["content-type"], content_type);
            assert_eq!(h["cache-control"], "no-store");
            assert_eq!(h["referrer-policy"], "no-referrer");
            assert_eq!(h["x-content-type-options"], "nosniff");
            assert_eq!(h["connection"], "close");
            assert!(h["content-security-policy"]
                .to_str()
                .unwrap()
                .starts_with("default-src 'none'"));
        }
        let body = page.text().await.unwrap();
        assert!(!body.to_ascii_lowercase().contains("<script"));
        assert!(!body.contains("abc") && !body.contains(&state));
        assert!(finish.await.unwrap().is_ok());
    }

    #[tokio::test]
    async fn wrong_state_is_ignored_and_login_keeps_waiting() {
        let (auth, _server) = auth_with_token_endpoint(1).await;
        let login = auth.start_browser_login().await.unwrap();
        let port = redirect_port(&login);
        let state = login.state_for_test().to_string();
        let finish = tokio::spawn(async move {
            auth.finish_browser_login(login, &pages(), Duration::from_secs(10))
                .await
        });
        let wrong = callback(port, "code=abc&state=x").await;
        assert_eq!(wrong.status(), 400);
        assert!(!wrong.text().await.unwrap().contains("abc"));
        assert_eq!(
            callback(port, &format!("code=abc&state={state}"))
                .await
                .status(),
            200
        );
        assert!(finish.await.unwrap().is_ok());
    }

    #[tokio::test]
    async fn callback_without_code_or_error_keeps_waiting() {
        let (auth, _server) = auth_with_token_endpoint(1).await;
        let login = auth.start_browser_login().await.unwrap();
        let port = redirect_port(&login);
        let state = login.state_for_test().to_string();
        let finish = tokio::spawn(async move {
            auth.finish_browser_login(login, &pages(), Duration::from_secs(10))
                .await
        });
        assert_eq!(
            callback(port, &format!("state={state}")).await.status(),
            400
        );
        assert_eq!(callback(port, "").await.status(), 400);
        // A parameter given twice is ambiguous and refused.
        assert_eq!(
            callback(port, &format!("code=abc&state={state}&state={state}"))
                .await
                .status(),
            400
        );
        assert_eq!(
            callback(port, &format!("code=abc&state={state}"))
                .await
                .status(),
            200
        );
        assert!(finish.await.unwrap().is_ok());
    }

    #[tokio::test]
    async fn other_paths_get_404() {
        let (auth, _server) = auth_with_token_endpoint(1).await;
        let login = auth.start_browser_login().await.unwrap();
        let port = redirect_port(&login);
        let state = login.state_for_test().to_string();
        let finish = tokio::spawn(async move {
            auth.finish_browser_login(login, &pages(), Duration::from_secs(10))
                .await
        });
        let r = client()
            .get(format!(
                "http://127.0.0.1:{port}/favicon.ico?code=abc&state={state}"
            ))
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 404);
        assert_eq!(r.text().await.unwrap(), "Not Found");
        assert_eq!(
            callback(port, &format!("code=abc&state={state}"))
                .await
                .status(),
            200
        );
        assert!(finish.await.unwrap().is_ok());
    }

    #[tokio::test]
    async fn other_methods_and_broken_requests_are_refused() {
        let (auth, _server) = auth_with_token_endpoint(1).await;
        let login = auth.start_browser_login().await.unwrap();
        let port = redirect_port(&login);
        let state = login.state_for_test().to_string();
        let finish = tokio::spawn(async move {
            auth.finish_browser_login(login, &pages(), Duration::from_secs(10))
                .await
        });
        let post = client()
            .post(format!("http://127.0.0.1:{port}/?code=abc&state={state}"))
            .send()
            .await
            .unwrap();
        assert_eq!(post.status(), 404);
        let broken = raw(port, b"GARBAGE\r\n\r\n").await;
        assert!(broken.starts_with("HTTP/1.1 400 "), "{broken}");
        let no_version = raw(port, b"GET /?code=abc HTTP/9\r\n\r\n").await;
        assert!(no_version.starts_with("HTTP/1.1 400 "), "{no_version}");
        // Too many header lines.
        let mut many = b"GET /?code=abc HTTP/1.1\r\n".to_vec();
        for i in 0..100 {
            many.extend_from_slice(format!("X-{i}: y\r\n").as_bytes());
        }
        many.extend_from_slice(b"\r\n");
        let many = raw(port, &many).await;
        assert!(many.starts_with("HTTP/1.1 400 "), "{many}");
        assert_eq!(
            callback(port, &format!("code=abc&state={state}"))
                .await
                .status(),
            200
        );
        assert!(finish.await.unwrap().is_ok());
    }

    #[tokio::test]
    async fn access_denied_maps_to_login_declined() {
        let (auth, _server) = auth_with_token_endpoint(0).await;
        let login = auth.start_browser_login().await.unwrap();
        let port = redirect_port(&login);
        let probe = login.listeners_probe();
        let state = login.state_for_test().to_string();
        let finish = tokio::spawn(async move {
            auth.finish_browser_login(login, &pages(), Duration::from_secs(10))
                .await
        });
        let r = callback(
            port,
            &format!("error=access_denied&error_description=secret+text&state={state}"),
        )
        .await;
        let body = r.text().await.unwrap();
        assert!(body.contains("FAILURE-TITLE") && body.contains("FAILURE-BODY"));
        assert!(!body.contains("secret text") && !body.contains("secret+text"));
        assert!(!body.contains("access_denied"));
        let err = finish.await.unwrap().unwrap_err();
        assert!(
            matches!(err, Error::Auth(AuthError::LoginDeclined)),
            "{err}"
        );
        assert_closed(&probe).await;
    }

    #[tokio::test]
    async fn other_error_keeps_only_the_code() {
        let err = run_error("error=server_error&error_description=secret+text").await;
        match &err {
            Error::Auth(AuthError::BrowserLoginFailed { code }) => assert_eq!(code, "server_error"),
            other => panic!("unexpected: {other}"),
        }
        assert!(!format!("{err} {err:?}").contains("secret"));
    }

    /// One complete run: the browser comes back with `query` plus the right state.
    async fn run_error(query: &str) -> Error {
        let (auth, _server) = auth_with_token_endpoint(0).await;
        let login = auth.start_browser_login().await.unwrap();
        let port = redirect_port(&login);
        let state = login.state_for_test().to_string();
        let finish = tokio::spawn(async move {
            auth.finish_browser_login(login, &pages(), Duration::from_secs(10))
                .await
        });
        let body = callback(port, &format!("{query}&state={state}"))
            .await
            .text()
            .await
            .unwrap();
        assert!(body.contains("FAILURE-TITLE"));
        finish.await.unwrap().unwrap_err()
    }

    #[tokio::test]
    async fn error_code_is_capped_and_sanitized() {
        let failed = |e: Error| match e {
            Error::Auth(AuthError::BrowserLoginFailed { code }) => code,
            other => panic!("unexpected: {other}"),
        };
        let long = "a".repeat(80);
        assert_eq!(
            failed(run_error(&format!("error={long}")).await),
            "a".repeat(64)
        );
        assert_eq!(failed(run_error("error=a%3Cb%3E%26x").await), "abx");
        assert_eq!(failed(run_error("error=%0D%0A").await), "unknown");
    }

    #[test]
    fn sanitize_keeps_only_safe_characters() {
        assert_eq!(sanitize_code("invalid_request"), "invalid_request");
        assert_eq!(sanitize_code("a.b-c_D9"), "a.b-c_D9");
        assert_eq!(sanitize_code("<script>"), "script");
        assert_eq!(sanitize_code(""), "unknown");
        assert_eq!(sanitize_code("\u{e4}\u{f6}"), "unknown");
        assert_eq!(sanitize_code(&"x".repeat(100)).len(), 64);
    }

    /// One complete run whose code exchange fails with `status` and the OAuth `error`.
    async fn run_failed_exchange(status: u16, error: &str) -> Error {
        let (auth, _server) = auth_with_token_error(status, error).await;
        let login = auth.start_browser_login().await.unwrap();
        let port = redirect_port(&login);
        let probe = login.listeners_probe();
        let state = login.state_for_test().to_string();
        let finish = tokio::spawn(async move {
            auth.finish_browser_login(login, &pages(), Duration::from_secs(10))
                .await
        });
        let body = callback(port, &format!("code=abc&state={state}"))
            .await
            .text()
            .await
            .unwrap();
        assert!(body.contains("FAILURE-TITLE") && !body.contains("SUCCESS-TITLE"));
        let err = finish.await.unwrap().unwrap_err();
        assert!(!format!("{err} {err:?}").contains("secret"), "{err:?}");
        assert_closed(&probe).await;
        err
    }

    #[tokio::test]
    async fn invalid_client_on_exchange_maps_to_app_misconfigured() {
        for status in [400, 401] {
            match run_failed_exchange(status, "invalid_client").await {
                Error::Auth(AuthError::AppMisconfigured { code }) => {
                    assert_eq!(code, "invalid_client")
                }
                other => panic!("unexpected: {other}"),
            }
        }
    }

    #[tokio::test]
    async fn oauth_errors_of_the_exchange_map_to_browser_login_failed() {
        for (error, code) in [
            ("invalid_grant", "invalid_grant"),
            ("invalid_request", "invalid_request"),
            ("<b>bad grant</b>", "bbadgrantb"),
        ] {
            match run_failed_exchange(400, error).await {
                Error::Auth(AuthError::BrowserLoginFailed { code: got }) => assert_eq!(got, code),
                other => panic!("unexpected for {error}: {other}"),
            }
        }
    }

    #[tokio::test]
    async fn page_texts_are_escaped() {
        let (auth, _server) = auth_with_token_endpoint(1).await;
        let login = auth.start_browser_login().await.unwrap();
        let port = redirect_port(&login);
        let state = login.state_for_test().to_string();
        let finish = tokio::spawn(async move {
            let pages = BrowserLoginPages {
                success_title: "<b>&".into(),
                success_body: "\"q\" 'a' <script>x</script>".into(),
                ..pages()
            };
            auth.finish_browser_login(login, &pages, Duration::from_secs(10))
                .await
        });
        let body = callback(port, &format!("code=abc&state={state}"))
            .await
            .text()
            .await
            .unwrap();
        assert!(body.contains("&lt;b&gt;&amp;"), "{body}");
        assert!(
            body.contains("&quot;q&quot; &#39;a&#39; &lt;script&gt;x&lt;/script&gt;"),
            "{body}"
        );
        assert!(!body.contains("<b>") && !body.contains("<script>"));
        assert!(finish.await.unwrap().is_ok());
    }

    #[tokio::test]
    async fn ipv6_localhost_reaches_the_listener_when_available() {
        if std::net::TcpListener::bind(("::1", 0)).is_err() {
            eprintln!("skipped: ::1 cannot be bound on this machine");
            return;
        }
        let (auth, _server) = auth_with_token_endpoint(1).await;
        let login = auth.start_browser_login().await.unwrap();
        let port = redirect_port(&login);
        let probe = login.listeners_probe();
        let state = login.state_for_test().to_string();
        let finish = tokio::spawn(async move {
            auth.finish_browser_login(login, &pages(), Duration::from_secs(10))
                .await
        });
        let r = client()
            .get(format!("http://[::1]:{port}/?code=abc&state={state}"))
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 200);
        assert!(finish.await.unwrap().is_ok());
        assert_closed(&probe).await;
    }

    #[tokio::test]
    async fn listens_on_loopback_only() {
        let (auth, _server) = auth_with_token_endpoint(0).await;
        let login = auth.start_browser_login().await.unwrap();
        let port = redirect_port(&login);
        let v4 = login.listeners.v4.local_addr().unwrap();
        assert_eq!(v4.ip(), Ipv4Addr::LOCALHOST);
        assert_eq!(v4.port(), port);
        if let Some(v6) = &login.listeners.v6 {
            let v6 = v6.local_addr().unwrap();
            assert_eq!(v6.ip(), Ipv6Addr::LOCALHOST);
            assert_eq!(v6.port(), port);
        }
    }

    fn ipv6_available() -> bool {
        std::net::TcpListener::bind(("::1", 0)).is_ok()
    }

    #[tokio::test]
    async fn a_port_taken_on_ipv6_is_given_up_for_a_fresh_one() {
        if !ipv6_available() {
            eprintln!("skipped: ::1 cannot be bound on this machine");
            return;
        }
        // A squatter takes [::1] on exactly the port the first attempt got, before the listener
        // binds it: a real "address in use" from the OS, no luck involved.
        let mut tried = Vec::new();
        let mut squatters = Vec::new();
        let (listeners, port) = bind_loopback(|p| {
            tried.push(p);
            if tried.len() == 1 {
                squatters.push(std::net::TcpListener::bind(("::1", p)));
            }
            TcpListener::bind((Ipv6Addr::LOCALHOST, p))
        })
        .await
        .unwrap();
        assert_eq!(tried.len(), 2, "{tried:?}");
        assert_ne!(port, tried[0]);
        assert_eq!(port, tried[1]);
        assert_eq!(listeners.v4.local_addr().unwrap().port(), port);
        let v6 = listeners
            .v6
            .as_ref()
            .expect("the second port is free on IPv6");
        assert_eq!(v6.local_addr().unwrap().port(), port);
        drop(squatters);
    }

    #[tokio::test]
    async fn every_port_taken_on_ipv6_means_loopback_unavailable() {
        let mut tried = 0;
        let err = bind_loopback(|_| {
            tried += 1;
            async { Err(std::io::Error::from(std::io::ErrorKind::AddrInUse)) }
        })
        .await
        .err()
        .expect("no listeners");
        assert!(
            matches!(err, Error::Auth(AuthError::LoopbackUnavailable)),
            "{err}"
        );
        assert_eq!(tried, BIND_ATTEMPTS);
        // Any other failure counts like a taken port.
        let err = bind_loopback(|_| async {
            Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
        })
        .await
        .err()
        .expect("no listeners");
        assert!(
            matches!(err, Error::Auth(AuthError::LoopbackUnavailable)),
            "{err}"
        );
    }

    #[tokio::test]
    async fn a_machine_without_ipv6_proceeds_on_ipv4_only() {
        let unavailable = [
            std::io::Error::from(std::io::ErrorKind::AddrNotAvailable),
            #[cfg(windows)]
            std::io::Error::from_raw_os_error(10047), // WSAEAFNOSUPPORT
            #[cfg(windows)]
            std::io::Error::from_raw_os_error(10049), // WSAEADDRNOTAVAIL
        ];
        for e in unavailable {
            let kind = e.kind();
            let raw = e.raw_os_error();
            let mut e = Some(e);
            let mut tried = 0;
            let (listeners, port) = bind_loopback(|_| {
                tried += 1;
                let e = e.take().unwrap();
                async move { Err(e) }
            })
            .await
            .unwrap();
            assert_eq!(tried, 1, "{kind:?} {raw:?}");
            assert!(listeners.v6.is_none());
            assert_eq!(listeners.v4.local_addr().unwrap().port(), port);
        }
    }

    #[tokio::test]
    async fn debug_never_shows_verifier_or_state() {
        let (auth, _server) = auth_with_token_endpoint(0).await;
        let login = auth.start_browser_login().await.unwrap();
        let shown = format!("{login:?} {login:#?}");
        assert!(shown.contains(login.redirect_uri()), "{shown}");
        assert!(!shown.contains(login.verifier_for_test()), "{shown}");
        assert!(!shown.contains(login.state_for_test()), "{shown}");
        assert!(!shown.contains("authorize"), "{shown}");
        assert!(
            !shown.contains(&challenge(login.verifier_for_test())),
            "{shown}"
        );
    }

    #[test]
    fn new_errors_never_show_planted_values() {
        // Free text from the URL (a description, markup, line breaks) never survives as such:
        // only the sanitized code reaches the message.
        let planted = "<b>PLANTED SECRET</b>\r\n";
        for e in [
            AuthError::BrowserLoginTimedOut,
            AuthError::LoopbackUnavailable,
            AuthError::BrowserLoginFailed {
                code: sanitize_code(planted),
            },
        ] {
            let e: Error = e.into();
            let shown = format!("{e} {e:?}");
            for banned in ["PLANTED SECRET", "<b>", "\r", "\n"] {
                assert!(!shown.contains(banned), "{shown}");
            }
        }
        assert_eq!(
            AuthError::BrowserLoginTimedOut.to_string(),
            "the browser sign-in was not completed within 5 minutes"
        );
        assert_eq!(
            AuthError::LoopbackUnavailable.to_string(),
            "the launcher cannot listen on localhost for the sign-in answer"
        );
        assert_eq!(
            AuthError::BrowserLoginFailed {
                code: "server_error".into()
            }
            .to_string(),
            "the browser sign-in failed (server_error)"
        );
    }

    #[tokio::test]
    async fn oversized_request_is_dropped() {
        let (auth, _server) = auth_with_token_endpoint(1).await;
        let login = auth.start_browser_login().await.unwrap();
        let port = redirect_port(&login);
        let state = login.state_for_test().to_string();
        let finish = tokio::spawn(async move {
            auth.finish_browser_login(login, &pages(), Duration::from_secs(10))
                .await
        });
        let mut big = format!("GET /?code=abc&state={state} HTTP/1.1\r\n").into_bytes();
        big.extend_from_slice(b"X-Fill: ");
        big.extend(std::iter::repeat_n(b'a', 9 * 1024));
        let answer = raw(port, &big).await;
        assert_eq!(answer, "", "an oversized request gets no answer");
        assert_eq!(
            callback(port, &format!("code=abc&state={state}"))
                .await
                .status(),
            200
        );
        assert!(finish.await.unwrap().is_ok());
    }

    #[tokio::test]
    async fn times_out_without_redirect() {
        let (auth, _server) = auth_with_token_endpoint(0).await;
        let login = auth.start_browser_login().await.unwrap();
        let probe = login.listeners_probe();
        let err = auth
            .finish_browser_login(login, &pages(), Duration::from_millis(200))
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Auth(AuthError::BrowserLoginTimedOut)));
        assert_closed(&probe).await;
    }

    #[tokio::test]
    async fn wrong_state_is_rejected_and_the_wait_continues_until_timeout() {
        let (auth, _server) = auth_with_token_endpoint(0).await;
        let login = auth.start_browser_login().await.unwrap();
        let port = redirect_port(&login);
        let probe = login.listeners_probe();
        let finish = tokio::spawn(async move {
            auth.finish_browser_login(login, &pages(), Duration::from_secs(5))
                .await
        });
        let first = callback(port, "code=abc&state=wrong").await;
        let second = callback(port, "code=abc").await;
        assert_eq!(first.status(), 400);
        assert_eq!(second.status(), 400);
        // Only the deadline ends the wait with a timeout: a rejected callback that ended it
        // would show up as another result.
        let err = finish.await.unwrap().unwrap_err();
        assert!(matches!(err, Error::Auth(AuthError::BrowserLoginTimedOut)));
        assert_closed(&probe).await;
    }

    #[tokio::test]
    async fn second_callback_is_refused() {
        let gate = gated_token_endpoint(
            200,
            json!({"access_token": "at", "refresh_token": "rt", "expires_in": 3600}),
        )
        .await;
        let auth = gate.auth;
        let login = auth.start_browser_login().await.unwrap();
        let port = redirect_port(&login);
        let state = login.state_for_test().to_string();
        let finish = tokio::spawn(async move {
            auth.finish_browser_login(login, &pages(), Duration::from_secs(60))
                .await
        });
        let first = tokio::spawn({
            let s = state.clone();
            async move { callback(port, &format!("code=abc&state={s}")).await }
        });
        // The exchange is running and held until the release below.
        tokio::time::timeout(Duration::from_secs(30), gate.received)
            .await
            .unwrap()
            .unwrap();
        let second = callback(port, &format!("code=other&state={state}")).await;
        assert_eq!(second.status(), 400, "only one valid callback is accepted");
        gate.release.send(()).unwrap();
        assert_eq!(first.await.unwrap().status(), 200);
        assert!(finish.await.unwrap().is_ok());
        assert_eq!(gate.calls.load(Ordering::SeqCst), 1, "exactly one exchange");
    }

    #[tokio::test]
    async fn connections_after_the_end_are_refused() {
        let (auth, _server) = auth_with_token_endpoint(1).await;
        let login = auth.start_browser_login().await.unwrap();
        let port = redirect_port(&login);
        let probe = login.listeners_probe();
        let state = login.state_for_test().to_string();
        let finish = tokio::spawn(async move {
            auth.finish_browser_login(login, &pages(), Duration::from_secs(10))
                .await
        });
        assert_eq!(
            callback(port, &format!("code=abc&state={state}"))
                .await
                .status(),
            200
        );
        assert!(finish.await.unwrap().is_ok());
        // The listening sockets are gone the moment the login returns, so nothing more can
        // connect to this login.
        assert!(probe.upgrade().is_none());
    }

    #[tokio::test]
    async fn success_page_is_sent_only_after_the_exchange() {
        let gate = gated_token_endpoint(
            400,
            json!({"error": "invalid_grant", "error_description": "secret detail"}),
        )
        .await;
        let auth = gate.auth;
        let login = auth.start_browser_login().await.unwrap();
        let port = redirect_port(&login);
        let probe = login.listeners_probe();
        let state = login.state_for_test().to_string();
        let finish = tokio::spawn(async move {
            auth.finish_browser_login(login, &pages(), Duration::from_secs(60))
                .await
        });
        let mut browser = tokio::spawn(async move {
            callback(port, &format!("code=abc&state={state}"))
                .await
                .text()
                .await
                .unwrap()
        });
        tokio::time::timeout(Duration::from_secs(30), gate.received)
            .await
            .unwrap()
            .unwrap();
        // While the exchange is held, the browser gets no answer. (A wrong implementation can
        // only pass this by luck, never fail it by load.)
        assert!(
            tokio::time::timeout(Duration::from_millis(200), &mut browser)
                .await
                .is_err(),
            "the browser answer waits for the token exchange"
        );
        gate.release.send(()).unwrap();
        let body = browser.await.unwrap();
        assert!(body.contains("FAILURE-TITLE") && !body.contains("SUCCESS-TITLE"));
        let err = finish.await.unwrap().unwrap_err();
        match &err {
            Error::Auth(AuthError::BrowserLoginFailed { code }) => {
                assert_eq!(code, "invalid_grant")
            }
            other => panic!("unexpected: {other}"),
        }
        assert!(!format!("{err} {err:?}").contains("secret"));
        assert_closed(&probe).await;
    }

    #[tokio::test]
    async fn stalled_connection_does_not_block_the_real_callback() {
        let (auth, _server) = auth_with_token_endpoint(1).await;
        let login = auth.start_browser_login().await.unwrap();
        let port = redirect_port(&login);
        let state = login.state_for_test().to_string();
        let finish = tokio::spawn(async move {
            auth.finish_browser_login(login, &pages(), Duration::from_secs(10))
                .await
        });
        // More open connections that send nothing than the limit of 32.
        let mut stalled = Vec::new();
        for _ in 0..40 {
            stalled.push(TcpStream::connect(("127.0.0.1", port)).await.unwrap());
        }
        // The read timeout of the stalled connections is 5 s; the real redirect must not wait
        // for it (a generous bound below that, not a speed measurement).
        let r = tokio::time::timeout(
            READ_TIMEOUT - Duration::from_millis(500),
            callback(port, &format!("code=abc&state={state}")),
        )
        .await
        .expect("the stalled connections must not delay the redirect");
        assert_eq!(r.status(), 200);
        assert!(finish.await.unwrap().is_ok());
        drop(stalled);
    }

    #[tokio::test]
    async fn slow_client_is_cut_off_after_the_read_timeout() {
        let (auth, _server) = auth_with_token_endpoint(0).await;
        let login = auth.start_browser_login().await.unwrap();
        let port = redirect_port(&login);
        let probe = login.listeners_probe();
        let finish = tokio::spawn(async move {
            auth.finish_browser_login(login, &pages(), Duration::from_secs(30))
                .await
        });
        let mut slow = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        slow.write_all(b"GET /?code=abc HTTP/1.1\r\n")
            .await
            .unwrap();
        let started = std::time::Instant::now();
        let mut buf = Vec::new();
        let read = tokio::time::timeout(Duration::from_secs(15), slow.read_to_end(&mut buf)).await;
        assert!(read.is_ok(), "the server closes a stalled connection");
        assert!(buf.is_empty(), "no answer to an incomplete request");
        assert!(started.elapsed() >= Duration::from_secs(4));
        assert!(!finish.is_finished(), "a slow client does not end the wait");
        finish.abort();
        let _ = finish.await;
        assert_closed(&probe).await;
    }

    fn head_with_host(host: Option<&str>, extra: &str) -> Vec<u8> {
        let mut r = String::from("GET /favicon.ico HTTP/1.1\r\n");
        if let Some(host) = host {
            r.push_str(&format!("Host: {host}\r\n"));
        }
        r.push_str(extra);
        r.push_str("\r\n");
        r.into_bytes()
    }

    #[tokio::test]
    async fn only_loopback_host_headers_with_the_right_port_are_served() {
        let (auth, _server) = auth_with_token_endpoint(1).await;
        let login = auth.start_browser_login().await.unwrap();
        let port = redirect_port(&login);
        let state = login.state_for_test().to_string();
        let finish = tokio::spawn(async move {
            auth.finish_browser_login(login, &pages(), Duration::from_secs(30))
                .await
        });
        // Accepted: answered for what the request is (404 for this path).
        for host in [
            format!("localhost:{port}"),
            format!("LocalHost:{port}"),
            format!("127.0.0.1:{port}"),
            format!("[::1]:{port}"),
            format!(" localhost:{port} "),
        ] {
            let answer = raw(port, &head_with_host(Some(&host), "")).await;
            assert!(answer.starts_with("HTTP/1.1 404 "), "{host}: {answer}");
        }
        let lower_case_name =
            format!("GET /favicon.ico HTTP/1.1\r\nhost: localhost:{port}\r\n\r\n");
        let answer = raw(port, lower_case_name.as_bytes()).await;
        assert!(answer.starts_with("HTTP/1.1 404 "), "{answer}");
        // Refused with the uniform 400.
        let other_port = port.wrapping_add(1);
        for (host, extra) in [
            (Some(format!("evil.example:{port}")), String::new()),
            (
                Some(format!("localhost.evil.example:{port}")),
                String::new(),
            ),
            (Some(format!("localhost:{other_port}")), String::new()),
            (Some(format!("localhost:0{port}")), String::new()),
            (Some("localhost".to_string()), String::new()),
            (Some(format!("::1:{port}")), String::new()),
            (Some(format!("user@localhost:{port}")), String::new()),
            (None, String::new()),
            (
                Some(format!("localhost:{port}")),
                format!("Host: localhost:{port}\r\n"),
            ),
        ] {
            let answer = raw(port, &head_with_host(host.as_deref(), &extra)).await;
            assert!(
                answer.starts_with("HTTP/1.1 400 "),
                "{host:?} {extra:?}: {answer}"
            );
            assert!(answer.ends_with("Bad Request"), "{answer}");
        }
        // A valid callback under a foreign host is refused and does not use up the state.
        let foreign =
            format!("GET /?code=abc&state={state} HTTP/1.1\r\nHost: evil.example:{port}\r\n\r\n");
        let answer = raw(port, foreign.as_bytes()).await;
        assert!(answer.starts_with("HTTP/1.1 400 "), "{answer}");
        assert_eq!(
            callback(port, &format!("code=abc&state={state}"))
                .await
                .status(),
            200
        );
        assert!(finish.await.unwrap().is_ok());
    }

    #[tokio::test]
    async fn a_connection_reset_before_accept_arrives_as_a_dead_socket() {
        // Observed on Windows 11: a client that connects and resets (SO_LINGER 0) before the
        // listener accepts is still accepted; its first read fails with ConnectionReset. A
        // platform that reports it from accept instead must report a per-connection kind.
        let l = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let s = TcpStream::connect(l.local_addr().unwrap()).await.unwrap();
        s.set_zero_linger().unwrap();
        drop(s);
        match tokio::time::timeout(Duration::from_secs(10), l.accept())
            .await
            .expect("the reset connection is reported")
        {
            Ok((mut c, _)) => {
                let mut buf = [0u8; 8];
                assert!(matches!(c.read(&mut buf).await, Err(_) | Ok(0)));
            }
            Err(e) => assert!(per_connection(&e), "{:?} {:?}", e.kind(), e.raw_os_error()),
        }
    }

    #[tokio::test]
    async fn connections_reset_before_accept_do_not_disturb_the_login() {
        let (auth, _server) = auth_with_token_endpoint(1).await;
        let login = auth.start_browser_login().await.unwrap();
        let port = redirect_port(&login);
        let state = login.state_for_test().to_string();
        let finish = tokio::spawn(async move {
            auth.finish_browser_login(login, &pages(), Duration::from_secs(30))
                .await
        });
        for _ in 0..50 {
            let s = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
            s.set_zero_linger().unwrap();
            drop(s);
        }
        let r = tokio::time::timeout(
            Duration::from_secs(10),
            callback(port, &format!("code=abc&state={state}")),
        )
        .await
        .expect("resets do not hold up the redirect");
        assert_eq!(r.status(), 200);
        assert!(finish.await.unwrap().is_ok());
    }

    /// The login's listeners with scripted accept errors: `script(n)` for the n-th accept (from
    /// 0) returns the error to report instead of waiting for a connection.
    struct Scripted<F> {
        listeners: Arc<Listeners>,
        script: F,
        calls: Arc<std::sync::atomic::AtomicUsize>,
    }

    impl<F: Fn(usize) -> Option<std::io::Error> + Send + Sync> Acceptor for Scripted<F> {
        async fn accept(&self) -> std::io::Result<TcpStream> {
            let n = self.calls.fetch_add(1, Ordering::SeqCst);
            if let Some(e) = (self.script)(n) {
                // A real accept error comes back through the reactor; so does this one.
                tokio::task::yield_now().await;
                return Err(e);
            }
            self.listeners.accept().await
        }
    }

    struct ScriptedRun {
        finish: tokio::task::JoinHandle<Result<MsaTokens>>,
        calls: Arc<std::sync::atomic::AtomicUsize>,
        port: u16,
        state: String,
        _server: MockServer,
    }

    /// Runs `finish` with scripted accept errors in front of the real listeners.
    async fn finish_scripted<F>(script: F, timeout: Duration, token_calls: u64) -> ScriptedRun
    where
        F: Fn(usize) -> Option<std::io::Error> + Send + Sync + 'static,
    {
        let (auth, server) = auth_with_token_endpoint(token_calls).await;
        let login = auth.start_browser_login().await.unwrap();
        let port = redirect_port(&login);
        let state = login.state_for_test().to_string();
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counted = calls.clone();
        let finish = tokio::spawn(async move {
            auth.finish_with(login, &pages(), timeout, move |listeners| Scripted {
                listeners,
                script,
                calls: counted,
            })
            .await
        });
        ScriptedRun {
            finish,
            calls,
            port,
            state,
            _server: server,
        }
    }

    fn resource_error() -> std::io::Error {
        if cfg!(windows) {
            std::io::Error::from_raw_os_error(10024) // WSAEMFILE
        } else {
            std::io::Error::other("too many open files")
        }
    }

    #[tokio::test]
    async fn a_burst_of_reset_errors_does_not_delay_the_callback() {
        let burst = 400;
        let run = finish_scripted(
            move |n| {
                (n < burst).then(|| {
                    std::io::Error::from(if n % 2 == 0 {
                        std::io::ErrorKind::ConnectionReset
                    } else {
                        std::io::ErrorKind::ConnectionAborted
                    })
                })
            },
            Duration::from_secs(60),
            1,
        )
        .await;
        // With a pause per error this would take burst * 50 ms = 20 s.
        let r = tokio::time::timeout(
            Duration::from_secs(5),
            callback(run.port, &format!("code=abc&state={}", run.state)),
        )
        .await
        .expect("per-connection accept errors must not pause accepting");
        assert_eq!(r.status(), 200);
        assert!(run.finish.await.unwrap().is_ok());
        assert!(run.calls.load(Ordering::SeqCst) > burst);
    }

    #[tokio::test]
    async fn endless_reset_errors_do_not_stop_the_deadline() {
        let run = finish_scripted(
            |_| Some(std::io::Error::from(std::io::ErrorKind::ConnectionReset)),
            Duration::from_millis(300),
            0,
        )
        .await;
        let err = tokio::time::timeout(Duration::from_secs(10), run.finish)
            .await
            .expect("the deadline fires")
            .unwrap()
            .unwrap_err();
        assert!(
            matches!(err, Error::Auth(AuthError::BrowserLoginTimedOut)),
            "{err}"
        );
        assert!(run.calls.load(Ordering::SeqCst) > 1);
    }

    #[tokio::test]
    async fn a_resource_error_pauses_accepting_but_the_deadline_still_fires() {
        let started = tokio::time::Instant::now();
        let run = finish_scripted(|_| Some(resource_error()), Duration::from_millis(300), 0).await;
        let err = tokio::time::timeout(Duration::from_secs(10), run.finish)
            .await
            .expect("the deadline fires during the pauses")
            .unwrap()
            .unwrap_err();
        assert!(
            matches!(err, Error::Auth(AuthError::BrowserLoginTimedOut)),
            "{err}"
        );
        // About one accept per pause, never a tight loop (an upper bound: load cannot break it).
        let n = run.calls.load(Ordering::SeqCst);
        let max = (started.elapsed().as_millis() / ACCEPT_BACKOFF.as_millis()) as usize + 2;
        assert!(n >= 2 && n <= max, "{n} accepts, at most {max} expected");
    }

    #[tokio::test]
    async fn accepting_resumes_after_a_resource_error() {
        let run =
            finish_scripted(|n| (n < 3).then(resource_error), Duration::from_secs(60), 1).await;
        let r = tokio::time::timeout(
            Duration::from_secs(10),
            callback(run.port, &format!("code=abc&state={}", run.state)),
        )
        .await
        .expect("accepting resumes");
        assert_eq!(r.status(), 200);
        assert!(run.finish.await.unwrap().is_ok());
    }

    #[test]
    fn only_per_connection_errors_skip_the_pause() {
        use std::io::{Error as IoError, ErrorKind};
        for kind in [
            ErrorKind::ConnectionReset,
            ErrorKind::ConnectionAborted,
            ErrorKind::Interrupted,
        ] {
            assert!(per_connection(&IoError::from(kind)), "{kind:?}");
        }
        assert!(!per_connection(&resource_error()));
        assert!(!per_connection(&IoError::other("unknown")));
        #[cfg(windows)]
        for raw in [10053, 10054] {
            assert!(per_connection(&IoError::from_raw_os_error(raw)), "{raw}");
        }
        #[cfg(windows)]
        for raw in [10024, 10055] {
            assert!(!per_connection(&IoError::from_raw_os_error(raw)), "{raw}");
        }
    }

    #[tokio::test]
    async fn dropping_finish_frees_the_port() {
        let (auth, _server) = auth_with_token_endpoint(0).await;
        let login = auth.start_browser_login().await.unwrap();
        let port = redirect_port(&login);
        let probe = login.listeners_probe();
        let finish = tokio::spawn(async move {
            auth.finish_browser_login(login, &pages(), Duration::from_secs(300))
                .await
        });
        // An answered request proves the wait is running; a held-open connection must not keep
        // the port either. The listener was bound by `start_browser_login`, before the address
        // existed, so the kernel queues this connection even if the wait loop has not been
        // scheduled yet: a refused or reset first request is NOT a race this code can explain.
        // The test therefore asks once, and a failure prints what happened (error kind, message,
        // the whole source chain) instead of being retried away.
        let answered = tokio::time::timeout(Duration::from_secs(20), async {
            client()
                .get(format!("http://127.0.0.1:{port}/?code=abc&state=wrong"))
                .send()
                .await
        })
        .await
        .expect("the wait loop answers within 20 s");
        let answered = match answered {
            Ok(response) => response.status(),
            Err(e) => {
                let mut chain = String::new();
                let mut source: Option<&(dyn std::error::Error + 'static)> = Some(&e);
                while let Some(err) = source {
                    chain.push_str(&format!(
                        "
  caused by: {err} ({err:?})"
                    ));
                    source = err.source();
                }
                println!(
                    "first callback request failed: connect={} timeout={} request={} body={}{chain}",
                    e.is_connect(),
                    e.is_timeout(),
                    e.is_request(),
                    e.is_body(),
                );
                panic!(
                    "the first callback request to the bound port failed (unexplained, see the output above): {e}"
                );
            }
        };
        assert_eq!(answered, 400);
        let _held = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        finish.abort(); // cancelled like login_cancel
        let _ = finish.await;
        assert_closed(&probe).await; // a new listener can bind the port right away
    }

    #[tokio::test]
    async fn dropping_an_unfinished_login_frees_the_port() {
        let (auth, _server) = auth_with_token_endpoint(0).await;
        let login = auth.start_browser_login().await.unwrap();
        let probe = login.listeners_probe();
        drop(login);
        assert_closed(&probe).await;
    }
}
