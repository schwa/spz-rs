/// A 24 bit fixed floating point number with N fractional bits
pub(crate) struct FixedPoint24(pub f32);

impl FixedPoint24 {
    pub(crate) fn new(value: f32) -> Self {
        Self(value)
    }

    pub(crate) fn into(self, fractional_bits: usize) -> [u8; 3] {
        // 1) Multiply float by 2^fractional_bits.
        let scaling_factor = (1 << fractional_bits) as f32;
        // 2) Convert to integer (rounding or truncating as desired).
        let scaled = (self.0 * scaling_factor).round();

        // 3) Clamp to the signed 24-bit range: [-2^23, 2^23 - 1].
        let int_val = scaled as i32;
        let clipped = int_val.clamp(-0x800000, 0x7FFFFF);

        // 4) Get the lower 24 bits in two's complement form.
        //    (For negative numbers, casting to u32 followed by masking preserves the correct bits.)
        let bits = (clipped as u32) & 0x00FF_FFFF;

        // 5) Pack into three bytes in **little-endian** order:
        [
            (bits & 0xFF) as u8, // LSB
            ((bits >> 8) & 0xFF) as u8,
            ((bits >> 16) & 0xFF) as u8, // MSB
        ]
    }

    pub(crate) fn from(bytes: [u8; 3], fractional_bits: usize) -> Self {
        // 1) Reconstruct the 24-bit unsigned integer from the little-endian byte order.
        let raw = (bytes[0] as u32) | ((bytes[1] as u32) << 8) | ((bytes[2] as u32) << 16);

        // 2) Sign-extend from 24 bits to 32 bits.
        //    We shift left so the 24th bit becomes the 32-bit sign bit, then arithmetic shift right.
        let extended = ((raw << 8) as i32) >> 8;

        // 3) Convert the integer back to float by dividing by 2^fractional_bits.
        let scaling_factor = (1 << fractional_bits) as f32;
        let value = extended as f32 / scaling_factor;

        Self(value)
    }

    fn optimal_fractional_bits(&self) -> usize {
        let value = self.0;
        if value == 0.0 {
            return 0;
        }
        let value = value.abs().ceil();
        let bits = value.log2().ceil() as usize;
        24 - bits - 1
    }
}

// Given an array of floats and the desired bit_count work out the ideal number of fractional bits needed to represent the floats with as much precision as possible.
pub(crate) fn compute_fixed_point_fractional_bits(floats: &[f32], bit_count: usize) -> usize {
    assert!(bit_count == 24);
    let max = floats
        .iter()
        .map(|v| v.abs().ceil())
        .max_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap_or(0.0);
    println!("Max: {}", max);
    FixedPoint24::new(max).optimal_fractional_bits()
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::{assert_abs_diff_eq, assert_relative_eq};

    #[test]
    fn test_fraction_bits() {
        assert_eq!(FixedPoint24::new(0.0).optimal_fractional_bits(), 0);
        assert_eq!(FixedPoint24::new(1.0).optimal_fractional_bits(), 23);
        assert_eq!(FixedPoint24::new(1.5).optimal_fractional_bits(), 22);
        assert_eq!(FixedPoint24::new(-1.0).optimal_fractional_bits(), 23);
        assert_eq!(FixedPoint24::new(-1.5).optimal_fractional_bits(), 22);
        assert_eq!(FixedPoint24::new(100.0).optimal_fractional_bits(), 16);
        assert_eq!(FixedPoint24::new(100.5).optimal_fractional_bits(), 16);
        assert_eq!(FixedPoint24::new(-105.12345).optimal_fractional_bits(), 16);
    }

    #[test]
    fn test_fixed_point() {
        let value = 0.0;
        let fractional_bits = 0;
        assert_relative_eq!(
            FixedPoint24::from(
                FixedPoint24::new(value).into(fractional_bits),
                fractional_bits
            )
            .0,
            value
        );

        let value = 1.0;
        let fractional_bits = 0;
        assert_relative_eq!(
            FixedPoint24::from(
                FixedPoint24::new(value).into(fractional_bits),
                fractional_bits
            )
            .0,
            value
        );

        let value = -1.0;
        let fractional_bits = 0;
        assert_relative_eq!(
            FixedPoint24::from(
                FixedPoint24::new(value).into(fractional_bits),
                fractional_bits
            )
            .0,
            value
        );

        let value = 0.0;
        let fractional_bits = 8;
        assert_relative_eq!(
            FixedPoint24::from(
                FixedPoint24::new(value).into(fractional_bits),
                fractional_bits
            )
            .0,
            value
        );

        let value = 1.0;
        let fractional_bits = 8;
        assert_relative_eq!(
            FixedPoint24::from(
                FixedPoint24::new(value).into(fractional_bits),
                fractional_bits
            )
            .0,
            value
        );

        let value = -1.0;
        let fractional_bits = 8;
        assert_relative_eq!(
            FixedPoint24::from(
                FixedPoint24::new(value).into(fractional_bits),
                fractional_bits
            )
            .0,
            value
        );

        let value = 3.145;
        let fractional_bits = 20;
        assert_abs_diff_eq!(
            FixedPoint24::from(
                FixedPoint24::new(value).into(fractional_bits),
                fractional_bits
            )
            .0,
            value,
            epsilon = 1e-6
        );

        let value = -3.145;
        let fractional_bits = 20;
        assert_relative_eq!(
            FixedPoint24::from(
                FixedPoint24::new(value).into(fractional_bits),
                fractional_bits
            )
            .0,
            value,
            epsilon = 1e-6
        );
    }

    #[test]
    fn test_compute_fixed_point_fractional_bits() {
        let floats = vec![-105.086426, 170.979, 1.3356934];

        let fractional_bits = compute_fixed_point_fractional_bits(&floats, 24);
        assert_eq!(fractional_bits, 15);

        let fixed = floats
            .iter()
            .map(|v| FixedPoint24::new(*v).into(fractional_bits))
            .collect::<Vec<_>>();
        let result = fixed
            .iter()
            .map(|v| FixedPoint24::from(*v, fractional_bits).0)
            .collect::<Vec<_>>();

        assert_relative_eq!(result[0], floats[0], epsilon = 1e-3);
        assert_relative_eq!(result[1], floats[1], epsilon = 1e-3);
        assert_relative_eq!(result[2], floats[2], epsilon = 1e-3);
    }
}
