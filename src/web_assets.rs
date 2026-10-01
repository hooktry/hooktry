use axum::{
    body::Body,
    http::{
        HeaderValue, Method, StatusCode, Uri,
        header::{CACHE_CONTROL, CONTENT_TYPE},
    },
    response::Response,
};

pub(crate) struct EmbeddedAsset {
    path: &'static str,
    bytes: &'static [u8],
}

include!(concat!(env!("OUT_DIR"), "/embedded_web_assets.rs"));

pub async fn fallback(method: Method, uri: Uri) -> Response {
    if method != Method::GET && method != Method::HEAD {
        return status_response(StatusCode::NOT_FOUND);
    }

    let path = uri.path().trim_start_matches('/');
    if path.is_empty() {
        return asset_response("index.html", method == Method::HEAD)
            .unwrap_or_else(|| status_response(StatusCode::INTERNAL_SERVER_ERROR));
    }

    if let Some(response) = asset_response(path, method == Method::HEAD) {
        return response;
    }

    if path.starts_with("assets/") {
        return status_response(StatusCode::NOT_FOUND);
    }

    asset_response("index.html", method == Method::HEAD)
        .unwrap_or_else(|| status_response(StatusCode::INTERNAL_SERVER_ERROR))
}

pub fn index_response() -> Response {
    asset_response("index.html", false)
        .unwrap_or_else(|| status_response(StatusCode::INTERNAL_SERVER_ERROR))
}

fn asset_response(path: &str, head_only: bool) -> Option<Response> {
    let asset = EMBEDDED_WEB_ASSETS
        .iter()
        .find(|asset| asset.path == path)?;

    let mut response = Response::new(if head_only {
        Body::empty()
    } else {
        Body::from(asset.bytes)
    });
    *response.status_mut() = StatusCode::OK;

    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static(content_type(path)),
    );
    response.headers_mut().insert(
        CACHE_CONTROL,
        HeaderValue::from_static(if path == "index.html" {
            "no-store, max-age=0"
        } else if path.starts_with("assets/") {
            "public, max-age=31536000, immutable"
        } else {
            "no-cache"
        }),
    );
    response.headers_mut().insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    response.headers_mut().insert(
        "referrer-policy",
        HeaderValue::from_static("no-referrer"),
    );
    response.headers_mut().insert(
        "content-security-policy",
        HeaderValue::from_static(
            "default-src 'self'; script-src 'self'; style-src 'self'; \
             img-src 'self' data:; connect-src 'self' ws: wss:; \
             base-uri 'none'; frame-ancestors 'none'",
        ),
    );

    Some(response)
}

fn content_type(path: &str) -> &'static str {
    match path.rsplit_once('.').map(|(_, extension)| extension) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") | Some("map") => "application/json; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("ico") => "image/x-icon",
        Some("woff2") => "font/woff2",
        Some("txt") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn status_response(status: StatusCode) -> Response {
    Response::builder()
        .status(status)
        .body(Body::empty())
        .expect("status response is valid")
}

#[cfg(test)]
mod tests {
    use axum::http::{Method, Uri};
    use http_body_util::BodyExt;

    use super::{EMBEDDED_WEB_ASSETS, fallback, index_response};

    #[tokio::test]
    async fn embedded_index_is_the_shared_react_shell() {
        let response = index_response();
        assert_eq!(response.status(), 200);
        assert_eq!(
            response.headers()["content-type"],
            "text/html; charset=utf-8"
        );
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let html = String::from_utf8(body.to_vec()).unwrap();
        assert!(html.contains(r#"<div id="root"></div>"#));
    }

    #[tokio::test]
    async fn spa_fallback_serves_index_but_missing_hashed_assets_are_not_html() {
        let spa = fallback(Method::GET, Uri::from_static("/view/vw_example")).await;
        assert_eq!(spa.status(), 200);

        let missing = fallback(
            Method::GET,
            Uri::from_static("/assets/definitely-missing.js"),
        )
        .await;
        assert_eq!(missing.status(), 404);
    }

    #[test]
    fn binary_contains_compiled_web_assets() {
        assert!(
            EMBEDDED_WEB_ASSETS
                .iter()
                .any(|asset| asset.path.starts_with("assets/"))
        );
    }
}
