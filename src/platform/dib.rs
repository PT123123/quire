// Device-independent bitmap → PNG, with no Win32 in sight (SPEC §三十七 批次 A's
// last open item: paste a screenshot). The clipboard reader in `super` hands
// this the bytes `GetClipboardData(CF_DIBV5 | CF_DIB)` returns; keeping the two
// apart is what lets the decode run as an ordinary test on any machine.

use std::io::Cursor;

const BI_RGB: u32 = 0;
const BI_BITFIELDS: u32 = 3;

/// A clipboard bitmap can be any size another app decided to publish; this is
// the point where a hostile or buggy one stops being our problem.
const MAX_EDGE: i32 = 16_384;
const MAX_PIXELS: i64 = 64_000_000;

/// Decode a DIB (the CF_DIB / CF_DIBV5 payload: a BITMAPINFOHEADER and
/// everything after it, with no BITMAPFILEHEADER) into PNG bytes.
pub fn dib_to_png(bytes: &[u8]) -> Option<Vec<u8>> {
    let image = decode(bytes)?;
    let mut png = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut png, image::ImageFormat::Png)
        .ok()?;
    Some(png.into_inner())
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

fn i32_at(b: &[u8], at: usize) -> Option<i32> {
    Some(i32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

fn u16_at(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(at..at + 2)?.try_into().ok()?))
}

/// One channel of a packed pixel: isolate the mask, shift it down, and scale
/// whatever width it has up to 8 bits (a 5-bit green is not 5 bits of nothing).
fn channel(v: u32, mask: u32) -> u8 {
    if mask == 0 {
        return 0;
    }
    let shift = mask.trailing_zeros();
    let bits = mask.count_ones();
    let raw = (v & mask) >> shift;
    ((raw as u64 * 255) / ((1u64 << bits) - 1)) as u8
}

struct Dib<'a> {
    width: i32,
    height: i32,
    top_down: bool,
    bpp: u16,
    masks: (u32, u32, u32, u32),
    palette: Vec<[u8; 3]>,
    pixels: &'a [u8],
    stride: usize,
}

fn default_masks(bpp: u16) -> (u32, u32, u32, u32) {
    match bpp {
        16 => (0x7C00, 0x03E0, 0x001F, 0),
        24 => (0x00FF_0000, 0x0000_FF00, 0x0000_00FF, 0),
        32 => (0x00FF_0000, 0x0000_FF00, 0x0000_00FF, 0xFF00_0000),
        _ => (0, 0, 0, 0),
    }
}

fn parse<'a>(b: &'a [u8]) -> Option<Dib<'a>> {
    let header = u32_at(b, 0)?;
    // 40 = BITMAPINFOHEADER, 56 = V3, 108 = V4, 124 = V5. A 12-byte
    // BITMAPCOREHEADER has 16-bit fields and a 3-byte palette; nothing on a
    // modern clipboard writes one.
    if !matches!(header, 40 | 56 | 108 | 124) {
        return None;
    }
    let width = i32_at(b, 4)?;
    let raw_height = i32_at(b, 8)?;
    let bpp = u16_at(b, 14)?;
    let compression = u32_at(b, 16)?;
    if width <= 0 || raw_height == 0 {
        return None;
    }
    let height = raw_height.abs();
    if width > MAX_EDGE || height > MAX_EDGE || i64::from(width) * i64::from(height) > MAX_PIXELS {
        return None;
    }
    if !matches!(compression, BI_RGB | BI_BITFIELDS) {
        return None; // RLE4/RLE8/JPEG/PNG payloads are not a pixel array
    }
    if !matches!(bpp, 1 | 4 | 8 | 16 | 24 | 32) {
        return None;
    }

    let mut after = header as usize;
    let mut masks = default_masks(bpp);
    if compression == BI_BITFIELDS {
        // V4 and V5 carry their masks inside the header; a plain
        // BITMAPINFOHEADER carries them right after it.
        let (r, g, bl, a) = if header >= 108 {
            (
                u32_at(b, 52)?,
                u32_at(b, 56)?,
                u32_at(b, 60)?,
                u32_at(b, 64)?,
            )
        } else {
            after += 12;
            (
                u32_at(b, header as usize)?,
                u32_at(b, header as usize + 4)?,
                u32_at(b, header as usize + 8)?,
                if bpp == 32 { 0xFF00_0000 } else { 0 },
            )
        };
        masks = (r, g, bl, a);
    }

    let palette = if bpp <= 8 {
        // biClrUsed (offset 32) is either the entry count or 0 for "all of
        // them". Entries are BGRA; the fourth byte is reserved and always 0,
        // so a palette colour is opaque by definition.
        let used = u32_at(b, 32).unwrap_or(0) as usize;
        let full = 1usize << bpp;
        let count = if used > 0 && used < full { used } else { full };
        let end = after + count * 4;
        b.get(after..end)?
            .chunks_exact(4)
            .map(|c| [c[2], c[1], c[0]])
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    after += palette.len() * 4;

    let stride = ((usize::from(bpp) * width as usize + 31) / 32) * 4;
    let need = after + stride * height as usize;
    if b.len() < need {
        return None;
    }
    Some(Dib {
        width,
        height,
        top_down: raw_height < 0,
        bpp,
        masks,
        palette,
        pixels: &b[after..],
        stride,
    })
}

