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
   Oklab(Oklab),
   Okhsv(Okhsv),
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
         AnyColor::Oklab(lab) => Srgb::from(LinearRgb::from(lab)),
         AnyColor::Okhsv(hsv) => Srgb::from(LinearRgb::from(Oklab::from(hsv))),
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
         AnyColor::Oklab(lab) => LinearRgb::from(lab),
         AnyColor::Okhsv(hsv) => LinearRgb::from(Oklab::from(hsv)),
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
         AnyColor::Oklab(lab) => Hsv::from(Srgb::from(LinearRgb::from(lab))),
         AnyColor::Okhsv(hsv) => Hsv::from(Srgb::from(LinearRgb::from(Oklab::from(hsv)))),
      }
   }
}

impl From<Oklab> for AnyColor {
   fn from(color: Oklab) -> Self {
      Self::Oklab(color)
   }
}

impl From<AnyColor> for Oklab {
   fn from(color: AnyColor) -> Self {
      match color {
         AnyColor::Srgb(srgb) => Oklab::from(LinearRgb::from(srgb)),
         AnyColor::LinearRgb(linear_rgb) => Oklab::from(linear_rgb),
         AnyColor::Hsv(hsv) => Oklab::from(LinearRgb::from(Srgb::from(hsv))),
         AnyColor::Oklab(lab) => lab,
         AnyColor::Okhsv(hsv) => Oklab::from(hsv),
      }
   }
}

impl From<Okhsv> for AnyColor {
   fn from(color: Okhsv) -> Self {
      Self::Okhsv(color)
   }
}

