const max_rect_count = 512u;

struct Rect {
   @align(16)
   rect: vec4f, // xy = top-left, zw = bottom-right
   corner_radius: f32,
   color: u32,
   outline: f32,
   rendition: u32,
}

const rendition_antialias         = 0x00000001u;
const rendition_premultiply_alpha = 0x00000002u;

struct Vertex {
   @builtin(position) position: vec4f,
   @location(0) in_position: vec2f,
   @location(1) rect_index: u32,
}

@group(0) @binding(0) var<uniform> rect_data: array<Rect, max_rect_count>;
@group(1) @binding(0) var<uniform> model_transform: mat3x3f;
@group(2) @binding(0) var<uniform> scene_transform: mat3x3f;

@vertex
fn main_vs(
   @builtin(instance_index) rect_index: u32,
   @location(0) position: vec2f,
) -> Vertex
{
   let data = rect_data[rect_index];
   let local_position = position * data.rect.zw + data.rect.xy;
   let scene_position = scene_transform * model_transform * vec3f(local_position, 1.0);

   var vertex: Vertex;
   vertex.position = vec4f(scene_position.xy, 0.0, 1.0);
   vertex.in_position = local_position;
   vertex.rect_index = rect_index;
   return vertex;
}

fn rectangle_sdf(uv: vec2f, half_extents: vec2f) -> f32 {
   let componentwise_edge_distance = abs(uv) - half_extents;
   let outside_distance = length(max(componentwise_edge_distance, vec2f(0.0)));
   let inside_distance = min(max(componentwise_edge_distance.x, componentwise_edge_distance.y), 0.0);
   return outside_distance + inside_distance;
}

@fragment
fn main_fs(vertex: Vertex) -> @location(0) vec4f {
   var data = rect_data[vertex.rect_index];
   // Prevent wonkiness around the edges by flooring the rect's coordinates.

   let center = data.rect.xy + data.rect.zw * 0.5;
   let half_extents = data.rect.zw * 0.5 - vec2f(data.corner_radius);
   let sdf = rectangle_sdf(vertex.in_position - center, half_extents) - data.corner_radius - 0.5;
   let outline = sdf + data.outline + 1.0;

   var alpha = clamp(-sdf, 0.0, 1.0);
   if data.outline > 0.0 {
      alpha *= clamp(outline, 0.0, 1.0);
   }

   if (data.rendition & rendition_antialias) == 0u {
      alpha = 1.0 - step(alpha, 0.5);
   }
   if alpha == 0.0 {
      discard;
   }

   var color = unpack4x8unorm(data.color);
   color.a *= alpha;
   if (data.rendition & rendition_premultiply_alpha) != 0u {
      color = vec4f(color.rgb * color.a, color.a);
   }
   return color;
}
