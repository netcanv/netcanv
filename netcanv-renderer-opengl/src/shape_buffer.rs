//! A helper for constructing vertex and index arrays for shapes.

use glam::{Mat3A, Vec3};
use netcanv_renderer::paws::{point, vector, Point, Vector};
use smallvec::SmallVec;

use crate::common::VectorMath;
use crate::rendering::Vertex;

pub(crate) struct ShapeBuffer {
   transform: Mat3A,
   pub vertices: Vec<Vertex>,
   pub indices: Vec<u32>,
}

impl ShapeBuffer {
   pub fn new() -> Self {
      Self {
         transform: Mat3A::IDENTITY,
         vertices: Vec::new(),
         indices: Vec::new(),
      }
   }

   pub fn start(&mut self, transform: Mat3A) {
      self.transform = transform;
      self.vertices.clear();
      self.indices.clear();
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
      self.indices.extend(indices.iter().copied());
   }

   /// Winds a quad using the given indices into the shape buffer.
   pub fn quad_indices(
      &mut self,
      top_left: u32,
      top_right: u32,
      bottom_right: u32,
      bottom_left: u32,
   ) {
      self.push_indices(&[
         top_left,
         top_right,
         bottom_right,
         bottom_right,
         bottom_left,
         top_left,
      ]);
   }

   /// Pushes a quad and its indices into the shape buffer.
   ///
   /// The vertices are assumed to be wound clockwise.
   ///
   /// The indices of the pushed vertices are returned in the order:
   /// `top_left, top_right, bottom_right, bottom_left` (clockwise starting from top left).
   pub fn quad(
      &mut self,
      top_left: Vertex,
      top_right: Vertex,
      bottom_right: Vertex,
      bottom_left: Vertex,
   ) -> (u32, u32, u32, u32) {
      let top_left = self.push_vertex(top_left);
      let top_right = self.push_vertex(top_right);
      let bottom_right = self.push_vertex(bottom_right);
      let bottom_left = self.push_vertex(bottom_left);
      self.quad_indices(top_left, top_right, bottom_right, bottom_left);
      (top_left, top_right, bottom_right, bottom_left)
   }

   /// Pushes a rectangle and its indices into the shape buffer.
   ///
   /// The top right and bottom left vertices are inferred from the provided corners.
   /// The color of these vertices is taken from the top left vertex.
   pub fn rect(&mut self, top_left: Vertex, bottom_right: Vertex) -> (u32, u32, u32, u32) {
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
      self.quad(top_left, top_right, bottom_right, bottom_left)
   }

   /// Returns the number of vertices an arc with the given radius, start, and end angles should
   /// have to look smooth.
   fn arc_vertex_count(radius: f32, start_angle: f32, end_angle: f32) -> usize {
      ((end_angle - start_angle).abs() * radius).max(6.0) as usize
   }

   /// Pushes a filled arc into the shape buffer.
   ///
   /// The vertex color is taken from the vertex pointed to by the given center index. However,
   /// the center position must be provided separately, because the one stored in the vertex under
   /// the given index has already been multiplied with the transform matrix.
   pub fn arc(
      &mut self,
      center_index: u32,
      center: Vector,
      radius: f32,
      start_angle: f32,
      end_angle: f32,
   ) {
      let Vertex { color, uv, .. } = self.vertices[center_index as usize];
      let vertex_count = Self::arc_vertex_count(radius, start_angle, end_angle);
      let mut perimeter_indices = SmallVec::<[u32; 32]>::new();
      for angle_vector in Rotate::new(start_angle, end_angle, vertex_count) {
         perimeter_indices.push(self.push_vertex(Vertex {
            position: center + angle_vector * radius,
            uv,
            color,
         }));
      }
      for pair in perimeter_indices.windows(2) {
         self.push_indices(&[center_index, pair[0], pair[1]]);
      }
   }

   pub fn arc_outline(
      &mut self,
      center: Vector,
      vertex_template: Vertex,
      radius: f32,
      thickness: f32,
      start_angle: f32,
      end_angle: f32,
   ) {
      let Vertex { color, uv, .. } = vertex_template;
      let vertex_count = Self::arc_vertex_count(radius, start_angle, end_angle);
      let inner_radius = radius - thickness / 2.0;
      let mut perimeter_positions = SmallVec::<[Point; 32]>::new();
      let mut perimeter_indices = SmallVec::<[u32; 32]>::new();
      for angle_vector in Rotate::new(start_angle, end_angle, vertex_count) {
         let perimeter = center + angle_vector * inner_radius;
         perimeter_indices.push(self.push_vertex(Vertex {
            position: perimeter,
            uv,
            color,
         }));
         perimeter_positions.push(perimeter);
      }
      let mut previous_b = None;
      for (i, pair) in perimeter_positions.windows(2).enumerate() {
         let a = pair[0];
         let b = pair[1];
         let direction = (b - a).normalize();
         let ccw = direction.perpendicular_ccw();
         let outer_a = self.push_vertex(Vertex {
            position: a + ccw * thickness,
            uv,
            color,
         });
         let outer_b = self.push_vertex(Vertex {
            position: b + ccw * thickness,
            uv,
            color,
         });
         self.quad_indices(
            outer_a,
            outer_b,
            perimeter_indices[i + 1],
            perimeter_indices[i],
         );
         if let Some((b, outer_b)) = previous_b {
            self.push_indices(&[outer_b, b, outer_a]);
         }
         previous_b = Some((perimeter_indices[i + 1], outer_b));
      }
   }
}

struct Rotate {
   current_vertex: usize,
   vertex_count: usize,
   angle_vector: Vector,
   sin: f32,
   cos: f32,
}

impl Rotate {
   fn new(start_angle: f32, end_angle: f32, vertex_count: usize) -> Self {
      let delta_angle = (end_angle - start_angle) / (vertex_count - 1) as f32;
      let cos = delta_angle.cos();
      let sin = delta_angle.sin();
      Self {
         current_vertex: 0,
         vertex_count,
         angle_vector: vector(start_angle.cos(), start_angle.sin()),
         sin,
         cos,
      }
   }
}

impl Iterator for Rotate {
   type Item = Vector;

   fn next(&mut self) -> Option<Self::Item> {
      if self.current_vertex < self.vertex_count {
         let angle_vector = self.angle_vector;
         self.angle_vector = vector(
            angle_vector.x * self.cos - angle_vector.y * self.sin,
            angle_vector.x * self.sin + angle_vector.y * self.cos,
         );
         self.current_vertex += 1;
         Some(angle_vector)
      } else {
         None
      }
   }
}
