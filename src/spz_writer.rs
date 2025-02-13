use anyhow::Result;
use std::io::Write;

use crate::fixedpoint24::{compute_fixed_point_fractional_bits, FixedPoint24};
use crate::spherical_harmonics::SphericalHarmonicsOrder;
use crate::support::sigmoid;
use crate::unpacked_gaussian::UnpackedGaussian;

use crate::spz_format::*;

pub struct SPZWriterOptions {
    pub omit_spherical_harmonics: bool,
}

pub struct SPZWriter<W: Write> {
    writer: W,
    options: SPZWriterOptions,
}

impl<W> SPZWriter<W>
where
    W: Write,
{
    pub fn new(writer: W, options: SPZWriterOptions) -> Self {
        Self { writer, options }
    }

    pub fn write(&mut self, gaussians: &Vec<UnpackedGaussian>) -> Result<()> {
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

        let header = SPZHeader::new(gaussians.len() as u32, sh_degree, fractional_bits as u8, 0);
        self.writer.write_all(bytemuck::bytes_of(&header))?;

        let mut position_data: Vec<u8> = Vec::new();
        for gaussian in gaussians {
            for &v in &gaussian.position {
                let f = FixedPoint24::new(v);
                position_data.extend_from_slice(&f.into(fractional_bits));
            }
        }
        assert!(position_data.len() == &gaussians.len() * 3 * 3);
        self.writer.write_all(&position_data)?;
        drop(position_data);

        let mut alpha_data = Vec::new();
        for gaussian in gaussians {
            let v = (sigmoid(gaussian.alpha) * 255.0) as u8;
            alpha_data.push(v);
        }
        assert!(alpha_data.len() == gaussians.len());
        self.writer.write_all(&alpha_data)?;
        drop(alpha_data);

        let mut color_data = Vec::new();
        for gaussian in gaussians {
            for &v in &gaussian.color {
                let v = (((v * COLOR_SCALE) + 0.5) * 255.0) as u8;
                color_data.push(v);
            }
        }
        assert!(color_data.len() == gaussians.len() * 3);
        self.writer.write_all(&color_data)?;
        drop(color_data);

        let mut scale_data = Vec::new();
        for gaussian in gaussians {
            for &v in &gaussian.scales {
                let v = ((v + 10.0) * 16.0) as u8;
                scale_data.push(v);
            }
        }
        assert!(scale_data.len() == gaussians.len() * 3);
        self.writer.write_all(&scale_data)?;
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
        assert!(rotation_data.len() == gaussians.len() * 3);
        self.writer.write_all(&rotation_data)?;
        drop(rotation_data);

        if !self.options.omit_spherical_harmonics {
            let mut sh_data = Vec::new();
            for gaussian in gaussians {
                let bytes = gaussian.spherical_harmonics.spz_bytes();
                sh_data.extend_from_slice(&bytes);
            }
            self.writer.write_all(&sh_data)?;
            drop(sh_data);
        }

        Ok(())
    }
}
