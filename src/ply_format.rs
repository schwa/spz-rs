use anyhow::Result;
use ply_rs::parser;
use ply_rs::ply;
use ply_rs::ply::{
    Addable, DefaultElement, ElementDef, Encoding, Ply, Property, PropertyDef, PropertyType,
    ScalarType,
};
use ply_rs::writer::Writer;
use std::io::{BufRead, Write};
use std::path::Path;
use vek::{Quaternion, Vec3};

use crate::spherical_harmonics::SphericalHarmonics;
use crate::unpacked_gaussian::UnpackedGaussian;

impl ply::PropertyAccess for UnpackedGaussian {
    fn new() -> Self {
        Self {
            position: Vec3::zero(),
            rotation: Quaternion::identity(),
            scales: Vec3::one(),
            color: Vec3::zero(),
            alpha: 0.0,
            spherical_harmonics: SphericalHarmonics::default(),
        }
    }

    fn set_property(&mut self, property_name: String, property: ply::Property) {
        match (property_name.as_ref(), property) {
            ("x", ply::Property::Float(v)) => self.position[0] = v,
            ("y", ply::Property::Float(v)) => self.position[1] = v,
            ("z", ply::Property::Float(v)) => self.position[2] = v,
            ("rot_0", ply::Property::Float(v)) => self.rotation.x = v,
            ("rot_1", ply::Property::Float(v)) => self.rotation.y = v,
            ("rot_2", ply::Property::Float(v)) => self.rotation.z = v,
            ("rot_3", ply::Property::Float(v)) => self.rotation.w = v,
            ("scale_0", ply::Property::Float(v)) => self.scales[0] = v,
            ("scale_1", ply::Property::Float(v)) => self.scales[1] = v,
            ("scale_2", ply::Property::Float(v)) => self.scales[2] = v,
            ("opacity", ply::Property::Float(v)) => self.alpha = v,
            ("nx", _) => (),
            ("ny", _) => (),
            ("nz", _) => (),
            ("f_dc_0", ply::Property::Float(v)) => self.color[0] = v,
            ("f_dc_1", ply::Property::Float(v)) => self.color[1] = v,
            ("f_dc_2", ply::Property::Float(v)) => self.color[2] = v,
            (name, ply::Property::Float(v)) if name.starts_with("f_rest_") => {
                let index: usize = name["f_rest_".len()..].parse().unwrap();
                self.spherical_harmonics.extend_scalar(index, v);
            }
            (k, _) => panic!("Vertex: Unexpected key/value combination: key: {}", k),
        }
    }
}

pub fn load_ply_stream<T: BufRead>(stream: &mut T) -> Result<Vec<UnpackedGaussian>> {
    let gaussian_parser = parser::Parser::<UnpackedGaussian>::new();
    let header = gaussian_parser.read_header(stream)?;
    let mut gaussian_list = Vec::new();
    for (_ignore_key, element) in &header.elements {
        match element.name.as_ref() {
            "vertex" => {
                gaussian_list =
                    gaussian_parser.read_payload_for_element(stream, element, &header)?;
            }
            _ => return Err(anyhow::anyhow!("unknown element")),
        }
    }
    Ok(gaussian_list)
}

