//! Drawing the layers with tiny-skia. Coordinates are physical pixels.

use crate::text::Weight;
use crate::{AMBER, CRIMSON, Fonts, HoverKind, Layer, ORANGE, Rect, View};
use tiny_skia::{
    BlendMode, Color, FillRule, GradientStop, LineCap, LineJoin, LinearGradient, Paint, Path, PathBuilder, Pixmap,
    PixmapPaint, Point, Shader, SpreadMode, Stroke, Transform,
};

const WHITE: [u8; 3] = [0xFF, 0xF6, 0xEE];

fn rgba(c: [u8; 3], a: f32) -> Color {
    Color::from_rgba8(c[0], c[1], c[2], (a.clamp(0.0, 1.0) * 255.0).round() as u8)
}

fn solid(c: Color) -> Paint<'static> {
    Paint { shader: Shader::SolidColor(c), anti_alias: true, ..Paint::default() }
}

fn burn(x0: f32, y0: f32, x1: f32, y1: f32) -> Paint<'static> {
    let stops = vec![
        GradientStop::new(0.0, rgba(AMBER, 1.0)),
        GradientStop::new(0.45, rgba(ORANGE, 1.0)),
        GradientStop::new(1.0, rgba(CRIMSON, 1.0)),
    ];
    let shader = LinearGradient::new(
        Point::from_xy(x0, y0),
        Point::from_xy(x1, y1),
        stops,
        SpreadMode::Pad,
        Transform::identity(),
    )
    .unwrap_or(Shader::SolidColor(rgba(ORANGE, 1.0)));
    Paint { shader, anti_alias: true, ..Paint::default() }
}

