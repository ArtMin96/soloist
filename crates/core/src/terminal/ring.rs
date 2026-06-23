//! A bounded FIFO ring used for terminal scrollback.
//!
//! Every buffer in a long-running supervisor needs a ceiling: a chatty process must
//! never grow memory without limit. [`Ring`] keeps at most `cap` most-recent items,
//! dropping the oldest as new ones arrive.

use std::collections::VecDeque;

/// A bounded queue that retains the most recent `cap` items, discarding the oldest
/// once full. `cap` is clamped to at least one so the ring is never degenerate.
pub struct Ring<T> {
    cap: usize,
    items: VecDeque<T>,
}

impl<T> Ring<T> {
    /// Creates a ring holding at most `cap` items (clamped to ≥ 1).
    pub fn new(cap: usize) -> Self {
        let cap = cap.max(1);
        Self {
            cap,
            items: VecDeque::with_capacity(cap.min(1024)),
        }
    }

    /// Appends an item, evicting the oldest if the ring is at capacity.
    pub fn push(&mut self, item: T) {
        if self.items.len() == self.cap {
            self.items.pop_front();
        }
        self.items.push_back(item);
    }

    /// Iterates over retained items, oldest first.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.items.iter()
    }

    /// Iterates over the most recent `n` retained items, oldest first (fewer than `n` if
    /// the ring holds fewer). Reads the tail without copying the whole buffer.
    pub fn tail(&self, n: usize) -> impl Iterator<Item = &T> {
        let skip = self.items.len().saturating_sub(n);
        self.items.iter().skip(skip)
    }

    /// Discards every retained item, leaving the ring empty (its capacity unchanged).
    pub fn clear(&mut self) {
        self.items.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evicts_oldest_when_over_capacity() {
        let mut ring = Ring::new(3);
        for n in 1..=5 {
            ring.push(n);
        }
        // The two oldest (1, 2) were dropped; the ring never exceeds its cap.
        assert_eq!(ring.iter().copied().collect::<Vec<_>>(), vec![3, 4, 5]);
    }

    #[test]
    fn a_zero_cap_is_clamped_to_one() {
        let mut ring = Ring::new(0);
        ring.push("a");
        ring.push("b");
        assert_eq!(ring.iter().copied().collect::<Vec<_>>(), vec!["b"]);
    }
}
