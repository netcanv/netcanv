#[derive(Clone)]
pub struct CachedChunk {
   pub png: Vec<u8>,
   pub webp: Option<Vec<u8>>,
}