pub fn write_ply_stream<W: Write>(gaussians: &Vec<UnpackedGaussian>, stream: &mut W) -> Result<()> {
    let mut ply = {
        let mut ply = Ply::<DefaultElement>::new();
        ply.header.encoding = Encoding::BinaryLittleEndian;

        let mut element = ElementDef::new("vertex".to_string());
        element.properties.add(PropertyDef::new(
            "x".to_string(),
            PropertyType::Scalar(ScalarType::Float),
        ));
        element.properties.add(PropertyDef::new(
            "y".to_string(),
            PropertyType::Scalar(ScalarType::Float),
        ));
        element.properties.add(PropertyDef::new(
            "z".to_string(),
            PropertyType::Scalar(ScalarType::Float),
        ));
        element.properties.add(PropertyDef::new(
            "rot_0".to_string(),
            PropertyType::Scalar(ScalarType::Float),
        ));
        element.properties.add(PropertyDef::new(
            "rot_1".to_string(),
            PropertyType::Scalar(ScalarType::Float),
        ));
        element.properties.add(PropertyDef::new(
            "rot_2".to_string(),
            PropertyType::Scalar(ScalarType::Float),
        ));
        element.properties.add(PropertyDef::new(
            "rot_3".to_string(),
            PropertyType::Scalar(ScalarType::Float),
        ));
        element.properties.add(PropertyDef::new(
            "scale_0".to_string(),
            PropertyType::Scalar(ScalarType::Float),
        ));
        element.properties.add(PropertyDef::new(
            "scale_1".to_string(),
            PropertyType::Scalar(ScalarType::Float),
        ));
        element.properties.add(PropertyDef::new(
            "scale_2".to_string(),
            PropertyType::Scalar(ScalarType::Float),
        ));
        element.properties.add(PropertyDef::new(
            "opacity".to_string(),
            PropertyType::Scalar(ScalarType::Float),
        ));
        element.properties.add(PropertyDef::new(
            "nx".to_string(),
            PropertyType::Scalar(ScalarType::Float),
        ));
        element.properties.add(PropertyDef::new(
            "ny".to_string(),
            PropertyType::Scalar(ScalarType::Float),
        ));
        element.properties.add(PropertyDef::new(
            "nz".to_string(),
            PropertyType::Scalar(ScalarType::Float),
        ));
        element.properties.add(PropertyDef::new(
            "f_dc_0".to_string(),
            PropertyType::Scalar(ScalarType::Float),
        ));
        element.properties.add(PropertyDef::new(
            "f_dc_1".to_string(),
            PropertyType::Scalar(ScalarType::Float),
        ));
        element.properties.add(PropertyDef::new(
            "f_dc_2".to_string(),
            PropertyType::Scalar(ScalarType::Float),
        ));
        for i in 0..gaussians[0].spherical_harmonics.order().index() {
            element.properties.add(PropertyDef::new(
                format!("f_rest_{}", i).to_string(),
                PropertyType::Scalar(ScalarType::Float),
            ));
        }

        ply.header.elements.add(element);

        let mut records = Vec::new();

        for gaussian in gaussians {
            let mut record = DefaultElement::new();

            record.insert("x".to_string(), Property::Float(gaussian.position.x));
            record.insert("y".to_string(), Property::Float(gaussian.position.y));
            record.insert("z".to_string(), Property::Float(gaussian.position.z));
            record.insert("rot_0".to_string(), Property::Float(gaussian.rotation.x));
            record.insert("rot_1".to_string(), Property::Float(gaussian.rotation.y));
            record.insert("rot_2".to_string(), Property::Float(gaussian.rotation.z));
            record.insert("rot_3".to_string(), Property::Float(gaussian.rotation.w));
            record.insert("scale_0".to_string(), Property::Float(gaussian.scales.x));
            record.insert("scale_1".to_string(), Property::Float(gaussian.scales.y));
            record.insert("scale_2".to_string(), Property::Float(gaussian.scales.z));
            record.insert("opacity".to_string(), Property::Float(gaussian.alpha));
            record.insert("nx".to_string(), Property::Float(0.0));
            record.insert("ny".to_string(), Property::Float(0.0));
            record.insert("nz".to_string(), Property::Float(0.0));
            record.insert("f_dc_0".to_string(), Property::Float(gaussian.color.x));
            record.insert("f_dc_1".to_string(), Property::Float(gaussian.color.y));
            record.insert("f_dc_2".to_string(), Property::Float(gaussian.color.z));

            for (i, v) in gaussian.spherical_harmonics.scalars().iter().enumerate() {
                record.insert(format!("f_rest_{}", i).to_string(), Property::Float(*v));
            }

            records.push(record)
        }

        ply.payload.insert("vertex".to_string(), records);

        ply
    };

    // set up a writer
    let w = Writer::new();
    w.write_ply(stream, &mut ply)?;

    Ok(())
}

pub fn load_ply(path: &Path) -> Result<Vec<UnpackedGaussian>> {
    let file = std::fs::File::open(path)?;
    let mut stream = std::io::BufReader::new(file);
    load_ply_stream(&mut stream)
}

pub fn write_ply(gaussians: &Vec<UnpackedGaussian>, path: &Path) -> Result<()> {
    let mut file = std::fs::File::create(path)?;
    write_ply_stream(gaussians, &mut file)
}

#[cfg(test)]
mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;

    #[test]
    fn test_ply_load_save() {
        // TODO: Test with spherical harmonics
        let ply = r#"ply
format ascii 1.0
element vertex 1
property float x
property float y
property float z
property float f_dc_0
property float f_dc_1
property float f_dc_2
property float opacity
property float scale_0
property float scale_1
property float scale_2
property float rot_0
property float rot_1
property float rot_2
property float rot_3
end_header
100.0 200.0 -100.0 1.0 0.5 0.25 0.95 1.0 -1.0 1.0 0.333 0.333 0.333 1.0
    "#;
        let mut stream = std::io::BufReader::new(ply.as_bytes());
        let gaussians = load_ply_stream(&mut stream).unwrap();
        assert_eq!(gaussians.len(), 1);
        assert_eq!(gaussians[0].position, Vec3::new(100.0, 200.0, -100.0));
        assert_eq!(
            gaussians[0].color,
            Vec3::new(0.78209484, 0.6410474, 0.5705237)
        );
        assert_eq!(gaussians[0].alpha, 0.61325896);
        assert_eq!(gaussians[0].scales, Vec3::new(1.0, -1.0, 1.0));
        assert_eq!(
            gaussians[0].rotation,
            Quaternion::from_xyzw(0.333, 0.333, 0.333, 1.0)
        );
        assert_eq!(gaussians[0].spherical_harmonics.order().index(), 0);
        let mut output = Vec::new();
        write_ply_stream(&gaussians, &mut output).unwrap();
        let mut stream = std::io::BufReader::new(output.as_slice());
        let result = load_ply_stream(&mut stream).unwrap();
        assert!(gaussians.len() == result.len());
        assert_eq!(gaussians[0], result[0]);
    }
}
