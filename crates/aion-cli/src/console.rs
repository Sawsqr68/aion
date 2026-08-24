//! Shared ops-console plumbing: the browsable URL for a bound HTTP listener,
//! liveness probing against `/health/live`, and the platform browser opener.
//!
//! Two commands share this seam: the bare `aion` launcher (which probes before
//! spawning and again while waiting for the spawned server) and
//! `aion server --open` (which waits for its own in-process listener). One
//! implementation, so the two cannot drift on what "up" means: an HTTP 200
//! from `/health/live`, the unauthenticated probe the production router
//! always mounts — not a bare TCP accept, which any process holding the port
//! can produce.

use std::net::SocketAddr;
use std::time::Duration;

use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

/// One classified answer from probing `GET /health/live` at an address.
#[derive(Debug)]
pub enum HealthProbe {
    /// The address answered HTTP 200 on `/health/live`: a live Aion server.
    Live,
    /// Something accepted the TCP connection but did not answer 200: the port
    /// is held by a process that is not a healthy Aion server. Carries the
    /// response's status line when one was readable.
    NotAion(Option<String>),
    /// Nothing accepted a connection at the address.
    Down,
}

/// Timeout for each connect/write/read step of a single probe. Loopback
/// answers in microseconds; this bounds the pathological cases (a listener
/// that accepts and then hangs) without slowing the healthy path. Shared
/// with `aion server status`, whose one-shot probes bound the same
/// pathology — one truth for "how long may a single local probe step take".
pub(crate) const PROBE_STEP_TIMEOUT: Duration = Duration::from_secs(2);

/// Interval between probes while waiting for a listener to come up.
const POLL_INTERVAL: Duration = Duration::from_millis(200);

/// The one liveness budget every waiting caller shares: how long a first
/// boot may take to answer `/health/live` before the waiter gives up.
/// Generous, because a first boot creates the store and installs the
/// embedded assistant package before the listener mounts — and the same
/// number for the bare launcher and `aion server --open`, so the two paths
/// cannot judge the same boot differently.
pub const LIVE_BUDGET: Duration = Duration::from_secs(60);

/// Probe `GET /health/live` at `address` once and classify the answer.
pub async fn probe_health(address: SocketAddr) -> HealthProbe {
    let Ok(Ok(mut stream)) =
        tokio::time::timeout(PROBE_STEP_TIMEOUT, tokio::net::TcpStream::connect(address)).await
    else {
        return HealthProbe::Down;
    };
    let request = b"GET /health/live HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
    match tokio::time::timeout(PROBE_STEP_TIMEOUT, stream.write_all(request)).await {
        Ok(Ok(())) => {}
        // Accepted the connection but would not take an HTTP request.
        Ok(Err(_)) | Err(_) => return HealthProbe::NotAion(None),
    }
    let mut response = Vec::new();
    match tokio::time::timeout(PROBE_STEP_TIMEOUT, stream.read_to_end(&mut response)).await {
        Ok(Ok(_)) => {}
        // Accepted the request but never answered (or hung past the timeout).
        Ok(Err(_)) | Err(_) => return HealthProbe::NotAion(None),
    }
    classify_response(&response)
}

/// Classify raw response bytes from the probe.
fn classify_response(response: &[u8]) -> HealthProbe {
    let text = String::from_utf8_lossy(response);
    let status_line = text.lines().next().unwrap_or_default().trim();
    if status_line.starts_with("HTTP/1.1 200") || status_line.starts_with("HTTP/1.0 200") {
        return HealthProbe::Live;
    }
    if status_line.is_empty() {
        return HealthProbe::NotAion(None);
    }
    // Something answered, but not with a liveness 200 — quote what it said,
    // bounded so a garbage (non-HTTP) first line cannot flood a message.
    let mut quoted: String = status_line.chars().take(120).collect();
    if quoted.len() < status_line.len() {
        quoted.push('…');
    }
    HealthProbe::NotAion(Some(quoted))
}

/// Poll `/health/live` until it answers 200 or the budget elapses.
///
/// Only [`HealthProbe::Live`] ends the wait early: a `NotAion` answer during
/// startup is indistinguishable from a server still mounting its router, so
/// the loop keeps polling until the budget decides.
pub async fn wait_until_live(address: SocketAddr, budget: Duration) -> bool {
    let started = std::time::Instant::now();
    loop {
        if matches!(probe_health(address).await, HealthProbe::Live) {
            return true;
        }
        if started.elapsed() >= budget {
            return false;
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

/// The browsable URL. A wildcard bind (`0.0.0.0`/`::`) is not browsable as-is,
/// so rewrite it to loopback.
pub fn served_url(address: SocketAddr) -> String {
    let host = if address.ip().is_unspecified() {
        if address.is_ipv6() {
            "[::1]".to_owned()
        } else {
            "127.0.0.1".to_owned()
        }
    } else if address.is_ipv6() {
        format!("[{}]", address.ip())
    } else {
        address.ip().to_string()
    };
    format!("http://{host}:{}/", address.port())
}

/// Launch the platform browser-opener for `url`.
///
/// # Errors
///
/// Returns the spawn error when the platform opener cannot be started; the
/// caller decides how to report it (both callers treat it as best-effort).
pub fn open_browser(url: &str) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    let program = "open";
    #[cfg(target_os = "windows")]
    let program = "explorer";
    #[cfg(all(unix, not(target_os = "macos")))]
    let program = "xdg-open";

    std::process::Command::new(program)
        .arg(url)
        .spawn()
        .map(drop)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn served_url_rewrites_wildcard_binds_to_loopback() -> Result<(), std::net::AddrParseError> {
        assert_eq!(
            served_url("0.0.0.0:8080".parse()?),
            "http://127.0.0.1:8080/"
        );
        assert_eq!(served_url("[::]:9000".parse()?), "http://[::1]:9000/");
        assert_eq!(
            served_url("192.168.1.7:80".parse()?),
            "http://192.168.1.7:80/"
        );
        assert_eq!(
            served_url("[fe80::1]:8080".parse()?),
            "http://[fe80::1]:8080/"
        );
        Ok(())
    }

    #[test]
    fn probe_classification_reads_the_status_line() -> Result<(), String> {
        match classify_response(b"HTTP/1.1 200 OK\r\ncontent-length: 0\r\n\r\n") {
            HealthProbe::Live => {}
            other => return Err(format!("a 200 must classify Live, got {other:?}")),
        }
        match classify_response(b"HTTP/1.0 200 OK\r\n\r\n") {
            HealthProbe::Live => {}
            other => return Err(format!("an HTTP/1.0 200 must classify Live, got {other:?}")),
        }
        match classify_response(b"HTTP/1.1 404 Not Found\r\n\r\n") {
            HealthProbe::NotAion(Some(line)) if line == "HTTP/1.1 404 Not Found" => {}
            other => return Err(format!("a 404 must quote its status line, got {other:?}")),
        }
        match classify_response(b"") {
            HealthProbe::NotAion(None) => {}
            other => return Err(format!("an empty answer carries no quote, got {other:?}")),
        }
        match classify_response(b"not http at all\r\n") {
            HealthProbe::NotAion(Some(line)) if line == "not http at all" => {}
            other => return Err(format!("garbage must be quoted as-is, got {other:?}")),
        }
        let long = [b'x'; 200];
        match classify_response(&long) {
            HealthProbe::NotAion(Some(line))
                if line.chars().count() == 121 && line.ends_with('…') => {}
            other => return Err(format!("a long answer must be truncated, got {other:?}")),
        }
        Ok(())
    }
}