impl From<AnyColor> for Okhsv {
   fn from(color: AnyColor) -> Self {
      match color {
         AnyColor::Srgb(srgb) => Okhsv::from(Oklab::from(LinearRgb::from(srgb))),
         AnyColor::LinearRgb(linear_rgb) => Okhsv::from(Oklab::from(linear_rgb)),
         AnyColor::Hsv(hsv) => Okhsv::from(Oklab::from(LinearRgb::from(Srgb::from(hsv)))),
         AnyColor::Oklab(lab) => Okhsv::from(lab),
         AnyColor::Okhsv(hsv) => hsv,
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
   let x = x.abs();
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

/// An Oklab color.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Oklab {
   /// The L (lightness) component of the color.
   pub l: f32,
   /// The a (green/red) component of the color.
   pub a: f32,
   /// The b (blue/yellow) component of the color.
   pub b: f32,
}

impl From<LinearRgb> for Oklab {
   fn from(LinearRgb { r, g, b }: LinearRgb) -> Self {
      let l = 0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b;
      let m = 0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b;
      let s = 0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b;

      let l_ = l.cbrt();
      let m_ = m.cbrt();
      let s_ = s.cbrt();

      return Self {
         l: 0.2104542553 * l_ + 0.7936177850 * m_ - 0.0040720468 * s_,
         a: 1.9779984951 * l_ - 2.4285922050 * m_ + 0.4505937099 * s_,
         b: 0.0259040371 * l_ + 0.7827717662 * m_ - 0.8086757660 * s_,
      };
   }
}

impl From<Oklab> for LinearRgb {
   fn from(Oklab { l, a, b }: Oklab) -> Self {
      let l_ = l + 0.3963377774 * a + 0.2158037573 * b;
      let m_ = l - 0.1055613458 * a - 0.0638541728 * b;
      let s_ = l - 0.0894841775 * a - 1.2914855480 * b;

      let l = l_ * l_ * l_;
      let m = m_ * m_ * m_;
      let s = s_ * s_ * s_;

      return Self {
         r: 4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
         g: -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
         b: -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s,
      };
   }
}

// NOTE(liquidev):
// I like how Oklab was this really easy thing to implement, and then implementing Okhsv is…
// well, have a look for yourself.
//
// Of course, I'm lying when I say that _I_ actually implemented this.
// All of this code is Björn's; I just took his C++ implementation of the color space,
// translated it into Rust, changing some names to snake_case because the Rust compiler was
// complaining about them not matching the official style guides. I kept all the comments so that
// the code is at least _somewhat_ readable to all of you math gigabrains out there, but me…
// I'm just a measly software developer. All of the math behind this basically goes over my head.
//
// So all credit goes to him. Please go check out his work.

struct Lc {
   l: f32,
   c: f32,
}

struct St {
   s: f32,
   t: f32,
}

impl From<Lc> for St {
   fn from(Lc { l, c }: Lc) -> Self {
      Self {
         s: c / l,
         t: c / (1.0 - l),
      }
   }
}

/// An Okhsv color.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Okhsv {
   /// The hue.
   pub h: f32,
   /// The saturation.
   pub s: f32,
   /// The value.
   pub v: f32,
}

impl Okhsv {
   fn compute_max_saturation(a: f32, b: f32) -> f32 {
      // Max saturation will be when one of r, g or b goes below zero.

      // Select different coefficients depending on which component goes below zero first
      let (k0, k1, k2, k3, k4, wl, wm, ws) = if -1.88170328 * a - 0.80936493 * b > 1.0 {
         // Red component
         (
            1.19086277,
            1.76576728,
            0.59662641,
            0.75515197,
            0.56771245,
            4.0767416621,
            -3.3077115913,
            0.2309699292,
         )
      } else if 1.81444104 * a - 1.19445276 * b > 1.0 {
         // Green component
         (
            0.73956515,
            -0.45954404,
            0.08285427,
            0.12541070,
            0.14503204,
            -1.2684380046,
            2.6097574011,
            -0.3413193965,
         )
      } else {
         // Blue component
         (
            1.35733652,
            -0.00915799,
            -1.15130210,
            -0.50559606,
            0.00692167,
            -0.0041960863,
            -0.7034186147,
            1.7076147010,
         )
      };

      // Approximate max saturation using a polynomial:
      let mut s = k0 + k1 * a + k2 * b + k3 * a * a + k4 * a * b;

      // Do one step Halley's method to get closer
      // this gives an error less than 10e6, except for some blue hues where the dS/dh is close to infinite
      // this should be sufficient for most applications, otherwise do two/three steps

      let k_l = 0.3963377774 * a + 0.2158037573 * b;
      let k_m = -0.1055613458 * a - 0.0638541728 * b;
      let k_s = -0.0894841775 * a - 1.2914855480 * b;

      {
         let l_ = 1.0 + s * k_l;
         let m_ = 1.0 + s * k_m;
         let s_ = 1.0 + s * k_s;

         let l = l_ * l_ * l_;
         let m = m_ * m_ * m_;
         let ss = s_ * s_ * s_;

         let l_ds = 3.0 * k_l * l_ * l_;
         let m_ds = 3.0 * k_m * m_ * m_;
         let s_ds = 3.0 * k_s * s_ * s_;

         let l_ds2 = 6.0 * k_l * k_l * l_;
         let m_ds2 = 6.0 * k_m * k_m * m_;
         let s_ds2 = 6.0 * k_s * k_s * s_;

         let f = wl * l + wm * m + ws * ss;
         let f1 = wl * l_ds + wm * m_ds + ws * s_ds;
         let f2 = wl * l_ds2 + wm * m_ds2 + ws * s_ds2;

         s = s - f * f1 / (f1 * f1 - 0.5 * f * f2);
      }

      return s;
   }

   fn find_cusp(a: f32, b: f32) -> Lc {
      // First, find the maximum saturation (saturation S = C/L)
      let s_cusp = Self::compute_max_saturation(a, b);

      // Convert to linear sRGB to find the first point where at least one of r, g, or b >= 1:
      let rgb_at_max = LinearRgb::from(Oklab {
         l: 1.0,
         a: s_cusp * a,
         b: s_cusp * b,
      });
      let l_cusp = (1.0 / f32::max(f32::max(rgb_at_max.r, rgb_at_max.g), rgb_at_max.b)).cbrt();
      let c_cusp = l_cusp * s_cusp;

      return Lc {
         l: l_cusp,
         c: c_cusp,
      };
   }

   fn toe(x: f32) -> f32 {
      const K1: f32 = 0.206;
      const K2: f32 = 0.03;
      const K3: f32 = (1.0 + K1) / (1.0 + K2);
      return 0.5 * (K3 * x - K1 + f32::sqrt((K3 * x - K1) * (K3 * x - K1) + 4.0 * K2 * K3 * x));
   }

