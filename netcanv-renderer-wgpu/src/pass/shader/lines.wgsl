const max_line_count = 512u;

struct Line {
   @align(16)
   line: vec4f, // xy = start point, zw = end point
   thickness: f32,
   cap: u32,
   color: u32,
}

struct Vertex {
   @builtin(position) position: vec4f,
   @location(0) line_index: u32,
   @location(1) local_position: vec2f,
}

@group(0) @binding(0) var<uniform> line_data: array<Line, max_line_count>;
@group(1) @binding(0) var<uniform> model_transform: mat3x3f;
@group(2) @binding(0) var<uniform> scene_transform: mat3x3f;

const cap_butt = 0u;
const cap_square = 1u;
const cap_round = 2u;

// WGSL doesn't have array literals for some reason so we use a vector.
// Conveniently there are only three types of line caps.
const should_extend_cap = vec3f(0.0, 1.0, 1.0);
const should_draw_square_cap = vec3f(0.0, 1.0, 0.0);

fn line_axes(line: vec4f, thickness: f32) -> vec4f {
   let x_axis = line.zw - line.xy;
   let x_axis_normalized = normalize(x_axis);
   let y_axis = vec2f(-x_axis_normalized.y, x_axis_normalized.x) * thickness;
   return vec4f(x_axis, y_axis);
}

@vertex
fn main_vs(
   @builtin(instance_index) line_index: u32,
   @location(0) position: vec2f,
) -> Vertex
{
   let data = line_data[line_index];

   // We need to extend the line by its thickness if we're using a cap that needs more sample
   // coverage.
   var line = data.line + vec4f(0.5);
   let direction = normalize(line.zw - line.xy);
   let square_cap = direction * data.thickness * should_extend_cap[data.cap] * 0.5;
   line += vec4f(-square_cap, square_cap);

   // For lines that are less than 2px wide using the thickness for fragment coverage is not enough
   // and we start losing pixels. Thus we need to push that upwards in that case.
   let axes = line_axes(line, max(data.thickness, 2.0) + 10.0);
   let x_axis = axes.xy;
   let y_axis = axes.zw;

   let local_position = line.xy + position.x * x_axis + position.y * y_axis;
   let scene_position = scene_transform * model_transform * vec3f(local_position, 1.0);

   var vertex: Vertex;
   vertex.position = vec4f(scene_position.xy, 0.0, 1.0);
   vertex.line_index = line_index;
   vertex.local_position = local_position;
   return vertex;
}

fn line_sdf(uv: vec2f, origin: vec2f, tangent: vec2f, normal: vec2f, thickness: f32) -> f32 {
   let line = abs(dot(normal, uv - origin));
   return line - thickness;
}

fn circle_sdf_squared(uv: vec2f, origin: vec2f, radius: f32) -> f32 {
   let delta = origin - uv;
   return length(delta) - radius;
}

@fragment
fn main_fs(vertex: Vertex) -> @location(0) vec4f {
   let data = line_data[vertex.line_index];

   let line = data.line + vec4f(0.5);
   let thickness = data.thickness + 1.0;
   let half_thickness = thickness * 0.5;

   let uv = vertex.local_position;
   let origin = line.xy;
   let center = (line.xy + line.zw) * 0.5;
   let tangent = normalize(line.zw - line.xy);
   let normal = vec2f(-tangent.y, tangent.x);
   let half_length = length(center - line.xy) + thickness * should_draw_square_cap[data.cap];

   let tangent_sdf = line_sdf(uv, origin, tangent, normal, half_thickness);
   let normal_sdf = line_sdf(uv, center, normal, tangent, half_length);

   var alpha = clamp(-tangent_sdf, 0.0, 1.0) * clamp(-normal_sdf, 0.0, 1.0);
   if data.cap == cap_round {
      let start = circle_sdf_squared(uv, line.xy, half_thickness);
      let end = circle_sdf_squared(uv, line.zw, half_thickness);
      alpha = clamp(alpha + clamp(-start, 0.0, 1.0) + clamp(-end, 0.0, 1.0), 0.0, 1.0);
   }

   var color = unpack4x8unorm(data.color);
   color *= alpha * color.a;
   return color;
}
