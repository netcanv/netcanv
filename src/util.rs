use skulpin::skia_safe::Color4f;

pub fn hex_color4f(hex: u32) -> Color4f {
    let r = ((hex >> 24) & 0xFF) as f32 / 255.0;
    let g = ((hex >> 16) & 0xFF) as f32 / 255.0;
    let b = ((hex >> 8) & 0xFF) as f32 / 255.0;
    let a = (hex & 0xFF) as f32 / 255.0;
    Color4f::new(r, g, b, a)
}
