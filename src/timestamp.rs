pub fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

const TIMESTAMP_TOLERANCE_SECS: u64 = 900; // 15 minutes

/// Returns true if the timestamp is within ±5 minutes of server time.
pub fn is_timestamp_fresh(ts: u64) -> bool {
    let now = current_timestamp();
    now.abs_diff(ts) <= TIMESTAMP_TOLERANCE_SECS
}
