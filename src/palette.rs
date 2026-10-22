//! Project colors from DESIGN.md: main, tint, border and ink, plus the soft
//! background used for a selected option in the new-project form.

use slint::Color;

use crate::model::ColorName;
use crate::Pal;

fn hex(rgb: u32) -> Color {
    Color::from_rgb_u8((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

pub fn pal(name: ColorName) -> Pal {
    let (main, tint, border, ink, soft) = match name {
        ColorName::Amber => (0xF0B445, 0x2A2418, 0x4A3C1E, 0x1A1405, 0x1F1C15),
        ColorName::Green => (0x7CC48A, 0x1A2A1E, 0x2F4A35, 0x0D1A10, 0x18201A),
        ColorName::Blue => (0x7FB0E0, 0x18222D, 0x2C4058, 0x0C1520, 0x171D24),
        ColorName::Rose => (0xE58FA0, 0x2C1C20, 0x553039, 0x1E0C10, 0x221A1C),
    };
    Pal { main: hex(main), tint: hex(tint), border: hex(border), ink: hex(ink), soft: hex(soft) }
}
