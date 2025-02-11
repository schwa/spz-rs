use vek::vec::Vec3;

/// A helper to get the component-wise min of two Vec3<f64>.
fn min_by_component(a: Vec3<f32>, b: Vec3<f32>) -> Vec3<f32> {
    Vec3::new(a.x.min(b.x), a.y.min(b.y), a.z.min(b.z))
}

/// A helper to get the component-wise max of two Vec3<f32>.
fn max_by_component(a: Vec3<f32>, b: Vec3<f32>) -> Vec3<f32> {
    Vec3::new(a.x.max(b.x), a.y.max(b.y), a.z.max(b.z))
}

/// This function “rotates” and/or flips (x, y, z) inside a subcube of size `s`,
/// based on which bits (rx, ry, rz) are 0 or 1.
///
/// * We assume x, y, z are already in [0..s).
/// * `rx, ry, rz` each ∈ {0,1}, extracted from the current bit.
/// * After this, (x, y, z) is transformed so the next iteration can proceed.
///
/// This is adapted from a known approach for 3D Hilbert curves in LSB→MSB order.
fn rotate_and_flip_3d(x: &mut u32, y: &mut u32, z: &mut u32, s: u32, rx: u32, ry: u32, rz: u32) {
    // If s < 2, no flipping is needed because there's no space to rotate within.
    if s <= 1 {
        return;
    }
    let n = s - 1;

    // If we're “below” in z, handle the x–y plane
    if rz == 0 {
        // If we're “below” in y
        if ry == 0 {
            // Possibly flip x,y
            if rx == 1 {
                *x = n - *x;
                *y = n - *y;
            }
            // Then swap x,y
            std::mem::swap(&mut (*x), &mut (*y));
        } else {
            // ry == 1
            // No swap, but maybe do something with x?
            // We'll do a fairly standard pattern:
            if rx == 1 {
                // Flip both
                *x = n - *x;
                *y = n - *y;
            }
            // No swap for ry==1 in the “rz==0” plane (some references do a swap here).
        }
    } else {
        // rz == 1 => “above” in z
        // We do a vertical rotation pattern, e.g. swap y,z or x,z
        if ry == 0 {
            // Possibly flip x,z
            if rx == 1 {
                *x = n - *x;
                *z = n - *z;
            }
            // Then swap x,z
            std::mem::swap(&mut (*x), &mut (*z));
        } else {
            // ry == 1
            // Possibly flip y,z
            if rx == 1 {
                *y = n - *y;
                *z = n - *z;
            }
            // Then swap y,z
            std::mem::swap(&mut (*y), &mut (*z));
        }
    }
}

/// Computes a 3D Hilbert index for (x, y, z) in the range [0..(2^bits - 1)].
///
/// - Processes bits from **least** significant to most.
/// - Each iteration extracts 1 bit from x,y,z → (rx,ry,rz).
/// - Rotates/flips the partial coordinates inside a subcube of size 2^i.
/// - Appends those 3 bits to the running Hilbert index.
///
/// This avoids underflow because we clamp x,y,z to [0..s) each step
/// and skip flips if s < 2.
fn hilbert_index_3d(mut x: u32, mut y: u32, mut z: u32, bits: u32) -> u64 {
    let mut index = 0u64;

    // We'll go bit-by-bit from i=0 (LSB) to i=bits-1 (MSB).
    for i in 0..bits {
        // s = subcube side length = 2^i
        let s = 1 << i;

        // rx, ry, rz = the i-th bit of x, y, z
        let rx = (x >> i) & 1;
        let ry = (y >> i) & 1;
        let rz = (z >> i) & 1;

        // Combine them into a 3-bit octant in [0..7], but we store them bit-by-bit
        let octant = (rx << 2) | (ry << 1) | rz;

        // We shift the final index by 3 and add `octant`
        index |= (octant as u64) << (3 * i);

        // Rotate/flip the partial coordinates in [0..s], so next iteration is consistent
        // First, clamp them down to the range 0..s (just to be safe).
        x &= s - 1; // i bits in x
        y &= s - 1;
        z &= s - 1;

        rotate_and_flip_3d(&mut x, &mut y, &mut z, s, rx, ry, rz);
    }

    index
}

pub fn hilbert_sort<T, F>(points: &Vec<T>, position: F) -> Vec<T>
where
    T: Clone,
    F: Fn(&T) -> Vec3<f32>,
{
    let mut min_corner = Vec3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY);
    let mut max_corner = Vec3::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
    for p in points {
        min_corner = min_by_component(min_corner, position(p));
        max_corner = max_by_component(max_corner, position(p));
    }

    // 2) Choose number of bits for discretizing your bounding box
    let bits = 16;
    let max_coord = (1 << bits) - 1; // up to 65535

    // Avoid zero range if all points coincide in any axis
    let sx = (max_corner.x - min_corner.x).max(f32::EPSILON);
    let sy = (max_corner.y - min_corner.y).max(f32::EPSILON);
    let sz = (max_corner.z - min_corner.z).max(f32::EPSILON);

    // 3) Discretize each point to [0..(2^bits -1)], compute Hilbert index
    let mut indexed: Vec<(u64, T)> = points
        .iter()
        .map(|p| {
            let pos = position(p);
            let dx = ((pos.x - min_corner.x) / sx * max_coord as f32).round() as u32;
            let dy = ((pos.y - min_corner.y) / sy * max_coord as f32).round() as u32;
            let dz = ((pos.z - min_corner.z) / sz * max_coord as f32).round() as u32;

            let h = hilbert_index_3d(dx, dy, dz, bits);
            (h, p.clone())
        })
        .collect();

    // 4) Sort by the Hilbert index
    indexed.sort_by_key(|&(h, _)| h);

    // 5) Extract sorted points
    let sorted_points: Vec<T> = indexed.into_iter().map(|(_, p)| p).collect();

    sorted_points
}
