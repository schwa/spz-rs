use anyhow::Result;
use bytemuck::{Pod, Zeroable};
use flate2::write::GzEncoder;
use flate2::Compression;
use itertools;
use std::fs::File;
use std::io::{Seek, Write};
use std::path::Path;
use std::vec;
use vek::{Quaternion, Vec3};

use crate::fixedpoint24::{compute_fixed_point_fractional_bits, FixedPoint24};
use crate::spherical_harmonics::{SphericalHarmonics, SphericalHarmonicsOrder};
use crate::support::{inv_sigmoid, sigmoid};
use crate::unpacked_gaussian::UnpackedGaussian;

const COLOR_SCALE: f32 = 0.15;

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
struct Header {
    magic: u32,
    version: u32,
    num_points: u32,
    sh_degree: u8,
    fractional_bits: u8,
    flags: u8,
    reserved: u8,
}

impl Header {
    fn from_bytes(bytes: &[u8]) -> Result<Self> {
        Ok(Self {
            magic: u32::from_le_bytes(bytes[0..4].try_into()?),
            version: u32::from_le_bytes(bytes[4..8].try_into()?),
            num_points: u32::from_le_bytes(bytes[8..12].try_into()?),
            sh_degree: bytes[12],
            fractional_bits: bytes[13],
            flags: bytes[14],
            reserved: bytes[15],
        })
    }

    fn is_valid(&self) -> bool {
        self.magic == 0x5053474e && self.version == 2 && self.sh_degree <= 3
    }

