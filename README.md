# spz-rs

A Rust library for reading and writing the Niantic Labs Gaussian Splat Point Cloud format <https://github.com/nianticlabs/spz/>.

## Current Status

Limited testing against Niantics original implementation has been done. There is current *no guarantee* that this library will work with all `.spz` files (yet).

This library has an accompanying CLI tool for converting `.ply` files (using the most common Gaussian Splat Point Cloud format) to `.spz` files.

## File Format

The file format is not currently well documented by Niantic. The following is based on their original implementation.

gz compressed file with the extension `.spz`.

```ascii
+-------------------------------------+
| header (16-bytes)                   |
+-------------------------------------+
| position data (24-bytes per splat)  |
+-------------------------------------+
| alpha data (1-byte per splat)       |
+-------------------------------------+
| color data (3-bytes per splat)      |
+-------------------------------------+
| scale data (3-bytes per splat)      |
+-------------------------------------+
| rotation data (3-bytes per splat)   |
+-------------------------------------+
| spherical harmonics data (varying)  |
+-------------------------------------+
```

### Header

```rust
struct Header {
    magic: u32, // Always 0x5053474e
    version: u32, // Always 0x00000002
    num_points: u32,
    sh_degree: u8, // 0, 1, or 3
    fractional_bits: u8, // 0-23
    flags: u8, // TODO: TODO
    reserved: u8, // Always 0
}
```

### Position Data

3 x 24-bit fixed point numbers, each representing the x, y, and z position of a splat. The fractional bits are specified in the header.

### Alpha Data

1 byte per splat. The sigmoid of the alpha value of a splat, scaled to 0-255.

### Color Data

3 bytes per splat. `((channel * 0.15) + 0.5) * 255.0`

### Scale Data

3 bytes per splat. `((v + 10.0) * 16.0)`

### Rotation Data

3 bytes per splat. Quaternion - drop the imaginary part, normalize and scale the real part to 0-255.

### Spherical Harmonics Data

"The data format uses 8 bits per coefficient, but when packing, we can quantize to fewer bits for better compression."

```c++
constexpr int sh1Bits = 5;
constexpr int shRestBits = 4;
const int shPerPoint = dimForDegree(g.shDegree) * 3;
for (size_t i = 0; i < numPoints * shPerPoint; i += shPerPoint) {
    size_t j = 0;
    for (; j < 9; j++) {  // There are 9 coefficients for degree 1
        packed.sh[i + j] = quantizeSH(g.sh[i + j], 1 << (8 - sh1Bits));
    }
    for (; j < shPerPoint; j++) {
        packed.sh[i + j] = quantizeSH(g.sh[i + j], 1 << (8 - shRestBits));
    }
}
// Quantizes to 8 bits, the round to nearest bucket center. 0 always maps to a bucket center.
uint8_t quantizeSH(float x, int bucketSize) {
  int q = static_cast<int>(std::round(x * 128.0f) + 128.0f);
  q = (q + bucketSize / 2) / bucketSize * bucketSize;
  return static_cast<uint8_t>(std::clamp(q, 0, 255));
}
```

## Links

<https://github.com/antimatter15/splat/blob/main/convert.py>
