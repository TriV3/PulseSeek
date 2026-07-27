/// Returns the index of the next track in queue, if any.
///
/// Returns `None` when the queue is empty or the current item is the last.
pub fn next_index(current: usize, total: usize) -> Option<usize> {
    let next = current + 1;
    if next < total {
        Some(next)
    } else {
        None
    }
}

/// Returns the index of the previous track in queue, if any.
///
/// Returns `None` when the queue is empty or already at the first item.
/// Clamps to the last valid index when `current` exceeds `total` (item removed).
pub fn prev_index(current: usize, total: usize) -> Option<usize> {
    if total == 0 || current == 0 {
        return None;
    }
    if current >= total {
        Some(total - 1)
    } else {
        Some(current - 1)
    }
}
