use anyhow::{Context, Result};
use itertools;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::vec;
use vek::{Quaternion, Vec3};

use crate::fixedpoint24::FixedPoint24;
use crate::spherical_harmonics::{SphericalHarmonics, SphericalHarmonicsOrder};
use crate::support::{inv_sigmoid, ReadExt};
use crate::unpacked_gaussian::UnpackedGaussian;

use crate::spz_format::*;

use flate2::read::GzDecoder;

#[derive(Debug, Default)]
pub struct SPZReaderOptions {
    pub skip_compression: bool,
}

impl SPZReaderOptions {
    pub fn new(skip_compression: bool) -> Self {
        Self {
            skip_compression,
        }
    }

    pub fn skip_compression(mut self, skip: bool) -> Self {
        self.skip_compression = skip;
        self
    }
}

pub struct SPZReader<'a> {
    reader: Box<dyn Read + 'a>,
    pub header: Option<SPZHeader>,
    pub gaussians: Option<Vec<UnpackedGaussian>>,
}

impl<'a> SPZReader<'a> {
    pub fn new(reader: Box<dyn Read + 'a>, options: SPZReaderOptions) -> Self {
        let reader = if !options.skip_compression {
            Box::new(GzDecoder::new(reader))
        } else {
            reader
        };

        SPZReader {
            reader,
            header: None,
            gaussians: None,
        }
    }

    pub fn new_from_slice(slice: &'a [u8], options: SPZReaderOptions) -> Self {
        Self::new(Box::new(std::io::Cursor::new(slice)), options)
    }

    pub fn new_from_path(path: &Path, options: SPZReaderOptions) -> Result<Self> {
        let file = File::open(path)?;
        Ok(Self::new(Box::new(file), options))
    }

    pub fn read(&mut self) -> Result<Vec<UnpackedGaussian>> {
        self.read_header()?;
        self.read_gaussians()?;
        Ok(self.gaussians.as_ref().unwrap().clone()) // TODO: Expensive clone.
    }

    pub fn read_header(&mut self) -> Result<SPZHeader> {
        let mut header_bytes = [0; std::mem::size_of::<SPZHeader>()];
        self.reader.my_read_exact(&mut header_bytes)?;
        let header: SPZHeader = *bytemuck::from_bytes(&header_bytes);
        if !header.is_valid() {
            return Err(anyhow::anyhow!("Invalid header"));
        }
        self.header = Some(header);
        Ok(header)
    }

    pub fn read_gaussians(&mut self) -> Result<Vec<UnpackedGaussian>> {
        let header = self.header.as_ref().ok_or(anyhow::anyhow!("No header"))?;

        let mut position_data = vec![0; header.num_points as usize * 3 * 3];
        self.reader.my_read_exact(&mut position_data)?;
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
        self.reader.my_read_exact(&mut alpha_data)?;
        let alphas = alpha_data
            .iter()
            .map(|&v| inv_sigmoid(v as f32 / 255.0))
            .collect::<Vec<_>>();
        drop(alpha_data);

        let mut color_data = vec![0; header.num_points as usize * 3];
        self.reader.my_read_exact(&mut color_data)?;
        let colors = color_data
            .iter()
            .map(|&v| (v as f32 / 255.0 - 0.5) / COLOR_SCALE)
            .collect::<Vec<_>>()
            .chunks(3)
            .map(|chunk| Vec3::new(chunk[0], chunk[1], chunk[2]))
            .collect::<Vec<_>>();
        drop(color_data);

        let mut scale_data = vec![0; header.num_points as usize * 3];
        self.reader.my_read_exact(&mut scale_data)?;
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
        self.reader.my_read_exact(&mut rotation_data)?;
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
            let count = header.num_points as usize * scalar_count;
            let mut spherical_harmonics_data = vec![0; count];
            self.reader
                .my_read_exact(&mut spherical_harmonics_data)
                .context(format!("Failed to read SH data (wanted {} bytes)", count))?;
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

        self.gaussians = Some(gaussians);

        Ok(self.gaussians.as_ref().unwrap().clone()) // TODO: Expensive clone.
    }
}
