use netcanv_renderer::BlendMode;

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

pub(crate) struct RenderPipelinePermutations {
   permutations: [wgpu::RenderPipeline; BlendMode::VARIANTS.len()],
}

impl RenderPipelinePermutations {
   pub fn new(make_permutation: impl Fn(&str, BlendMode) -> wgpu::RenderPipeline) -> Self {
      Self {
         permutations: BlendMode::VARIANTS.map(|blend_mode| {
            make_permutation(&format!("(blend_mode={blend_mode:?})"), blend_mode)
         }),
      }
   }

   pub fn get(&self, blend_mode: BlendMode) -> &wgpu::RenderPipeline {
      &self.permutations[blend_mode as usize]
   }
}