    fn new(num_points: u32, sh_degree: u8, fractional_bits: u8, flags: u8) -> Self {
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
}

pub fn write_spz_to_stream<W: Write>(
    gaussians: &Vec<UnpackedGaussian>,
    stream: &mut W,
    omit_spherical_harmonics: bool,
) -> Result<()> {
    let sh_count = gaussians
        .iter()
        .map(|g| g.spherical_harmonics.order().index())
        .collect::<std::collections::HashSet<_>>();

    if sh_count.len() != 1 {
        return Err(anyhow::anyhow!(
            "All gaussians must have the same spherical harmonic degree"
        ));
    }
    let sh_count = *sh_count
        .iter()
        .next()
        .ok_or(anyhow::anyhow!("No spherical harmonics"))?;

    let order = SphericalHarmonicsOrder::order_for_degree(sh_count as u8);

    let positions: Vec<f32> = gaussians
        .iter()
        .flat_map(|g| g.position.iter())
        .cloned()
        .collect();
    let fractional_bits = compute_fixed_point_fractional_bits(&positions, 24);

    let sh_degree = order.ok_or(anyhow::anyhow!("Invalid SH degree"))?.index() as u8;

    let header = Header::new(gaussians.len() as u32, sh_degree, fractional_bits as u8, 0);
    stream.write_all(bytemuck::bytes_of(&header))?;

    let mut position_data: Vec<u8> = Vec::new();
    for gaussian in gaussians {
        for &v in &gaussian.position {
            let f = FixedPoint24::new(v);
            position_data.extend_from_slice(&f.into(fractional_bits));
        }
    }
    assert!(position_data.len() == &gaussians.len() * 3 * 3);
    stream.write_all(&position_data)?;
    drop(position_data);

    let mut alpha_data = Vec::new();
    for gaussian in gaussians {
        let v = (sigmoid(gaussian.alpha) * 255.0) as u8;
        alpha_data.push(v);
    }
    assert!(alpha_data.len() == gaussians.len());
    stream.write_all(&alpha_data)?;
    drop(alpha_data);

    let mut color_data = Vec::new();
    for gaussian in gaussians {
        for &v in &gaussian.color {
            let v = (((v * COLOR_SCALE) + 0.5) * 255.0) as u8;
            color_data.push(v);
        }
    }
    assert!(color_data.len() == gaussians.len() * 3);
    stream.write_all(&color_data)?;
    drop(color_data);

    let mut scale_data = Vec::new();
    for gaussian in gaussians {
        for &v in &gaussian.scales {
            let v = ((v + 10.0) * 16.0) as u8;
            scale_data.push(v);
        }
    }
    assert!(scale_data.len() == gaussians.len() * 3);
    stream.write_all(&scale_data)?;
    drop(scale_data);

    let mut rotation_data = Vec::new();
    for gaussian in gaussians {
        let q = &gaussian.rotation.normalized();
        for v in q.into_vec3() {
            let v = v * if q.w < 0.0 { -127.5 } else { 127.5 };
            let v = v + 127.5;
            rotation_data.push(v as u8);
        }
    }
    assert!(rotation_data.len() == gaussians.len() * 4);
    stream.write_all(&rotation_data)?;
    drop(rotation_data);

    if !omit_spherical_harmonics {
        let mut sh_data = Vec::new();
        for gaussian in gaussians {
            let bytes = gaussian.spherical_harmonics.spz_bytes();
            sh_data.extend_from_slice(&bytes);
        }
        stream.write_all(&sh_data)?;
        drop(sh_data);
    }

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

pub fn load_spz(path: &Path, compressed: bool) -> Result<Vec<UnpackedGaussian>> {
    let mut file = File::open(path)?;
    let mut reader = std::io::BufReader::new(&file);
    let gaussians = if !compressed {
        load_spz_from_stream(&mut reader)?
    } else {
        let mut decoder = flate2::read::GzDecoder::new(reader);
        load_spz_from_stream(&mut decoder)?
    };

    if file.stream_position()? != file.metadata()?.len() {
        return Err(anyhow::anyhow!("Did not consume all of file."));
    }
    Ok(gaussians)
}

fn load_spz_from_stream(file: &mut dyn std::io::Read) -> Result<Vec<UnpackedGaussian>> {
    let mut header_bytes = [0; std::mem::size_of::<Header>()];
    file.read_exact(&mut header_bytes)?;
    let header = Header::from_bytes(&header_bytes)?;
    if !header.is_valid() {
        return Err(anyhow::anyhow!("Invalid header"));
    }

    let mut position_data = vec![0; header.num_points as usize * 3 * 3];
    file.read_exact(&mut position_data)?;
    let positions: Vec<Vec3<f32>> = position_data
        .chunks_exact(3)
        .map(|chunk| {
            let chunk: [u8; 3] = chunk.try_into()?;
            Ok(FixedPoint24::from(chunk, header.fractional_bits as usize).0)
        })
        .collect::<Result<Vec<_>>>()?
        .chunks(3)
        .map(|chunk| Vec3::new(chunk[0], chunk[1], chunk[2]))
        .collect();
    drop(position_data);

    let mut alpha_data = vec![0; header.num_points as usize];
    file.read_exact(&mut alpha_data)?;
    let alphas = alpha_data
        .iter()
        .map(|&v| inv_sigmoid(v as f32 / 255.0))
        .collect::<Vec<_>>();
    drop(alpha_data);

    let mut color_data = vec![0; header.num_points as usize * 3];
    file.read_exact(&mut color_data)?;
    let colors = color_data
        .iter()
        .map(|&v| (v as f32 / 255.0 - 0.5) / COLOR_SCALE)
        .collect::<Vec<_>>()
        .chunks(3)
        .map(|chunk| Vec3::new(chunk[0], chunk[1], chunk[2]))
        .collect::<Vec<_>>();
    drop(color_data);

    let mut scale_data = vec![0; header.num_points as usize * 3];
    file.read_exact(&mut scale_data)?;
    let scales = scale_data
        .iter()
        .map(|&v| (v as f32 / 16.0 - 10.0))
        .collect::<Vec<_>>();
    let scales = scales
        .chunks(3)
        .map(|chunk| Vec3::new(chunk[0], chunk[1], chunk[2]))
        .collect::<Vec<_>>();
    drop(scale_data);

    let mut rotation_data = vec![0; header.num_points as usize * 3];
    file.read_exact(&mut rotation_data)?;
    let rotations = rotation_data
        .iter()
        .map(|&v| v as f32)
        .collect::<Vec<_>>()
        .chunks(3)
        .map(|v| {
            let xyz = Vec3::new(v[0], v[1], v[2]) * 1.0 / 127.5 + Vec3::new(-1.0, -1.0, -1.0);
            let w = f32::max(0.0, 1.0 - xyz.dot(xyz)).sqrt();
            Quaternion::from_xyzw(xyz[0], xyz[1], xyz[2], w)
        })
        .collect::<Vec<_>>();
    drop(rotation_data);

    let order = SphericalHarmonicsOrder::order_for_degree(header.sh_degree)
        .ok_or(anyhow::anyhow!("Invalid SH degree"))?;
    let spherical_harmonics = if order != SphericalHarmonicsOrder::Order0 {
        let scalar_count = order.scalar_count();
        let mut spherical_harmonics_data = vec![0; header.num_points as usize * scalar_count];
        file.read_exact(&mut spherical_harmonics_data)?;
        spherical_harmonics_data
            .chunks(scalar_count)
            .map(|chunk| SphericalHarmonics::from_spz_bytes(chunk.to_vec()))
            .collect::<Vec<_>>()
    } else {
        vec![SphericalHarmonics::default(); header.num_points as usize]
    };

    let gaussians: Vec<UnpackedGaussian> = itertools::izip!(
        positions.iter(),
        scales.iter(),
        rotations.iter(),
        alphas.iter(),
        colors.iter(),
        spherical_harmonics.iter()
    )
    .map({
        |(position, scale, rotation, alpha, color, spherical_harmonics)| UnpackedGaussian {
            position: *position,
            scales: *scale,
            rotation: *rotation,
            alpha: *alpha,
            color: *color,
            spherical_harmonics: *spherical_harmonics,
        }
    })
    .collect();

    Ok(gaussians)
}

const SH1_BITS: i32 = 5;
const SH_REST_BITS: i32 = 4;

impl SphericalHarmonics {
    fn from_spz_bytes(bytes: Vec<u8>) -> Self {
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

    fn spz_bytes(&self) -> Vec<u8> {
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
    use crate::unpacked_gaussian::*;

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

        write_spz_to_stream(&vec![gaussian.clone()], &mut buffer, true).unwrap();

        let result = load_spz_from_stream(&mut buffer.as_slice()).unwrap();
        assert!(result.len() == 1);
        let result = &result[0];
        assert!(gaussian.position == result.position);
        assert!(gaussian.scales == result.scales);
        assert_relative_eq!(gaussian.rotation.x, result.rotation.x, epsilon = 1e-2);
        assert_relative_eq!(gaussian.rotation.y, result.rotation.y, epsilon = 1e-2);
        assert_relative_eq!(gaussian.rotation.z, result.rotation.z, epsilon = 1e-2);
        assert_relative_eq!(gaussian.rotation.w, result.rotation.w, epsilon = 1e-2);
        assert_relative_eq!(gaussian.color[0], result.color[0], epsilon = 1e-1);
        assert_relative_eq!(gaussian.color[1], result.color[1], epsilon = 1e-1);
        assert_relative_eq!(gaussian.color[2], result.color[2], epsilon = 1e-1);
        assert_relative_eq!(gaussian.alpha, result.alpha, epsilon = 1e-1);
        assert!(gaussian.spherical_harmonics == result.spherical_harmonics);
    }

    #[test]
    fn test_single() {
        // This is from the "official" niantic spz file. Source looks like this:
        // "x": 100.0, "y": 200.0, "z": -100.0, "f_dc_0": 1.0, "f_dc_1": 0.5, "f_dc_2": 0.25, "opacity": 0.95, "scale_0": 1.0. "scale_1": -1.0, "scale_2": 10, "rot0": 0.0, "rot_1": 0.0, "rot_2": 0.0, "rot_3": 1.0
        let hex = "4E475350 02000000 01000000 000C0000 00400600 800C00C0 F9B8A693 89B090B0 8080FF";
        let bytes = dehex(hex);
        let mut buffer = std::io::Cursor::new(bytes);
        let gaussians = load_spz_from_stream(&mut buffer).unwrap();
        assert!(gaussians.len() == 1);
        let gaussian = &gaussians[0];
        assert!(gaussian.position == Vec3::new(100.0, 200.0, -100.0));
        assert!(gaussian.scales == Vec3::new(1.0, -1.0, 1.0));
        assert_relative_eq!(gaussian.rotation.x, 0.0, epsilon = 1e-2);
        assert_relative_eq!(gaussian.rotation.y, 0.0, epsilon = 1e-2);
        assert_relative_eq!(gaussian.rotation.z, 0.0, epsilon = 1e-2);
        assert_relative_eq!(gaussian.rotation.w, 1.0, epsilon = 1e-2);
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
}
