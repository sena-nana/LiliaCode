use std::time::{SystemTime, UNIX_EPOCH};

/// 当前墙钟毫秒。失败回 0——只有系统时钟出问题时才会发生，业务侧不必特别处理。
pub fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
