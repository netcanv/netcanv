mod images;
mod lines;
mod rounded_rects;
mod vertex;

pub(crate) use images::*;
pub(crate) use lines::*;
pub(crate) use rounded_rects::*;

use crate::gpu::Gpu;

pub(crate) struct PassCreationContext<'a> {
   pub gpu: &'a Gpu,
   pub model_transform_bind_group_layout: &'a wgpu::BindGroupLayout,
}
