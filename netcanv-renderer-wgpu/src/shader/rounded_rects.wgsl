struct SceneUniforms {
   transform: mat3x3f,
}

struct Rect {
   @align(16)
   rect: vec4f, // xy = top-left, zw = bottom-right
   depth_index: u32,
   corner_radius: f32,
   color: u32,
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

fn diamond_sdf(uv: vec2f, radius: f32) -> f32 {
   return abs(uv.x) + abs(uv.y) - radius;
}

fn corner_alpha(uv: vec2f, radius: f32) -> f32 {
   return max(0.0, 1.0 - diamond_sdf(uv, radius) / radius) * 0.5;
}

@fragment
fn main_fs(vertex: Vertex) -> @location(0) vec4f {
   let data = rect_data[vertex.rect_index];

   let center = (data.rect.xy + data.rect.zw) * 0.5;
   let half_extents = ((data.rect.zw - data.rect.xy) * 0.5 - vec2f(data.corner_radius));
   let sdf = rectangle_sdf(vertex.in_position - center, half_extents) / data.corner_radius;

   let corner_radius = data.corner_radius * 1.5;
   let corners = corner_alpha(vertex.in_position - data.rect.xy, corner_radius)
      + corner_alpha(vertex.in_position - data.rect.zy, corner_radius)
      + corner_alpha(vertex.in_position - data.rect.zw, corner_radius)
      + corner_alpha(vertex.in_position - data.rect.xw, corner_radius);

   let delta = clamp(1.0 / data.corner_radius, 0.0, 1.0) * corners;
   let alpha = smoothstep(1.0, 1.0 - delta, clamp(sdf, 0.0, 1.0));

   var color = unpack4x8unorm(data.color);
   color *= alpha;
   return color;
   // return vec4f(vec3f(delta), 1.0);
}
