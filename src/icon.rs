use windows_sys::Win32::Graphics::Gdi::{
    CreateBitmap, CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, BITMAPINFO,
    BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateIconIndirect, DestroyIcon, GetSystemMetrics, HICON, ICONINFO, SM_CXSMICON,
};

// Colors (RGBA):
pub const COLOR_GREEN: [u8; 4] = [34, 197, 94, 255]; // > 40%: Vibrant emerald green (#22c55e)
pub const COLOR_AMBER: [u8; 4] = [251, 191, 36, 255]; // 21 - 40%: Bright amber (#fbbf24)
pub const COLOR_RED: [u8; 4] = [239, 68, 68, 255]; // <= 20% or Disconnected: Crisp alert red (#ef4444)
pub const COLOR_CYAN: [u8; 4] = [6, 182, 212, 255]; // Charging: Electric cyan (#06b6d4)
pub const COLOR_GRAY: [u8; 4] = [203, 213, 225, 255]; // Sleeping: Slate silver (#cbd5e1)
pub const COLOR_OUTLINE_ALPHA: u8 = 160; // 1px subtle dark outline for contrast on light taskbars

pub fn get_status_color(
    battery: u8,
    is_charging: bool,
    connected: bool,
    sleeping: bool,
) -> [u8; 4] {
    if !connected {
        COLOR_RED // Bold red X when disconnected
    } else if sleeping {
        COLOR_GRAY // Silver Z when sleeping
    } else if is_charging {
        COLOR_CYAN // Electric cyan number when charging
    } else if battery <= 20 {
        COLOR_RED
    } else if battery <= 40 {
        COLOR_AMBER
    } else {
        COLOR_GREEN
    }
}

// =========================================================================
// 20x20 Native Font Table (Used for 125% DPI display scaling)
// =========================================================================

// 8x13 bold digits 0-9 (8 bits per row, MSB is col 0, LSB is col 7)
const GLYPHS_8X13: [[u8; 13]; 10] = [
    [
        0x3C, 0x7E, 0xC3, 0xC3, 0xC3, 0xC3, 0xC3, 0xC3, 0xC3, 0xC3, 0xC3, 0x7E, 0x3C,
    ], // 0
    [
        0x18, 0x38, 0x78, 0x38, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x7E, 0x7E,
    ], // 1
    [
        0x3C, 0x7E, 0xC3, 0x03, 0x07, 0x0E, 0x1C, 0x38, 0x70, 0xE0, 0xC0, 0xFF, 0xFF,
    ], // 2
    [
        0x3C, 0x7E, 0xC3, 0x03, 0x07, 0x1E, 0x0F, 0x03, 0x03, 0x03, 0xC3, 0x7E, 0x3C,
    ], // 3
    [
        0x0C, 0x1C, 0x3C, 0x6C, 0xCC, 0xCC, 0xFF, 0xFF, 0x0C, 0x0C, 0x0C, 0x1E, 0x1E,
    ], // 4
    [
        0xFF, 0xFF, 0xC0, 0xC0, 0xFE, 0xFF, 0x03, 0x03, 0x03, 0x03, 0xC3, 0x7E, 0x3C,
    ], // 5
    [
        0x1E, 0x3F, 0x70, 0xC0, 0xFE, 0xFF, 0xC3, 0xC3, 0xC3, 0xC3, 0xC3, 0x7E, 0x3C,
    ], // 6
    [
        0xFF, 0xFF, 0x03, 0x06, 0x0C, 0x18, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30,
    ], // 7
    [
        0x3C, 0x7E, 0xC3, 0xC3, 0x7E, 0x3C, 0x7E, 0xC3, 0xC3, 0xC3, 0xC3, 0x7E, 0x3C,
    ], // 8
    [
        0x3C, 0x7E, 0xC3, 0xC3, 0xC3, 0xC3, 0xFF, 0x7F, 0x03, 0x06, 0x0E, 0x7C, 0x38,
    ], // 9
];

// Narrow 4x13 "1" for three-digit 100 in 20x20
const GLYPH_1_NARROW_20: [u8; 13] = [
    0x04, 0x0C, 0x0E, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C, 0x0F, 0x0F,
];

