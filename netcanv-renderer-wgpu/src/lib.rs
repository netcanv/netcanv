use std::mem::size_of;

use anyhow::Context;
use cli::RendererCli;
use glam::Mat3A;
use gpu::Gpu;
use image::ImageStorage;
use netcanv_renderer::paws::{Color, Ui};
use rendering::Pass;
use text::TextRenderer;
use tracing::{info, info_span};
use transform::TransformState;
use wgpu::util::DeviceExt;
use winit::dpi::PhysicalSize;
use winit::event_loop::EventLoop;
use winit::window::{Window, WindowBuilder};

pub use winit;

mod batch_storage;
pub mod cli;
mod common;
mod framebuffer;
mod gpu;
mod image;
mod pass;
mod rendering;
mod text;
mod transform;

pub use framebuffer::Framebuffer;
pub use image::Image;
pub use text::Font;

use crate::batch_storage::{BatchStorage, BatchStorageConfig};
use crate::gpu::SceneUniformCache;
use crate::pass::PassCreationContext;

pub struct WgpuBackend {
   window: Window,
   gpu: Gpu,

   // TODO: We should have this be event-driven instead of polling every frame.
   context_size: PhysicalSize<u32>,

   image_storage: ImageStorage,
   text_renderer: TextRenderer,
   transform_stack: Vec<TransformState>,
   scene_uniform_cache: SceneUniformCache,
   identity_model_transform_bind_group: wgpu::BindGroup,
   model_transform_storage: BatchStorage,

   clear: Option<Color>,
   last_pass: Option<Pass>,

   rounded_rects: pass::RoundedRects,
   lines: pass::Lines,
   images: pass::Images,
   text: pass::Text,

   present: pass::Present,

   command_buffers: Vec<wgpu::CommandBuffer>,

   frame_counter: usize,
}

impl WgpuBackend {
   pub async fn new(
      window_builder: WindowBuilder,
      event_loop: &EventLoop<()>,
      cli: &RendererCli,
   ) -> anyhow::Result<Self> {
      let window = window_builder.build(event_loop).context("Failed to create window")?;
      let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
         backends: if let Some(backend) = cli.wgpu_backend {
            wgpu::Backends::from(backend)
         } else {
            wgpu::Backends::all()
         },
         ..Default::default()
      });

      let surface = unsafe { instance.create_surface(&window) }
         .context("Failed to create surface from window")?;
      let adapter = instance
         .request_adapter(&wgpu::RequestAdapterOptionsBase {
            power_preference: wgpu::PowerPreference::LowPower,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
         })
         .await
         .context(
            "Failed to find a graphics adapter. Please make sure your drivers are up to date",
         )?;

      let capabilities = surface.get_capabilities(&adapter);
      info!("adapter capabilities: {capabilities:#?}");
      info!("adapter limits: {:#?}", adapter.limits());
      let limits = wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits());
      info!("using compatible set of limits: {limits:#?}");

      let (device, queue) = adapter.request_device(
         &wgpu::DeviceDescriptor {
            label: None,
            features: wgpu::Features::empty(),
            limits,
         },
         None,
      ).await.context("Failed to acquire graphics device. Try updating your graphics drivers. If that doesn't work, your hardware may be too old to run NetCanv.")?;

      let screen_texture_bind_group_layout =
         device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Screen Texture Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
               binding: 0,
               visibility: wgpu::ShaderStages::FRAGMENT,
               ty: wgpu::BindingType::Texture {
                  sample_type: wgpu::TextureSampleType::Float { filterable: false },
                  view_dimension: wgpu::TextureViewDimension::D2,
                  multisampled: false,
               },
               count: None,
            }],
         });
      let (screen_texture, screen_texture_view, screen_texture_bind_group) =
         Gpu::create_screen_texture_view_and_bind_group(
            &device,
            &screen_texture_bind_group_layout,
            window.inner_size(),
         );

      let identity_mat3x3f = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
         label: Some("Identity mat3x3f"),
         contents: bytemuck::cast_slice(&[
            1.0_f32, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0,
         ]),
         usage: wgpu::BufferUsages::UNIFORM,
      });
      let linear_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
         label: Some("Linear Sampler"),
         address_mode_u: wgpu::AddressMode::ClampToEdge,
         address_mode_v: wgpu::AddressMode::ClampToEdge,
         address_mode_w: wgpu::AddressMode::ClampToEdge,
         mag_filter: wgpu::FilterMode::Linear,
         min_filter: wgpu::FilterMode::Linear,
         ..Default::default()
      });
      let nearest_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
         label: Some("Nearest Sampler"),
         address_mode_u: wgpu::AddressMode::ClampToEdge,
         address_mode_v: wgpu::AddressMode::ClampToEdge,
         address_mode_w: wgpu::AddressMode::ClampToEdge,
         mag_filter: wgpu::FilterMode::Nearest,
         min_filter: wgpu::FilterMode::Nearest,
         ..Default::default()
      });

      let scene_uniform_bind_group_layout =
         device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Scene Uniform Bind Group Layout"),
            entries: &[
               wgpu::BindGroupLayoutEntry {
                  binding: 0,
                  visibility: wgpu::ShaderStages::VERTEX,
                  ty: wgpu::BindingType::Buffer {
                     ty: wgpu::BufferBindingType::Uniform,
                     has_dynamic_offset: false,
                     min_binding_size: None,
                  },
                  count: None,
               },
               wgpu::BindGroupLayoutEntry {
                  binding: 1,
                  visibility: wgpu::ShaderStages::FRAGMENT,
                  ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                  count: None,
               },
               wgpu::BindGroupLayoutEntry {
                  binding: 2,
                  visibility: wgpu::ShaderStages::FRAGMENT,
                  ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                  count: None,
               },
            ],
         });

      let model_transform_bind_group_layout =
         device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Model Transform Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
               binding: 0,
               visibility: wgpu::ShaderStages::VERTEX,
               ty: wgpu::BindingType::Buffer {
                  ty: wgpu::BufferBindingType::Uniform,
                  has_dynamic_offset: false,
                  min_binding_size: None,
               },
               count: None,
            }],
         });
      let identity_model_transform_bind_group =
         device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Identity Model Transform Bind Group"),
            layout: &model_transform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
               binding: 0,
               resource: identity_mat3x3f.as_entire_binding(),
            }],
         });

      let mut gpu = Gpu {
         surface,
         adapter,
         capabilities,
         device,
         queue,

         linear_sampler,
         nearest_sampler,
         scene_uniform_bind_group_layout,

         current_render_target: Some(screen_texture_view),
         current_render_target_size: (screen_texture.width(), screen_texture.height()),
         screen_texture,
         screen_texture_bind_group_layout,
         screen_texture_bind_group,
      };
      gpu.handle_resize(window.inner_size());

      let image_storage = ImageStorage::new(&gpu);
      let text_renderer = TextRenderer::new(&gpu);

      let context_size = window.inner_size();
      let pass_creation_context = PassCreationContext {
         gpu: &gpu,
         model_transform_bind_group_layout: &model_transform_bind_group_layout,
      };
      Ok(Self {
         rounded_rects: pass::RoundedRects::new(&pass_creation_context),
         lines: pass::Lines::new(&pass_creation_context),
         images: pass::Images::new(&pass_creation_context, &image_storage),
         text: pass::Text::new(&pass_creation_context, &text_renderer),

         present: pass::Present::new(&gpu),

         image_storage,
         text_renderer,
         transform_stack: vec![TransformState::default()],
         scene_uniform_cache: SceneUniformCache::new(30),
         identity_model_transform_bind_group,
         model_transform_storage: BatchStorage::new(BatchStorageConfig {
            name: "Model Transforms",
            buffer_size: size_of::<Mat3A>() as wgpu::BufferAddress,
            bind_group_layout: model_transform_bind_group_layout,
         }),

         window,
         gpu,

         clear: None,
         last_pass: None,
         context_size,
         command_buffers: vec![],

         frame_counter: 0,
      })
   }

   pub fn window(&self) -> &Window {
      &self.window
   }
}

