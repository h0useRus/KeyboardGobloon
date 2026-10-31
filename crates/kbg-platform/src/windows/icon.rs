//! The tray icon: the logo's green keycap without face or ears, with the current language as
//! two letters (assets/icon/drafts/tray-en.svg), drawn at the size the taskbar wants.
//!
//! Shapes are drawn with anti-aliased edges straight into the pixels; the letters come from
//! GDI (grey-scale anti-aliased) as a coverage mask over the keycap.

use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Gdi::{
    ANTIALIASED_QUALITY, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CLIP_DEFAULT_PRECIS, CreateBitmap,
    CreateCompatibleDC, CreateDIBSection, CreateFontW, DEFAULT_CHARSET, DIB_RGB_COLORS, DT_CENTER,
    DT_SINGLELINE, DT_VCENTER, DeleteDC, DeleteObject, DrawTextW, FF_DONTCARE, FW_HEAVY, HGDIOBJ,
    OUT_DEFAULT_PRECIS, SelectObject, SetBkMode, SetTextColor, TRANSPARENT,
};
use windows::Win32::UI::WindowsAndMessaging::{CreateIconIndirect, HICON, ICONINFO};
use windows::core::w;

type Rgb = (u8, u8, u8);

const OUTLINE: Rgb = (0xff, 0xff, 0xff);
const RIM: Rgb = (0x6f, 0x9a, 0x1c);
const FACE: Rgb = (0x9a, 0xcd, 0x32);
const INK: Rgb = (0x1d, 0x2b, 0x05);

/// The drawing in the 200-unit box of tray-en.svg (its viewBox moved to the origin).
struct Shape {
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
    radius: f32,
}

const OUTLINE_SHAPE: Shape = Shape {
    left: 3.0,
    top: 3.0,
    right: 197.0,
    bottom: 197.0,
    radius: 45.0,
};
const RIM_SHAPE: Shape = Shape {
    left: 8.0,
    top: 8.0,
    right: 192.0,
    bottom: 192.0,
    radius: 40.0,
};
const FACE_SHAPE: Shape = Shape {
    left: 20.0,
    top: 12.0,
    right: 180.0,
    bottom: 168.0,
    radius: 30.0,
};

impl Shape {
    /// How much of the pixel at (`x`, `y`) the rounded rectangle covers, 0..1, at `scale`
    /// pixels per unit.
    fn coverage(&self, x: f32, y: f32, scale: f32) -> f32 {
        let r = self.radius * scale;
        let cx = (self.left + self.right) / 2.0 * scale;
        let cy = (self.top + self.bottom) / 2.0 * scale;
        let hw = (self.right - self.left) / 2.0 * scale - r;
        let hh = (self.bottom - self.top) / 2.0 * scale - r;
        let qx = (x - cx).abs() - hw;
        let qy = (y - cy).abs() - hh;
        let outside = qx.max(0.0).hypot(qy.max(0.0));
        let distance = outside + qx.max(qy).min(0.0) - r;
        (0.5 - distance).clamp(0.0, 1.0)
    }
}

/// `color` over the premultiplied pixel `px` with `alpha` 0..1.
fn over(px: &mut [f32; 4], color: Rgb, alpha: f32) {
    let keep = 1.0 - alpha;
    px[0] = f32::from(color.2) / 255.0 * alpha + px[0] * keep;
    px[1] = f32::from(color.1) / 255.0 * alpha + px[1] * keep;
    px[2] = f32::from(color.0) / 255.0 * alpha + px[2] * keep;
    px[3] = alpha + px[3] * keep;
}

