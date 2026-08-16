use pulseseek_domain::playback::navigation::{next_index, prev_index};

// Empty queue
#[test]
fn empty_queue_next() {
    assert_eq!(next_index(0, 0), None);
}

#[test]
fn empty_queue_prev() {
    assert_eq!(prev_index(0, 0), None);
}

// Single-item
#[test]
fn single_item_next() {
    assert_eq!(next_index(0, 1), None);
}

#[test]
fn single_item_prev() {
    assert_eq!(prev_index(0, 1), None);
}

// First item in 3-item queue
#[test]
fn first_item_next() {
    assert_eq!(next_index(0, 3), Some(1));
}

#[test]
fn first_item_prev() {
    assert_eq!(prev_index(0, 3), None);
}

// Middle item in 3-item queue
#[test]
fn middle_item_next() {
    assert_eq!(next_index(1, 3), Some(2));
}

#[test]
fn middle_item_prev() {
    assert_eq!(prev_index(1, 3), Some(0));
}

// Last item in 3-item queue
#[test]
fn last_item_next() {
    assert_eq!(next_index(2, 3), None);
}

#[test]
fn last_item_prev() {
    assert_eq!(prev_index(2, 3), Some(1));
}

// Current item index >= total (item removed from queue)
#[test]
fn removed_item_next() {
    assert_eq!(next_index(3, 3), None);
}

#[test]
fn removed_item_prev() {
    assert_eq!(prev_index(3, 3), Some(2));
}
