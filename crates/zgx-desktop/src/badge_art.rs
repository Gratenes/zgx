//! The app icon's badge as pixels: a pill in the accent colour with the count
//! in white, drawn at whatever size the taskbar's overlay is at the window's
//! DPI (`notify`'s Windows `set_badge`: 16, 20, 24 or 32 pixels). Pure, so it
//! is tested everywhere; only Windows draws it.
//!
//! - The pill is a rounded rectangle, a circle for one digit, its edge
//!   antialiased by coverage (the distance from its edge).
//! - The count is Geist Bold's digits (`assets/Geist-Bold-digits.ttf`, a
//!   subset of the SIL OFL font, `assets/Geist-OFL.txt`), rasterized by
//!   `fontdue` at four times the size and averaged down, so it sits where it
//!   should to a quarter pixel at any size.
//! - 100 or more says `99+`; text that is no count is a plain dot.
//!
//! Pixels are premultiplied BGRA, `0xAARRGGBB` a `u32`, rows from the top, as
//! `CreateIconIndirect` takes them.

use std::sync::OnceLock;

/// The pill's colour, `0xRRGGBB`: Zeron's accent. The one place to change it.
pub(crate) const FILL: u32 = 0x5B43E8;
/// The count's colour.
pub(crate) const TEXT: u32 = 0xFFFFFF;

/// A badge's colours: the pill and the count.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Style {
    pub fill: u32,
    pub text: u32,
}

impl Style {
    pub(crate) const DEFAULT: Style = Style {
        fill: FILL,
        text: TEXT,
    };
}

/// What the badge says of `badge` (a count, else text that is no count): the
/// number, `99+` from a hundred, and none for a dot. Zero is no badge.
pub(crate) fn label(badge: &str) -> Option<String> {
    match badge.trim().parse::<u32>().ok()? {
        0 => None,
        n if n < 100 => Some(n.to_string()),
        _ => Some("99+".to_string()),
    }
}

fn font() -> &'static fontdue::Font {
    static FONT: OnceLock<fontdue::Font> = OnceLock::new();
    FONT.get_or_init(|| {
        fontdue::Font::from_bytes(
            include_bytes!("../assets/Geist-Bold-digits.ttf") as &[u8],
            fontdue::FontSettings::default(),
        )
        .expect("the badge's digits")
    })
}

/// Samples a side, for the count: its pixels are averaged from this many.
const SS: usize = 4;

/// The count's coverage, 0 to 1 a pixel, `size` by `size`: `text` set in an
/// em of `em` pixels, its ink centred on (`cx`, `cy`).
fn text_coverage(text: &str, em: f32, size: usize, cx: f32, cy: f32) -> Vec<f32> {
    let font = font();
    let px = em * SS as f32;
    let big = size * SS;
    let mut mask = vec![0u8; big * big];
    // The glyphs laid on a baseline: where each starts and ends, its ink.
    let glyphs: Vec<(fontdue::Metrics, Vec<u8>)> =
        text.chars().map(|c| font.rasterize(c, px)).collect();
    let mut pen = 0f32;
    let mut at = Vec::with_capacity(glyphs.len());
    let (mut left, mut right) = (f32::MAX, f32::MIN);
    let (mut top, mut bottom) = (f32::MAX, f32::MIN);
    for (m, _) in &glyphs {
        let x = pen + m.xmin as f32;
        at.push(pen);
        left = left.min(x);
        right = right.max(x + m.width as f32);
        // (Rows up from the baseline: the ink's top and bottom.)
        top = top.min(-((m.ymin + m.height as i32) as f32));
        bottom = bottom.max(-(m.ymin as f32));
        pen += m.advance_width;
    }
    if glyphs.is_empty() {
        return vec![0.; size * size];
    }
    let (ink_w, ink_h) = (right - left, bottom - top);
    // The ink's top-left, in the supersampled raster, where it is centred.
    let (ox, oy) = (
        cx * SS as f32 - ink_w / 2. - left,
        cy * SS as f32 - ink_h / 2. - top,
    );
    for ((m, bitmap), pen) in glyphs.iter().zip(at) {
        let gx = (ox + pen + m.xmin as f32).round() as i64;
        let gy = (oy - (m.ymin + m.height as i32) as f32).round() as i64;
        for row in 0..m.height {
            for col in 0..m.width {
                let (x, y) = (gx + col as i64, gy + row as i64);
                if x < 0 || y < 0 || x >= big as i64 || y >= big as i64 {
                    continue;
                }
                let v = bitmap[row * m.width + col];
                let cell = &mut mask[y as usize * big + x as usize];
                *cell = (*cell).max(v);
            }
        }
    }
    let mut out = vec![0f32; size * size];
    for y in 0..size {
        for x in 0..size {
            let mut sum = 0u32;
            for sy in 0..SS {
                for sx in 0..SS {
                    sum += mask[(y * SS + sy) * big + x * SS + sx] as u32;
                }
            }
            out[y * size + x] = sum as f32 / (255. * (SS * SS) as f32);
        }
    }
    out
}

