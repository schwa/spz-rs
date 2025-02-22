use anyhow::Result;
use bytemuck::{Pod, Zeroable};
use flate2::write::GzEncoder;
use flate2::Compression;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::vec;
use vek::{Quaternion, Vec3};

use crate::spherical_harmonics::{SphericalHarmonics, SphericalHarmonicsOrder};
use crate::spz_writer::*;
use crate::unpacked_gaussian::UnpackedGaussian;

pub(crate) const COLOR_SCALE: f32 = 0.15;

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
pub struct SPZHeader {
    pub magic: u32,
    pub version: u32,
    pub num_points: u32,
    pub sh_degree: u8,
    pub fractional_bits: u8,
    pub flags: u8,
    pub reserved: u8,
}

impl SPZHeader {
    pub fn is_valid(&self) -> bool {
        self.magic == 0x5053474e && self.version == 2 && self.sh_degree <= 3
    }

    pub fn new(num_points: u32, sh_degree: u8, fractional_bits: u8, flags: u8) -> Self {
        Self {
            magic: 0x5053474e,
            version: 2,
            num_points,
            sh_degree,
            fractional_bits,
            flags,
            reserved: 0,
        }
    }

    pub fn expected_uncompressed_size(&self) -> usize {
        let header_size = std::mem::size_of::<SPZHeader>();
        let position_size = 3 * 3;
        let alpha_size = 1;
        let color_size = 3;
        let scale_size = 3;
        let rotation_size = 3;
        let order = SphericalHarmonicsOrder::order_for_degree(self.sh_degree).unwrap();
        let sh_size = order.scalar_count();
        let size_per_point =
            position_size + alpha_size + color_size + scale_size + rotation_size + sh_size;
        header_size + size_per_point * self.num_points as usize
    }
}

pub fn write_spz_to_stream<W: Write>(
    gaussians: &Vec<UnpackedGaussian>,
    stream: &mut W,
    omit_spherical_harmonics: bool,
) -> Result<()> {
    let options = SPZWriterOptions {
        omit_spherical_harmonics,
    };
    let mut writer = SPZWriter::new(stream, options);
    writer.write(gaussians)?;
    Ok(())
}

pub fn write_spz(
    gaussians: Vec<UnpackedGaussian>,
    path: &Path,
    compressed: bool,
    omit_spherical_harmonics: bool,
) -> Result<()> {
    let file = File::create(path)?;
    if compressed {
        let mut stream = GzEncoder::new(file, Compression::best());
        write_spz_to_stream(&gaussians, &mut stream, omit_spherical_harmonics)?;
        stream.finish()?;
    } else {
        let mut stream = Box::new(file);
        write_spz_to_stream(&gaussians, &mut stream, omit_spherical_harmonics)?;
    }
    Ok(())
}

const SH1_BITS: i32 = 5;
const SH_REST_BITS: i32 = 4;

impl SphericalHarmonics {
    pub(crate) fn from_spz_bytes(bytes: Vec<u8>) -> Self {
        fn unquantize_sh(x: u8) -> f32 {
            (x as f32 - 128.0) / 128.0
        }
        let values = bytes
            .iter()
            .map(|&x| unquantize_sh(x))
            .collect::<Vec<f32>>();
        let mut sh = SphericalHarmonics::default();
        sh.set_scalars(&values);
        sh
    }

    pub(crate) fn spz_bytes(&self) -> Vec<u8> {
        fn quantize_sh(x: f32, bucket_size: i32) -> u8 {
            let mut q = ((x * 128.0).round() as i32) + 128;
            q = (q + bucket_size / 2) / bucket_size * bucket_size;
            q.clamp(0, 255) as u8
        }

        let scalar_count = self.order().scalar_count();
        let scalars = self.scalars();
        assert!(scalars.len() == scalar_count);
        let mut sh: Vec<u8> = vec![0; scalar_count];

        let i = 0;
        let mut j = 0;
        while j < 9 {
            sh[i + j] = quantize_sh(scalars[i + j], 1 << (8 - SH1_BITS));
            j += 1;
        }
        while j < scalar_count {
            sh[i + j] = quantize_sh(scalars[i + j], 1 << (8 - SH_REST_BITS));
            j += 1;
        }
        sh
    }
}

#[cfg(test)]
mod tests {
    use approx::assert_relative_eq;

    use super::*;
    use crate::spz_reader::*;
    use crate::unpacked_gaussian::*;
    use vek::{Quaternion, Vec3};

