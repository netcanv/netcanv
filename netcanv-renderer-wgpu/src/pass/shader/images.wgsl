const max_image_count = 512u;

struct Vertex {
   @builtin(position) position: vec4f,
   @location(0) uv: vec2f,
   @location(1) rect_index: u32,
}

struct ImageRect {
   @align(16)
   rect: vec4f,
   depth_index: u32,
   color: u32,
   colorize: u32,
}

@group(0) @binding(0) var image_texture: texture_2d<f32>;
@group(1) @binding(0) var<uniform> image_rect_data: array<ImageRect, max_image_count>;
@group(2) @binding(0) var<uniform> model_transform: mat3x3f;
@group(3) @binding(0) var<uniform> scene_transform: mat3x3f;
@group(3) @binding(1) var image_sampler: sampler;

@vertex
fn main_vs(
   @builtin(instance_index) rect_index: u32,
   @location(0) position: vec2f,
) -> Vertex {
   let data = image_rect_data[rect_index];
   let local_position = position * data.rect.zw + data.rect.xy;
   let scene_position = scene_transform * model_transform * vec3f(local_position, 1.0);

   var vertex: Vertex;
   vertex.position = vec4f(scene_position.xy, f32(data.depth_index) / 65535.0, 1.0);
   vertex.uv = position;
   vertex.rect_index = rect_index;
   return vertex;
}

@fragment
fn main_fs(vertex: Vertex) -> @location(0) vec4f {
   let data = image_rect_data[vertex.rect_index];

   var color = textureSample(image_texture, image_sampler, vertex.uv);
   if data.colorize != 0u {
      let tint_color = unpack4x8unorm(data.color);
      color = vec4f(tint_color.r, tint_color.g, tint_color.b, tint_color.a * color.a);
   }
   color = vec4f(color.rgb * color.a, color.a);
   return color;
}
