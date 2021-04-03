// configurable, framerate-independent timer for use in process()

use std::time::{Duration, Instant};

pub struct Timer {
    interval: i64,
    last_tick: Instant,
    lag: i64,
}

pub struct Tick<'a> {
    timer: &'a mut Timer,
}

impl Timer {

    pub fn new(interval: Duration) -> Self {
        Self {
            interval: interval.as_micros() as i64,
            last_tick: Instant::now(),
            lag: Default::default(),
        }
    }

    // returns a ticking iterator
    pub fn tick<'a>(&'a mut self) -> Tick<'a> {
        let now = Instant::now();
        let elapsed = now - self.last_tick;
        self.last_tick = now;
        self.lag += elapsed.as_micros() as i64;
        Tick {
            timer: self,
        }
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

