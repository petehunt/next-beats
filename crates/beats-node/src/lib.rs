use napi_derive::napi;

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
