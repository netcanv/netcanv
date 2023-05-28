use glam::{vec2, Vec2};
use netcanv_renderer::paws::Vector;

pub fn vector_to_vec2(vector: Vector) -> Vec2 {
   vec2(vector.x, vector.y)
}
