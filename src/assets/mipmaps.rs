//! Average texture radiance in linear RGB, excluding invisible texel colors.
use image::{Rgba, RgbaImage};
use std::sync::OnceLock;
fn linear() -> &'static [f32; 256] {
    static TABLE: OnceLock<[f32; 256]> = OnceLock::new();
    TABLE.get_or_init(|| {
        std::array::from_fn(|i| {
            let c = i as f32 / 255.;
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        })
    })
}
fn srgb(c: f32) -> u8 {
    let c = c.clamp(0., 1.);
    let s = if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1. / 2.4) - 0.055
    };
    (s * 255.).round() as u8
}
pub fn level(source: &RgbaImage, size: u32) -> RgbaImage {
    assert!(
        size > 0 && source.width().is_multiple_of(size) && source.height().is_multiple_of(size)
    );
    if source.dimensions() == (size, size) {
        return source.clone();
    }
    let (sx, sy) = (source.width() / size, source.height() / size);
    let table = linear();
    RgbaImage::from_fn(size, size, |x, y| {
        let mut color = [0.; 3];
        let mut alpha = 0.;
        for yy in y * sy..(y + 1) * sy {
            for xx in x * sx..(x + 1) * sx {
                let p = source.get_pixel(xx, yy);
                let a = p[3] as f32 / 255.;
                alpha += a;
                for c in 0..3 {
                    color[c] += table[p[c] as usize] * a;
                }
            }
        }
        if alpha == 0. {
            return Rgba([0; 4]);
        }
        Rgba([
            srgb(color[0] / alpha),
            srgb(color[1] / alpha),
            srgb(color[2] / alpha),
            (alpha / (sx * sy) as f32 * 255.).round() as u8,
        ])
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lower_mips_keep_opaque_linear_energy_instead_of_averaging_encoded_srgb() {
        let source = RgbaImage::from_fn(64, 64, |x, y| {
            let c = if (x + y) % 2 == 0 { 255 } else { 0 };
            Rgba([c, c, c, 255])
        });
        for size in [32, 16, 8, 4, 2, 1] {
            let out = level(&source, size);
            assert!(out.pixels().all(|p| p.0 == [188, 188, 188, 255]));
        }
        assert_eq!(level(&source, 64), source);
    }
    #[test]
    fn transparent_black_cannot_darken_colored_foliage_when_resolution_changes() {
        let source = RgbaImage::from_fn(64, 64, |x, y| {
            if (x + y) % 2 == 0 {
                Rgba([43, 151, 62, 255])
            } else {
                Rgba([0; 4])
            }
        });
        for size in [32, 16, 8, 4, 2, 1] {
            assert!(level(&source, size)
                .pixels()
                .all(|p| p.0 == [43, 151, 62, 128]));
        }
        assert!(level(&RgbaImage::new(64, 64), 1)
            .pixels()
            .all(|p| p.0 == [0; 4]));
    }
}
