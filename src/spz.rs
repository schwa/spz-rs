use anyhow::Result;
use bytemuck::{Pod, Zeroable};
use flate2::write::GzEncoder;
use flate2::Compression;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::vec;
use vek::{Vec3, Vec4};

use crate::support::{compute_fixed_point_fractional_bits, inv_sigmoid, sigmoid, FixedPoint24};
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
    fn from_bytes(bytes: &[u8]) -> Self {
        Self {
            magic: u32::from_le_bytes(bytes[0..4].try_into().unwrap()),
            version: u32::from_le_bytes(bytes[4..8].try_into().unwrap()),
            num_points: u32::from_le_bytes(bytes[8..12].try_into().unwrap()),
            sh_degree: bytes[12],
            fractional_bits: bytes[13],
            flags: bytes[14],
            reserved: bytes[15],
        }
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
    skip_spherical_harmonics: bool,
) -> Result<()> {
    println!("Count: {:?}", gaussians.len());

    let sh_count = gaussians
        .iter()
        .map(|g| g.spherical_harmonics.len())
        .collect::<std::collections::HashSet<_>>();

    if sh_count.len() != 1 {
        return Err(anyhow::anyhow!(
            "All gaussians must have the same spherical harmonic degree"
        ));
    }
    let sh_count = sh_count.iter().next().unwrap();
    let sh_degree = match sh_count {
        0 => 0,
        3 => 1,
        8 => 2,
        15 => 3,
        _ => {
            return Err(anyhow::anyhow!(format!(
                "Invalid number of spherical harmonics: {}",
                sh_count
            )))
        }
    };
    println!("sh_degree: {}", sh_degree);

    let positions: Vec<f32> = gaussians
        .iter()
        .flat_map(|g| g.position.iter())
        .cloned()
        .collect();
    let fractional_bits = compute_fixed_point_fractional_bits(&positions, 24);

    println!("Fractional bits: {}", fractional_bits);
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
        for &v in q {
            let v = v * if q[3] < 0.0 { -127.5 } else { 127.5 };
            let v = v + 127.5;
            rotation_data.push(v as u8);
        }
    }
    assert!(rotation_data.len() == gaussians.len() * 4);
    stream.write_all(&rotation_data)?;
    drop(rotation_data);

    if !skip_spherical_harmonics {
        unimplemented!();
        // let mut sh_data = Vec::new();
        // for gaussian in gaussians {
        //     for sh in &gaussian.spherical_harmonics {
        //         for &v in sh {
        //             let v = (v * 127.0 + 128.0) as u8;
        //             sh_data.push(v);
        //         }
        //     }
        // }
        // assert!(sh_data.len() == gaussians.len() * sh_count * 3);
        // stream.write_all(&sh_data)?;
        // drop(sh_data);
    }

    Ok(())
}

pub fn write_spz(
    gaussians: Vec<UnpackedGaussian>,
    path: &PathBuf,
    compressed: bool,
    skip_spherical_harmonics: bool,
) -> Result<()> {
    let file = File::create(path)?;
    if compressed {
        let mut stream = GzEncoder::new(file, Compression::best());
        write_spz_to_stream(&gaussians, &mut stream, skip_spherical_harmonics)?;
        stream.finish()?;
    } else {
        let mut stream = Box::new(file);
        write_spz_to_stream(&gaussians, &mut stream, skip_spherical_harmonics)?;
    }
    Ok(())
}

pub fn load_spz(path: &PathBuf, compressed: bool) -> Result<Vec<UnpackedGaussian>> {
    let file = File::open(path)?;
    let mut file = std::io::BufReader::new(file);
    if !compressed {
        load_spz_from_stream(&mut file)
    } else {
        let mut decoder = flate2::read::GzDecoder::new(file);
        load_spz_from_stream(&mut decoder)
    }
}

fn load_spz_from_stream(file: &mut dyn std::io::Read) -> Result<Vec<UnpackedGaussian>> {
    let mut header_bytes = [0; std::mem::size_of::<Header>()];
    file.read_exact(&mut header_bytes)?;
    let header = Header::from_bytes(&header_bytes);
    if !header.is_valid() {
        return Err(anyhow::anyhow!("Invalid header"));
    }

    let mut position_data = vec![0; header.num_points as usize * 3 * 3];
    file.read_exact(&mut position_data)?;
    let positions: Vec<Vec3<f32>> = position_data
        .chunks_exact(3)
        .map(|chunk| {
            let chunk: [u8; 3] = chunk.try_into().unwrap();
            FixedPoint24::from(chunk, header.fractional_bits as usize).0
        })
        .collect::<Vec<_>>()
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
            Vec4::new(xyz[0], xyz[1], xyz[2], w)
        })
        .collect::<Vec<_>>();
    drop(rotation_data);

    // TODO: ignore sh for now

    let gaussians: Vec<UnpackedGaussian> = positions
        .into_iter()
        .zip(scales)
        .zip(rotations)
        .zip(alphas)
        .zip(colors)
        .map(
            |((((position, scales), rotation), alpha), color)| UnpackedGaussian {
                position,
                scales,
                rotation,
                alpha,
                color,
                spherical_harmonics: Vec::new(),
            },
        )
        .collect();

    Ok(gaussians)
}

#[cfg(test)]
mod tests {
    use approx::assert_relative_eq;

    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;
    use crate::unpacked_gaussian::*;

    #[test]
    fn test_spz() {
        let gaussian = UnpackedGaussian {
            position: Vec3::new(100.0, 200.0, -100.0),
            rotation: Vec4::new(0.0, 0.0, 0.0, 1.0),
            scales: Vec3::new(1.0, 2.0, 1.0),
            color: Vec3::new(1.0, 0.5, 0.25),
            alpha: 0.95,
            spherical_harmonics: Vec::new(),
        };

        let mut buffer = Vec::new();

        write_spz_to_stream(&vec![gaussian.clone()], &mut buffer, true).unwrap();

        let result = load_spz_from_stream(&mut buffer.as_slice()).unwrap();
        assert!(result.len() == 1);
        let result = &result[0];
        println!("{:?}", gaussian);
        println!("{:?}", result);
        assert!(gaussian.position == result.position);
        assert!(gaussian.scales == result.scales);
        assert_relative_eq!(gaussian.rotation[0], result.rotation[0], epsilon = 1e-2);
        assert_relative_eq!(gaussian.rotation[1], result.rotation[1], epsilon = 1e-2);
        assert_relative_eq!(gaussian.rotation[2], result.rotation[2], epsilon = 1e-2);
        assert_relative_eq!(gaussian.rotation[3], result.rotation[3], epsilon = 1e-2);
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
        assert_relative_eq!(gaussian.rotation[0], 0.0, epsilon = 1e-2);
        assert_relative_eq!(gaussian.rotation[1], 0.0, epsilon = 1e-2);
        assert_relative_eq!(gaussian.rotation[2], 1.0, epsilon = 1e-2);
        assert_relative_eq!(gaussian.rotation[3], 0.0, epsilon = 1e-2);
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
