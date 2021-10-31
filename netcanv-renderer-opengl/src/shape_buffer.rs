//! A helper for constructing vertex and index arrays for shapes.

use glam::{Mat3A, Vec3};
use netcanv_renderer::paws::{point, vector};
use smallvec::SmallVec;

use crate::rendering::{Mesh, Vertex};

pub(crate) struct ShapeBuffer<const NV: usize, const NI: usize> {
   transform: Mat3A,
   pub vertices: SmallVec<[Vertex; NV]>,
   pub indices: SmallVec<[u32; NI]>,
}

impl<const NV: usize, const NI: usize> ShapeBuffer<NV, NI> {
   pub fn new(transform: Mat3A) -> Self {
      Self {
         transform,
         vertices: SmallVec::new(),
         indices: SmallVec::new(),
      }
   }

   /// Pushes a vertex into the shape buffer and returns its index.
   pub fn push_vertex(&mut self, mut vertex: Vertex) -> u32 {
      let index = self.vertices.len() as u32;
      let position = vertex.position;
      let position = self.transform.mul_vec3(Vec3::new(position.x, position.y, 1.0));
      vertex.position = vector(position.x, position.y);
      self.vertices.push(vertex);
      index
   }

   /// Pushes a list of indices into the shape buffer.
   pub fn push_indices(&mut self, indices: &[u32]) {
      self.indices.extend(indices.iter().map(|x| *x));
   }

   /// Pushes a quad and its indices into the shape buffer.
   ///
   /// The vertices are assumed to be wound clockwise.
   pub fn quad(
      &mut self,
      top_left: Vertex,
      top_right: Vertex,
      bottom_right: Vertex,
      bottom_left: Vertex,
   ) {
      let top_left = self.push_vertex(top_left);
      let top_right = self.push_vertex(top_right);
      let bottom_right = self.push_vertex(bottom_right);
      let bottom_left = self.push_vertex(bottom_left);
      self.push_indices(&[
         top_left,
         top_right,
         bottom_right,
         bottom_right,
         bottom_left,
         top_left,
      ]);
   }

   /// Pushes a rectangle and its indices into the shape buffer.
   ///
   /// The top right and bottom left vertices are inferred from the provided corners.
   /// The color of these vertices is taken from the top left vertex.
   pub fn rect(&mut self, top_left: Vertex, bottom_right: Vertex) {
      let top_right = Vertex {
         position: point(bottom_right.position.x, top_left.position.y),
         uv: point(bottom_right.uv.x, top_left.uv.y),
         color: top_left.color,
      };
      let bottom_left = Vertex {
         position: point(top_left.position.x, bottom_right.position.y),
         uv: point(top_left.uv.x, bottom_right.uv.y),
         color: top_left.color,
      };
      self.quad(top_left, top_right, bottom_right, bottom_left);
   }
}

impl<const NV: usize, const NI: usize> Mesh for ShapeBuffer<NV, NI> {
   fn vertices(&self) -> &[Vertex] {
      &self.vertices
   }

   fn indices(&self) -> &[u32] {
      &self.indices
   }
}