/// A pill's coverage at a pixel's centre: a rounded rectangle `w` by `h`
/// about (`cx`, `cy`), its ends round, by the distance from its edge.
fn pill_coverage(x: f32, y: f32, cx: f32, cy: f32, w: f32, h: f32) -> f32 {
    let r = h / 2.;
    // A capsule: the distance to the segment between its two ends' centres.
    let half = (w / 2. - r).max(0.);
    let dx = ((x - cx).abs() - half).max(0.);
    let dy = y - cy;
    let d = (dx * dx + dy * dy).sqrt() - r;
    (0.5 - d).clamp(0., 1.)
}

fn premultiplied(color: u32, alpha: f32) -> [f32; 4] {
    let a = alpha.clamp(0., 1.);
    let c = |shift: u32| ((color >> shift) & 0xFF) as f32 / 255. * a;
    [c(0), c(8), c(16), a]
}

/// `badge` drawn `size` pixels square (premultiplied BGRA, rows from the top).
pub(crate) fn pixels(badge: &str, size: u32, style: Style) -> Vec<u32> {
    let size = size.max(8) as usize;
    let u = size as f32 / 16.;
    let (cx, cy) = (size as f32 / 2., size as f32 / 2.);
    let label = label(badge);
    // A count's pill: a circle for a digit, wider for more, 14 sixteenths
    // high so it leaves the icon's edge a sixteenth either side. Anything
    // else is a dot of 10.
    let h = if label.is_some() { 14. * u } else { 10. * u };
    let w = match label.as_deref().map_or(0, str::len) {
        0 | 1 => h,
        _ => 16. * u,
    };

    let text = label.as_deref().map(|label| {
        // The em that suits the count's length, brought down if its ink
        // wouldn't fit inside the pill with a margin.
        let em = match label.len() {
            1 => 13.4,
            2 => 11.,
            _ => 8.6,
        } * u;
        let room = w - 2.4 * u;
        let wide = ink_width(label, em);
        let em = if wide > room && wide > 0. {
            em * room / wide
        } else {
            em
        };
        text_coverage(label, em, size, cx, cy)
    });

    let fill = premultiplied(style.fill, 1.);
    let text_color = premultiplied(style.text, 1.);
    let mut out = vec![0u32; size * size];
    for y in 0..size {
        for x in 0..size {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            // The pill, then the count over it (each premultiplied, `over`).
            let cov = pill_coverage(px, py, cx, cy, w, h);
            let mut c = [fill[0] * cov, fill[1] * cov, fill[2] * cov, cov];
            if let Some(text) = &text {
                let t = text[y * size + x];
                for i in 0..3 {
                    c[i] = text_color[i] * t + c[i] * (1. - t);
                }
                c[3] = t + c[3] * (1. - t);
            }
            let q = |v: f32| (v.clamp(0., 1.) * 255. + 0.5) as u32;
            let a = q(c[3]);
            // (Premultiplied: no channel above the alpha.)
            let (r, g, b) = (q(c[2]).min(a), q(c[1]).min(a), q(c[0]).min(a));
            out[y * size + x] = (a << 24) | (r << 16) | (g << 8) | b;
        }
    }
    out
}

