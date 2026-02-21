pub fn is_cancelled(_task_id: &str) -> bool {
    false
}

pub fn set_total(_task_id: &str, _total: Option<u64>) {}

pub fn update_progress(
    _task_id: &str,
    _delta: u64,
    _speed: Option<u64>,
    _message: Option<&str>,
) {
}
