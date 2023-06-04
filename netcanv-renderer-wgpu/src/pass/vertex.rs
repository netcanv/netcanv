use std::mem::size_of;

use bytemuck::{Pod, Zeroable};
use glam::{vec2, Vec2};

#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct Vertex {
   pub position: Vec2,
}

unsafe impl Zeroable for Vertex {}
unsafe impl Pod for Vertex {}

impl Vertex {
   pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
      array_stride: size_of::<Self>() as wgpu::BufferAddress,
      step_mode: wgpu::VertexStepMode::Vertex,
      attributes: &[wgpu::VertexAttribute {
         format: wgpu::VertexFormat::Float32x2,
         offset: 0,
         shader_location: 0,
      }],
   };
}

pub fn vertex(x: f32, y: f32) -> Vertex {
   Vertex {
      position: vec2(x, y),
   }
}