/// The ink's width of `text` set in `em` pixels.
fn ink_width(text: &str, em: f32) -> f32 {
    let font = font();
    let (mut pen, mut left, mut right) = (0f32, f32::MAX, f32::MIN);
    for c in text.chars() {
        let m = font.metrics(c, em);
        left = left.min(pen + m.xmin as f32);
        right = right.max(pen + m.xmin as f32 + m.width as f32);
        pen += m.advance_width;
    }
    if right > left { right - left } else { 0. }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIZES: [u32; 4] = [16, 20, 24, 32];

    fn at(pixels: &[u32], size: u32, x: u32, y: u32) -> (u32, u32, u32, u32) {
        let p = pixels[(y * size + x) as usize];
        (p >> 24, (p >> 16) & 0xFF, (p >> 8) & 0xFF, p & 0xFF)
    }

    /// The columns and rows with any alpha in them.
    fn extent(pixels: &[u32], size: u32) -> (u32, u32, u32, u32) {
        let (mut l, mut t, mut r, mut b) = (size, size, 0, 0);
        for y in 0..size {
            for x in 0..size {
                if at(pixels, size, x, y).0 > 0 {
                    l = l.min(x);
                    t = t.min(y);
                    r = r.max(x);
                    b = b.max(y);
                }
            }
        }
        (l, t, r, b)
    }

    /// Pixels of the count's colour (white) in the badge.
    fn whites(pixels: &[u32], size: u32) -> Vec<(u32, u32)> {
        let mut out = vec![];
        for y in 0..size {
            for x in 0..size {
                let (a, r, g, b) = at(pixels, size, x, y);
                if a == 255 && r > 200 && g > 200 && b > 200 {
                    out.push((x, y));
                }
            }
        }
        out
    }

    #[test]
    fn a_count_says_its_number_a_hundred_says_99_plus() {
        assert_eq!(label("1").as_deref(), Some("1"));
        assert_eq!(label(" 54 ").as_deref(), Some("54"));
        assert_eq!(label("99").as_deref(), Some("99"));
        assert_eq!(label("100").as_deref(), Some("99+"));
        assert_eq!(label("4000").as_deref(), Some("99+"));
        assert_eq!(label("0"), None);
        assert_eq!(label("new"), None);
    }

    #[test]
    fn it_is_as_many_pixels_as_asked() {
        for size in SIZES {
            for badge in ["3", "42", "100", "new"] {
                assert_eq!(
                    pixels(badge, size, Style::DEFAULT).len(),
                    (size * size) as usize,
                    "{badge} at {size}"
                );
            }
        }
    }

    #[test]
    fn the_corners_are_clear_and_the_middle_is_the_colour() {
        for size in SIZES {
            let p = pixels("7", size, Style::DEFAULT);
            for (x, y) in [(0, 0), (size - 1, 0), (0, size - 1), (size - 1, size - 1)] {
                assert_eq!(at(&p, size, x, y).0, 0, "corner {x},{y} at {size}");
            }
            // (Left of the digit, inside the disc: the pill's colour.)
            let (a, r, g, b) = at(&p, size, size / 2 - size / 4, size / 2);
            assert_eq!((a, r, g, b), (255, 0x5B, 0x43, 0xE8), "at {size}");
        }
    }

    #[test]
    fn nothing_is_brighter_than_its_alpha() {
        for size in SIZES {
            for badge in ["3", "42", "100", "new"] {
                for style in [
                    Style::DEFAULT,
                    Style {
                        fill: 0xFFFFFF,
                        text: 0xFFFFFF,
                    },
                ] {
                    for (i, px) in pixels(badge, size, style).iter().enumerate() {
                        let (a, r, g, b) =
                            (px >> 24, (px >> 16) & 0xFF, (px >> 8) & 0xFF, px & 0xFF);
                        assert!(
                            r <= a && g <= a && b <= a,
                            "{badge} at {size}, pixel {i}: {px:08x}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn the_edge_is_antialiased() {
        for size in SIZES {
            let p = pixels("1", size, Style::DEFAULT);
            let partial = p
                .iter()
                .filter(|px| (1..255).contains(&(**px >> 24)))
                .count();
            assert!(partial >= size as usize, "{partial} soft pixels at {size}");
            // And the count: grey between its white and the pill's colour.
            let soft_text = p
                .iter()
                .filter(|px| {
                    let (a, r) = (**px >> 24, (**px >> 16) & 0xFF);
                    a == 255 && r > 0x5B + 8 && r < 255 - 8
                })
                .count();
            assert!(soft_text > 0, "no soft count at {size}");
        }
    }

    #[test]
    fn a_digit_is_a_circle_and_more_are_a_wider_pill() {
        for size in SIZES {
            let one = extent(&pixels("1", size, Style::DEFAULT), size);
            let two = extent(&pixels("10", size, Style::DEFAULT), size);
            let three = extent(&pixels("100", size, Style::DEFAULT), size);
            let wide = |e: (u32, u32, u32, u32)| e.2 - e.0 + 1;
            let high = |e: (u32, u32, u32, u32)| e.3 - e.1 + 1;
            assert_eq!(wide(one), high(one), "a circle at {size}");
            assert!(wide(two) > wide(one), "a wider pill at {size}");
            assert_eq!(high(two), high(one), "as high at {size}");
            assert_eq!(
                wide(three),
                wide(two),
                "100 is `99+` in the same pill at {size}"
            );
            // Room round it: the pill is inside the icon's square.
            assert!(
                two.0 <= 1 && two.2 >= size - 2,
                "fills the width at {size}: {two:?}"
            );
            assert!(
                one.1 >= 1 && one.3 <= size - 2,
                "a margin above and below at {size}: {one:?}"
            );
        }
    }

    #[test]
    fn the_count_sits_in_the_middle_of_the_pill() {
        for size in SIZES {
            for badge in ["1", "7", "10", "88", "100"] {
                let p = pixels(badge, size, Style::DEFAULT);
                let white = whites(&p, size);
                assert!(!white.is_empty(), "no count in {badge} at {size}");
                let (l, r) = (
                    white.iter().map(|w| w.0).min().unwrap(),
                    white.iter().map(|w| w.0).max().unwrap(),
                );
                let (t, b) = (
                    white.iter().map(|w| w.1).min().unwrap(),
                    white.iter().map(|w| w.1).max().unwrap(),
                );
                // (Ink centred to a pixel: the margins either side differ by
                // at most one, and the count is inside the pill's edge.)
                let (left, right) = (l, size - 1 - r);
                let (top, bottom) = (t, size - 1 - b);
                assert!(
                    left.abs_diff(right) <= 2,
                    "{badge} at {size}: {left} left, {right} right"
                );
                assert!(
                    top.abs_diff(bottom) <= 2,
                    "{badge} at {size}: {top} above, {bottom} below"
                );
                assert!(left >= 2 && top >= 2, "{badge} at {size} touches the edge");
            }
        }
    }

    #[test]
    fn text_that_is_no_count_is_a_plain_dot() {
        for size in SIZES {
            let p = pixels("new", size, Style::DEFAULT);
            assert!(whites(&p, size).is_empty(), "a count in a dot at {size}");
            let e = extent(&p, size);
            assert!(e.2 - e.0 + 1 < size - 2, "a smaller dot at {size}: {e:?}");
            let (a, r, g, b) = at(&p, size, size / 2, size / 2);
            assert_eq!((a, r, g, b), (255, 0x5B, 0x43, 0xE8));
        }
    }

    #[test]
    fn the_colour_is_one_constant() {
        assert_eq!(Style::DEFAULT.fill, FILL);
        let p = pixels(
            "5",
            24,
            Style {
                fill: 0x112233,
                ..Style::DEFAULT
            },
        );
        // (Left of the digit.)
        assert_eq!(at(&p, 24, 3, 12), (255, 0x11, 0x22, 0x33));
    }

    /// `ZGX_BADGE_DUMP=<dir> cargo test -p zgx-desktop dump_badges -- --ignored`
    /// writes each badge as raw RGBA (`<label>-<size>.rgba`, un-premultiplied)
    /// for a look at them.
    #[test]
    #[ignore]
    fn dump_badges() {
        let Some(dir) = std::env::var_os("ZGX_BADGE_DUMP") else {
            return;
        };
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        let rings: [(&str, Style); 1] = [("plain", Style::DEFAULT)];
        for (name, style) in rings {
            for size in SIZES {
                for badge in ["1", "9", "10", "100"] {
                    let mut bytes = Vec::new();
                    for px in pixels(badge, size, style) {
                        let a = px >> 24;
                        let un = |c: u32| {
                            (c * 255 + a / 2)
                                .checked_div(a)
                                .map_or(0, |v| v.min(255) as u8)
                        };
                        bytes.extend([
                            un((px >> 16) & 0xFF),
                            un((px >> 8) & 0xFF),
                            un(px & 0xFF),
                            a as u8,
                        ]);
                    }
                    std::fs::write(dir.join(format!("{name}-{badge}-{size}.rgba")), bytes).unwrap();
                }
            }
        }
    }
}
