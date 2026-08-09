//! Plain-data Streamable-HTTP shapes.
//!
//! The dispatcher consumes an [`HttpRequest`] and produces an [`HttpResponse`];
//! adapting those to a concrete HTTP framework is the host's job and is a
//! dozen lines. Keeping the framework out of this crate is what lets the whole
//! transport contract be tested without binding a socket.

use serde_json::Value;

use crate::protocol::headers::HeaderView;

/// One inbound HTTP request to the MCP endpoint.
#[derive(Clone, Debug)]
pub struct HttpRequest {
    /// The HTTP method, uppercase (`POST`, `GET`, `DELETE`, …).
    pub method: String,
    /// Request headers, in any casing.
    pub headers: HeaderView,
    /// The raw request body.
    pub body: Vec<u8>,
}

impl HttpRequest {
    /// Build a request.
    #[must_use]
    pub fn new(method: impl Into<String>, headers: HeaderView, body: Vec<u8>) -> Self {
        Self {
            method: method.into(),
            headers,
            body,
        }
    }

    /// The `Origin` header, if the client sent one.
    #[must_use]
    pub fn origin(&self) -> Option<&str> {
        self.headers.get("origin")
    }
}

/// One outbound HTTP response from the MCP endpoint.
#[derive(Clone, Debug)]
pub struct HttpResponse {
    /// The HTTP status code.
    pub status: u16,
    /// Response headers to set, as `(name, value)` pairs.
    pub headers: Vec<(String, String)>,
    /// The response body, empty for a `202 Accepted` acknowledgement.
    pub body: Vec<u8>,
}

impl HttpResponse {
    /// A JSON response.
    ///
    /// A serialization failure cannot be papered over with a JSON body (the
    /// serializer is what just failed), so it degrades to a `500` with a plain
    /// text body rather than risking a second failure inside the error path.
    #[must_use]
    pub fn json(status: u16, body: &Value) -> Self {
        match serde_json::to_vec(body) {
            Ok(bytes) => Self {
                status,
                headers: vec![("content-type".to_owned(), "application/json".to_owned())],
                body: bytes,
            },
            Err(error) => Self {
                status: 500,
                headers: vec![("content-type".to_owned(), "text/plain".to_owned())],
                body: format!("the MCP response could not be serialized: {error}").into_bytes(),
            },
        }
    }

    /// A body-less response.
    #[must_use]
    pub fn empty(status: u16) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    /// A `405 Method Not Allowed` naming the one method the endpoint accepts.
    ///
    /// GET and DELETE were the standalone-SSE-stream and session-termination
    /// verbs of the handshake era; both were removed in this revision, so the
    /// endpoint answers POST and nothing else.
    #[must_use]
    pub fn method_not_allowed() -> Self {
        Self {
            status: 405,
            headers: vec![("allow".to_owned(), "POST".to_owned())],
            body: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{HeaderView, HttpRequest, HttpResponse};

    #[test]
    fn a_json_response_sets_its_content_type() {
        let response = HttpResponse::json(200, &json!({ "ok": true }));
        assert_eq!(response.status, 200);
        assert_eq!(
            response.headers,
            vec![("content-type".to_owned(), "application/json".to_owned())]
        );
    }

    #[test]
    fn method_not_allowed_names_post() {
        let response = HttpResponse::method_not_allowed();
        assert_eq!(response.status, 405);
        assert_eq!(
            response.headers,
            vec![("allow".to_owned(), "POST".to_owned())]
        );
        assert!(response.body.is_empty());
    }

    #[test]
    fn the_origin_header_is_read_case_insensitively() {
        let request = HttpRequest::new(
            "POST",
            HeaderView::new([("Origin", "http://localhost:8080")]),
            Vec::new(),
        );
        assert_eq!(request.origin(), Some("http://localhost:8080"));
    }
}
