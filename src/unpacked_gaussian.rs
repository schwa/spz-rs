use crate::spherical_harmonics::SphericalHarmonics;
use serde::{Deserialize, Serialize};
use vek::{Quaternion, Vec3};

#[derive(Debug, PartialEq, Copy, Clone, Serialize, Deserialize, Default)]
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
