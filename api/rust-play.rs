use std::{env, sync::OnceLock};

use beats_core::App;
use http_body_util::BodyExt;
use tokio::sync::OnceCell;
use vercel_runtime::{Error, Request, Response, ResponseBody, run, service_fn};

const MAX_BODY_BYTES: usize = 64 * 1024;
static APP: OnceCell<App> = OnceCell::const_new();
static DATABASE_URL: OnceLock<String> = OnceLock::new();

#[tokio::main]
async fn main() -> Result<(), Error> {
    run(service_fn(handler)).await
}

async fn handler(request: Request) -> Result<Response<ResponseBody>, Error> {
    if request.method().as_str() != "POST" {
        return response(405);
    }
    let cookie = request
        .headers()
        .get("cookie")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let body = request.into_body().collect().await?.to_bytes();
    if body.len() > MAX_BODY_BYTES {
        return response(413);
    }

    let database_url = DATABASE_URL.get_or_init(|| {
        env::var("DATABASE_URL").expect("DATABASE_URL must be configured for the Rust function")
    });
    let app = APP
        .get_or_try_init(|| async { App::connect_lazy(database_url) })
        .await?;
    match app.handle_play(cookie.as_deref(), &body).await {
        Ok(outcome) => response(outcome.status),
        Err(error) => {
            eprintln!("play mutation failed: {error}");
            response(500)
        }
    }
}

fn response(status: u16) -> Result<Response<ResponseBody>, Error> {
    Ok(Response::builder()
        .status(status)
        .body(ResponseBody::from(()))?)
}