// Narrow 6x13 "0" for three-digit 100 in 20x20
const GLYPH_0_NARROW_20: [u8; 13] = [
    0x1E, 0x3F, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x3F, 0x1E,
];

// 14x12 bold Disconnected "X" in 20x20 (3px thick stroke, 14 bits MSB at bit 13)
const GLYPH_X_20: [u16; 12] = [
    0x3807, // ###........###
    0x1C0E, // .###......###.
    0x0E1C, // ..###....###..
    0x0738, // ...###..###...
    0x03F0, // ....######....
    0x01E0, // .....####.....
    0x01E0, // .....####.....
    0x03F0, // ....######....
    0x0738, // ...###..###...
    0x0E1C, // ..###....###..
    0x1C0E, // .###......###.
    0x3807, // ###........###
];

// 12x12 bold Sleeping "Z" in 20x20 (3px thick diagonal, 12 bits MSB at bit 11)
const GLYPH_Z_20: [u16; 12] = [
    0x0FFF, // ############
    0x0FFF, // ############
    0x000E, // ........###.
    0x001C, // .......###..
    0x0038, // ......###...
    0x0070, // .....###....
    0x00E0, // ....###.....
    0x01C0, // ...###......
    0x0380, // ..###.......
    0x0700, // .###........
    0x0FFF, // ############
    0x0FFF, // ############
];

// =========================================================================
// 16x16 Native Font Table (Used for 100% DPI display scaling)
// =========================================================================

// 6x11 bold digits 0-9 (6 bits per row, bit 5 is col 0, bit 0 is col 5)
const GLYPHS_6X11: [[u8; 11]; 10] = [
    [
        0x1E, 0x3F, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x3F, 0x1E,
    ], // 0
    [
        0x0C, 0x1C, 0x3C, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C, 0x3F, 0x3F,
    ], // 1
    [
        0x1E, 0x3F, 0x33, 0x03, 0x06, 0x0C, 0x18, 0x30, 0x30, 0x3F, 0x3F,
    ], // 2
    [
        0x1E, 0x3F, 0x03, 0x07, 0x0F, 0x07, 0x03, 0x03, 0x33, 0x3F, 0x1E,
    ], // 3
    [
        0x06, 0x0E, 0x1E, 0x36, 0x36, 0x3F, 0x3F, 0x06, 0x06, 0x0F, 0x0F,
    ], // 4
    [
        0x3F, 0x3F, 0x30, 0x3E, 0x3F, 0x03, 0x03, 0x03, 0x33, 0x3F, 0x1E,
    ], // 5
    [
        0x1E, 0x3F, 0x30, 0x3E, 0x3F, 0x33, 0x33, 0x33, 0x33, 0x3F, 0x1E,
    ], // 6
    [
        0x3F, 0x3F, 0x03, 0x06, 0x06, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C,
    ], // 7
    [
        0x1E, 0x3F, 0x33, 0x33, 0x1E, 0x3F, 0x33, 0x33, 0x33, 0x3F, 0x1E,
    ], // 8
    [
        0x1E, 0x3F, 0x33, 0x33, 0x33, 0x3F, 0x3F, 0x03, 0x03, 0x3F, 0x1E,
    ], // 9
];

// Narrow 3x11 "1" for three-digit 100 in 16x16
const GLYPH_1_NARROW_16: [u8; 11] = [
    0x02, 0x06, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x07, 0x07,
];

// Narrow 5x11 "0" for three-digit 100 in 16x16
const GLYPH_0_NARROW_16: [u8; 11] = [
    0x0E, 0x1F, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1F, 0x0E,
];

// 12x12 bold Disconnected "X" in 16x16 (2px thick stroke, 12 bits MSB at bit 11)
const GLYPH_X_16: [u16; 12] = [
    0x0C03, // ##........##
    0x0606, // .##......##.
    0x030C, // ..##....##..
    0x0198, // ...##..##...
    0x00F0, // ....####....
    0x0060, // .....##.....
    0x0060, // .....##.....
    0x00F0, // ....####....
    0x0198, // ...##..##...
    0x030C, // ..##....##..
    0x0606, // .##......##.
    0x0C03, // ##........##
];

