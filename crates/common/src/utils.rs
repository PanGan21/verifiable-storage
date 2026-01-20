/// Get current timestamp in milliseconds since Unix epoch
/// Used to ensure each signature is unique, even for identical requests
pub fn get_current_timestamp_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}
