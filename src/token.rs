//! An automatically incrementing token.

use std::sync::atomic::{AtomicUsize, Ordering};

pub struct Token {
   next: AtomicUsize,
}

impl Token {
   pub const fn new(initial_value: usize) -> Self {
      Self {
         next: AtomicUsize::new(initial_value),
      }
   }

   pub fn next(&self) -> usize {
      self.next.fetch_add(1, Ordering::Relaxed)
   }
}