    #[test]
    fn test_spz() {
        let gaussian = UnpackedGaussian {
            position: Vec3::new(100.0, 200.0, -100.0),
            rotation: Quaternion::identity(),
            scales: Vec3::new(1.0, 2.0, 1.0),
            color: Vec3::new(1.0, 0.5, 0.25),
            alpha: 0.95,
            spherical_harmonics: SphericalHarmonics::default(),
        };

        let mut buffer = Vec::new();
        write_spz_to_stream(&vec![gaussian], &mut buffer, true).unwrap();

        let result =
            SPZReader::new_from_slice(&buffer, SPZReaderOptions::default().skip_compression(true))
                .read()
                .unwrap();
        assert!(result.len() == 1);
        let result = &result[0];
        gaussian_approx_eq(&gaussian, result);
    }

    #[test]
    fn test_single() {
        // This is from the "official" niantic spz file. Source looks like this:
        // "x": 100.0, "y": 200.0, "z": -100.0, "f_dc_0": 1.0, "f_dc_1": 0.5, "f_dc_2": 0.25, "opacity": 0.95, "scale_0": 1.0. "scale_1": -1.0, "scale_2": 10, "rot0": 0.0, "rot_1": 0.0, "rot_2": 0.0, "rot_3": 1.0
        let hex = "4E475350 02000000 01000000 000C0000 00400600 800C00C0 F9B8A693 89B090B0 8080FF";
        let bytes = dehex(hex);

        let gaussians =
            SPZReader::new_from_slice(&bytes, SPZReaderOptions::default().skip_compression(true))
                .read()
                .unwrap();
        assert!(gaussians.len() == 1);
        let gaussian = &gaussians[0];
        assert!(gaussian.position == Vec3::new(100.0, 200.0, -100.0));
        assert!(gaussian.scales == Vec3::new(1.0, -1.0, 1.0));
        assert_relative_eq!(gaussian.rotation.x, 0.0, epsilon = 1e-2);
        assert_relative_eq!(gaussian.rotation.y, 0.0, epsilon = 1e-2);
        assert_relative_eq!(gaussian.rotation.z, 1.0, epsilon = 1e-2);
        assert_relative_eq!(gaussian.rotation.w, 0.0, epsilon = 1e-2);
        assert_relative_eq!(gaussian.alpha, 0.95, epsilon = 1e-2);
        assert_relative_eq!(gaussian.color[0], 1.0, epsilon = 1e-2);
        assert_relative_eq!(gaussian.color[1], 0.5, epsilon = 1e-2);
        assert_relative_eq!(gaussian.color[2], 0.25, epsilon = 1e-2);
    }

    fn dehex(hex: &str) -> Vec<u8> {
        hex.replace(" ", "")
            .as_bytes()
            .chunks_exact(2)
            .map(|chunk| u8::from_str_radix(std::str::from_utf8(chunk).unwrap(), 16).unwrap())
            .collect::<Vec<_>>()
    }

    #[test]
    fn test2() {
        let gaussian = UnpackedGaussian {
            position: Vec3 {
                x: -105.086426,
                y: 170.979,
                z: 1.3356934,
            },
            rotation: Quaternion {
                x: -0.35686272,
                y: 0.62352943,
                z: 0.24705887,
                w: 0.6502476,
            },
            scales: Vec3 {
                x: 1.125,
                y: 1.125,
                z: 1.125,
            },
            color: Vec3 {
                x: -0.14379084,
                y: -0.06535947,
                z: 0.013072093,
            },
            alpha: 3.5675187,
            spherical_harmonics: SphericalHarmonics::Order3([
                Vec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                Vec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                Vec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                Vec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                Vec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                Vec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                Vec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                Vec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                Vec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                Vec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                Vec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                Vec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                Vec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                Vec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                Vec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
            ]),
        };

        let mut buffer = Vec::new();
        let gaussians = vec![gaussian];
        write_spz_to_stream(&gaussians, &mut buffer, false).unwrap();
        let options = SPZReaderOptions::default().skip_compression(true);
        let mut reader = SPZReader::new_from_slice(&buffer, options);
        let header = reader.read_header().unwrap();
        assert!(header.expected_uncompressed_size() == buffer.len());
        let result = reader.read_gaussians().unwrap()[0];
        assert!(gaussian.position == result.position);
        assert!(gaussian.scales == result.scales);
        gaussian_approx_eq(&gaussian, &result);
    }

    fn gaussian_approx_eq(left: &UnpackedGaussian, right: &UnpackedGaussian) {
        assert!(left.position == right.position);
        assert!(left.scales == right.scales);
        assert_relative_eq!(left.rotation.x, right.rotation.x, epsilon = 1e-2);
        assert_relative_eq!(left.rotation.y, right.rotation.y, epsilon = 1e-2);
        assert_relative_eq!(left.rotation.z, right.rotation.z, epsilon = 1e-2);
        assert_relative_eq!(left.rotation.w, right.rotation.w, epsilon = 1e-2);
        assert_relative_eq!(left.color[0], right.color[0], epsilon = 1e-1);
        assert_relative_eq!(left.color[1], right.color[1], epsilon = 1e-1);
        assert_relative_eq!(left.color[2], right.color[2], epsilon = 1e-1);
        assert_relative_eq!(left.alpha, right.alpha, epsilon = 1e-1);
        assert!(left.spherical_harmonics == right.spherical_harmonics);
    }
}