   fn toe_inv(x: f32) -> f32 {
      const K1: f32 = 0.206;
      const K2: f32 = 0.03;
      const K3: f32 = (1.0 + K1) / (1.0 + K2);
      return (x * x + K1 * x) / (K3 * (x + K2));
   }
}

impl From<Oklab> for Okhsv {
   fn from(Oklab { l, a, b }: Oklab) -> Self {
      let c = (a * a + b * b).sqrt();
      let a_ = if c != 0.0 { a / c } else { 1.0 };
      let b_ = if c != 0.0 { b / c } else { 1.0 };

      let h = 0.5 + 0.5 * (-b).atan2(-a) / std::f32::consts::PI;

      let cusp = Self::find_cusp(a_, b_);
      let St { s: s_max, t: t_max } = St::from(cusp);
      let s_0 = 0.5;
      let k = 1.0 - s_0 / s_max;

      // first we find L_v, C_v, L_vt and C_vt

      let t = if c != 0.0 && l != 0.0 {
         t_max / (c + l * t_max)
      } else {
         0.0
      };
      let l_v = t * l;
      let c_v = t * c;

      let l_vt = Self::toe_inv(l_v);
      let c_vt = if l_v != 0.0 { c_v * l_vt / l_v } else { 0.0 };

      // we can then use these to invert the step that compensates for the toe and the curved top part of the triangle:
      let rgb_scale = LinearRgb::from(Oklab {
         l: l_vt,
         a: a_ * c_vt,
         b: b_ * c_vt,
      });
      let scale_l = (1.0
         / f32::max(
            f32::max(rgb_scale.r, rgb_scale.g),
            f32::max(rgb_scale.b, 0.0),
         ))
      .cbrt();

      // NOTE(liquidev): there was a variable 'C' in the C++ version, but its value didn't seem
      // to be used? I removed it.

      let l = l / scale_l;
      let c = ((c / scale_l) * Self::toe(l)) / l;
      let l = Self::toe(l);

      // we can now compute v and s:

      let s = if c != 0.0 {
         (s_0 + t_max) * c_v / ((t_max * s_0) + t_max * k * c_v)
      } else {
         0.0
      };
      let v = if l != 0.0 { l / l_v } else { 0.0 };

      return Okhsv { h, s, v };
   }
}

impl From<Okhsv> for Oklab {
   fn from(Okhsv { h, s, v }: Okhsv) -> Self {
      let a_ = (2.0 * std::f32::consts::PI * h).cos();
      let b_ = (2.0 * std::f32::consts::PI * h).sin();

      let cusp = Okhsv::find_cusp(a_, b_);
      let St { s: s_max, t: t_max } = St::from(cusp);
      let s_0 = 0.5;
      let k = 1.0 - s_0 / s_max;

      // first we compute L and V as if the gamut is a perfect triangle:

      // L, C when v==1:
      let l_v = 1.0 - s * s_0 / (s_0 + t_max - t_max * k * s);
      let c_v = s * t_max * s_0 / (s_0 + t_max - t_max * k * s);

      let mut l = v * l_v;
      let mut c = v * c_v;

      // then we compensate for both toe and the curved top part of the triangle:
      let l_vt = Okhsv::toe_inv(l_v);
      let c_vt = c_v * l_vt / l_v;

      let l_new = Okhsv::toe_inv(l);
      c = if l != 0.0 { c * l_new / l } else { 0.0 };
      l = l_new;

      let rgb_scale = LinearRgb::from(Oklab {
         l: l_vt,
         a: a_ * c_vt,
         b: b_ * c_vt,
      });
      let max_rgb = f32::max(
         f32::max(rgb_scale.r, rgb_scale.g),
         f32::max(rgb_scale.b, 0.0),
      );
      // Fail safe: when max_rgb is 0, the color is black and thus, is achromatic.
      let scale_l = if max_rgb != 0.0 {
         (1.0 / max_rgb).cbrt()
      } else {
         0.0
      };

      l = l * scale_l;
      c = c * scale_l;

      Oklab {
         l,
         a: c * a_,
         b: c * b_,
      }
   }
}
