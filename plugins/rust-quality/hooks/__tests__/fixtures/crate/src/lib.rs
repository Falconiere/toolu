//! Library root with inline tests (VIOLATION).
//!
//! # Public API
//! Adds two numbers.

pub fn add(a: u32, b: u32) -> u32 {
    a + b
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn it_adds() {
        assert_eq!(add(1, 2), 3);
    }
}
