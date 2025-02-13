use std::vec;

use crate::spherical_harmonics::SphericalHarmonics;
use serde::{Deserialize, Serialize};
use vek::{Quaternion, Vec3};

#[derive(Debug, PartialEq, Copy, Clone, Serialize, Deserialize)]
pub struct UnpackedGaussian {
    pub position: Vec3<f32>,
    pub rotation: Quaternion<f32>,
    pub scales: Vec3<f32>,
    /// The _linear_ color of the Gaussian.
    pub color: Vec3<f32>,
    /// The _linear_ alpha value of the Gaussian.
    pub alpha: f32,
    pub spherical_harmonics: SphericalHarmonics,
}

impl Default for UnpackedGaussian {
    fn default() -> Self {
        Self {
            position: Vec3::zero(),
            rotation: Quaternion::identity(),
            scales: Vec3::one(),
            color: Vec3::zero(),
            alpha: 0.0,
            spherical_harmonics: SphericalHarmonics::default(),
        }
    }
}

impl UnpackedGaussian {
    fn scalars(&self) -> vec::Vec<f32> {
        let mut scalars = vec![
            self.position.x,
            self.position.y,
            self.position.z,
            self.rotation.x,
            self.rotation.y,
            self.rotation.z,
            self.rotation.w,
            self.scales.x,
            self.scales.y,
            self.scales.z,
            self.color.x,
            self.color.y,
            self.color.z,
            self.alpha,
        ];
        scalars.extend(self.spherical_harmonics.scalars());
        return scalars;
    }

    pub fn is_valid(&self) -> bool {
        let scalars = self.scalars();
        for scalar in scalars.iter() {
            if scalar.is_nan() {
                return false;
            }
            if !scalar.is_finite() {
                return false;
            }
        }
        return true;
    }
}
