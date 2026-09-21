/// A tiny smoke test for every Rust integration surface.
pub fn hello(name: &str) -> String {
    format!("Hello, {name}, from Rust!")
}

pub const SESSION_COOKIE: &str = "beats-user";

#[derive(Debug, PartialEq, Eq)]
pub enum ProxyDecision {
    Continue,
    Redirect { pathname: &'static str },
}

/// Evaluate the auth gate shared by the Next proxy and the Rust front door.
pub fn proxy_decision(pathname: &str, cookie_header: Option<&str>) -> ProxyDecision {
    let is_authed = cookie_header.is_some_and(|header| has_cookie(header, SESSION_COOKIE));

    if pathname != "/login" && !is_authed {
        ProxyDecision::Redirect { pathname: "/login" }
    } else {
        ProxyDecision::Continue
    }
}

fn has_cookie(header: &str, name: &str) -> bool {
    header.split(';').any(|cookie| {
        cookie
            .trim()
            .split_once('=')
            .is_some_and(|(cookie_name, value)| cookie_name == name && !value.is_empty())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn says_hello() {
        assert_eq!(hello("Next.js"), "Hello, Next.js, from Rust!");
    }

    #[test]
    fn redirects_anonymous_page_requests() {
        assert_eq!(
            proxy_decision("/library", None),
            ProxyDecision::Redirect { pathname: "/login" }
        );
    }

    #[test]
    fn permits_login_and_authenticated_requests() {
        assert_eq!(proxy_decision("/login", None), ProxyDecision::Continue);
        assert_eq!(
            proxy_decision("/library", Some("theme=dark; beats-user=user-1")),
            ProxyDecision::Continue
        );
    }

    #[test]
    fn does_not_accept_cookie_name_prefixes_or_empty_values() {
        assert!(matches!(
            proxy_decision("/", Some("not-beats-user=user-1; beats-user=")),
            ProxyDecision::Redirect { .. }
        ));
    }
}
