mod images;
mod lines;
mod present;
mod rounded_rects;
mod text;
mod vertex;

pub(crate) use images::*;
pub(crate) use lines::*;
pub(crate) use present::*;
pub(crate) use rounded_rects::*;
pub(crate) use text::*;

use crate::gpu::Gpu;

pub(crate) struct PassCreationContext<'a> {
   pub gpu: &'a Gpu,
   pub model_transform_bind_group_layout: &'a wgpu::BindGroupLayout,
}
