//! Nitrate's player controls: layout, hit testing and drawing.
//!
//! Everything here is plain CPU code without windowing or GPU calls, so it is
//! tested on any machine. The app turns player state into a [`View`]; when the
//! view changes it calls [`paint`], which returns a few small premultiplied
//! RGBA [`Layer`]s that the GPU composites over the video. An unchanged view
//! is never painted again (AGENTS.md rule 6).

mod draw;
mod text;

pub use draw::mark_rgba;
pub use text::Fonts;

/// Brand colours (assets/logo): the burn gradient from amber to crimson.
pub const AMBER: [u8; 3] = [0xFF, 0xD2, 0x5A];
pub const ORANGE: [u8; 3] = [0xFF, 0x7A, 0x1F];
pub const CRIMSON: [u8; 3] = [0xE5, 0x17, 0x3F];

/// Player state the controls show. The app owns it and updates it from mpv
/// events and input.
#[derive(Debug, Clone, PartialEq)]
pub struct State {
    pub has_file: bool,
    pub title: String,
    pub position: f64,
    pub duration: f64,
    pub paused: bool,
    /// 0 to 130 (mpv's volume scale).
    pub volume: f64,
    pub muted: bool,
    pub fullscreen: bool,
    pub controls_visible: bool,
    /// Remote session: no gradients or translucency, solid fills only.
    pub remote: bool,
    /// Pointer position in physical pixels, when it is over the window.
    pub pointer: Option<(f32, f32)>,
    /// The target being dragged (timeline or volume), if any.
    pub dragging: Option<Target>,
    /// A message in the middle of the window (errors, hints).
    pub message: Option<String>,
    /// A small notice in the top right corner (an available update).
    pub notice: Option<String>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            has_file: false,
            title: String::new(),
            position: 0.0,
            duration: 0.0,
            paused: false,
            volume: 100.0,
            muted: false,
            fullscreen: false,
            controls_visible: true,
            remote: false,
            pointer: None,
            dragging: None,
            message: None,
            notice: None,
        }
    }
}

/// Something the pointer can act on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Target {
    PlayPause,
    /// Fraction 0..=1 of the timeline.
    Timeline(f64),
    Mute,
    /// Fraction 0..=1 of the volume slider.
    Volume(f64),
    Fullscreen,
}

impl Target {
    /// Same kind of target, ignoring the fraction.
    pub fn same_kind(&self, other: &Target) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
    fn center_y(&self) -> f32 {
        self.y + self.h / 2.0
    }
}

/// Where everything sits, in physical pixels, for one window size and scale.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Layout {
    pub width: u32,
    pub height: u32,
    pub scale: f32,
    /// The bottom area that holds the timeline and the button row.
    pub bar: Rect,
    pub top: Rect,
    pub timeline: Rect,
    pub play: Rect,
    pub time_x: f32,
    pub mute: Rect,
    pub volume: Rect,
    pub fullscreen: Rect,
}

impl Layout {
    pub fn new(width: u32, height: u32, scale: f32) -> Self {
        let s = scale.max(0.5);
        let (w, h) = (width as f32, height as f32);
        let margin = 24.0 * s;
        let bar_h = (112.0 * s).min(h);
        let bar = Rect { x: 0.0, y: h - bar_h, w, h: bar_h };
        let row_y = h - 40.0 * s;
        let button = 40.0 * s;
        let timeline_y = h - 82.0 * s;
        let timeline = Rect { x: margin, y: timeline_y - 12.0 * s, w: (w - 2.0 * margin).max(0.0), h: 24.0 * s };
        let play = Rect { x: margin - 8.0 * s, y: row_y - button / 2.0, w: button, h: button };
        let fullscreen = Rect { x: w - margin - button + 8.0 * s, y: row_y - button / 2.0, w: button, h: button };
        let volume_w = 96.0 * s;
        let volume = Rect { x: fullscreen.x - 12.0 * s - volume_w, y: row_y - 12.0 * s, w: volume_w, h: 24.0 * s };
        let mute = Rect { x: volume.x - 4.0 * s - button, y: row_y - button / 2.0, w: button, h: button };
        let top = Rect { x: 0.0, y: 0.0, w, h: (64.0 * s).min(h) };
        Self {
            width,
            height,
            scale: s,
            bar,
            top,
            timeline,
            play,
            time_x: play.x + button + 12.0 * s,
            mute,
            volume,
            fullscreen,
        }
    }

