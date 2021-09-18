//! An automatically incrementing token.

use std::marker::PhantomData;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct Token {
    next: AtomicUsize,
}

impl Token {
    pub const fn new() -> Self {
        Self {
            next: AtomicUsize::new(0),
        }
    }

    pub fn next(&self) -> usize {
        self.next.fetch_add(1, Ordering::Relaxed)
    }
}