// 10x10 bold Sleeping "Z" in 16x16 (2px thick stroke, 10 bits MSB at bit 9)
const GLYPH_Z_16: [u16; 10] = [
    0x03FF, // ##########
    0x03FF, // ##########
    0x000C, // ......##..
    0x0018, // .....##...
    0x0030, // ....##....
    0x0060, // ...##.....
    0x00C0, // ..##......
    0x0180, // .##.......
    0x03FF, // ##########
    0x03FF, // ##########
];

pub struct IconRenderer;

impl IconRenderer {
    /// Renders a native 20x20 foreground boolean mask.
    pub fn render_mask_20(
        battery: u8,
        is_charging: bool,
        connected: bool,
        sleeping: bool,
    ) -> [bool; 20 * 20] {
        let mut mask = [false; 20 * 20];
        let mut plot = |x: usize, y: usize| {
            if x < 20 && y < 20 {
                mask[y * 20 + x] = true;
            }
        };

        if !connected {
            // Draw 14x12 bold "X" centered: x in 3..16, y in 4..15
            for (row, &bits) in GLYPH_X_20.iter().enumerate() {
                for col in 0..14 {
                    if (bits & (1 << (13 - col))) != 0 {
                        plot(3 + col, 4 + row);
                    }
                }
            }
        } else if sleeping {
            // Draw 12x12 bold "Z" centered: x in 4..15, y in 4..15
            for (row, &bits) in GLYPH_Z_20.iter().enumerate() {
                for col in 0..12 {
                    if (bits & (1 << (11 - col))) != 0 {
                        plot(4 + col, 4 + row);
                    }
                }
            }
        } else if battery >= 100 {
            // Draw "100" (1 at x=1..4, 0 at x=6..11, 0 at x=13..18, y in 3..15)
            for (row, &bits) in GLYPH_1_NARROW_20.iter().enumerate() {
                for col in 0..4 {
                    if (bits & (1 << (3 - col))) != 0 {
                        plot(1 + col, 3 + row);
                    }
                }
            }
            for (row, &bits) in GLYPH_0_NARROW_20.iter().enumerate() {
                for col in 0..6 {
                    if (bits & (1 << (5 - col))) != 0 {
                        plot(6 + col, 3 + row);
                        plot(13 + col, 3 + row);
                    }
                }
            }
        } else if battery >= 10 {
            // Draw two 8x13 digits side-by-side:
            // d1 at x=1..8, 1px gap at x=9, d2 at x=10..17, y in 3..15
            let d1 = (battery / 10) as usize;
            let d2 = (battery % 10) as usize;
            for (row, &bits) in GLYPHS_8X13[d1].iter().enumerate() {
                for col in 0..8 {
                    if (bits & (1 << (7 - col))) != 0 {
                        plot(1 + col, 3 + row);
                    }
                }
            }
            for (row, &bits) in GLYPHS_8X13[d2].iter().enumerate() {
                for col in 0..8 {
                    if (bits & (1 << (7 - col))) != 0 {
                        plot(10 + col, 3 + row);
                    }
                }
            }
        } else {
            // Draw single 8x13 digit centered: x in 6..13, y in 3..15
            let d = battery as usize;
            for (row, &bits) in GLYPHS_8X13[d].iter().enumerate() {
                for col in 0..8 {
                    if (bits & (1 << (7 - col))) != 0 {
                        plot(6 + col, 3 + row);
                    }
                }
            }
        }

        // Small 2x2 charging dot in top-right corner if charging
        if is_charging && connected && !sleeping {
            plot(18, 1);
            plot(19, 1);
            plot(18, 2);
            plot(19, 2);
        }

        mask
    }

