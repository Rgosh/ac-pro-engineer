use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// A thread-safe, efficient bounded ring buffer using VecDeque.
/// Push and pop operations are O(1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RingBuffer<T> {
    buffer: VecDeque<T>,
    capacity: usize,
}

impl<T> RingBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        Self {
            buffer: VecDeque::with_capacity(cap),
            capacity: cap,
        }
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Resize capacity dynamically, preserving up to `new_cap` newest elements.
    pub fn set_capacity(&mut self, new_cap: usize) {
        let new_cap = new_cap.max(1);
        self.capacity = new_cap;
        while self.buffer.len() > new_cap {
            self.buffer.pop_front();
        }
    }

    /// Push an element to the buffer in O(1) time.
    /// If full, drops the oldest element.
    pub fn push(&mut self, item: T) {
        if self.buffer.len() >= self.capacity {
            self.buffer.pop_front();
        }
        self.buffer.push_back(item);
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    pub fn last(&self) -> Option<&T> {
        self.buffer.back()
    }

    pub fn first(&self) -> Option<&T> {
        self.buffer.front()
    }

    pub fn iter(&self) -> std::collections::vec_deque::Iter<'_, T> {
        self.buffer.iter()
    }

    pub fn as_slices(&self) -> (&[T], &[T]) {
        self.buffer.as_slices()
    }
}

impl<T> std::ops::Index<usize> for RingBuffer<T> {
    type Output = T;
    fn index(&self, index: usize) -> &Self::Output {
        &self.buffer[index]
    }
}

impl<T> Default for RingBuffer<T> {
    fn default() -> Self {
        Self::new(300)
    }
}

impl<'a, T> IntoIterator for &'a RingBuffer<T> {
    type Item = &'a T;
    type IntoIter = std::collections::vec_deque::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.buffer.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_buffer_bounds_capacity() {
        let mut buf = RingBuffer::new(3);
        buf.push(1);
        buf.push(2);
        buf.push(3);
        assert_eq!(buf.len(), 3);
        assert_eq!(buf[0], 1);
        assert_eq!(buf[2], 3);

        buf.push(4); // drops 1
        assert_eq!(buf.len(), 3);
        assert_eq!(buf[0], 2);
        assert_eq!(buf[2], 4);
    }

    #[test]
    fn ring_buffer_dynamic_capacity_change() {
        let mut buf = RingBuffer::new(5);
        for i in 1..=5 {
            buf.push(i);
        }
        assert_eq!(buf.len(), 5);

        // Shrink capacity to 3 -> retains [3, 4, 5]
        buf.set_capacity(3);
        assert_eq!(buf.len(), 3);
        assert_eq!(buf[0], 3);
        assert_eq!(buf[2], 5);

        // Expand capacity to 10
        buf.set_capacity(10);
        assert_eq!(buf.capacity(), 10);
        assert_eq!(buf.len(), 3);
        buf.push(6);
        assert_eq!(buf.len(), 4);
        assert_eq!(buf[3], 6);
    }
}
