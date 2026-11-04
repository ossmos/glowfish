use bevy::prelude::*;

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
}
