#![allow(dead_code)]

use anyhow::Result;
use std::io::Read;

pub(crate) fn linear_to_log(v: f32) -> f32 {
    1.0 - f32::exp(-v)
}

pub(crate) fn log_to_linear(v: f32) -> f32 {
    -f32::ln(1.0 - v)
}

// ---- Spherical Harmonics ----

#[allow(clippy::excessive_precision)]
const SPHERICAL_HARMONICS_ORDER0_COEFFICIENT: f32 = 0.282_094_791_773_878_14;

pub(crate) fn sph0_to_linear(v: f32) -> f32 {
    0.5 + SPHERICAL_HARMONICS_ORDER0_COEFFICIENT * v
}

pub(crate) fn linear_to_sph0(v: f32) -> f32 {
    (v - 0.5) / SPHERICAL_HARMONICS_ORDER0_COEFFICIENT
}

// float sigmoid(float x) { return 1 / (1 + std::exp(-x)); }

pub(crate) fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + f32::exp(-x))
}

// float invSigmoid(float x) { return std::log(x / (1.0f - x)); }

pub(crate) fn inv_sigmoid(x: f32) -> f32 {
    f32::ln(x / (1.0 - x))
}

pub(crate) trait ReadExt: Read {
    fn my_read_exact(&mut self, buffer: &mut [u8]) -> Result<()> {
        let mut total_read = 0;
        while total_read < buffer.len() {
            let count = self.read(&mut buffer[total_read..])?;
            if count == 0 {
                return Err(anyhow::anyhow!(format!(
                    "Failed to read {} bytes, got only {} bytes instead.",
                    buffer.len(),
                    total_read
                )));
            }
            total_read += count;
        }
        Ok(())
    }
}

impl<R: Read + ?Sized> ReadExt for R {}
