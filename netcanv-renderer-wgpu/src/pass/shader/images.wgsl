const max_image_count = 512u;

struct Vertex {
   @builtin(position) position: vec4f,
   @location(0) uv: vec2f,
   @location(1) rect_index: u32,
}

struct ImageRect {
   @align(16)
   rect: vec4f,
   color: u32,
   rendition: u32,
}

const rendition_colorize = 0x00000001u;
const rendition_nearest  = 0x00000002u;

@group(0) @binding(0) var image_texture: texture_2d<f32>;
@group(1) @binding(0) var<uniform> image_rect_data: array<ImageRect, max_image_count>;
@group(2) @binding(0) var<uniform> model_transform: mat3x3f;
@group(3) @binding(0) var<uniform> scene_transform: mat3x3f;
@group(3) @binding(1) var linear_sampler: sampler;
@group(3) @binding(2) var nearest_sampler: sampler;

@vertex
fn main_vs(
   @builtin(instance_index) rect_index: u32,
   @location(0) position: vec2f,
) -> Vertex {
   let data = image_rect_data[rect_index];
   let local_position = position * data.rect.zw + data.rect.xy;
   let scene_position = scene_transform * model_transform * vec3f(local_position, 1.0);

   var vertex: Vertex;
   vertex.position = vec4f(scene_position.xy, 0.0, 1.0);
   vertex.uv = position;
   vertex.rect_index = rect_index;
   return vertex;
}

@fragment
fn main_fs(vertex: Vertex) -> @location(0) vec4f {
   let data = image_rect_data[vertex.rect_index];

   let color_nearest = textureSample(image_texture, nearest_sampler, vertex.uv);
   let color_linear = textureSample(image_texture, linear_sampler, vertex.uv);

   let nearest_factor = f32((data.rendition & rendition_nearest) != 0u);
   var color = mix(color_linear, color_nearest, nearest_factor);
   if (data.rendition & rendition_colorize) != 0u {
      let tint_color = unpack4x8unorm(data.color);
      color = vec4f(tint_color.r, tint_color.g, tint_color.b, tint_color.a * color.a);
   }
   return color;
}
