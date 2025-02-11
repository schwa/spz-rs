use serde::{Deserialize, Serialize};
use std::vec;
use vek::Vec3;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub enum SphericalHarmonicsOrder {
    Order0, // 0 floats / 0 vectors
    Order1, // 9 floats / 3 vectors
    Order2, // 24 floats / 8 vectors
    Order3, // 45 floats / 15 vectors
}

impl SphericalHarmonicsOrder {
    pub fn index(&self) -> usize {
        match self {
            SphericalHarmonicsOrder::Order0 => 0,
            SphericalHarmonicsOrder::Order1 => 1,
            SphericalHarmonicsOrder::Order2 => 2,
            SphericalHarmonicsOrder::Order3 => 3,
        }
    }

    pub fn order_for_degree(degree: u8) -> Option<Self> {
        match degree {
            0 => Some(SphericalHarmonicsOrder::Order0),
            1 => Some(SphericalHarmonicsOrder::Order1),
            2 => Some(SphericalHarmonicsOrder::Order2),
            3 => Some(SphericalHarmonicsOrder::Order3),
            _ => None,
        }
    }

    pub fn order_for_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(SphericalHarmonicsOrder::Order0),
            1..=3 => Some(SphericalHarmonicsOrder::Order1),
            4..=8 => Some(SphericalHarmonicsOrder::Order2),
            9..=15 => Some(SphericalHarmonicsOrder::Order3),
            _ => None,
        }
    }

    pub fn vector_count(&self) -> usize {
        match self {
            SphericalHarmonicsOrder::Order0 => 0,
            SphericalHarmonicsOrder::Order1 => 3,
            SphericalHarmonicsOrder::Order2 => 8,
            SphericalHarmonicsOrder::Order3 => 15,
        }
    }

    pub fn scalar_count(&self) -> usize {
        self.vector_count() * 3
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum SphericalHarmonics {
    Order0(()),
    Order1([Vec3<f32>; 3]),
    Order2([Vec3<f32>; 8]),
    Order3([Vec3<f32>; 15]),
}

impl Default for SphericalHarmonics {
    fn default() -> Self {
        SphericalHarmonics::Order0(())
    }
}

impl SphericalHarmonics {
    pub fn order(&self) -> SphericalHarmonicsOrder {
        match self {
            SphericalHarmonics::Order0(_) => SphericalHarmonicsOrder::Order0,
            SphericalHarmonics::Order1(_) => SphericalHarmonicsOrder::Order1,
            SphericalHarmonics::Order2(_) => SphericalHarmonicsOrder::Order2,
            SphericalHarmonics::Order3(_) => SphericalHarmonicsOrder::Order3,
        }
    }

    pub fn reorder(&mut self, order: SphericalHarmonicsOrder) {
        let mut values = self.values();
        values.resize(order.vector_count(), Vec3::zero());

        self.set_values(values);
    }

    pub fn values(&self) -> Vec<Vec3<f32>> {
        match self {
            SphericalHarmonics::Order0(_) => vec![],
            SphericalHarmonics::Order1(values) => values.to_vec(),
            SphericalHarmonics::Order2(values) => values.to_vec(),
            SphericalHarmonics::Order3(values) => values.to_vec(),
        }
    }

    pub fn set_values(&mut self, values: Vec<Vec3<f32>>) {
        let mut values = values;
        match values.len() {
            0 => *self = SphericalHarmonics::Order0(()),
            1..=3 => {
                if values.len() != 3 {
                    values.resize(3, Vec3::zero());
                }
                *self = SphericalHarmonics::Order1([values[0], values[1], values[2]]);
            }
            4..=8 => {
                if values.len() != 8 {
                    values.resize(8, Vec3::zero());
                }
                *self = SphericalHarmonics::Order2([
                    values[0], values[1], values[2], values[3], values[4], values[5], values[6],
                    values[7],
                ]);
            }
            9..=15 => {
                if values.len() != 15 {
                    values.resize(15, Vec3::zero());
                }
                *self = SphericalHarmonics::Order3([
                    values[0], values[1], values[2], values[3], values[4], values[5], values[6],
                    values[7], values[8], values[9], values[10], values[11], values[12],
                    values[13], values[14],
                ]);
            }
            _ => panic!("Invalid number of values for SphericalHarmonics"),
        };
    }

    pub fn scalars(&self) -> Vec<f32> {
        self.values()
            .iter()
            .flat_map(|v| v.iter())
            .copied()
            .collect()
    }

    pub fn set_scalars(&mut self, scalars: &[f32]) {
        let values = scalars
            .chunks(3)
            .map(|chunk| Vec3::new(chunk[0], chunk[1], chunk[2]))
            .collect();
        self.set_values(values);
    }

    pub fn extend_scalar(&mut self, scalar_index: usize, value: f32) {
        let sh_index = scalar_index / 3;
        let mut values = self.values();
        if values.len() <= sh_index {
            values.resize(sh_index + 1, Vec3::zero());
        }
        values[sh_index][scalar_index % 3] = value;
        self.set_values(values);
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_spherical_harmonics() {
        let mut sh = SphericalHarmonics::default();
        assert!(sh.order() == SphericalHarmonicsOrder::Order0);
        assert!(sh.order().vector_count() == 0);
        assert!(sh.values().is_empty());

        sh.reorder(SphericalHarmonicsOrder::Order1);
        assert!(sh.order() == SphericalHarmonicsOrder::Order1);
        assert!(sh.order().vector_count() == 3);
        assert!(sh.values() == vec![Vec3::zero(); 3]);

        sh.reorder(SphericalHarmonicsOrder::Order2);
        assert!(sh.order() == SphericalHarmonicsOrder::Order2);
        assert!(sh.order().vector_count() == 8);
        assert!(sh.values() == vec![Vec3::zero(); 8]);

        sh.reorder(SphericalHarmonicsOrder::Order1);
        assert!(sh.order() == SphericalHarmonicsOrder::Order1);
        assert!(sh.order().vector_count() == 3);
        assert!(sh.values() == vec![Vec3::zero(); 3]);

        sh = SphericalHarmonics::Order1([
            Vec3::new(1.0, 2.0, 3.0),
            Vec3::new(4.0, 5.0, 6.0),
            Vec3::new(7.0, 8.0, 9.0),
        ]);
        assert!(sh.order() == SphericalHarmonicsOrder::Order1);
        assert!(sh.order().vector_count() == 3);
        assert!(
            sh.values()
                == vec![
                    Vec3::new(1.0, 2.0, 3.0),
                    Vec3::new(4.0, 5.0, 6.0),
                    Vec3::new(7.0, 8.0, 9.0)
                ]
        );

        sh.reorder(SphericalHarmonicsOrder::Order2);
        assert!(sh.order() == SphericalHarmonicsOrder::Order2);
        assert!(sh.order().vector_count() == 8);
        assert!(
            sh.values()[..3]
                == vec![
                    Vec3::new(1.0, 2.0, 3.0),
                    Vec3::new(4.0, 5.0, 6.0),
                    Vec3::new(7.0, 8.0, 9.0)
                ]
        );

        sh.reorder(SphericalHarmonicsOrder::Order1);
        assert!(sh.order() == SphericalHarmonicsOrder::Order1);
        assert!(sh.order().vector_count() == 3);
        assert!(
            sh.values()
                == vec![
                    Vec3::new(1.0, 2.0, 3.0),
                    Vec3::new(4.0, 5.0, 6.0),
                    Vec3::new(7.0, 8.0, 9.0)
                ]
        );
    }

    #[test]
    fn test_extend() {
        let mut sh = SphericalHarmonics::default();
        for n in 0..45 {
            sh.extend_scalar(n, n as f32);
        }
        assert!(sh.order() == SphericalHarmonicsOrder::Order3);
        assert!(sh.order().vector_count() == 15);
        assert!(
            sh.values()
                == vec![
                    Vec3::new(0.0, 1.0, 2.0),
                    Vec3::new(3.0, 4.0, 5.0),
                    Vec3::new(6.0, 7.0, 8.0),
                    Vec3::new(9.0, 10.0, 11.0),
                    Vec3::new(12.0, 13.0, 14.0),
                    Vec3::new(15.0, 16.0, 17.0),
                    Vec3::new(18.0, 19.0, 20.0),
                    Vec3::new(21.0, 22.0, 23.0),
                    Vec3::new(24.0, 25.0, 26.0),
                    Vec3::new(27.0, 28.0, 29.0),
                    Vec3::new(30.0, 31.0, 32.0),
                    Vec3::new(33.0, 34.0, 35.0),
                    Vec3::new(36.0, 37.0, 38.0),
                    Vec3::new(39.0, 40.0, 41.0),
                    Vec3::new(42.0, 43.0, 44.0)
                ]
        );
    }
}