pub trait UiRenderFrame {
   fn render_frame(&mut self, f: impl FnOnce(&mut Self)) -> anyhow::Result<()>;
}

impl UiRenderFrame for Ui<WgpuBackend> {
   fn render_frame(&mut self, f: impl FnOnce(&mut Self)) -> anyhow::Result<()> {
      let _span = info_span!("render_frame", frame = self.frame_counter).entered();

      let window_size = self.window.inner_size();
      if self.context_size != window_size {
         self.gpu.handle_resize(window_size);
         self.context_size = window_size;
      }

      let frame = self
         .gpu
         .surface
         .get_current_texture()
         .context("Failed to acquire next swapchain texture")?;
      let frame_view = frame.texture.create_view(&wgpu::TextureViewDescriptor {
         label: Some("Frame View"),
         ..Default::default()
      });

      self.rewind();
      {
         let _span = info_span!("main_render_pass").entered();
         f(self);
      }
      self.flush("render_frame");

      {
         let _span = info_span!("present_render_pass").entered();
         let mut present_commands =
            self.gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
               label: Some("Screen -> Frame Copy Commands"),
            });
         {
            let mut render_pass = present_commands.begin_render_pass(&wgpu::RenderPassDescriptor {
               label: Some("Present"),
               color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                  view: &frame_view,
                  resolve_target: None,
                  ops: wgpu::Operations {
                     load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                     store: true,
                  },
               })],
               depth_stencil_attachment: None,
            });
            self.present.render(&self.gpu, &mut render_pass);
         }
         {
            // Slight borrow checker hack here because borrowing out individual fields doesn't work
            // through Deref.
            let renderer = self.render();
            renderer
               .gpu
               .queue
               .submit(renderer.command_buffers.drain(..).chain([present_commands.finish()]));
         }
      }

      {
         let _span = info_span!("swap").entered();
         frame.present();
      }

      self.frame_counter += 1;

      Ok(())
   }
}
