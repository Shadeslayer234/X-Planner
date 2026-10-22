//! One-off tool that draws the app icon and the power icon as `.ico` files.
//! The results are checked in under `assets/`, so this only needs to run when
//! the icons change:
//!
//!     cargo run --example make_icons
//!
//! Shapes are drawn with signed distance functions on a 24×24 grid (the same
//! grid as the line icons) and anti-aliased by supersampling.

use std::io::Write;

type Rgb = (u8, u8, u8);

/// One icon design: background, border, glyph color, and a distance function
/// for the glyph stroke's centre line (in 24-unit glyph coordinates).
struct Design {
    bg: Rgb,
    border: Rgb,
    fg: Rgb,
    stroke: f32,
    glyph: fn(f32, f32) -> f32,
}

fn seg(px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let (pax, pay, bax, bay) = (px - ax, py - ay, bx - ax, by - ay);
    let h = ((pax * bax + pay * bay) / (bax * bax + bay * bay)).clamp(0.0, 1.0);
    ((pax - bax * h).powi(2) + (pay - bay * h).powi(2)).sqrt()
}

fn x_glyph(x: f32, y: f32) -> f32 {
    seg(x, y, 6.0, 6.0, 18.0, 18.0).min(seg(x, y, 18.0, 6.0, 6.0, 18.0))
}

fn power_glyph(x: f32, y: f32) -> f32 {
    let line = seg(x, y, 12.0, 2.0, 12.0, 12.0);
    // A ring of radius 9 around (12, 13) with a 90° gap at the top.
    let (dx, dy) = (x - 12.0, y - 13.0);
    let r = 9.0;
    let ring = if dy < 0.0 && dx.abs() < -dy {
        let e = r * std::f32::consts::FRAC_1_SQRT_2;
        seg(x, y, 12.0 - e, 13.0 - e, 12.0 - e, 13.0 - e).min(seg(x, y, 12.0 + e, 13.0 - e, 12.0 + e, 13.0 - e))
    } else {
        ((dx * dx + dy * dy).sqrt() - r).abs()
    };
    line.min(ring)
}

/// Distance to a rounded square of half-size `h` and corner radius `r`,
/// centred at the origin. Negative inside.
fn rounded_square(x: f32, y: f32, h: f32, r: f32) -> f32 {
    let qx = x.abs() - h + r;
    let qy = y.abs() - h + r;
    (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - r
}

/// Renders one size as straight (not premultiplied) RGBA, top row first.
fn render(d: &Design, size: u32) -> Vec<[u8; 4]> {
    const SS: u32 = 4;
    let mut out = Vec::with_capacity((size * size) as usize);
    for py in 0..size {
        for px in 0..size {
            let (mut a, mut r, mut g, mut b) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
            for sy in 0..SS {
                for sx in 0..SS {
                    // Map to -1..1 across the icon.
                    let u = ((px * SS + sx) as f32 + 0.5) / (size * SS) as f32 * 2.0 - 1.0;
                    let v = ((py * SS + sy) as f32 + 0.5) / (size * SS) as f32 * 2.0 - 1.0;
                    let outer = rounded_square(u, v, 0.94, 0.30);
                    if outer > 0.0 {
                        continue;
                    }
                    // The glyph uses the middle 64% of the icon.
                    let gx = (u / 0.64 + 1.0) * 12.0;
                    let gy = (v / 0.64 + 1.0) * 12.0;
                    let c = if (d.glyph)(gx, gy) < d.stroke / 2.0 {
                        d.fg
                    } else if outer > -0.06 {
                        d.border
                    } else {
                        d.bg
                    };
                    a += 1.0;
                    r += c.0 as f32;
                    g += c.1 as f32;
                    b += c.2 as f32;
                }
            }
            let n = (SS * SS) as f32;
            if a == 0.0 {
                out.push([0, 0, 0, 0]);
            } else {
                out.push([(r / a) as u8, (g / a) as u8, (b / a) as u8, (a / n * 255.0) as u8]);
            }
        }
    }
    out
}

/// Writes a multi-size .ico with 32-bit BMP images.
fn write_ico(path: &str, d: &Design) {
    let sizes = [16u32, 20, 24, 32, 40, 48, 64, 256];
    let images: Vec<Vec<u8>> = sizes
        .iter()
        .map(|&s| {
            let px = render(d, s);
            let mut bmp = Vec::new();
            // BITMAPINFOHEADER; the height counts the colour and mask rows.
            bmp.extend(40u32.to_le_bytes());
            bmp.extend((s as i32).to_le_bytes());
            bmp.extend((s as i32 * 2).to_le_bytes());
            bmp.extend(1u16.to_le_bytes());
            bmp.extend(32u16.to_le_bytes());
            bmp.extend([0u8; 24]);
            for row in (0..s).rev() {
                for col in 0..s {
                    let [r, g, b, a] = px[(row * s + col) as usize];
                    bmp.extend([b, g, r, a]);
                }
            }
            // AND mask: all zero (alpha does the work), rows padded to 4 bytes.
            let mask_row = s.div_ceil(32) * 4;
            bmp.extend(vec![0u8; (mask_row * s) as usize]);
            bmp
        })
        .collect();

    let mut f = std::fs::File::create(path).unwrap();
    f.write_all(&[0, 0, 1, 0]).unwrap();
    f.write_all(&(sizes.len() as u16).to_le_bytes()).unwrap();
    let mut offset = 6 + 16 * sizes.len() as u32;
    for (s, img) in sizes.iter().zip(&images) {
        let dim = if *s >= 256 { 0 } else { *s as u8 };
        f.write_all(&[dim, dim, 0, 0]).unwrap();
        f.write_all(&1u16.to_le_bytes()).unwrap();
        f.write_all(&32u16.to_le_bytes()).unwrap();
        f.write_all(&(img.len() as u32).to_le_bytes()).unwrap();
        f.write_all(&offset.to_le_bytes()).unwrap();
        offset += img.len() as u32;
    }
    for img in &images {
        f.write_all(img).unwrap();
    }
}

fn main() {
    write_ico(
        "assets/app.ico",
        &Design { bg: (0x14, 0x16, 0x19), border: (0x4A, 0x3C, 0x1E), fg: (0xF0, 0xB4, 0x45), stroke: 3.4, glyph: x_glyph },
    );
    write_ico(
        "assets/power.ico",
        &Design { bg: (0x2A, 0x24, 0x18), border: (0x6B, 0x55, 0x24), fg: (0xF0, 0xB4, 0x45), stroke: 2.6, glyph: power_glyph },
    );
    println!("Wrote assets/app.ico and assets/power.ico");
}
