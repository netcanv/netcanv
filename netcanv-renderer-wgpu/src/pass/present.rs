use wgpu::RenderPass;

use crate::gpu::Gpu;

pub struct Present {
   render_pipeline: wgpu::RenderPipeline,
}

impl Present {
   pub fn new(gpu: &Gpu) -> Self {
      let shader = gpu.device.create_shader_module(wgpu::include_wgsl!("shader/present.wgsl"));

      let pipeline_layout = gpu.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
         label: Some("Present: Pipeline Layout"),
         bind_group_layouts: &[&gpu.screen_texture_bind_group_layout],
         push_constant_ranges: &[],
      });
      let render_pipeline = gpu.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
         label: Some("Present: Render Pipeline"),
         layout: Some(&pipeline_layout),
         vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "main_vs",
            buffers: &[],
         },
         primitive: wgpu::PrimitiveState::default(),
         depth_stencil: None,
         multisample: wgpu::MultisampleState::default(),
         fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "main_fs",
            targets: &[Some(wgpu::ColorTargetState {
               format: gpu.surface_format(),
               blend: None,
               write_mask: wgpu::ColorWrites::ALL,
            })],
         }),
         multiview: None,
      });
      Self { render_pipeline }
   }

   pub fn render<'a>(&'a self, gpu: &'a Gpu, render_pass: &mut RenderPass<'a>) {
      render_pass.set_pipeline(&self.render_pipeline);
      render_pass.set_bind_group(0, &gpu.screen_texture_bind_group, &[]);
      render_pass.draw(0..3, 0..1);
   }
}