    /// What the pointer at (x, y) would act on.
    pub fn hit(&self, x: f32, y: f32) -> Option<Target> {
        if self.play.contains(x, y) {
            Some(Target::PlayPause)
        } else if self.timeline.contains(x, y) {
            Some(Target::Timeline(self.timeline_fraction(x)))
        } else if self.mute.contains(x, y) {
            Some(Target::Mute)
        } else if self.volume.contains(x, y) {
            Some(Target::Volume(self.volume_fraction(x)))
        } else if self.fullscreen.contains(x, y) {
            Some(Target::Fullscreen)
        } else {
            None
        }
    }

    pub fn timeline_fraction(&self, x: f32) -> f64 {
        fraction(x, self.timeline)
    }

    pub fn volume_fraction(&self, x: f32) -> f64 {
        fraction(x, self.volume)
    }

    /// Whether (x, y) is over the control area (keeps the controls shown).
    pub fn over_controls(&self, x: f32, y: f32) -> bool {
        self.bar.contains(x, y)
    }
}

fn fraction(x: f32, r: Rect) -> f64 {
    if r.w <= 0.0 { 0.0 } else { (((x - r.x) / r.w) as f64).clamp(0.0, 1.0) }
}

/// `m:ss`, or `h:mm:ss` when `hours` is set (from the duration, so both
/// times in "position / duration" have the same shape).
pub fn format_time(seconds: f64, hours: bool) -> String {
    let t = if seconds.is_finite() && seconds > 0.0 { seconds.floor() as u64 } else { 0 };
    let (h, m, s) = (t / 3600, (t / 60) % 60, t % 60);
    if hours || h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") }
}

