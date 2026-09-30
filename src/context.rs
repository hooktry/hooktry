use axum::http::HeaderMap;

use crate::domain::{CorrelationContext, InteractionContext};

const REQUEST_ID_HEADERS: &[&str] = &["x-request-id", "request-id"];
const CORRELATION_ID_HEADERS: &[&str] = &["x-correlation-id", "correlation-id"];
const CAUSATION_ID_HEADERS: &[&str] = &["x-causation-id", "causation-id"];
const MESSAGE_ID_HEADERS: &[&str] = &["x-message-id", "message-id"];
const IDEMPOTENCY_KEY_HEADERS: &[&str] = &["idempotency-key", "x-idempotency-key"];

pub fn from_http_headers(headers: &HeaderMap) -> InteractionContext {
    let (trace_id, parent_span_id) = headers
        .get("traceparent")
        .and_then(|value| value.to_str().ok())
        .and_then(parse_traceparent)
        .map(|(trace_id, parent_span_id)| (Some(trace_id), Some(parent_span_id)))
        .unwrap_or((None, None));

    InteractionContext {
        correlation: CorrelationContext {
            trace_id,
            parent_span_id,
            request_id: first_header(headers, REQUEST_ID_HEADERS),
            correlation_id: first_header(headers, CORRELATION_ID_HEADERS),
            causation_id: first_header(headers, CAUSATION_ID_HEADERS),
            message_id: first_header(headers, MESSAGE_ID_HEADERS),
            idempotency_key: first_header(headers, IDEMPOTENCY_KEY_HEADERS),
        },
        attributes: Default::default(),
    }
}

fn first_header(headers: &HeaderMap, names: &[&str]) -> Option<String> {
    names.iter().find_map(|name| {
        headers
            .get(*name)
            .and_then(|value| value.to_str().ok())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
    })
}

fn parse_traceparent(value: &str) -> Option<(String, String)> {
    let value = value.trim();
    let mut parts = value.split('-');

    let version = parts.next()?;
    let trace_id = parts.next()?;
    let parent_id = parts.next()?;
    let trace_flags = parts.next()?;

    if parts.next().is_some()
        || version != "00"
        || !valid_lower_hex(version, 2)
        || !valid_lower_hex(trace_id, 32)
        || !valid_lower_hex(parent_id, 16)
        || !valid_lower_hex(trace_flags, 2)
        || all_zero(trace_id)
        || all_zero(parent_id)
    {
        return None;
    }

    Some((trace_id.to_owned(), parent_id.to_owned()))
}

fn valid_lower_hex(value: &str, len: usize) -> bool {
    value.len() == len
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn all_zero(value: &str) -> bool {
    value.bytes().all(|byte| byte == b'0')
}

#[cfg(test)]
mod tests {
    use axum::http::{HeaderMap, HeaderValue};

    use super::from_http_headers;

    #[test]
    fn extracts_supported_http_identifiers_with_deterministic_precedence() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "traceparent",
            HeaderValue::from_static("00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"),
        );
        headers.insert("x-request-id", HeaderValue::from_static("request-primary"));
        headers.insert("request-id", HeaderValue::from_static("request-fallback"));
        headers.insert(
            "x-correlation-id",
            HeaderValue::from_static("correlation-123"),
        );
        headers.insert("x-causation-id", HeaderValue::from_static("cause-456"));
        headers.insert("x-message-id", HeaderValue::from_static("message-789"));
        headers.insert("idempotency-key", HeaderValue::from_static("payment-42"));

        let context = from_http_headers(&headers);

        assert_eq!(
            context.correlation.trace_id.as_deref(),
            Some("4bf92f3577b34da6a3ce929d0e0e4736")
        );
        assert_eq!(
            context.correlation.parent_span_id.as_deref(),
            Some("00f067aa0ba902b7")
        );
        assert_eq!(
            context.correlation.request_id.as_deref(),
            Some("request-primary")
        );
        assert_eq!(
            context.correlation.correlation_id.as_deref(),
            Some("correlation-123")
        );
        assert_eq!(
            context.correlation.causation_id.as_deref(),
            Some("cause-456")
        );
        assert_eq!(
            context.correlation.message_id.as_deref(),
            Some("message-789")
        );
        assert_eq!(
            context.correlation.idempotency_key.as_deref(),
            Some("payment-42")
        );
        assert!(context.attributes.is_empty());
    }

    #[test]
    fn invalid_traceparent_is_not_promoted_to_normalized_context() {
        for value in [
            "00-00000000000000000000000000000000-00f067aa0ba902b7-01",
            "00-4bf92f3577b34da6a3ce929d0e0e4736-0000000000000000-01",
            "00-4BF92F3577B34DA6A3CE929D0E0E4736-00f067aa0ba902b7-01",
            "ff-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
            "00-too-short-00f067aa0ba902b7-01",
        ] {
            let mut headers = HeaderMap::new();
            headers.insert("traceparent", HeaderValue::from_str(value).unwrap());

            let context = from_http_headers(&headers);

            assert!(context.correlation.trace_id.is_none(), "{value}");
            assert!(context.correlation.parent_span_id.is_none(), "{value}");
        }
    }

    #[test]
    fn does_not_promote_unlisted_or_sensitive_headers() {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", HeaderValue::from_static("Bearer secret"));
        headers.insert("stripe-signature", HeaderValue::from_static("signature"));
        headers.insert("x-custom-id", HeaderValue::from_static("custom"));

        let context = from_http_headers(&headers);

        assert!(context.is_empty());
    }
}
