use bevy::prelude::*;

use crate::Colormap;

/// Colour of cells without data.
pub const PLACEHOLDER: Color = Color::srgb(0.33, 0.33, 0.35);

/// The viridis colormap for `t` in `0.0..=1.0`, as a polynomial fit (also used in the shader,
/// keep both in sync). Perceptually uniform and readable with colour vision deficiencies.
pub fn viridis(t: f32) -> Color {
    const C: [[f32; 3]; 7] = [
        [0.277_727_3, 0.005_407_344, 0.334_099_8],
        [0.105_093_04, 1.404_613_5, 1.384_590_2],
        [-0.330_861_83, 0.214_847_56, 0.095_095_16],
        [-4.634_230_5, -5.799_101, -19.332_441],
        [6.228_27, 14.179_933, 56.690_55],
        [4.776_385, -13.745_145, -65.353_035],
        [-5.435_456, 4.645_852_6, 26.312_435],
    ];
    let t = t.clamp(0.0, 1.0);
    let channel = |i: usize| {
        C.iter()
            .rev()
            .fold(0.0, |acc, c| acc * t + c[i])
            .clamp(0.0, 1.0)
    };
    Color::srgb(channel(0), channel(1), channel(2))
}

/// The jet colormap for `t` in `0.0..=1.0`: dark blue, blue, cyan, yellow, red, dark red,
/// piecewise linear like MATLAB's `jet` (also used in the shader, keep both in sync). Familiar,
/// but not perceptually uniform: its bright cyan and yellow bands look like edges in the data.
pub fn jet(t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    let channel = |center: f32| (1.5 - (4.0 * t - center).abs()).clamp(0.0, 1.0);
    Color::srgb(channel(3.0), channel(2.0), channel(1.0))
}

impl Colormap {
    /// The colour for `t` in `0.0..=1.0`.
    pub fn color(self, t: f32) -> Color {
        match self {
            Colormap::Viridis => viridis(t),
            Colormap::Jet => jet(t),
        }
    }
}

/// RGBA8 sRGB bytes of `color`.
pub fn rgba8(color: Color) -> [u8; 4] {
    color.to_srgba().to_u8_array()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viridis_endpoints() {
        // dark purple to yellow
        let [r, g, b, _] = rgba8(viridis(0.0));
        assert!(r < 80 && g < 10 && b > 70, "{r} {g} {b}");
        let [r, g, b, _] = rgba8(viridis(1.0));
        assert!(r > 230 && g > 220 && b < 60, "{r} {g} {b}");
        assert_eq!(viridis(-1.0), viridis(0.0));
    }

    #[test]
    fn jet_runs_from_dark_blue_over_green_to_dark_red() {
        assert_eq!(rgba8(jet(0.0)), [0, 0, 128, 255]);
        assert_eq!(rgba8(jet(0.5)), [128, 255, 128, 255]);
        assert_eq!(rgba8(jet(1.0)), [128, 0, 0, 255]);
        // pure blue, cyan, yellow and red on the way
        assert_eq!(rgba8(jet(0.125)), [0, 0, 255, 255]);
        assert_eq!(rgba8(jet(0.375)), [0, 255, 255, 255]);
        assert_eq!(rgba8(jet(0.625)), [255, 255, 0, 255]);
        assert_eq!(rgba8(jet(0.875)), [255, 0, 0, 255]);
        assert_eq!(jet(2.0), jet(1.0));
        assert_eq!(Colormap::Jet.color(0.3), jet(0.3));
        assert_eq!(Colormap::default().color(0.3), viridis(0.3));
    }
}
