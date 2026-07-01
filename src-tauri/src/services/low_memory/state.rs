use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static LOW_MEMORY_MODE: AtomicBool = AtomicBool::new(false);
static USER_REQUESTED_EXIT: AtomicBool = AtomicBool::new(false);
static AUTO_MANAGER_STARTED: AtomicBool = AtomicBool::new(false);
static LAST_WINDOW_ACTIVITY_AT_MS: AtomicU64 = AtomicU64::new(0);
static EXITING_LOW_MEMORY: AtomicBool = AtomicBool::new(false);

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

pub fn is_low_memory_mode() -> bool {
    LOW_MEMORY_MODE.load(Ordering::SeqCst)
}

pub fn set_low_memory_mode(active: bool) {
    LOW_MEMORY_MODE.store(active, Ordering::SeqCst);
}

pub fn mark_window_activity() {
    LAST_WINDOW_ACTIVITY_AT_MS.store(now_unix_ms(), Ordering::SeqCst);
}

pub fn last_window_activity_at_ms() -> u64 {
    LAST_WINDOW_ACTIVITY_AT_MS.load(Ordering::SeqCst)
}

pub fn init_window_activity_timestamp() {
    mark_window_activity();
}

/// Pure decision for the auto low-memory idle watcher. Returns true only when
/// the feature is enabled, no tracked window is currently visible, activity has
/// been recorded at least once (`last_activity_ms != 0`), and the elapsed idle
/// time has reached the threshold.
pub fn should_enter_low_memory(
    enabled: bool,
    any_window_visible: bool,
    last_activity_ms: u64,
    now_ms: u64,
    idle_threshold_ms: u64,
) -> bool {
    if !enabled || any_window_visible || last_activity_ms == 0 {
        return false;
    }

    now_ms.saturating_sub(last_activity_ms) >= idle_threshold_ms
}

pub fn try_mark_auto_manager_started() -> bool {
    !AUTO_MANAGER_STARTED.swap(true, Ordering::SeqCst)
}

// 标记用户主动请求退出
pub fn set_user_requested_exit(requested: bool) {
    USER_REQUESTED_EXIT.store(requested, Ordering::SeqCst);
}

// 检查是否是用户主动请求退出
pub fn is_user_requested_exit() -> bool {
    USER_REQUESTED_EXIT.load(Ordering::SeqCst)
}

// 尝试开始退出低占用模式（防止并发退出）
pub fn try_start_exit_low_memory() -> bool {
    !EXITING_LOW_MEMORY.swap(true, Ordering::SeqCst)
}

// 完成退出低占用模式
pub fn finish_exit_low_memory() {
    EXITING_LOW_MEMORY.store(false, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_enter_when_hidden_and_idle_reaches_threshold() {
        assert!(should_enter_low_memory(true, false, 1_000, 62_000, 60_000));
    }

    #[test]
    fn should_not_enter_when_window_visible() {
        assert!(!should_enter_low_memory(true, true, 1_000, 61_000, 60_000));
    }

    #[test]
    fn should_not_enter_when_disabled() {
        assert!(!should_enter_low_memory(false, false, 1_000, 61_000, 60_000));
    }

    #[test]
    fn should_not_enter_without_recorded_activity() {
        assert!(!should_enter_low_memory(true, false, 0, 30_000, 60_000));
    }

    #[test]
    fn should_enter_at_threshold_boundary_only() {
        assert!(should_enter_low_memory(true, false, 1_000, 61_000, 60_000));
        assert!(!should_enter_low_memory(true, false, 1_000, 60_999, 60_000));
    }
}
