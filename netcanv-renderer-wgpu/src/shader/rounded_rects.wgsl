struct SceneUniforms {
   transform: mat3x3f,
}

@group(0) @binding(0)
var<uniform> scene_uniforms: SceneUniforms;

struct Vertex {
   @builtin(position) position: vec4f,
   @location(0) color: vec4f,
}

@vertex
fn main_vs(
   @location(0) position: vec2f,
   @location(1) depth_index: u32,
   @location(2) color: vec4u,
) -> Vertex {
   let transformed_position = scene_uniforms.transform * vec3f(position, 1.0);

   var vertex: Vertex;
   vertex.position = vec4f(transformed_position.xy, f32(depth_index), 1.0);
   vertex.color = vec4f(color) / 255.0;
   return vertex;
}

@fragment
fn main_fs(vertex: Vertex) -> @location(0) vec4f {
   return vertex.color;
}
