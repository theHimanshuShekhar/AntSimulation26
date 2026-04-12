/// FBM Perlin noise generator.
/// Produces values in [0.0, 1.0] over a GRID_W × GRID_H field.

// Ken Perlin's reference permutation table
const PERM: [u8; 256] = [
    151,160,137, 91, 90, 15,131, 13,201, 95, 96, 53,194,233,  7,225,
    140, 36,103, 30, 69,142,  8, 99, 37,240, 21, 10, 23,190,  6,148,
    247,120,234, 75,  0, 26,197, 62, 94,252,219,203,117, 35, 11, 32,
     57,177, 33, 88,237,149, 56, 87,174, 20,125,136,171,168, 68,175,
     74,165, 71,134,139, 48, 27,166, 77,146,158,231, 83,111,229,122,
     60,211,133,230,220,105, 92, 41, 55, 46,245, 40,244,102,143, 54,
     65, 25, 63,161,  1,216, 80, 73,209, 76,132,187,208, 89, 18,169,
    200,196,135,130,116,188,159, 86,164,100,109,198,173,186,  3, 64,
     52,217,226,250,124,123,  5,202, 38,147,118,126,255, 82, 85,212,
    207,206, 59,227, 47, 16, 58, 17,182,189, 28, 42,223,183,170,213,
    119,248,152,  2, 44,154,163, 70,221,153,101,155,167, 43,172,  9,
    129, 22, 39,253, 19, 98,108,110, 79,113,224,232,178,185,112,104,
    218,246, 97,228,251, 34,242,193,238,210,144, 12,191,179,162,241,
     81, 51,145,235,249, 14,239,107, 49,192,214, 31,181,199,106,157,
    184, 84,204,176,115,121, 50, 45,127,  4,150,254,138,236,205, 93,
    222,114, 67, 29, 24, 72,243,141,128,195, 78, 66,215, 61,156,180,
];

fn fade(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + t * (b - a)
}

fn grad2(hash: u8, x: f32, y: f32) -> f32 {
    match hash & 3 {
        0 =>  x + y,
        1 => -x + y,
        2 =>  x - y,
        _ => -x - y,
    }
}

fn perlin2(x: f32, y: f32) -> f32 {
    let xi = (x.floor() as i32).rem_euclid(256) as usize;
    let yi = (y.floor() as i32).rem_euclid(256) as usize;
    let xf = x - x.floor();
    let yf = y - y.floor();
    let u = fade(xf);
    let v = fade(yf);

    let p = |i: usize| PERM[i & 255] as usize;
    let aa = p(p(xi)     + yi);
    let ab = p(p(xi)     + yi + 1);
    let ba = p(p(xi + 1) + yi);
    let bb = p(p(xi + 1) + yi + 1);

    lerp(
        lerp(
            grad2(PERM[aa], xf,       yf),
            grad2(PERM[ba], xf - 1.0, yf),
            u,
        ),
        lerp(
            grad2(PERM[ab], xf,       yf - 1.0),
            grad2(PERM[bb], xf - 1.0, yf - 1.0),
            u,
        ),
        v,
    )
}

/// Generate an FBM (Fractal Brownian Motion) noise field.
///
/// Returns a `Vec<f32>` of length `grid_w * grid_h` with values in `[0.0, 1.0]`.
/// `seed` offsets the sampling position so different seeds produce different terrain.
pub fn generate_fbm(
    grid_w: usize,
    grid_h: usize,
    scale: f32,
    layers: usize,
    lacunarity: f32,
    persistence: f32,
    seed: f32,
) -> Vec<f32> {
    let mut field = vec![0.0f32; grid_w * grid_h];

    // Distinct axis offsets avoid diagonal symmetry in the noise pattern
    let sx = seed;
    let sy = seed * 1.7;

    for gy in 0..grid_h {
        for gx in 0..grid_w {
            // Normalized coordinates in [0, 1]
            let nx = gx as f32 / grid_w as f32;
            let ny = gy as f32 / grid_h as f32;

            let mut value = 0.0f32;
            let mut amplitude = 1.0f32;
            let mut frequency = scale;
            let mut max_val = 0.0f32;

            for _ in 0..layers {
                value += perlin2(nx * frequency + sx, ny * frequency + sy) * amplitude;
                max_val += amplitude;
                frequency *= lacunarity;
                amplitude *= persistence;
            }

            // Map from approximately [-max_val, max_val] to [0, 1]
            field[gy * grid_w + gx] = (value / max_val) * 0.5 + 0.5;
        }
    }

    field
}