pub fn encode(v: f32, min: f32, max: f32) -> u8 {
    assert!(v >= min && v <= max);
    let v = (v - min) / (max - min) * 255.0;
    v as u8
}

pub fn decode(v: u8, min: f32, max: f32) -> f32 {
    let v = v as f32 / 255.0;
    v * (max - min) + min
}

#[cfg(test)]
mod test_encode {

    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn test_encode() {
        assert_eq!(encode(0.0, 0.0, 1.0), 0);
        assert_eq!(encode(0.5, 0.0, 1.0), 127);
        assert_eq!(encode(1.0, 0.0, 1.0), 255);
        assert_eq!(encode(-1.0, -1.0, 1.0), 0);
        assert_eq!(encode(-0.5, -1.0, 1.0), 63);
        assert_eq!(encode(0.0, -1.0, 1.0), 127);
        assert_eq!(encode(0.5, -1.0, 1.0), 191);
        assert_eq!(encode(1.0, -1.0, 1.0), 255);
    }

    #[test]
    fn test_decode() {
        assert_relative_eq!(decode(0, 0.0, 1.0), 0.0, epsilon = 1e-2);
        assert_relative_eq!(decode(127, 0.0, 1.0), 0.5, epsilon = 1e-2);
        assert_relative_eq!(decode(255, 0.0, 1.0), 1.0, epsilon = 1e-2);
        assert_relative_eq!(decode(0, -1.0, 1.0), -1.0, epsilon = 1e-2);
        assert_relative_eq!(decode(63, -1.0, 1.0), -0.5, epsilon = 1e-2);
        assert_relative_eq!(decode(127, -1.0, 1.0), 0.0, epsilon = 1e-2);
        assert_relative_eq!(decode(191, -1.0, 1.0), 0.5, epsilon = 1e-2);
        assert_relative_eq!(decode(255, -1.0, 1.0), 1.0, epsilon = 1e-2);
    }
}

#[derive(Debug)]
pub struct SPZQuaternion(pub [u8; 3]);

impl Into<Quaternion<f32>> for SPZQuaternion {
    fn into(self) -> Quaternion<f32> {
        let x = decode(self.0[0], -1.0, 1.0);
        let y = decode(self.0[1], -1.0, 1.0);
        let z = decode(self.0[2], -1.0, 1.0);
        let xyz = Vec3::new(x, y, z);
        let w = f32::max(0.0, 1.0 - xyz.dot(xyz)).sqrt();
        println!("*** {:?} {:?}", xyz, w);
        Quaternion::from_xyzw(xyz[0], xyz[1], xyz[2], w)
    }
}

impl From<Quaternion<f32>> for SPZQuaternion {
    fn from(q: Quaternion<f32>) -> Self {
        fn convert(v: f32, w: f32) -> u8 {
            encode(v, -1.0, 1.0)
        }
        let q = q.normalized();
        let x = convert(q.x, q.w);
        let y = convert(q.y, q.w);
        let z = convert(q.z, q.w);
        SPZQuaternion([x, y, z])
    }
}

#[cfg(test)]
mod test_quaternion {

    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn test() {
        let q = vek::Quaternion::from_xyzw(0.9401622, 0.09101284, -0.03510389, 0.090294436);
        let spz_q: SPZQuaternion = q.into();
        let q2: Quaternion<f32> = spz_q.into();
        assert_relative_eq!(q.normalized().x, q2.x, epsilon = 1e-1);
        assert_relative_eq!(q.normalized().y, q2.y, epsilon = 1e-1);
        assert_relative_eq!(q.normalized().z, q2.z, epsilon = 1e-1);
        assert_relative_eq!(q.normalized().w, q2.w, epsilon = 1e-1);


        let q = vek::Quaternion::from_xyzw(1.0344028, -0.19919053, -0.10477345, -0.014542822);
        let spz_q: SPZQuaternion = q.into();
        let q2: Quaternion<f32> = spz_q.into();
        assert_relative_eq!(q.normalized().x, q2.x, epsilon = 1e-1);
        assert_relative_eq!(q.normalized().y, q2.y, epsilon = 1e-1);
        assert_relative_eq!(q.normalized().z, q2.z, epsilon = 1e-1);
        assert_relative_eq!(q.normalized().w, q2.w, epsilon = 1e-1);
    }
}
