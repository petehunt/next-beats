use napi_derive::napi;
use tokio::sync::OnceCell;

static APP: OnceCell<beats_core::App> = OnceCell::const_new();

#[napi]
pub fn hello(name: String) -> String {
    beats_core::hello(&name)
}

#[napi(object)]
pub struct ProxyResult {
    pub redirect_pathname: Option<String>,
}

#[napi]
pub fn proxy_decision(pathname: String, cookie_header: Option<String>) -> ProxyResult {
    let redirect_pathname = match beats_core::proxy_decision(&pathname, cookie_header.as_deref()) {
        beats_core::ProxyDecision::Continue => None,
        beats_core::ProxyDecision::Redirect { pathname } => Some(pathname.to_owned()),
    };

    ProxyResult { redirect_pathname }
}

#[napi(object)]
pub struct PlayResult {
    pub status: u32,
    pub revalidation_tags: Vec<String>,
}

#[napi]
pub async fn handle_play(
    database_url: String,
    cookie_header: Option<String>,
    body: String,
) -> napi::Result<PlayResult> {
    let app = APP
        .get_or_try_init(|| async { beats_core::App::connect(&database_url).await })
        .await
        .map_err(|error| napi::Error::from_reason(error.to_string()))?;
    let outcome = app
        .handle_play(cookie_header.as_deref(), body.as_bytes())
        .await
        .map_err(|error| napi::Error::from_reason(error.to_string()))?;

    Ok(PlayResult {
        status: outcome.status.into(),
        revalidation_tags: outcome.revalidation_tags,
    })
}
