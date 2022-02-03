//! Configurable, framerate-independent timer for use in `process()`.

use instant::{Duration, Instant};

/// A framerate-independent timer.
pub struct Timer {
   interval: i64,
   last_tick: Instant,
   lag: i64,
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

   /// Sets the timer up such that `update()` can be called to process ticks.
   pub fn tick(&mut self) {
      let now = Instant::now();
      let elapsed = now - self.last_tick;
      self.last_tick = now;
      self.lag += elapsed.as_micros() as i64;
   }

   /// Processes a single tick, and returns whether there are more ticks to be done.
   pub fn update(&mut self) -> bool {
      let requires_update = self.lag >= self.interval;
      if requires_update {
         self.lag -= self.interval;
         true
      } else {
         false
      }
   }
}