/// Coverage of `letters` drawn in white on black, centered on the keycap face, as one byte per
/// pixel (row by row).
fn letters_mask(letters: &str, size: i32, scale: f32) -> Option<Vec<u8>> {
    let header = BITMAPINFOHEADER {
        biSize: size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: size,
        // Negative: rows from the top.
        biHeight: -size,
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB.0,
        ..Default::default()
    };
    let info = BITMAPINFO {
        bmiHeader: header,
        ..Default::default()
    };
    let text: Vec<u16> = letters.encode_utf16().collect();
    // SAFETY: GDI objects created here are selected out and deleted before returning; the bits
    // pointer stays valid while the DIB section lives.
    unsafe {
        let dc = CreateCompatibleDC(None);
        let mut bits = std::ptr::null_mut();
        let Ok(bitmap) = CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0)
        else {
            let _ = DeleteDC(dc);
            return None;
        };
        let old_bitmap = SelectObject(dc, HGDIOBJ(bitmap.0));
        // Two letters fill most of the face; the face is 160 units wide, 156 high.
        let height = (100.0 * scale).round() as i32;
        let font = CreateFontW(
            -height,
            0,
            0,
            0,
            FW_HEAVY.0 as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            ANTIALIASED_QUALITY,
            FF_DONTCARE.0 as u32,
            w!("Segoe UI"),
        );
        let old_font = SelectObject(dc, HGDIOBJ(font.0));
        SetBkMode(dc, TRANSPARENT);
        SetTextColor(dc, windows::Win32::Foundation::COLORREF(0x00FF_FFFF));
        let mut rect = RECT {
            left: (FACE_SHAPE.left * scale).floor() as i32,
            top: (FACE_SHAPE.top * scale).floor() as i32,
            right: (FACE_SHAPE.right * scale).ceil() as i32,
            bottom: (FACE_SHAPE.bottom * scale).ceil() as i32,
        };
        let mut text = text;
        DrawTextW(
            dc,
            &mut text,
            &mut rect,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );
        let pixels = std::slice::from_raw_parts(bits as *const u8, (size * size * 4) as usize);
        // Grey-scale text: every channel holds the coverage.
        let mask = pixels
            .as_chunks::<4>()
            .0
            .iter()
            .map(|p| p[0].max(p[1]).max(p[2]))
            .collect();
        SelectObject(dc, old_font);
        SelectObject(dc, old_bitmap);
        let _ = DeleteObject(HGDIOBJ(font.0));
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        let _ = DeleteDC(dc);
        Some(mask)
    }
}

/// The keycap with `letters`, `size` pixels square; grey and faded when `enabled` is false.
pub fn language_icon(letters: &str, size: i32, enabled: bool) -> Option<HICON> {
    let scale = size as f32 / 200.0;
    let mask = letters_mask(letters, size, scale)?;
    let mut bgra = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let mut px = [0.0f32; 4];
            over(&mut px, OUTLINE, OUTLINE_SHAPE.coverage(fx, fy, scale));
            over(&mut px, RIM, RIM_SHAPE.coverage(fx, fy, scale));
            let face = FACE_SHAPE.coverage(fx, fy, scale);
            over(&mut px, FACE, face);
            let ink = f32::from(mask[(y * size + x) as usize]) / 255.0;
            over(&mut px, INK, ink * face);
            if !enabled {
                // Grey, at 55%: off at a glance.
                let grey = 0.3 * px[2] + 0.59 * px[1] + 0.11 * px[0];
                px = [grey * 0.55, grey * 0.55, grey * 0.55, px[3] * 0.55];
            }
            bgra.extend(px.map(|v| (v * 255.0).round().clamp(0.0, 255.0) as u8));
        }
    }
    // SAFETY: the bitmaps are created from our buffer and deleted after the icon copied them.
    unsafe {
        let color = CreateBitmap(size, size, 1, 32, Some(bgra.as_ptr().cast()));
        let blank = vec![0u8; (((size + 15) / 16) * 2 * size) as usize];
        let mono = CreateBitmap(size, size, 1, 1, Some(blank.as_ptr().cast()));
        let info = ICONINFO {
            fIcon: true.into(),
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: mono,
            hbmColor: color,
        };
        let icon = CreateIconIndirect(&info).ok();
        let _ = DeleteObject(HGDIOBJ(color.0));
        let _ = DeleteObject(HGDIOBJ(mono.0));
        icon
    }
}
