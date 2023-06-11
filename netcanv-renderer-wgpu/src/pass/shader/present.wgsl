@group(0) @binding(0) var screen_texture: texture_2d<f32>;

@vertex
fn main_vs(@builtin(vertex_index) index: u32) -> @builtin(position) vec4f {
   let position = vec2f(
      // This triangle is probably larger than it needs to be, but who cares.
      vec3f(0.0, 2.0, -2.0)[index],
      vec3f(3.0, -1.0, -1.0)[index],
   );
   return vec4f(position, 0.0, 1.0);
}

@fragment
fn main_fs(@builtin(position) frag_coord: vec4f) -> @location(0) vec4f {
   return textureLoad(screen_texture, vec2u(frag_coord.xy), 0);
}
