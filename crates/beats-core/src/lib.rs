/// A tiny smoke test for every Rust integration surface.
pub fn hello(name: &str) -> String {
    format!("Hello, {name}, from Rust!")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn says_hello() {
        assert_eq!(hello("Next.js"), "Hello, Next.js, from Rust!");
    }
}
