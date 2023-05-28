struct Uniforms {
   transform: mat3x3f,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct Vertex {
   @builtin(position)
   position: vec4f,
}

@vertex
fn main_vs(
   @location(0)
   position: vec2f,
) -> Vertex {
   let transformed_position = uniforms.transform * vec3f(position, 1.0);

   var vertex: Vertex;
   vertex.position = vec4f(transformed_position.xy, 0.0, 1.0);
   return vertex;
}

@fragment
fn main_fs() -> @location(0) vec4f {
   return vec4f(1.0, 1.0, 1.0, 1.0);
}