/// Everything [`paint`] draws, quantised to what can change pixels. Two
/// equal views paint identical layers, so the app repaints only on change.
#[derive(Debug, Clone, PartialEq)]
pub struct View {
    pub layout: Layout,
    pub controls: bool,
    pub remote: bool,
    pub has_file: bool,
    pub paused: bool,
    pub title: String,
    pub time_text: String,
    /// Progress in whole pixels along the timeline.
    pub progress_px: u32,
    pub volume_px: u32,
    pub muted: bool,
    pub fullscreen: bool,
    pub hover: Option<HoverKind>,
    /// Hover position on the timeline (x in pixels) and its time label.
    pub preview: Option<(u32, String)>,
    pub message: Option<String>,
    pub notice: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoverKind {
    PlayPause,
    Timeline,
    Mute,
    Volume,
    Fullscreen,
}

impl View {
    pub fn new(state: &State, layout: Layout) -> Self {
        let duration = if state.duration.is_finite() && state.duration > 0.0 { state.duration } else { 0.0 };
        let hours = duration >= 3600.0;
        let progress = if duration > 0.0 { (state.position / duration).clamp(0.0, 1.0) } else { 0.0 };
        let target = state.dragging.or_else(|| state.pointer.and_then(|(x, y)| layout.hit(x, y)));
        let hover = target.map(|t| match t {
            Target::PlayPause => HoverKind::PlayPause,
            Target::Timeline(_) => HoverKind::Timeline,
            Target::Mute => HoverKind::Mute,
            Target::Volume(_) => HoverKind::Volume,
            Target::Fullscreen => HoverKind::Fullscreen,
        });
        let preview = match (state.has_file && duration > 0.0, target, state.pointer) {
            (true, Some(Target::Timeline(f)), Some((x, _))) => {
                let x = x.clamp(layout.timeline.x, layout.timeline.x + layout.timeline.w);
                Some((x.round() as u32, format_time(f * duration, hours)))
            }
            _ => None,
        };
        let controls = state.controls_visible || !state.has_file;
        Self {
            layout,
            controls,
            remote: state.remote,
            has_file: state.has_file,
            paused: state.paused,
            title: state.title.clone(),
            time_text: if state.has_file {
                format!("{} / {}", format_time(state.position, hours), format_time(duration, hours))
            } else {
                String::new()
            },
            progress_px: (progress * layout.timeline.w as f64).round() as u32,
            volume_px: ((state.volume / 100.0).clamp(0.0, 1.0) * layout.volume.w as f64).round() as u32,
            muted: state.muted,
            fullscreen: state.fullscreen,
            hover,
            preview,
            message: state.message.clone(),
            notice: state.notice.clone(),
        }
    }
}

/// A premultiplied RGBA image placed at (x, y) in the window.
#[derive(Debug, Clone, PartialEq)]
pub struct Layer {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Draws the view into layers (bottom bar, title, centre message).
pub fn paint(view: &View, fonts: &Fonts) -> Vec<Layer> {
    draw::paint(view, fonts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_times() {
        assert_eq!(format_time(0.0, false), "0:00");
        assert_eq!(format_time(65.9, false), "1:05");
        assert_eq!(format_time(3725.0, false), "1:02:05");
        assert_eq!(format_time(65.0, true), "0:01:05");
        assert_eq!(format_time(f64::NAN, false), "0:00");
        assert_eq!(format_time(-3.0, false), "0:00");
    }

    #[test]
    fn hit_testing_finds_each_control() {
        let l = Layout::new(1280, 720, 1.0);
        let c = |r: Rect| (r.x + r.w / 2.0, r.y + r.h / 2.0);
        let (x, y) = c(l.play);
        assert_eq!(l.hit(x, y), Some(Target::PlayPause));
        let (x, y) = c(l.fullscreen);
        assert_eq!(l.hit(x, y), Some(Target::Fullscreen));
        let (x, y) = c(l.mute);
        assert_eq!(l.hit(x, y), Some(Target::Mute));
        let (_, y) = c(l.timeline);
        assert_eq!(l.hit(l.timeline.x, y), Some(Target::Timeline(0.0)));
        match l.hit(l.timeline.x + l.timeline.w / 2.0, y) {
            Some(Target::Timeline(f)) => assert!((f - 0.5).abs() < 1e-6),
            other => panic!("{other:?}"),
        }
        assert_eq!(l.hit(640.0, 300.0), None, "the video area is not a control");
    }

    #[test]
    fn controls_do_not_overlap() {
        for (w, s) in [(640, 1.0), (1280, 1.0), (1920, 1.5), (3840, 2.0)] {
            let l = Layout::new(w, w * 9 / 16, s);
            let rects = [l.play, l.mute, l.volume, l.fullscreen];
            for (i, a) in rects.iter().enumerate() {
                for b in &rects[i + 1..] {
                    let overlap = a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;
                    assert!(!overlap, "{a:?} overlaps {b:?} at width {w}");
                }
            }
            assert!(l.time_x < l.mute.x, "the time label must fit before the volume control at width {w}");
        }
    }

    #[test]
    fn view_changes_only_when_pixels_would() {
        let layout = Layout::new(1280, 720, 1.0);
        let mut state = State { has_file: true, duration: 7200.0, position: 100.0, ..State::default() };
        let a = View::new(&state, layout);
        state.position = 100.2; // same second, same pixel
        assert_eq!(a, View::new(&state, layout));
        state.position = 101.0; // the time label changes
        assert_ne!(a, View::new(&state, layout));
    }

    #[test]
    fn paints_layers_inside_the_window_without_fonts() {
        let layout = Layout::new(800, 450, 1.0);
        let state = State { has_file: true, duration: 60.0, position: 30.0, title: "clip".into(), ..State::default() };
        let layers = paint(&View::new(&state, layout), &Fonts::none());
        assert!(!layers.is_empty());
        for l in &layers {
            assert!(l.x >= 0 && l.y >= 0);
            assert!(l.x as u32 + l.width <= 800 && l.y as u32 + l.height <= 450);
            assert_eq!(l.rgba.len(), (l.width * l.height * 4) as usize);
        }
    }

    #[test]
    fn the_update_notice_is_a_top_right_layer_inside_the_window() {
        let layout = Layout::new(800, 450, 1.0);
        let base = State { has_file: true, duration: 60.0, ..State::default() };
        let with = State { notice: Some("Nitrate 0.2.0 is available".into()), ..base.clone() };
        let (a, b) =
            (paint(&View::new(&base, layout), &Fonts::none()), paint(&View::new(&with, layout), &Fonts::none()));
        assert_eq!(b.len(), a.len() + 1);
        let n = b.iter().find(|l| !a.contains(l)).expect("the notice layer");
        assert!(n.x > 400 && n.y < 40, "top right: {}, {}", n.x, n.y);
        assert!(n.x as u32 + n.width <= 800);
    }

    #[test]
    fn hidden_controls_paint_nothing_while_playing() {
        let layout = Layout::new(800, 450, 1.0);
        let state = State { has_file: true, duration: 60.0, controls_visible: false, ..State::default() };
        assert!(paint(&View::new(&state, layout), &Fonts::none()).is_empty());
    }

    #[test]
    fn the_mark_is_opaque_in_the_middle_and_clear_in_the_corner() {
        let rgba = mark_rgba(64);
        assert_eq!(rgba.len(), 64 * 64 * 4);
        assert_eq!(rgba[3], 0, "corner outside the rounded tile is transparent");
        let mid = (32 * 64 + 32) * 4;
        assert_eq!(rgba[mid + 3], 255);
    }
}