    /// Renders a native 16x16 foreground boolean mask.
    pub fn render_mask_16(
        battery: u8,
        is_charging: bool,
        connected: bool,
        sleeping: bool,
    ) -> [bool; 16 * 16] {
        let mut mask = [false; 16 * 16];
        let mut plot = |x: usize, y: usize| {
            if x < 16 && y < 16 {
                mask[y * 16 + x] = true;
            }
        };

        if !connected {
            // Draw 12x12 bold "X" centered: x in 2..13, y in 2..13
            for (row, &bits) in GLYPH_X_16.iter().enumerate() {
                for col in 0..12 {
                    if (bits & (1 << (11 - col))) != 0 {
                        plot(2 + col, 2 + row);
                    }
                }
            }
        } else if sleeping {
            // Draw 10x10 bold "Z" centered: x in 3..12, y in 3..12
            for (row, &bits) in GLYPH_Z_16.iter().enumerate() {
                for col in 0..10 {
                    if (bits & (1 << (9 - col))) != 0 {
                        plot(3 + col, 3 + row);
                    }
                }
            }
        } else if battery >= 100 {
            // Draw "100" in 16x16: 1 at x=1..3, 0 at x=5..9, 0 at x=11..15, y in 2..12
            for (row, &bits) in GLYPH_1_NARROW_16.iter().enumerate() {
                for col in 0..3 {
                    if (bits & (1 << (2 - col))) != 0 {
                        plot(1 + col, 2 + row);
                    }
                }
            }
            for (row, &bits) in GLYPH_0_NARROW_16.iter().enumerate() {
                for col in 0..5 {
                    if (bits & (1 << (4 - col))) != 0 {
                        plot(5 + col, 2 + row);
                        plot(11 + col, 2 + row);
                    }
                }
            }
        } else if battery >= 10 {
            // Draw two 6x11 digits: d1 at x=1..6, d2 at x=8..13, y in 2..12
            let d1 = (battery / 10) as usize;
            let d2 = (battery % 10) as usize;
            for (row, &bits) in GLYPHS_6X11[d1].iter().enumerate() {
                for col in 0..6 {
                    if (bits & (1 << (5 - col))) != 0 {
                        plot(1 + col, 2 + row);
                    }
                }
            }
            for (row, &bits) in GLYPHS_6X11[d2].iter().enumerate() {
                for col in 0..6 {
                    if (bits & (1 << (5 - col))) != 0 {
                        plot(8 + col, 2 + row);
                    }
                }
            }
        } else {
            // Single 6x11 digit centered: x in 5..10, y in 2..12
            let d = battery as usize;
            for (row, &bits) in GLYPHS_6X11[d].iter().enumerate() {
                for col in 0..6 {
                    if (bits & (1 << (5 - col))) != 0 {
                        plot(5 + col, 2 + row);
                    }
                }
            }
        }

        // Small 2x2 charging dot in top-right corner if charging
        if is_charging && connected && !sleeping {
            plot(14, 1);
            plot(15, 1);
            plot(14, 2);
            plot(15, 2);
        }

        mask
    }

    /// Converts a boolean mask of dimensions `(width, height)` into 32-bit pre-multiplied BGRA,
    /// adding a crisp 1-pixel dark outline around foreground pixels for universal dark/light contrast.
    pub fn mask_to_bgra(mask: &[bool], width: usize, height: usize, fg_color: [u8; 4]) -> Vec<u8> {
        let mut bgra = vec![0u8; width * height * 4];

        for y in 0..height {
            for x in 0..width {
                let idx = (y * width + x) * 4;
                if mask[y * width + x] {
                    // Foreground pixel (fully opaque, vivid color)
                    let a = fg_color[3];
                    let r = (fg_color[0] as u32 * a as u32 / 255) as u8;
                    let g = (fg_color[1] as u32 * a as u32 / 255) as u8;
                    let b = (fg_color[2] as u32 * a as u32 / 255) as u8;

                    bgra[idx] = b; // Blue
                    bgra[idx + 1] = g; // Green
                    bgra[idx + 2] = r; // Red
                    bgra[idx + 3] = a; // Alpha
                } else {
                    // Check if adjacent to a foreground pixel (orthogonal 4-connectivity)
                    let has_fg_neighbor = (x > 0 && mask[y * width + (x - 1)])
                        || (x + 1 < width && mask[y * width + (x + 1)])
                        || (y > 0 && mask[(y - 1) * width + x])
                        || (y + 1 < height && mask[(y + 1) * width + x]);

                    if has_fg_neighbor {
                        // 1px black outline with subtle alpha (160/255)
                        // Pre-multiplied black is (0, 0, 0, alpha)
                        bgra[idx] = 0;
                        bgra[idx + 1] = 0;
                        bgra[idx + 2] = 0;
                        bgra[idx + 3] = COLOR_OUTLINE_ALPHA;
                    }
                }
            }
        }

        bgra
    }

