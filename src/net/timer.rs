//! Configurable, framerate-independent timer for use in `process()`.

use std::time::{Duration, Instant};

/// A framerate-independent timer.
pub struct Timer {
   interval: i64,
   last_tick: Instant,
   lag: i64,
}

/// An iterator over ticks since the last time the timer was ticked.
pub struct Tick<'a> {
   timer: &'a mut Timer,
}

impl Timer {
   /// Creates a new timer with the given tick interval.
   pub fn new(interval: Duration) -> Self {
      Self {
         interval: interval.as_micros() as i64,
         last_tick: Instant::now(),
         lag: Default::default(),
      }
   }

   /// Returns an iterator which will execute as many times as necessary to maintain a roughly
   /// constant amount of time (specified by the timer's interval) between subsequent calls to
   /// `tick`.
   pub fn tick<'a>(&'a mut self) -> Tick<'a> {
      let now = Instant::now();
      let elapsed = now - self.last_tick;
      self.last_tick = now;
      self.lag += elapsed.as_micros() as i64;
      Tick { timer: self }
   }
}

impl Iterator for Tick<'_> {
   type Item = ();

   fn next(&mut self) -> Option<Self::Item> {
      let requires_update = self.timer.lag >= self.timer.interval;
      if requires_update {
         self.timer.lag -= self.timer.interval;
         Some(())
      } else {
         None
      }
   }
}