fn decode(b: &[u8]) -> Option<image::RgbaImage> {
    let dib = parse(b)?;
    let (w, h) = (dib.width as u32, dib.height as u32);
    let mut buf = vec![0u8; w as usize * h as usize * 4];
    for y in 0..h {
        // a positive biHeight means the array starts at the picture's bottom
        let row = if dib.top_down { y } else { h - 1 - y };
        let src = &dib.pixels[(row as usize) * dib.stride..];
        for x in 0..w as usize {
            let at = (y as usize * w as usize + x) * 4;
            buf[at..at + 4].copy_from_slice(&color(&dib, src, x).0);
        }
    }
    let mut out = image::RgbaImage::from_raw(w, h, buf)?;
    // Screenshots carry a garbage alpha byte, usually all zero. Left alone it
    // pastes an invisible picture, which reads as a failed paste.
    if dib.bpp == 32 && dib.masks.3 != 0 && out.pixels().all(|p| p[3] == 0) {
        for p in out.pixels_mut() {
            p[3] = 255;
        }
    }
    Some(out)
}

fn color(dib: &Dib, src: &[u8], x: usize) -> image::Rgba<u8> {
    let (rm, gm, bm, am) = dib.masks;
    let (r, g, b, a) = match dib.bpp {
        32 => {
            let v = u32_at(src, x * 4).unwrap_or(0);
            (channel(v, rm), channel(v, gm), channel(v, bm), channel(v, am))
        }
        24 => {
            let i = x * 3;
            let v = u32::from(src[i]) | (u32::from(src[i + 1]) << 8) | (u32::from(src[i + 2]) << 16);
            (channel(v, rm), channel(v, gm), channel(v, bm), 255)
        }
        16 => {
            let v = u16_at(src, x * 2).unwrap_or(0) as u32;
            (channel(v, rm), channel(v, gm), channel(v, bm), 255)
        }
        bpp => {
            let index = match bpp {
                8 => u32::from(src[x]),
                4 => {
                    let byte = src[x / 2];
                    if x % 2 == 0 {
                        (byte >> 4) as u32
                    } else {
                        (byte & 0x0F) as u32
                    }
                }
                _ => {
                    let byte = src[x / 8];
                    u32::from((byte >> (7 - (x % 8))) & 1)
                }
            };
            match dib.palette.get(index as usize) {
                Some(p) => (p[0], p[1], p[2], 255),
                None => (0, 0, 0, 255),
            }
        }
    };
    image::Rgba([r, g, b, a])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A BITMAPINFOHEADER: 40 bytes, in field order.
    fn header(bpp: u16, w: i32, h: i32, compression: u32) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&40u32.to_le_bytes()); // biSize
        v.extend_from_slice(&w.to_le_bytes()); // biWidth
        v.extend_from_slice(&h.to_le_bytes()); // biHeight
        v.extend_from_slice(&1u16.to_le_bytes()); // biPlanes
        v.extend_from_slice(&bpp.to_le_bytes()); // biBitCount
        v.extend_from_slice(&compression.to_le_bytes()); // biCompression
        v.extend_from_slice(&0u32.to_le_bytes()); // biSizeImage
        v.extend_from_slice(&0i32.to_le_bytes()); // biXPelsPerMeter
        v.extend_from_slice(&0i32.to_le_bytes()); // biYPelsPerMeter
        v.extend_from_slice(&0u32.to_le_bytes()); // biClrUsed
        v.extend_from_slice(&0u32.to_le_bytes()); // biClrImportant
        assert_eq!(v.len(), 40);
        v
    }

    fn decode_png(png: &[u8]) -> image::RgbaImage {
        image::load_from_memory(png).unwrap().to_rgba8()
    }

    #[test]
    fn a_bottom_up_dib_is_flipped_and_a_top_down_one_is_left_alone() {
        // DIB bytes are B,G,R(,A). Bottom-up: the array's first row is the
        // picture's *second* row.
        let mut b = header(24, 1, 2, BI_RGB);
        b.extend_from_slice(&[255, 0, 0, 0]); // array row 0 = bottom, blue
        b.extend_from_slice(&[0, 0, 255, 0]); // array row 1 = top, red
        let img = decode_png(&dib_to_png(&b).unwrap());
        assert_eq!(img.get_pixel(0, 0).0, [255, 0, 0, 255], "top is red");
        assert_eq!(img.get_pixel(0, 1).0, [0, 0, 255, 255], "bottom is blue");

        let mut t = header(24, 1, -2, BI_RGB);
        t.extend_from_slice(&[0, 0, 255, 0]); // negative height: array row 0 *is* the top
        t.extend_from_slice(&[255, 0, 0, 0]);
        let img = decode_png(&dib_to_png(&t).unwrap());
        assert_eq!(img.get_pixel(0, 0).0, [255, 0, 0, 255]);
        assert_eq!(img.get_pixel(0, 1).0, [0, 0, 255, 255]);
    }

    #[test]
    fn rows_are_padded_to_four_bytes() {
        // three 24-bit pixels = 9 bytes, padded to 12
        let mut b = header(24, 3, 1, BI_RGB);
        // BGR rows: blue, green, black, then the pad byte
        b.extend_from_slice(&[255, 0, 0, 0, 255, 0, 0, 0, 0, 0, 0, 0]);
        let img = decode_png(&dib_to_png(&b).unwrap());
        assert_eq!(img.get_pixel(0, 0).0, [0, 0, 255, 255]);
        assert_eq!(img.get_pixel(1, 0).0, [0, 255, 0, 255]);
        assert_eq!(img.get_pixel(2, 0).0, [0, 0, 0, 255]);
    }

    #[test]
    fn a_zeroed_alpha_byte_becomes_opaque_because_screenshots_have_no_alpha() {
        let mut b = header(32, 2, 1, BI_RGB);
        b.extend_from_slice(&[10, 20, 30, 0]);
        b.extend_from_slice(&[40, 50, 60, 0]);
        let img = decode_png(&dib_to_png(&b).unwrap());
        assert_eq!(img.get_pixel(0, 0).0, [30, 20, 10, 255]);
        assert_eq!(img.get_pixel(1, 0).0, [60, 50, 40, 255]);
    }

    #[test]
    fn a_real_alpha_channel_survives() {
        let mut b = header(32, 1, 1, BI_RGB);
        b.extend_from_slice(&[10, 20, 30, 128]);
        let img = decode_png(&dib_to_png(&b).unwrap());
        assert_eq!(img.get_pixel(0, 0).0, [30, 20, 10, 128]);
    }

    #[test]
    fn bitfields_masks_are_read_rather_than_assumed() {
        // 5-6-5 is the common 16-bit layout and is not the BI_RGB 5-5-5
        // default: pure green is 0x07E0 here and 0x03E0 there.
        let mut b = header(16, 1, 1, BI_BITFIELDS);
        b.extend_from_slice(&[0x00, 0xF8, 0x00, 0x00]); // red mask
        b.extend_from_slice(&[0xE0, 0x07, 0x00, 0x00]); // green mask
        b.extend_from_slice(&[0x1F, 0x00, 0x00, 0x00]); // blue mask
        b.extend_from_slice(&[0xE0, 0x07, 0x00, 0x00]); // 0x07E0, padded to 4
        let img = decode_png(&dib_to_png(&b).unwrap());
        assert_eq!(img.get_pixel(0, 0).0, [0, 255, 0, 255], "6-bit green scales to 255");

        // the same pixel read as 5-5-5 is not green at all
        let mut five = header(16, 1, 1, BI_RGB);
        five.extend_from_slice(&[0xE0, 0x07, 0x00, 0x00]);
        let img = decode_png(&dib_to_png(&five).unwrap());
        assert_ne!(img.get_pixel(0, 0).0, [0, 255, 0, 255]);
    }

    #[test]
    fn a_palette_index_finds_its_colour_whether_the_header_lists_two_or_256() {
        let mut b = header(8, 2, 1, BI_RGB);
        b.extend_from_slice(&[0, 0, 200, 0]); // index 0: BGRA -> red
        b.extend_from_slice(&[0, 100, 0, 0]); // index 1: green
        for _ in 2..256 {
            b.extend_from_slice(&[0, 0, 0, 0]);
        }
        b.extend_from_slice(&[1, 0, 0, 0]); // index 1, index 0
        let img = decode_png(&dib_to_png(&b).unwrap());
        assert_eq!(img.get_pixel(0, 0).0, [0, 100, 0, 255]);
        assert_eq!(img.get_pixel(1, 0).0, [200, 0, 0, 255]);

        // biClrUsed = 2: the palette really is two entries, and the pixel
        // array starts right after them
        let mut two = header(8, 2, 1, BI_RGB);
        two[32..36].copy_from_slice(&2u32.to_le_bytes());
        two.extend_from_slice(&[0, 0, 200, 0]);
        two.extend_from_slice(&[0, 100, 0, 0]);
        two.extend_from_slice(&[1, 0, 0, 0]);
        let img = decode_png(&dib_to_png(&two).unwrap());
        assert_eq!(img.get_pixel(0, 0).0, [0, 100, 0, 255]);
        assert_eq!(img.get_pixel(1, 0).0, [200, 0, 0, 255]);
    }

    #[test]
    fn one_bit_pixels_unpack_msb_first() {
        let mut b = header(1, 3, 1, BI_RGB);
        b.extend_from_slice(&[0, 0, 0, 0]);
        b.extend_from_slice(&[255, 255, 255, 255]);
        b.extend_from_slice(&[0b1010_0000, 0, 0, 0]);
        let img = decode_png(&dib_to_png(&b).unwrap());
        assert_eq!(img.get_pixel(0, 0).0, [255, 255, 255, 255]);
        assert_eq!(img.get_pixel(1, 0).0, [0, 0, 0, 255]);
        assert_eq!(img.get_pixel(2, 0).0, [255, 255, 255, 255]);
    }

    #[test]
    fn a_truncated_or_rle_payload_is_not_a_picture() {
        assert!(dib_to_png(&[]).is_none());
        assert!(dib_to_png(&[40, 0, 0]).is_none());
        // a full header that promises 8 pixels and delivers none
        assert!(dib_to_png(&header(24, 4, 2, BI_RGB)).is_none());
        // BI_RLE8 = 1
        assert!(dib_to_png(&header(8, 1, 1, 1)).is_none());
        // a header size that is not one of the four we know
        let mut odd = header(24, 1, 1, BI_RGB);
        odd[0] = 124;
        assert!(dib_to_png(&odd).is_none(), "a 124-byte header with 1 byte of data");
    }

    /// A reading for `docs/PERFORMANCE.md`, not a gate — a timing assert would
    /// fail on a loaded machine and prove nothing. Run by hand:
    /// `cargo test --release --lib screenshot_decode -- --ignored --nocapture`
    /// The rasters are one flat colour, so the PNG leg is the cheapest it can
    /// ever be; a real screenshot's pixels cost more to compress.
    #[test]
    #[ignore = "prints a measurement instead of asserting one"]
    fn a_pasted_screenshot_decode_is_timed() {
        for (w, h) in [(1920u32, 1080u32), (3840, 2160)] {
            let mut b = header(32, w as i32, h as i32, BI_RGB);
            b.resize(40 + w as usize * h as usize * 4, 0x40);
            let started = std::time::Instant::now();
            let raster = decode(&b).expect("a synthetic screenshot must decode");
            let decoded = started.elapsed().as_millis();
            let started = std::time::Instant::now();
            let mut png = std::io::Cursor::new(Vec::new());
            image::DynamicImage::ImageRgba8(raster)
                .write_to(&mut png, image::ImageFormat::Png)
                .unwrap();
            println!(
                "{w}x{h}: {} ms decode + {} ms encode, {} DIB bytes -> {} PNG bytes",
                decoded,
                started.elapsed().as_millis(),
                b.len(),
                png.into_inner().len()
            );
        }
    }
}
