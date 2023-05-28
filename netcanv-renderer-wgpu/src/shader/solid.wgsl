struct Vertex {
   @builtin(position)
   position: vec4f,
}

@vertex
fn main_vs(
   @location(0)
   position: vec4f,
) -> Vertex {
   var vertex: Vertex;
   vertex.position = position;
   return vertex;
}

@fragment
fn main_fs() -> @location(0) vec4f {
   return vec4f(1.0, 1.0, 1.0, 1.0);
}
