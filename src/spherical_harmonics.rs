// TODO: Currently WIP

enum SphericalHarmonicsOrder {
    Order0,
    Order1,
    Order2,
    Order3,
}

#[derive(Debug, Clone, Copy)]
enum SphericalHarmonics {
    Order0(()),
    Order1([Vec3<f32>; 3]),
    Order2([Vec3<f32>; 8]),
    Order3([Vec3<f32>; 15]),
}

impl SphericalHarmonics {
    fn order(&self) -> SphericalHarmonicsOrder {
        match self {
            SphericalHarmonics::Order0(_) => SphericalHarmonicsOrder::Order0,
            SphericalHarmonics::Order1(_) => SphericalHarmonicsOrder::Order1,
            SphericalHarmonics::Order2(_) => SphericalHarmonicsOrder::Order2,
            SphericalHarmonics::Order3(_) => SphericalHarmonicsOrder::Order3,
        }
    }

    fn expand(&self, order: SphericalHarmonicsOrder) -> Self {
        let new = match (order, self) {
            (SphericalHarmonicsOrder::Order0, SphericalHarmonics::Order0(_)) => return self.clone(),
            (SphericalHarmonicsOrder::Order0, _) => SphericalHarmonics::Order0(()),
            (SphericalHarmonicsOrder::Order1, SphericalHarmonics::Order1(_)) => return self.clone(),
            (SphericalHarmonicsOrder::Order1, _) => SphericalHarmonics::Order1([Vec3::zero(); 3]),
            (SphericalHarmonicsOrder::Order2, SphericalHarmonics::Order2(_)) => return self.clone(),
            (SphericalHarmonicsOrder::Order2, _) => SphericalHarmonics::Order2([Vec3::zero(); 8]),
            (SphericalHarmonicsOrder::Order3, SphericalHarmonics::Order3(_)) => return self.clone(),
            (SphericalHarmonicsOrder::Order3, _) => SphericalHarmonics::Order3([Vec3::zero(); 15]),
        };
        return new;
    }
}
