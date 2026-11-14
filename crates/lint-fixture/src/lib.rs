//! Test-only fixture for the workspace lint policy (spec FR-009, FR-011, SC-008).
//!
//! The clean code below must pass `cargo clippy -D warnings`. Each `lint_fixture_*`
//! cfg enables exactly one deliberate violation; `la-core/tests/lint_policy.rs`
//! enables them one at a time and expects the lint run to fail. The cfgs are never
//! set in normal builds, so this crate is always clean in CI.

/// Clean production code: no panicking constructs.
#[must_use]
pub fn first_or_zero(values: &[u32]) -> u32 {
    values.first().copied().unwrap_or(0)
}

/// Clean production code: checked arithmetic.
#[must_use]
pub fn checked_sum(a: u32, b: u32) -> Option<u32> {
    a.checked_add(b)
}

#[cfg(lint_fixture_unwrap)]
pub fn violation_unwrap(v: Option<u32>) -> u32 {
    v.unwrap()
}

#[cfg(lint_fixture_expect)]
pub fn violation_expect(v: Option<u32>) -> u32 {
    v.expect("value")
}

#[cfg(lint_fixture_index)]
pub fn violation_index(v: &[u32]) -> u32 {
    v[0]
}

#[cfg(lint_fixture_arith)]
pub fn violation_arith(a: u32, b: u32) -> u32 {
    a + b
}

#[cfg(lint_fixture_panic)]
pub fn violation_panic() {
    panic!("boom");
}

#[cfg(lint_fixture_unsafe)]
pub fn violation_unsafe() -> u8 {
    let x = 1_u8;
    unsafe { std::ptr::read(&x) }
}

// Test code is allowed to use panicking constructs.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tests_may_use_panicking_constructs() {
        let v = vec![1_u32, 2];
        assert_eq!(v[0], 1);
        assert_eq!(v.get(1).copied().unwrap(), 2);
        assert_eq!(first_or_zero(&v), 1);
        assert_eq!(checked_sum(1, 2).expect("sum"), 3);
        if v.len() > 5 {
            panic!("unreachable in practice");
        }
    }
}