    /// Creates a native Windows HICON at the system's exact small icon resolution.
    pub fn create_hicon(battery: u8, is_charging: bool, connected: bool, sleeping: bool) -> HICON {
        let sys_metric = unsafe { GetSystemMetrics(SM_CXSMICON) as usize };
        let icon_size = sys_metric.clamp(16, 64);
        let color = get_status_color(battery, is_charging, connected, sleeping);

        let bgra: Vec<u8> = if icon_size <= 18 {
            // Native 16x16 for 100% DPI (96 DPI)
            let mask = Self::render_mask_16(battery, is_charging, connected, sleeping);
            let native_bgra = Self::mask_to_bgra(&mask, 16, 16, color);
            if icon_size == 16 {
                native_bgra
            } else {
                Self::center_or_pad_bgra(&native_bgra, 16, 16, icon_size)
            }
        } else if icon_size <= 22 {
            // Native 20x20 for 125% DPI (120 DPI) - EXACT physical match on user's system!
            let mask = Self::render_mask_20(battery, is_charging, connected, sleeping);
            let native_bgra = Self::mask_to_bgra(&mask, 20, 20, color);
            if icon_size == 20 {
                native_bgra
            } else {
                Self::center_or_pad_bgra(&native_bgra, 20, 20, icon_size)
            }
        } else if icon_size <= 26 {
            // 24x24 for 150% DPI: pad 20x20 with 2px border for 100% sharp pixel rendering
            let mask = Self::render_mask_20(battery, is_charging, connected, sleeping);
            let native_bgra = Self::mask_to_bgra(&mask, 20, 20, color);
            Self::center_or_pad_bgra(&native_bgra, 20, 20, icon_size)
        } else {
            // 32x32+ (200% DPI): integer 2x scale 16x16 grid for tack-sharp 2px -> 4px strokes
            let mask = Self::render_mask_16(battery, is_charging, connected, sleeping);
            let native_bgra = Self::mask_to_bgra(&mask, 16, 16, color);
            Self::integer_scale_bgra(&native_bgra, 16, 16, icon_size)
        };

        unsafe {
            let hdc = CreateCompatibleDC(0);
            let mut bi: BITMAPINFO = std::mem::zeroed();
            bi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            bi.bmiHeader.biWidth = icon_size as i32;
            bi.bmiHeader.biHeight = -(icon_size as i32); // Top-down DIB
            bi.bmiHeader.biPlanes = 1;
            bi.bmiHeader.biBitCount = 32;
            bi.bmiHeader.biCompression = BI_RGB;

            let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
            let hbm_color = CreateDIBSection(hdc, &bi, DIB_RGB_COLORS, &mut bits, 0, 0);

            if !bits.is_null() {
                std::ptr::copy_nonoverlapping(bgra.as_ptr(), bits as *mut u8, bgra.len());
            }

            // 1-bit monochrome mask for icon info (explicit zero-filled buffer prevents driver artifacts)
            let mask_stride = icon_size.div_ceil(16) * 2;
            let mask_bytes = vec![0u8; mask_stride * icon_size];
            let hbm_mask = CreateBitmap(
                icon_size as i32,
                icon_size as i32,
                1,
                1,
                mask_bytes.as_ptr() as *const _,
            );

            let icon_info = ICONINFO {
                fIcon: 1, // TRUE
                xHotspot: 0,
                yHotspot: 0,
                hbmMask: hbm_mask,
                hbmColor: hbm_color,
            };

            let hicon = CreateIconIndirect(&icon_info);

            DeleteObject(hbm_color);
            DeleteObject(hbm_mask);
            DeleteDC(hdc);

            hicon
        }
    }

