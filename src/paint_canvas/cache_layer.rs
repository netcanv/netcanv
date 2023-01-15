use std::collections::HashMap;

use instant::{Duration, Instant};

use super::cached_chunk::CachedChunk;

pub struct CacheLayer {
   chunks: HashMap<(i32, i32), CachedChunk>,
   chunk_cache_timers: HashMap<(i32, i32), Instant>,
}

impl CacheLayer {
   /// The duration for which encoded chunk images are held in memory.
   /// Once this duration expires, the cached images are dropped.
   const CHUNK_CACHE_DURATION: Duration = Duration::from_secs(5 * 60);

   pub fn new() -> Self {
      CacheLayer {
         chunks: HashMap::new(),
         chunk_cache_timers: HashMap::new(),
      }
   }

   pub fn chunk(&mut self, position: (i32, i32)) -> Option<&CachedChunk> {
      self.chunk_cache_timers.insert(position, Instant::now());
      self.chunks.get(&position)
   }

   pub fn set_chunk(&mut self, position: (i32, i32), chunk: CachedChunk) {
      self.chunks.insert(position, chunk);
      self.chunk_cache_timers.insert(position, Instant::now());
   }

   pub fn update_timers(&mut self) {
      for (position, instant) in &self.chunk_cache_timers {
         if instant.elapsed() > Self::CHUNK_CACHE_DURATION {
            self.chunks.remove_entry(position);
         }
      }
   }
}