fn rrect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<Path> {
    let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
    let k = 0.552_284_8 * r;
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.cubic_to(x + w - r + k, y, x + w, y + r - k, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.cubic_to(x + w, y + h - r + k, x + w - r + k, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.cubic_to(x + r - k, y + h, x, y + h - r + k, x, y + h - r);
    pb.line_to(x, y + r);
    pb.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
    pb.close();
    pb.finish()
}

fn fill(p: &mut Pixmap, path: Option<Path>, paint: &Paint) {
    if let Some(path) = path {
        p.fill_path(&path, paint, FillRule::Winding, Transform::identity(), None);
    }
}

fn stroke(p: &mut Pixmap, path: Option<Path>, paint: &Paint, width: f32) {
    if let Some(path) = path {
        let s = Stroke { width, line_cap: LineCap::Round, line_join: LineJoin::Round, ..Stroke::default() };
        p.stroke_path(&path, paint, &s, Transform::identity(), None);
    }
}

fn vertical_scrim(p: &mut Pixmap, top_alpha: f32, bottom_alpha: f32) {
    let h = p.height() as f32;
    let stops = vec![
        GradientStop::new(0.0, Color::from_rgba8(8, 5, 4, (top_alpha * 255.0) as u8)),
        GradientStop::new(1.0, Color::from_rgba8(8, 5, 4, (bottom_alpha * 255.0) as u8)),
    ];
    if let Some(shader) = LinearGradient::new(
        Point::from_xy(0.0, 0.0),
        Point::from_xy(0.0, h),
        stops,
        SpreadMode::Pad,
        Transform::identity(),
    ) {
        let paint = Paint { shader, ..Paint::default() };
        let full = PathBuilder::from_rect(
            tiny_skia::Rect::from_xywh(0.0, 0.0, p.width() as f32, h)
                .unwrap_or(tiny_skia::Rect::from_xywh(0.0, 0.0, 1.0, 1.0).expect("a 1x1 rect is valid")),
        );
        p.fill_path(&full, &paint, FillRule::Winding, Transform::identity(), None);
    }
}

fn layer(p: Pixmap, x: f32, y: f32) -> Layer {
    let (width, height) = (p.width(), p.height());
    Layer { x: x.round() as i32, y: y.round() as i32, width, height, rgba: p.take() }
}

fn new_pixmap(w: f32, h: f32) -> Option<Pixmap> {
    Pixmap::new(w.round().max(1.0) as u32, h.round().max(1.0) as u32)
}

pub fn paint(view: &View, fonts: &Fonts) -> Vec<Layer> {
    let mut layers = Vec::new();
    if view.has_file && view.controls {
        if let Some(l) = bottom_bar(view, fonts) {
            layers.push(l);
        }
        if let Some(l) = title(view, fonts) {
            layers.push(l);
        }
    }
    if view.controls
        && let Some(l) = notice(view, fonts)
    {
        layers.push(l);
    }
    if let Some(l) = centre(view, fonts) {
        layers.push(l);
    }
    layers
}

fn bottom_bar(v: &View, fonts: &Fonts) -> Option<Layer> {
    let l = &v.layout;
    let s = l.scale;
    // The layer reaches above the bar for the hover time bubble.
    let extra = 40.0 * s;
    let top = (l.bar.y - extra).max(0.0);
    let mut p = new_pixmap(l.bar.w, l.height as f32 - top)?;
    let oy = top;
    let local = |r: Rect| Rect { y: r.y - oy, ..r };

    let bar_top = l.bar.y - oy;
    if v.remote {
        fill(&mut p, rrect(0.0, bar_top, l.bar.w, l.bar.h, 0.0), &solid(Color::from_rgba8(14, 10, 9, 255)));
    } else {
        // Scrim only over the bar, transparent above it.
        let mut scrim = new_pixmap(l.bar.w, l.bar.h)?;
        vertical_scrim(&mut scrim, 0.0, 0.78);
        p.draw_pixmap(0, bar_top.round() as i32, scrim.as_ref(), &PixmapPaint::default(), Transform::identity(), None);
    }

    // Timeline.
    let tl = local(l.timeline);
    let hover_tl = v.hover == Some(HoverKind::Timeline);
    let thick = if hover_tl { 6.0 * s } else { 4.0 * s };
    let cy = tl.center_y();
    fill(&mut p, rrect(tl.x, cy - thick / 2.0, tl.w, thick, thick / 2.0), &solid(rgba(WHITE, 0.28)));
    let px = v.progress_px as f32;
    if px > 0.0 {
        fill(&mut p, rrect(tl.x, cy - thick / 2.0, px, thick, thick / 2.0), &burn(tl.x, 0.0, tl.x + tl.w, 0.0));
    }
    if hover_tl {
        fill(&mut p, PathBuilder::from_circle(tl.x + px, cy, 7.0 * s), &solid(rgba(WHITE, 1.0)));
    }
    if let Some((x, label)) = &v.preview {
        let x = *x as f32;
        let w = fonts.measure(label, 13.0 * s, Weight::Semibold) + 16.0 * s;
        let h = 24.0 * s;
        let bx = (x - w / 2.0).clamp(0.0, (l.bar.w - w).max(0.0));
        let by = cy - 14.0 * s - h;
        fill(&mut p, rrect(bx, by, w, h, 6.0 * s), &solid(Color::from_rgba8(20, 14, 12, 235)));
        fill(&mut p, rrect(x - 1.0 * s, cy - thick, 2.0 * s, thick * 2.0, 1.0 * s), &solid(rgba(WHITE, 0.9)));
        let width = p.width();
        fonts.draw(p.data_mut(), width, label, bx + 8.0 * s, by + 17.0 * s, 13.0 * s, Weight::Semibold, WHITE, 1.0);
    }

    // Play / pause.
    let play = local(l.play);
    hover_disc(&mut p, play, v.hover == Some(HoverKind::PlayPause), s);
    let (cx, cy2) = (play.x + play.w / 2.0, play.center_y());
    let icon = solid(rgba(WHITE, 1.0));
    if v.paused {
        let mut pb = PathBuilder::new();
        pb.move_to(cx - 6.0 * s, cy2 - 9.0 * s);
        pb.line_to(cx + 10.0 * s, cy2);
        pb.line_to(cx - 6.0 * s, cy2 + 9.0 * s);
        pb.close();
        stroke(&mut p, pb.finish(), &icon, 2.5 * s);
        let mut pb = PathBuilder::new();
        pb.move_to(cx - 6.0 * s, cy2 - 9.0 * s);
        pb.line_to(cx + 10.0 * s, cy2);
        pb.line_to(cx - 6.0 * s, cy2 + 9.0 * s);
        pb.close();
        fill(&mut p, pb.finish(), &icon);
    } else {
        fill(&mut p, rrect(cx - 7.5 * s, cy2 - 9.0 * s, 5.0 * s, 18.0 * s, 1.5 * s), &icon);
        fill(&mut p, rrect(cx + 2.5 * s, cy2 - 9.0 * s, 5.0 * s, 18.0 * s, 1.5 * s), &icon);
    }

    // Time.
    let width = p.width();
    fonts.draw(p.data_mut(), width, &v.time_text, l.time_x, cy2 + 5.0 * s, 14.0 * s, Weight::Semibold, WHITE, 0.92);

    // Mute and volume.
    let mute = local(l.mute);
    hover_disc(&mut p, mute, v.hover == Some(HoverKind::Mute), s);
    speaker(
        &mut p,
        mute.x + mute.w / 2.0 - 3.0 * s,
        mute.center_y(),
        s,
        v.muted,
        v.volume_px as f32 / l.volume.w.max(1.0),
    );
    let vol = local(l.volume);
    let vy = vol.center_y();
    let vt = 4.0 * s;
    fill(&mut p, rrect(vol.x, vy - vt / 2.0, vol.w, vt, vt / 2.0), &solid(rgba(WHITE, 0.28)));
    let vpx = if v.muted { 0.0 } else { v.volume_px as f32 };
    if vpx > 0.0 {
        fill(&mut p, rrect(vol.x, vy - vt / 2.0, vpx, vt, vt / 2.0), &solid(rgba(WHITE, 0.95)));
    }
    if v.hover == Some(HoverKind::Volume) {
        fill(&mut p, PathBuilder::from_circle(vol.x + vpx, vy, 6.0 * s), &solid(rgba(WHITE, 1.0)));
    }

    // Fullscreen.
    let fs = local(l.fullscreen);
    hover_disc(&mut p, fs, v.hover == Some(HoverKind::Fullscreen), s);
    corners(&mut p, fs.x + fs.w / 2.0, fs.center_y(), s, v.fullscreen);

    Some(layer(p, 0.0, top))
}

fn hover_disc(p: &mut Pixmap, r: Rect, on: bool, s: f32) {
    if on {
        fill(p, PathBuilder::from_circle(r.x + r.w / 2.0, r.center_y(), 18.0 * s), &solid(rgba(WHITE, 0.14)));
    }
}

fn speaker(p: &mut Pixmap, cx: f32, cy: f32, s: f32, muted: bool, level: f32) {
    let paint = solid(rgba(WHITE, 1.0));
    let mut pb = PathBuilder::new();
    pb.move_to(cx - 9.0 * s, cy - 4.0 * s);
    pb.line_to(cx - 4.0 * s, cy - 4.0 * s);
    pb.line_to(cx + 2.0 * s, cy - 9.0 * s);
    pb.line_to(cx + 2.0 * s, cy + 9.0 * s);
    pb.line_to(cx - 4.0 * s, cy + 4.0 * s);
    pb.line_to(cx - 9.0 * s, cy + 4.0 * s);
    pb.close();
    fill(p, pb.finish(), &paint);
    if muted {
        let mut pb = PathBuilder::new();
        pb.move_to(cx + 6.0 * s, cy - 4.5 * s);
        pb.line_to(cx + 15.0 * s, cy + 4.5 * s);
        pb.move_to(cx + 15.0 * s, cy - 4.5 * s);
        pb.line_to(cx + 6.0 * s, cy + 4.5 * s);
        stroke(p, pb.finish(), &paint, 2.0 * s);
        return;
    }
    for (i, r) in [6.0f32, 11.0].iter().enumerate() {
        if level <= 0.0 || (i == 1 && level < 0.5) {
            break;
        }
        let r = r * s;
        let mut pb = PathBuilder::new();
        let a = 0.75f32; // radians either side of the horizontal
        pb.move_to(cx + 3.0 * s + r * a.cos(), cy - r * a.sin());
        pb.quad_to(cx + 3.0 * s + r * 1.35, cy, cx + 3.0 * s + r * a.cos(), cy + r * a.sin());
        stroke(p, pb.finish(), &paint, 2.0 * s);
    }
}

fn corners(p: &mut Pixmap, cx: f32, cy: f32, s: f32, inward: bool) {
    let paint = solid(rgba(WHITE, 1.0));
    let (o, k) = (8.0 * s, 5.0 * s);
    let mut pb = PathBuilder::new();
    for (sx, sy) in [(-1.0f32, -1.0f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
        let (x, y) = (cx + sx * o, cy + sy * o);
        if inward {
            pb.move_to(x - sx * k, y);
            pb.line_to(x - sx * k, y - sy * k);
            pb.line_to(x, y - sy * k);
        } else {
            pb.move_to(x, y - sy * k);
            pb.line_to(x, y);
            pb.line_to(x - sx * k, y);
        }
    }
    stroke(p, pb.finish(), &paint, 2.0 * s);
}

fn ellipsize(fonts: &Fonts, text: &str, px: f32, weight: Weight, max: f32) -> String {
    if fonts.measure(text, px, weight) <= max {
        return text.to_owned();
    }
    let mut chars: Vec<char> = text.chars().collect();
    while !chars.is_empty() {
        chars.pop();
        let candidate: String = chars.iter().collect::<String>() + "…";
        if fonts.measure(&candidate, px, weight) <= max {
            return candidate;
        }
    }
    String::new()
}

fn title(v: &View, fonts: &Fonts) -> Option<Layer> {
    if v.title.is_empty() {
        return None;
    }
    let l = &v.layout;
    let s = l.scale;
    let mut p = new_pixmap(l.top.w, l.top.h)?;
    if v.remote {
        fill(&mut p, rrect(0.0, 0.0, l.top.w, l.top.h, 0.0), &solid(Color::from_rgba8(14, 10, 9, 255)));
    } else {
        vertical_scrim(&mut p, 0.62, 0.0);
    }
    let px = 15.0 * s;
    let text = ellipsize(fonts, &v.title, px, Weight::Semibold, l.top.w - 48.0 * s);
    let width = p.width();
    fonts.draw(p.data_mut(), width, &text, 24.0 * s, 36.0 * s, px, Weight::Semibold, WHITE, 0.95);
    Some(layer(p, 0.0, 0.0))
}

fn centre(v: &View, fonts: &Fonts) -> Option<Layer> {
    let l = &v.layout;
    let s = l.scale;
    let (ww, wh) = (l.width as f32, l.height as f32);
    if !v.has_file {
        let w = (460.0 * s).min(ww);
        let h = (300.0 * s).min(wh);
        let mut p = new_pixmap(w, h)?;
        let size = 120.0 * s;
        draw_mark(&mut p, (w - size) / 2.0, 20.0 * s, size, false);
        let width = p.width();
        let line1 = v.message.as_deref().unwrap_or("Drop a video here");
        let px1 = 22.0 * s;
        let x1 = (w - fonts.measure(line1, px1, Weight::Semibold)) / 2.0;
        fonts.draw(p.data_mut(), width, line1, x1, 20.0 * s + size + 52.0 * s, px1, Weight::Semibold, WHITE, 0.95);
        let line2 = "or press Ctrl+O to open a file";
        let px2 = 14.0 * s;
        let x2 = (w - fonts.measure(line2, px2, Weight::Regular)) / 2.0;
        fonts.draw(p.data_mut(), width, line2, x2, 20.0 * s + size + 82.0 * s, px2, Weight::Regular, WHITE, 0.6);
        return Some(layer(p, (ww - w) / 2.0, (wh - h) / 2.0));
    }
    let message = v.message.as_deref()?;
    let px = 15.0 * s;
    let tw = fonts.measure(message, px, Weight::Semibold);
    let w = (tw + 40.0 * s).min(ww);
    let h = (44.0 * s).min(wh);
    let mut p = new_pixmap(w, h)?;
    fill(&mut p, rrect(0.0, 0.0, w, h, 10.0 * s), &solid(Color::from_rgba8(20, 14, 12, 235)));
    let width = p.width();
    fonts.draw(p.data_mut(), width, message, 20.0 * s, 28.0 * s, px, Weight::Semibold, WHITE, 1.0);
    Some(layer(p, (ww - w) / 2.0, (wh - h) / 2.0))
}

/// The logo mark (assets/logo/nitrate-mark.svg) at `size` pixels, at (x, y).
/// `tile` draws the dark rounded tile behind it (the app icon).
fn draw_mark(p: &mut Pixmap, x: f32, y: f32, size: f32, tile: bool) {
    let k = size / 512.0;
    let t = |px: f32, py: f32| (x + px * k, y + py * k);
    if tile {
        let stops = vec![
            GradientStop::new(0.0, Color::from_rgba8(0x2A, 0x16, 0x10, 255)),
            GradientStop::new(1.0, Color::from_rgba8(0x0E, 0x09, 0x08, 255)),
        ];
        let (x0, y0) = t(140.0, 120.0);
        let (x1, y1) = t(512.0, 512.0);
        let shader = LinearGradient::new(
            Point::from_xy(x0, y0),
            Point::from_xy(x1, y1),
            stops,
            SpreadMode::Pad,
            Transform::identity(),
        )
        .unwrap_or(Shader::SolidColor(Color::from_rgba8(0x14, 0x0C, 0x0A, 255)));
        fill(p, rrect(x, y, size, size, 116.0 * k), &Paint { shader, anti_alias: true, ..Paint::default() });
    }
    // The triangle goes into its own pixmap so the cuts show what is beneath.
    let Some(mut tri) = Pixmap::new(p.width(), p.height()) else { return };
    let (ax, ay) = t(164.0, 128.0);
    let (bx, by) = t(164.0, 384.0);
    let (cx, cy) = t(406.0, 256.0);
    let mut pb = PathBuilder::new();
    pb.move_to(ax, ay);
    pb.line_to(bx, by);
    pb.line_to(cx, cy);
    pb.close();
    let path = pb.finish();
    let (g0x, g0y) = t(150.0, 100.0);
    let (g1x, g1y) = t(420.0, 410.0);
    let paint = burn(g0x, g0y, g1x, g1y);
    if let Some(path) = path {
        tri.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
        let s = Stroke { width: 44.0 * k, line_join: LineJoin::Round, ..Stroke::default() };
        tri.stroke_path(&path, &paint, &s, Transform::identity(), None);
    }
    let clear = Paint { blend_mode: BlendMode::Clear, anti_alias: true, ..Paint::default() };
    let (sx, sy) = t(204.0, 60.0);
    fill(&mut tri, rrect(sx, sy, 14.0 * k, 392.0 * k, 0.0), &clear);
    for hy in [168.0, 220.0, 272.0, 324.0] {
        let (hx, hy) = t(157.0, hy);
        fill(&mut tri, rrect(hx, hy, 32.0 * k, 24.0 * k, 6.0 * k), &clear);
    }
    p.draw_pixmap(0, 0, tri.as_ref(), &PixmapPaint::default(), Transform::identity(), None);
}

/// The app icon (mark on its tile) as straight (not premultiplied) RGBA,
/// `size` × `size`, for window and taskbar icons.
pub fn mark_rgba(size: u32) -> Vec<u8> {
    let Some(mut p) = Pixmap::new(size.max(1), size.max(1)) else { return Vec::new() };
    draw_mark(&mut p, 0.0, 0.0, size as f32, true);
    p.pixels()
        .iter()
        .flat_map(|c| {
            let c = c.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        })
        .collect()
}

/// The update notice: a dark pill with an amber dot, top right.
fn notice(v: &View, fonts: &Fonts) -> Option<Layer> {
    let text = v.notice.as_deref()?;
    let l = &v.layout;
    let s = l.scale;
    let px = 13.0 * s;
    let w = (fonts.measure(text, px, Weight::Semibold) + 44.0 * s).min(l.width as f32);
    let h = 32.0 * s;
    let mut p = new_pixmap(w, h)?;
    let bg = if v.remote { Color::from_rgba8(30, 22, 18, 255) } else { Color::from_rgba8(20, 14, 12, 230) };
    fill(&mut p, rrect(0.0, 0.0, w, h, h / 2.0), &solid(bg));
    fill(&mut p, PathBuilder::from_circle(16.0 * s, h / 2.0, 4.0 * s), &solid(rgba(AMBER, 1.0)));
    let width = p.width();
    fonts.draw(p.data_mut(), width, text, 28.0 * s, h / 2.0 + 4.5 * s, px, Weight::Semibold, WHITE, 0.95);
    Some(layer(p, l.width as f32 - w - 16.0 * s, 16.0 * s))
}