    /// Centers an unscaled BGRA buffer into a larger target icon size with transparent padding.
    fn center_or_pad_bgra(src: &[u8], src_w: usize, src_h: usize, dst_size: usize) -> Vec<u8> {
        let mut dst = vec![0u8; dst_size * dst_size * 4];
        let offset_x = (dst_size.saturating_sub(src_w)) / 2;
        let offset_y = (dst_size.saturating_sub(src_h)) / 2;

        for y in 0..src_h {
            let dst_y = offset_y + y;
            if dst_y >= dst_size {
                break;
            }
            for x in 0..src_w {
                let dst_x = offset_x + x;
                if dst_x >= dst_size {
                    break;
                }
                let src_idx = (y * src_w + x) * 4;
                let dst_idx = (dst_y * dst_size + dst_x) * 4;
                dst[dst_idx..dst_idx + 4].copy_from_slice(&src[src_idx..src_idx + 4]);
            }
        }
        dst
    }

    /// Scales a BGRA buffer using nearest-neighbor / integer replication for razor-sharp edges.
    fn integer_scale_bgra(src: &[u8], src_w: usize, src_h: usize, dst_size: usize) -> Vec<u8> {
        let mut dst = vec![0u8; dst_size * dst_size * 4];
        for dst_y in 0..dst_size {
            let src_y = (dst_y * src_h) / dst_size;
            for dst_x in 0..dst_size {
                let src_x = (dst_x * src_w) / dst_size;
                let src_idx = (src_y * src_w + src_x) * 4;
                let dst_idx = (dst_y * dst_size + dst_x) * 4;
                dst[dst_idx..dst_idx + 4].copy_from_slice(&src[src_idx..src_idx + 4]);
            }
        }
        dst
    }

