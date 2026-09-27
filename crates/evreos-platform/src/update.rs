//! FR-014: the browser updating itself.

pub mod wake;

use self::wake::Wake;

/// The update check's wake, as `budgets.toml` states it: the check is armed
/// under this entry and no other, every `period_seconds`, and one check may
/// use at most `processor_time_bound_ms` of processor time.
pub const UPDATE_CHECK_WAKE: Wake = include!(concat!(env!("OUT_DIR"), "/update_wake.rs"));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_compiled_wake_is_the_files() {
        let budgets = include_str!("../../../budgets.toml");
        assert_eq!(
            Ok(UPDATE_CHECK_WAKE),
            wake::read(budgets, wake::UPDATE_CHECK)
        );
    }
}
