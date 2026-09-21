use std::{env, net::SocketAddr, sync::Arc};

use axum::{
    Router,
    body::{Body, to_bytes},
    extract::{ConnectInfo, Request, State},
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode, Uri, header},
    response::{IntoResponse, Response},
    routing::{get, get_service, post},
};
use reqwest::Client;
use serde::Serialize;
use tower_http::{
    services::{ServeDir, ServeFile},
    set_header::SetResponseHeaderLayer,
};

const MAX_PLAY_BODY_BYTES: usize = 64 * 1024;
const LOCAL_INTERNAL_TOKEN: &str = "local-development-only";

#[derive(Clone)]
struct FrontState {
    app: beats_core::App,
    client: Client,
    internal_token: String,
    next_origin: String,
}

#[derive(Serialize)]
struct RevalidateRequest<'a> {
    tags: &'a [String],
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    load_env();

    let database_url = env::var("DATABASE_URL")?;
    let next_origin = env::var("BEATS_NEXT_ORIGIN")
        .unwrap_or_else(|_| "http://127.0.0.1:3001".to_owned())
        .trim_end_matches('/')
        .to_owned();
    let internal_token = env::var("BEATS_INTERNAL_TOKEN").unwrap_or_else(|_| {
        if env::var("NODE_ENV").as_deref() == Ok("production") {
            panic!("BEATS_INTERNAL_TOKEN is required in production");
        }
        LOCAL_INTERNAL_TOKEN.to_owned()
    });
    let port = env::var("PORT")
        .unwrap_or_else(|_| "3000".to_owned())
        .parse::<u16>()?;

    let state = Arc::new(FrontState {
        app: beats_core::App::connect_lazy(&database_url)?,
        client: Client::builder().build()?,
        internal_token,
        next_origin,
    });
    let immutable = SetResponseHeaderLayer::overriding(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=31536000, immutable"),
    );
    let static_files = Router::new()
        .nest_service(
            "/covers",
            ServeDir::new("public/covers").append_index_html_on_directories(false),
        )
        .route_service("/logo.svg", get_service(ServeFile::new("public/logo.svg")))
        .layer(immutable);
    let router = Router::new()
        .route("/_rust/health", get(health))
        .route("/api/play", post(play))
        .merge(static_files)
        .fallback(proxy_to_next)
        .with_state(state);
    let address = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(address).await?;

    eprintln!("Rust front door listening on http://{address}");
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;
    Ok(())
}

fn load_env() {
    let _ = dotenvy::from_filename(".env");
    let _ = dotenvy::from_filename_override(".env.local");
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("install Ctrl-C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
}

async fn health() -> impl IntoResponse {
    (StatusCode::OK, "Hello from Rust!\n")
}

async fn play(State(state): State<Arc<FrontState>>, request: Request) -> Response {
    let cookie = request
        .headers()
        .get(header::COOKIE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let body = match to_bytes(request.into_body(), MAX_PLAY_BODY_BYTES).await {
        Ok(body) => body,
        Err(_) => return StatusCode::PAYLOAD_TOO_LARGE.into_response(),
    };
    let outcome = match state.app.handle_play(cookie.as_deref(), &body).await {
        Ok(outcome) => outcome,
        Err(error) => {
            eprintln!("play mutation failed: {error}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    if !outcome.revalidation_tags.is_empty()
        && let Err(error) = revalidate(&state, &outcome.revalidation_tags).await
    {
        // The mutation already committed. Returning success avoids double-counting
        // a play if a client retries solely because the cache adapter was down.
        eprintln!("Next cache revalidation failed: {error}");
    }

    StatusCode::from_u16(outcome.status)
        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
        .into_response()
}

async fn revalidate(state: &FrontState, tags: &[String]) -> Result<(), reqwest::Error> {
    state
        .client
        .post(format!("{}/api/_rust/revalidate", state.next_origin))
        .header("x-beats-internal-token", &state.internal_token)
        .json(&RevalidateRequest { tags })
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

async fn proxy_to_next(
    State(state): State<Arc<FrontState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    request: Request,
) -> Response {
    if request.uri().path().starts_with("/api/_rust/") {
        return StatusCode::NOT_FOUND.into_response();
    }

    if should_auth_gate(request.uri().path()) {
        let cookie = request
            .headers()
            .get(header::COOKIE)
            .and_then(|value| value.to_str().ok());
        if let beats_core::ProxyDecision::Redirect { pathname } =
            beats_core::proxy_decision(request.uri().path(), cookie)
        {
            let location = match request.uri().query() {
                Some(query) => format!("{pathname}?{query}"),
                None => pathname.to_owned(),
            };
            return (
                StatusCode::TEMPORARY_REDIRECT,
                [(header::LOCATION, location)],
            )
                .into_response();
        }
    }

    match forward(&state, peer, request).await {
        Ok(response) => response,
        Err(error) => {
            eprintln!("Next origin request failed: {error}");
            StatusCode::BAD_GATEWAY.into_response()
        }
    }
}

fn should_auth_gate(pathname: &str) -> bool {
    let path = pathname.trim_start_matches('/');
    !["_next", "api", "icon", "favicon"]
        .iter()
        .any(|excluded| path.starts_with(excluded))
}

async fn forward(
    state: &FrontState,
    peer: SocketAddr,
    request: Request,
) -> Result<Response, reqwest::Error> {
    let (parts, body) = request.into_parts();
    let upstream_url = format!("{}{}", state.next_origin, path_and_query(&parts.uri));
    let original_host = parts
        .headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let mut upstream = state.client.request(parts.method, upstream_url);
    for (name, value) in &parts.headers {
        if !is_hop_by_hop(name) {
            upstream = upstream.header(name, value);
        }
    }
    if let Some(host) = original_host {
        upstream = upstream.header("x-forwarded-host", host);
    }
    upstream = upstream
        .header("x-forwarded-for", peer.ip().to_string())
        .header("x-forwarded-proto", "http")
        .body(reqwest::Body::wrap_stream(body.into_data_stream()));

    let upstream_response = upstream.send().await?;
    let status = upstream_response.status();
    let headers = upstream_response.headers().clone();
    let mut response = Response::new(Body::from_stream(upstream_response.bytes_stream()));
    *response.status_mut() = status;
    copy_response_headers(&headers, response.headers_mut());
    Ok(response)
}

fn path_and_query(uri: &Uri) -> &str {
    uri.path_and_query().map_or("/", |value| value.as_str())
}

fn copy_response_headers(source: &HeaderMap, destination: &mut HeaderMap) {
    for (name, value) in source {
        if !is_hop_by_hop(name) {
            destination.append(name, value.clone());
        }
    }
}

fn is_hop_by_hop(name: &HeaderName) -> bool {
    matches!(
        name.as_str(),
        "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_gate_matches_the_next_proxy_scope() {
        assert!(should_auth_gate("/library"));
        assert!(!should_auth_gate("/_next/static/chunk.js"));
        assert!(!should_auth_gate("/api/play"));
        assert!(!should_auth_gate("/icon"));
        assert!(!should_auth_gate("/favicon.ico"));
    }

    #[test]
    fn recognizes_hop_by_hop_headers() {
        assert!(is_hop_by_hop(&header::CONNECTION));
        assert!(!is_hop_by_hop(&header::CONTENT_TYPE));
    }
}