    pub fn destroy_hicon(icon: HICON) {
        if icon != 0 {
            unsafe {
                DestroyIcon(icon);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dpi_metric() {
        unsafe {
            windows_sys::Win32::UI::HiDpi::SetProcessDpiAwarenessContext(
                windows_sys::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
            );
            let cx = GetSystemMetrics(SM_CXSMICON);
            assert!(cx > 0, "SM_CXSMICON should be greater than zero");
        }
    }

    #[test]
    fn test_masks() {
        let m20 = IconRenderer::render_mask_20(85, false, true, false);
        assert_eq!(m20.len(), 400);

        let bgra20 = IconRenderer::mask_to_bgra(&m20, 20, 20, COLOR_GREEN);
        assert_eq!(bgra20.len(), 1600);

        let m_x = IconRenderer::render_mask_20(0, false, false, false);
        let bgra_x = IconRenderer::mask_to_bgra(&m_x, 20, 20, COLOR_RED);
        assert_eq!(bgra_x.len(), 1600);

        let m16 = IconRenderer::render_mask_16(85, false, true, false);
        assert_eq!(m16.len(), 256);
    }

    #[test]
    fn test_generate_html_preview() {
        use std::fs::File;
        use std::io::Write;

        let test_cases = [
            ("85% Normal", 85, false, true, false),
            ("35% Amber", 35, false, true, false),
            ("15% Critical", 15, false, true, false),
            ("100% Full", 100, false, true, false),
            ("7% Single", 7, false, true, false),
            ("Charging 85%", 85, true, true, false),
            ("Disconnected", 0, false, false, false),
            ("Sleeping", 85, false, true, true),
        ];

        let mut html = String::from("<!DOCTYPE html><html><head><meta charset='utf-8'><title>Tray Icon Preview</title><style>
body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; background: #0f172a; color: #f8fafc; padding: 24px; }
h1 { font-size: 20px; margin-bottom: 8px; }
p { color: #94a3b8; font-size: 14px; margin-bottom: 24px; }
.grid { display: flex; flex-wrap: wrap; gap: 20px; }
.card { background: #1e293b; border-radius: 12px; padding: 16px; border: 1px solid #334155; }
.card-title { font-size: 14px; font-weight: 600; margin-bottom: 12px; color: #e2e8f0; }
.preview-row { display: flex; gap: 16px; align-items: center; margin-bottom: 8px; }
.taskbar-dark { background: #202020; padding: 12px; border-radius: 8px; display: inline-flex; align-items: center; gap: 8px; border: 1px solid #333; }
.taskbar-light { background: #f3f3f3; padding: 12px; border-radius: 8px; display: inline-flex; align-items: center; gap: 8px; border: 1px solid #ccc; }
.pixel-canvas { image-rendering: pixelated; border-radius: 4px; }
.label { font-size: 11px; color: #64748b; }
</style></head><body>");
        html.push_str("<h1>IRIS v2 Tray Icon Previews (Native 20x20 at 125% DPI)</h1>");
        html.push_str("<p>Simulated on Windows Dark Taskbar (#202020) and Light Taskbar (#f3f3f3). Zoomed 8x for pixel inspection and 1x native.</p><div class='grid'>");

        for (label, bat, chg, conn, sleep) in test_cases {
            let color = get_status_color(bat, chg, conn, sleep);
            let mask = IconRenderer::render_mask_20(bat, chg, conn, sleep);
            let bgra = IconRenderer::mask_to_bgra(&mask, 20, 20, color);

            // Create SVG data URL or table for 20x20
            let mut svg = String::from("<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 20 20' width='20' height='20'>");
            for y in 0..20 {
                for x in 0..20 {
                    let idx = (y * 20 + x) * 4;
                    let b = bgra[idx];
                    let g = bgra[idx + 1];
                    let r = bgra[idx + 2];
                    let a = bgra[idx + 3] as f32 / 255.0;
                    if a > 0.0 {
                        svg.push_str(&format!("<rect x='{}' y='{}' width='1' height='1' fill='rgba({},{},{},{:.2})'/>", x, y, r, g, b, a));
                    }
                }
            }
            svg.push_str("</svg>");

            html.push_str(&format!(
                "<div class='card'><div class='card-title'>{}</div>",
                label
            ));
            html.push_str("<div class='preview-row'>");
            html.push_str("<div class='taskbar-dark'>");
            html.push_str(&format!(
                "<div style='width:20px;height:20px'>{}</div>",
                svg
            ));
            html.push_str(&format!("<div style='width:80px;height:80px;transform:scale(4);transform-origin:top left;'>{}</div>", svg));
            html.push_str(
                "<span class='label' style='margin-left:65px;'>Dark 1x & 4x</span></div>",
            );

            html.push_str("<div class='taskbar-light'>");
            html.push_str(&format!(
                "<div style='width:20px;height:20px'>{}</div>",
                svg
            ));
            html.push_str(&format!("<div style='width:80px;height:80px;transform:scale(4);transform-origin:top left;'>{}</div>", svg));
            html.push_str(
                "<span class='label' style='margin-left:65px;'>Light 1x & 4x</span></div>",
            );

            html.push_str("</div></div>");
        }

        html.push_str("</div></body></html>");
        let _ = std::fs::create_dir_all("target");
        let mut file = File::create("target/icon_preview.html").unwrap();
        file.write_all(html.as_bytes()).unwrap();
        assert!(std::path::Path::new("target/icon_preview.html").exists());
    }

    #[test]
    fn test_print_composite_20() {
        let cases = [
            ("85% (Battery)", 85, false, true, false),
            ("100% (Full)", 100, false, true, false),
            ("X (Disconnected)", 0, false, false, false),
            ("Z (Sleeping)", 85, false, true, true),
        ];

        for (_title, bat, chg, conn, sleep) in cases {
            let mask = IconRenderer::render_mask_20(bat, chg, conn, sleep);
            let bgra = IconRenderer::mask_to_bgra(&mask, 20, 20, [255, 255, 255, 255]);
            assert_eq!(bgra.len(), 20 * 20 * 4);
        }
    }
}
