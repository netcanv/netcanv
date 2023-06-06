use glam::{Mat3A, Vec2};

use crate::WgpuBackend;

#[derive(Debug, Clone, Copy)]
pub enum Transform {
   // Translation is used whenever there isn't any scaling applied.
   // This is the fast path which doesn't involve sending anything to the GPU.
   Translation(Vec2),
   Matrix(Mat3A),
}

impl WgpuBackend {
   pub(crate) fn current_transform(&self) -> Transform {
      *self.transform_stack.last().expect("transform stack is empty")
   }

   pub(crate) fn current_transform_mut(&mut self) -> &mut Transform {
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
}
