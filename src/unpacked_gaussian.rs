use crate::spherical_harmonics::SphericalHarmonics;
use serde::{Deserialize, Serialize};
use vek::{Vec3, Vec4};

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct UnpackedGaussian {
    pub position: Vec3<f32>,
    pub rotation: Vec4<f32>,
    pub scales: Vec3<f32>,
    /// The _linear_ color of the Gaussian.
    pub color: Vec3<f32>,
    /// The _linear_ alpha value of the Gaussian.
    pub alpha: f32,
    pub spherical_harmonics: SphericalHarmonics,
}
