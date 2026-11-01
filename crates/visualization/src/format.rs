//! Human readable numbers for labels.

/// Groups the integer part in threes with thin spaces: `12 040.5`.
pub fn grouped(value: f64, decimals: usize) -> String {
    let text = format!("{:.*}", decimals, value.abs());
    let (int, frac) = text
        .split_once('.')
        .map_or((text.as_str(), None), |(i, f)| (i, Some(f)));
    let mut out = String::new();
    for (i, digit) in int.chars().enumerate() {
        if i > 0 && (int.len() - i) % 3 == 0 {
            out.push('\u{2009}');
        }
        out.push(digit);
    }
    if let Some(frac) = frac {
        out.push('.');
        out.push_str(frac);
    }
    if value < 0.0 && out.chars().any(|c| c.is_ascii_digit() && c != '0') {
        out.insert(0, '-');
    }
    out
}

/// A time in ms with a precision that suits its magnitude.
pub fn milliseconds(ms: f64) -> String {
    let decimals = if ms.abs() >= 1000.0 {
        1
    } else if ms.abs() >= 1.0 {
        2
    } else {
        4
    };
    format!("{} ms", grouped(ms, decimals))
}

pub fn frequency(hz: f64) -> String {
    match hz.abs() {
        f if f >= 1e6 => format!("{:.2} MHz", hz / 1e6),
        f if f >= 1e3 => format!("{:.1} kHz", hz / 1e3),
        _ => format!("{hz:.0} Hz"),
    }
}

/// An amplitude: integers grouped, large or tiny floats in scientific notation.
pub fn amplitude(value: f32) -> String {
    let v = value as f64;
    if v.fract() == 0.0 && v.abs() < 1e12 {
        grouped(v, 0)
    } else if v.abs() >= 1e6 || (v != 0.0 && v.abs() < 1e-2) {
        format!("{v:.3e}")
    } else {
        grouped(v, 2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grouping() {
        assert_eq!(grouped(12040.5, 1), "12\u{2009}040.5");
        assert_eq!(grouped(999.0, 0), "999");
        assert_eq!(grouped(-1234567.0, 0), "-1\u{2009}234\u{2009}567");
        assert_eq!(grouped(-0.0001, 1), "0.0");
    }

    #[test]
    fn units() {
        assert_eq!(milliseconds(1204.31), "1\u{2009}204.3 ms");
        assert_eq!(milliseconds(0.08192), "0.0819 ms");
        assert_eq!(frequency(2400.0), "2.4 kHz");
        assert_eq!(frequency(850.0), "850 Hz");
        assert_eq!(frequency(1_562_500.0), "1.56 MHz");
        assert_eq!(amplitude(3_390_949.0), "3\u{2009}390\u{2009}949");
        assert_eq!(amplitude(1.5), "1.50");
        assert_eq!(amplitude(0.001), "1.000e-3");
    }
}
