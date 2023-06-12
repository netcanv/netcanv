use glam::{Mat3A, Vec2};
use netcanv_renderer::paws::{vector, Rect, Vector};
use netcanv_renderer::BlendMode;

use crate::WgpuBackend;

#[derive(Debug, Clone, Copy)]
pub struct TransformState {
   pub transform: Transform,
   pub clip: Option<Rect>,
   pub blend_mode: BlendMode,
}

impl Default for TransformState {
   fn default() -> Self {
      Self {
         transform: Transform::Translation(Vec2::ZERO),
         clip: None,
         blend_mode: BlendMode::Alpha,
      }
   }
}

#[derive(Debug, Clone, Copy)]
pub enum Transform {
   // Translation is used whenever there isn't any scaling applied.
   // This is the fast path which doesn't involve sending anything to the GPU.
   Translation(Vec2),
   Matrix(Mat3A),
}

impl WgpuBackend {
   pub(crate) fn current_transform(&self) -> &TransformState {
      self.transform_stack.last().expect("transform stack is empty")
   }

   pub(crate) fn current_transform_mut(&mut self) -> &mut TransformState {
      self.transform_stack.last_mut().expect("transform stack is empty")
   }
}

impl Transform {
   pub fn translate(&self, translation: Vec2) -> Self {
      match *self {
         Transform::Translation(t) => Transform::Translation(t + translation),
         Transform::Matrix(m) => Transform::Matrix(m * Mat3A::from_translation(translation)),
      }
   }

   pub fn scale(&self, scale: Vec2) -> Self {
      match *self {
         Transform::Translation(t) => {
            Transform::Matrix(Mat3A::from_translation(t) * Mat3A::from_scale(scale))
         }
         Transform::Matrix(m) => Transform::Matrix(m * Mat3A::from_scale(scale)),
      }
   }

   pub fn is_matrix(&self) -> bool {
      matches!(self, Self::Matrix(..))
   }

   pub fn translate_vector(&self, vec: Vector) -> Vector {
      match self {
         Transform::Translation(t) => vector(vec.x + t.x, vec.y + t.y),
         _ => vec,
      }
   }

   pub fn translate_rect(&self, rect: Rect) -> Rect {
      match self {
         Transform::Translation(t) => Rect::new(rect.position + vector(t.x, t.y), rect.size),
         _ => rect,
      }
   }
}
