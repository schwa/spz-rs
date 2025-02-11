use vek::{Vec3, Vec4};

#[derive(Debug, PartialEq, Clone)]
pub struct UnpackedGaussian {
    pub position: Vec3<f32>,
    pub rotation: Vec4<f32>,
    pub scales: Vec3<f32>,
    pub color: Vec3<f32>,
    pub alpha: f32,
    pub spherical_harmonics: Vec<Vec3<f32>>, // 0, 9, 24, 45 -> 0 3 8 15
}
