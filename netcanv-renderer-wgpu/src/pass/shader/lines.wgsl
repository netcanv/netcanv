struct SceneUniforms {
   transform: mat3x3f,
}

struct Line {
   @align(16)
   line: vec4f, // xy = start point, zw = end point
   depth_index: u32,
   thickness: f32,
   cap: u32,
   color: u32,
}

struct Vertex {
   @builtin(position) position: vec4f,
   @location(0) line_index: u32,
}

@group(0) @binding(0) var<uniform> scene_uniforms: SceneUniforms;
@group(0) @binding(1) var<uniform> line_data: array<Line, 256>;

const cap_butt = 0;
const cap_square = 1;
const cap_round = 2;

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
   let axes = line_axes(data.line, data.thickness);
   let x_axis = axes.xy;
   let y_axis = axes.zw;

   let local_position = data.line.xy + position.x * x_axis + position.y * y_axis;
   let scene_position = scene_uniforms.transform * vec3f(local_position, 1.0);

   var vertex: Vertex;
   vertex.position = vec4f(scene_position.xy, f32(data.depth_index) / 65535.0, 1.0);
   vertex.line_index = line_index;
   return vertex;
}

@fragment
fn main_fs(vertex: Vertex) -> @location(0) vec4f {
   let data = line_data[vertex.line_index];

   let color = unpack4x8unorm(data.color);
   return color;
}
