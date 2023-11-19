use anyhow::bail;
use wgpu::Backend;

#[derive(Debug, Clone, clap::Args)]
pub struct RendererCli {
   /// Which rendering backend to use. Available backends: vulkan, metal, dx11, dx12, gl
   ///
   /// Note that if you pass in a backend that is not available for your platform of choice,
   /// the app will fail to open.
   #[clap(long, value_parser = backend_from_str)]
   pub wgpu_backend: Option<Backend>,
}

fn backend_from_str(s: &str) -> anyhow::Result<Backend> {
   Ok(match s {
      "vulkan" => Backend::Vulkan,
      "metal" => Backend::Metal,
      "dx11" => Backend::Dx11,
      "dx12" => Backend::Dx12,
      "gl" => Backend::Gl,
      // No BrowserWebGPU since that would be silly.
      _ => {
         bail!("invalid backend. available backends: empty, vulkan, metal, dx11, dx12, gl");
      }
   })
}
