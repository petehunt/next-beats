use napi_derive::napi;

#[napi]
pub fn hello(name: String) -> String {
    beats_core::hello(&name)
}
