//! Color space conversions.

// A lot of the code was adapted from Björn Ottosson's blog posts:
// https://bottosson.github.io/posts/colorwrong/
// https://bottosson.github.io/posts/oklab/
// https://bottosson.github.io/posts/colorpicker/
// I highly encourage you to check all of them out!

use netcanv_renderer::paws::Color;

/// An enum consolidating all the colors to a single type, for storing colors in their original
/// space, losslessly.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnyColor {
   Srgb(Srgb),
   LinearRgb(LinearRgb),
   Hsv(Hsv),
}

impl From<Srgb> for AnyColor {
   fn from(color: Srgb) -> Self {
      Self::Srgb(color)
   }
}

impl From<AnyColor> for Srgb {
   fn from(color: AnyColor) -> Self {
      match color {
         AnyColor::Srgb(srgb) => srgb,
         AnyColor::LinearRgb(linear_rgb) => Srgb::from(linear_rgb),
         AnyColor::Hsv(hsv) => Srgb::from(hsv),
      }
   }
}

impl From<LinearRgb> for AnyColor {
   fn from(color: LinearRgb) -> Self {
      Self::LinearRgb(color)
   }
}

impl From<AnyColor> for LinearRgb {
   fn from(color: AnyColor) -> Self {
      match color {
         AnyColor::Srgb(srgb) => LinearRgb::from(srgb),
         AnyColor::LinearRgb(linear_rgb) => linear_rgb,
         AnyColor::Hsv(hsv) => LinearRgb::from(Srgb::from(hsv)),
      }
   }
}

impl From<Hsv> for AnyColor {
   fn from(color: Hsv) -> Self {
      Self::Hsv(color)
   }
}

impl From<AnyColor> for Hsv {
   fn from(color: AnyColor) -> Self {
      match color {
         AnyColor::Srgb(srgb) => Hsv::from(srgb),
         AnyColor::LinearRgb(linear_rgb) => Hsv::from(Srgb::from(linear_rgb)),
         AnyColor::Hsv(hsv) => hsv,
      }
   }
}

/// An sRGB color.
///
/// The gamma is assumed to be 2.4.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Srgb {
   /// The red channel, in range `0.0..=1.0`.
   pub r: f32,
   /// The green channel, in range `0.0..=1.0`.
   pub g: f32,
   /// The blue channel, in range `0.0..=1.0`.
   pub b: f32,
}

impl Srgb {
   /// Creates an sRGB color from a `Color`. The alpha channel is discarded.
   pub fn from_color(color: Color) -> Self {
      Self {
         r: color.r as f32 / 255.0,
         g: color.g as f32 / 255.0,
         b: color.b as f32 / 255.0,
      }
   }

   /// Converts an sRGB color to a `Color`. The provided alpha value is used.
   pub fn to_color(&self, alpha: f32) -> Color {
      Color {
         r: (self.r * 255.0) as u8,
         g: (self.g * 255.0) as u8,
         b: (self.b * 255.0) as u8,
         a: (alpha * 255.0) as u8,
      }
   }
}

impl From<LinearRgb> for Srgb {
   fn from(color: LinearRgb) -> Self {
      Self {
         r: linear_to_srgb(color.r),
         g: linear_to_srgb(color.g),
         b: linear_to_srgb(color.b),
      }
   }
}

/// The linear RGB to sRGB mapping function.
fn linear_to_srgb(x: f32) -> f32 {
   if x >= 0.0031308 {
      ((1.055) * x).powf(1.0 / 2.4) - 0.055
   } else {
      12.92 * x
   }
}

/// A linear RGB color.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinearRgb {
   /// The red channel, in range `0.0..=1.0`.
   pub r: f32,
   /// The green channel, in range `0.0..=1.0`.
   pub g: f32,
   /// The blue channel, in range `0.0..=1.0`.
   pub b: f32,
}

impl From<Srgb> for LinearRgb {
   fn from(color: Srgb) -> Self {
      Self {
         r: srgb_to_linear(color.r),
         g: srgb_to_linear(color.g),
         b: srgb_to_linear(color.b),
      }
   }
}

/// The sRGB to linear RGB mapping function.
fn srgb_to_linear(x: f32) -> f32 {
   if x >= 0.04045 {
      ((x + 0.055) / (1.0 + 0.055)).powf(2.4)
   } else {
      x / 12.92
   }
}

/// An HSV color.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hsv {
   /// The hue, in range `0.0..=6.0`. Multiply by 60 to get the number of degrees.
   pub h: f32,
   /// The saturation, in range `0.0..=1.0`.
   pub s: f32,
   /// The value, in range `0.0..=1.0`.
   pub v: f32,
}

impl From<Srgb> for Hsv {
   fn from(Srgb { r, g, b }: Srgb) -> Self {
      // https://en.wikipedia.org/wiki/HSL_and_HSV#From_RGB
      let v = f32::max(r, f32::max(g, b));
      let c = v - f32::min(r, f32::min(g, b));
      Self {
         h: if c < f32::EPSILON {
            0.0
         } else if v == r {
            (g - b) / c
         } else if v == g {
            2.0 + (b - r) / c
         } else if v == b {
            4.0 + (r - g) / c
         } else {
            // This should have already been caught by the first branch, but the Rust
            // compiler wants us to specify an else branch just in case.
            0.0
         }
         .rem_euclid(6.0),
         s: if v == 0.0 { 0.0 } else { c / v },
         v,
      }
   }
}

impl From<Hsv> for Srgb {
   fn from(Hsv { h, s, v }: Hsv) -> Self {
      // https://en.wikipedia.org/wiki/HSL_and_HSV#To_RGB
      let c = v * s;
      let x = c * (1.0 - f32::abs(h.rem_euclid(2.0) - 1.0));
      let (r1, g1, b1) = if h >= 0.0 && h < 1.0 {
         (c, x, 0.0)
      } else if h >= 1.0 && h < 2.0 {
         (x, c, 0.0)
      } else if h >= 2.0 && h < 3.0 {
         (0.0, c, x)
      } else if h >= 3.0 && h < 4.0 {
         (0.0, x, c)
      } else if h >= 4.0 && h < 5.0 {
         (x, 0.0, c)
      } else if h >= 5.0 && h < 6.0 {
         (c, 0.0, x)
      } else {
         (0.0, 0.0, 0.0)
      };
      let m = v - c;
      let (r, g, b) = (r1 + m, g1 + m, b1 + m);
      Self { r, g, b }
   }
}
