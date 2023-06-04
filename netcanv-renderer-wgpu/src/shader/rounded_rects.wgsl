struct SceneUniforms {
   transform: mat3x3f,
}

struct Rect {
   @align(16)
   rect: vec4f, // xy = top-left, zw = bottom-right
   depth_index: u32,
   corner_radius: f32,
   color: u32,
   outline: f32,
}

struct Vertex {
   @builtin(position) position: vec4f,
   @location(0) in_position: vec2f,
   @location(1) rect_index: u32,
   @location(2) transformed_position: vec2f,
}

@group(0) @binding(0) var<uniform> scene_uniforms: SceneUniforms;
@group(0) @binding(1) var<uniform> rect_data: array<Rect, 256>;

@vertex
fn main_vs(
   @location(0) position: vec2f,
   @location(1) rect_index: u32,
) -> Vertex
{
   let transformed_position = scene_uniforms.transform * vec3f(position, 1.0);
   let depth_index = rect_data[rect_index].depth_index;

   var vertex: Vertex;
   vertex.position = vec4f(transformed_position.xy, f32(depth_index) / 65535.0, 1.0);
   vertex.in_position = position;
   vertex.rect_index = rect_index;
   vertex.transformed_position = transformed_position.xy;
   return vertex;
}

fn rectangle_sdf(uv: vec2f, half_extents: vec2f) -> f32 {
   let componentwise_edge_distance = abs(uv) - half_extents;
   let outside_distance = length(max(componentwise_edge_distance, vec2f(0.0)));
   let inside_distance = min(max(componentwise_edge_distance.x, componentwise_edge_distance.y), 0.0);
   return outside_distance + inside_distance;
}

fn corner_sdf(uv: vec2f, radius: f32) -> f32 {
   let componentwise_edge_distance = abs(uv) - vec2f(radius);
   return max(componentwise_edge_distance.x, componentwise_edge_distance.y);
}

fn corner_alpha(uv: vec2f, radius: f32) -> f32 {
   return max(0.0, 1.0 - corner_sdf(uv, radius) / radius) * 0.5;
}

const pi = 3.141592654;
const corner_offset = 0.2928932188; // 1.0 - sqrt(2.0) / 2.0;

@fragment
fn main_fs(vertex: Vertex) -> @location(0) vec4f {
   var data = rect_data[vertex.rect_index];
   // Prevent wonkiness around the edges by flooring the rect's coordinates.
   data.rect = floor(data.rect);

   let center = (data.rect.xy + data.rect.zw) * 0.5;
   let half_extents = ((data.rect.zw - data.rect.xy) * 0.5 - vec2f(data.corner_radius));
   let sdf = rectangle_sdf(vertex.in_position - center, half_extents) - data.corner_radius + 0.5;
   let outline = sdf + data.outline;

   let width = abs(data.rect.x - data.rect.z);
   let height = abs(data.rect.y - data.rect.w);
   let corner_radius = clamp(data.corner_radius, 0.0, min(width / 2.0, height / 2.0));
   let inner_rect = data.rect + vec4f(corner_offset, corner_offset, -corner_offset, -corner_offset);
   let smoothing = corner_radius * 0.1;
   let R = 2.0 * pi * corner_radius / 8.0 + smoothing;
   var corners = clamp(
      corner_alpha(vertex.in_position - inner_rect.xy, R)
      + corner_alpha(vertex.in_position - inner_rect.zy, R)
      + corner_alpha(vertex.in_position - inner_rect.zw, R)
      + corner_alpha(vertex.in_position - inner_rect.xw, R),
      0.0,
      1.0,
   );
   corners = smoothstep(0.1, 0.5, corners);

   // Some drivers don't behave very well on smoothstep(x, x, y) so we have to account
   // for that case and force alpha to be 1.
   var alpha = smoothstep(corners, 0.0, sdf);
   if corners <= 0.0 {
      alpha = 1.0;
   }
   if data.outline > 0.0 {
      alpha *= clamp(outline, 0.0, 1.0);
   }

   var color = unpack4x8unorm(data.color);
   color *= alpha * color.a;
   return color;
}
