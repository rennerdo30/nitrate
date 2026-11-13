//! Text with fontdue, drawn into premultiplied RGBA.
//!
//! Fonts come from the system (Segoe UI on Windows) and are never shipped.
//! Without a font, text is skipped and everything else still draws.

use fontdue::{Font, FontSettings, Metrics};
use std::cell::RefCell;
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum Weight {
    Regular,
    Semibold,
}

type GlyphKey = (Weight, char, u32);

pub struct Fonts {
    regular: Option<Font>,
    semibold: Option<Font>,
    glyphs: RefCell<HashMap<GlyphKey, (Metrics, Vec<u8>)>>,
}

impl Fonts {
    /// No fonts: text is not drawn (tests, or a system without the fonts).
    pub fn none() -> Self {
        Self { regular: None, semibold: None, glyphs: RefCell::new(HashMap::new()) }
    }

    /// Loads the system UI font. Missing files are not an error.
    pub fn system() -> Self {
        let load = |names: &[&str]| -> Option<Font> {
            for dir in font_dirs() {
                for name in names {
                    if let Ok(bytes) = std::fs::read(dir.join(name))
                        && let Ok(font) = Font::from_bytes(bytes, FontSettings::default())
                    {
                        return Some(font);
                    }
                }
            }
            None
        };
        let regular = load(&["segoeui.ttf", "SFNS.ttf", "DejaVuSans.ttf", "NotoSans-Regular.ttf"]);
        let semibold = load(&["seguisb.ttf", "DejaVuSans-Bold.ttf", "NotoSans-SemiBold.ttf"]);
        Self { regular, semibold, glyphs: RefCell::new(HashMap::new()) }
    }

    fn font(&self, weight: Weight) -> Option<&Font> {
        match weight {
            Weight::Regular => self.regular.as_ref(),
            Weight::Semibold => self.semibold.as_ref().or(self.regular.as_ref()),
        }
    }

    /// Width of `text` in pixels at `px`.
    pub fn measure(&self, text: &str, px: f32, weight: Weight) -> f32 {
        let Some(font) = self.font(weight) else { return 0.0 };
        let mut width = 0.0;
        let mut prev = None;
        for c in text.chars() {
            if let Some(p) = prev {
                width += font.horizontal_kern(p, c, px).unwrap_or(0.0);
            }
            width += font.metrics(c, px).advance_width;
            prev = Some(c);
        }
        width
    }

    /// Draws `text` with its baseline at (x, baseline) into `rgba` (`width`
    /// pixels per row), in `color` with alpha `alpha`. Clips at the edges.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        rgba: &mut [u8],
        width: u32,
        text: &str,
        x: f32,
        baseline: f32,
        px: f32,
        weight: Weight,
        color: [u8; 3],
        alpha: f32,
    ) {
        let Some(font) = self.font(weight) else { return };
        let height = (rgba.len() / 4) as i64 / width.max(1) as i64;
        let mut pen = x;
        let mut prev = None;
        let mut cache = self.glyphs.borrow_mut();
        for c in text.chars() {
            if let Some(p) = prev {
                pen += font.horizontal_kern(p, c, px).unwrap_or(0.0);
            }
            prev = Some(c);
            let (m, bitmap) = cache.entry((weight, c, px.to_bits())).or_insert_with(|| font.rasterize(c, px));
            let gx = (pen + m.xmin as f32).round() as i64;
            let gy = (baseline - m.ymin as f32 - m.height as f32).round() as i64;
            for row in 0..m.height as i64 {
                let y = gy + row;
                if y < 0 || y >= height {
                    continue;
                }
                for col in 0..m.width as i64 {
                    let xx = gx + col;
                    if xx < 0 || xx >= width as i64 {
                        continue;
                    }
                    let coverage = bitmap[(row * m.width as i64 + col) as usize] as f32 / 255.0 * alpha;
                    if coverage <= 0.0 {
                        continue;
                    }
                    let i = ((y * width as i64 + xx) * 4) as usize;
                    let inv = 1.0 - coverage;
                    for k in 0..3 {
                        rgba[i + k] = (color[k] as f32 * coverage + rgba[i + k] as f32 * inv).round() as u8;
                    }
                    rgba[i + 3] = (255.0 * coverage + rgba[i + 3] as f32 * inv).round() as u8;
                }
            }
            pen += m.advance_width;
        }
    }
}

fn font_dirs() -> Vec<std::path::PathBuf> {
    let mut dirs = Vec::new();
    if let Some(windir) = std::env::var_os("WINDIR") {
        dirs.push(std::path::PathBuf::from(windir).join("Fonts"));
    }
    dirs.push("/System/Library/Fonts".into());
    dirs.push("/usr/share/fonts/truetype/dejavu".into());
    dirs.push("/usr/share/fonts/noto".into());
    dirs
}
