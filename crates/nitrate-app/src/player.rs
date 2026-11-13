//! The player window (Windows): input, mpv events and the controls. The UI
//! thread never waits for mpv: it sends async commands and reads events with
//! a zero timeout (AGENTS.md rule 1). Video frames never pass through here.

use crate::Options;
use crate::d3d::{Presenter, Started};
use nitrate_mpv::{EndReason, Event, Format, Libmpv, Mpv, Value};
use nitrate_ui::{Fonts, Layout, State, Target, View};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition};
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::{Fullscreen, Icon, Window, WindowId};

const HIDE_AFTER: Duration = Duration::from_millis(2500);
const DOUBLE_CLICK: Duration = Duration::from_millis(300);

const MEDIA_EXTENSIONS: &[&str] = &[
    "mkv", "mp4", "m4v", "mov", "webm", "avi", "wmv", "flv", "ts", "m2ts", "mts", "mpg", "mpeg", "ogv", "3gp", "mp3",
    "flac", "m4a", "ogg", "opus", "wav",
];

#[derive(Debug, Clone, Copy)]
enum UserEvent {
    MpvWakeup,
}

// Observed property ids.
const P_TIME: u64 = 1;
const P_DURATION: u64 = 2;
const P_PAUSE: u64 = 3;
const P_VOLUME: u64 = 4;
const P_MUTE: u64 = 5;
const P_TITLE: u64 = 6;
const P_IDLE: u64 = 7;
const P_HWDEC: u64 = 8;

struct App {
    options: Options,
    mpv: Mpv,
    window: Option<Arc<Window>>,
    presenter: Option<Presenter>,
    started: Option<Started>,
    state: State,
    fonts: Fonts,
    last_view: Option<View>,
    last_activity: Instant,
    pending_click: Option<Instant>,
    modifiers: ModifiersState,
    hwdec: String,
    launched: Instant,
    first_frame: Option<Duration>,
    smoke_until: Option<Instant>,
    finished: bool,
}

pub fn run(options: Options) -> Result<(), String> {
    let lib = Arc::new(Libmpv::load().map_err(|e| e.to_string())?);
    let mpv = Mpv::create(lib).map_err(|e| e.to_string())?;
    let opts: &[(&str, &str)] = &[
        ("vo", "libmpv"),
        ("hwdec", "auto-safe"),
        ("keep-open", "yes"),
        ("idle", "yes"),
        ("input-default-bindings", "no"),
        ("input-vo-keyboard", "no"),
        ("osc", "no"),
        ("osd-level", "0"),
        ("terminal", "no"),
        ("config", "no"),
        ("load-scripts", "no"),
        ("ytdl", "no"),
        ("audio-client-name", "Nitrate"),
    ];
    for (k, v) in opts {
        mpv.set_option(k, v).map_err(|e| format!("mpv option {k}={v}: {e}"))?;
    }
    if options.mute {
        mpv.set_option("mute", "yes").map_err(|e| e.to_string())?;
    }
    mpv.initialize().map_err(|e| e.to_string())?;
    for (id, name, format) in [
        (P_TIME, "time-pos", Format::Double),
        (P_DURATION, "duration", Format::Double),
        (P_PAUSE, "pause", Format::Flag),
        (P_VOLUME, "volume", Format::Double),
        (P_MUTE, "mute", Format::Flag),
        (P_TITLE, "media-title", Format::String),
        (P_IDLE, "idle-active", Format::Flag),
        (P_HWDEC, "hwdec-current", Format::String),
    ] {
        mpv.observe(id, name, format).map_err(|e| e.to_string())?;
    }

    let event_loop = EventLoop::<UserEvent>::with_user_event().build().map_err(|e| e.to_string())?;
    let proxy: EventLoopProxy<UserEvent> = event_loop.create_proxy();
    mpv.set_wakeup_callback(move || {
        let _ = proxy.send_event(UserEvent::MpvWakeup);
    });

    let remote = is_remote_session();
    let smoke_until = options.smoke.map(|s| Instant::now() + Duration::from_secs_f64(s));
    let mut app = App {
        options,
        mpv,
        window: None,
        presenter: None,
        started: None,
        state: State { remote, ..State::default() },
        fonts: Fonts::system(),
        last_view: None,
        last_activity: Instant::now(),
        pending_click: None,
        modifiers: ModifiersState::empty(),
        hwdec: String::new(),
        launched: Instant::now(),
        first_frame: None,
        smoke_until,
        finished: false,
    };
    event_loop.run_app(&mut app).map_err(|e| e.to_string())
}

fn is_remote_session() -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_REMOTESESSION};
    // SAFETY: plain query.
    unsafe { GetSystemMetrics(SM_REMOTESESSION) != 0 }
}

impl App {
    fn layout(&self) -> Option<Layout> {
        let w = self.window.as_ref()?;
        let size = w.inner_size();
        Some(Layout::new(size.width, size.height, w.scale_factor() as f32))
    }

    fn command(&self, args: &[&str]) {
        if let Err(e) = self.mpv.command(args) {
            eprintln!("nitrate: mpv {args:?}: {e}");
        }
    }

    fn open(&mut self, path: &str) {
        self.state.message = None;
        self.command(&["loadfile", path, "replace"]);
    }

    fn open_dialog(&mut self) {
        let picked =
            rfd::FileDialog::new().add_filter("Media", MEDIA_EXTENSIONS).add_filter("All files", &["*"]).pick_file();
        if let Some(path) = picked {
            self.open(&path.to_string_lossy());
        }
    }

    fn toggle_fullscreen(&mut self) {
        if let Some(w) = &self.window {
            let full = w.fullscreen().is_some();
            w.set_fullscreen(if full { None } else { Some(Fullscreen::Borderless(None)) });
            self.state.fullscreen = !full;
        }
    }

    fn seek_fraction(&self, f: f64, exact: bool) {
        if self.state.duration > 0.0 {
            let t = format!("{:.3}", f * self.state.duration);
            self.command(&["seek", &t, if exact { "absolute+exact" } else { "absolute+keyframes" }]);
        }
    }

    fn set_volume(&mut self, v: f64) {
        let v = v.clamp(0.0, 130.0);
        self.state.volume = v; // shown at once; mpv confirms through the property
        self.command(&["set", "volume", &format!("{v:.0}")]);
        if self.state.muted && v > 0.0 {
            self.command(&["set", "mute", "no"]);
        }
    }

    fn activity(&mut self) {
        self.last_activity = Instant::now();
    }

    fn drain_mpv(&mut self, event_loop: &ActiveEventLoop) {
        while let Some(event) = self.mpv.wait_event(0.0) {
            match event {
                Event::Shutdown => event_loop.exit(),
                Event::FileLoaded => {
                    self.state.has_file = true;
                    self.state.message = None;
                    self.activity();
                }
                Event::PlaybackRestart if self.first_frame.is_none() && self.state.has_file => {
                    self.first_frame = Some(self.launched.elapsed());
                }
                Event::EndFile { reason: EndReason::Error, error } => {
                    let why = error.map(|e| e.message).unwrap_or_else(|| "unknown error".into());
                    self.state.message = Some(format!("Can't play this file: {why}"));
                }
                Event::PropertyChange { id, value, .. } => match (id, value) {
                    (P_TIME, Value::Double(t)) => self.state.position = t,
                    (P_TIME, Value::None) => self.state.position = 0.0,
                    (P_DURATION, Value::Double(d)) => self.state.duration = d,
                    (P_DURATION, Value::None) => self.state.duration = 0.0,
                    (P_PAUSE, Value::Flag(p)) => {
                        self.state.paused = p;
                        self.activity();
                    }
                    (P_VOLUME, Value::Double(v)) => self.state.volume = v,
                    (P_MUTE, Value::Flag(m)) => self.state.muted = m,
                    (P_TITLE, Value::String(t)) => {
                        if let Some(w) = &self.window {
                            w.set_title(
                                if t.is_empty() { "Nitrate".into() } else { format!("{t} - Nitrate") }.as_str(),
                            );
                        }
                        self.state.title = t;
                    }
                    (P_IDLE, Value::Flag(idle)) => {
                        if idle {
                            self.state.has_file = false;
                        }
                    }
                    (P_HWDEC, Value::String(h)) => self.hwdec = h,
                    _ => {}
                },
                _ => {}
            }
        }
    }

    fn refresh(&mut self) {
        let Some(layout) = self.layout() else { return };
        let over_controls = self.state.pointer.is_some_and(|(x, y)| layout.over_controls(x, y));
        let visible = !self.state.has_file
            || self.state.paused
            || self.state.dragging.is_some()
            || over_controls
            || self.last_activity.elapsed() < HIDE_AFTER;
        self.state.controls_visible = visible;
        if let Some(w) = &self.window {
            w.set_cursor_visible(visible || self.state.pointer.is_none());
        }
        let view = View::new(&self.state, layout);
        if self.last_view.as_ref() != Some(&view) {
            let layers = nitrate_ui::paint(&view, &self.fonts);
            if let Some(p) = &self.presenter {
                p.set_layers(Arc::new(layers));
            }
            self.last_view = Some(view);
        }
    }

    fn finish_smoke(&mut self, event_loop: &ActiveEventLoop) {
        if std::mem::replace(&mut self.finished, true) {
            return;
        }
        if let Some(path) = self.options.snapshot.clone() {
            // Show the controls in the shot.
            self.activity();
            self.refresh();
            if let Some(p) = &self.presenter
                && let Err(e) = p.snapshot(path.into())
            {
                eprintln!("nitrate: snapshot failed: {e}");
            }
        }
        let frames = self.presenter.as_ref().map(|p| p.stats.video_frames.load(Ordering::Relaxed)).unwrap_or(0);
        let presents = self.presenter.as_ref().map(|p| p.stats.presents.load(Ordering::Relaxed)).unwrap_or(0);
        let drops = self.mpv.get_i64("frame-drop-count").unwrap_or(-1);
        let delayed = self.mpv.get_i64("vo-delayed-frame-count").unwrap_or(-1);
        let (adapter, software) = self.started.as_ref().map(|s| (s.adapter.clone(), s.software)).unwrap_or_default();
        println!(
            "smoke: adapter={adapter} software={software} remote={} hwdec={} first_frame_ms={} video_frames={frames} presents={presents} dropped={drops} delayed={delayed}",
            self.state.remote,
            if self.hwdec.is_empty() { "no" } else { &self.hwdec },
            self.first_frame.map(|d| d.as_millis().to_string()).unwrap_or_else(|| "none".into()),
        );
        event_loop.exit();
    }
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        use winit::platform::windows::WindowAttributesExtWindows;
        let icon = Icon::from_rgba(nitrate_ui::mark_rgba(64), 64, 64).ok();
        let mut attrs = Window::default_attributes()
            .with_title("Nitrate")
            .with_inner_size(LogicalSize::new(1280.0, 720.0))
            .with_min_inner_size(LogicalSize::new(480.0, 270.0))
            .with_window_icon(icon.clone())
            .with_taskbar_icon(icon);
        if self.smoke_until.is_some() {
            // Agent runs never show on the owner's screen (AGENTS.md).
            attrs =
                attrs.with_position(PhysicalPosition::new(-20000, -20000)).with_skip_taskbar(true).with_active(false);
        }
        let window = match event_loop.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                eprintln!("nitrate: cannot create the window: {e}");
                event_loop.exit();
                return;
            }
        };
        let hwnd = match window.window_handle().map(|h| h.as_raw()) {
            Ok(RawWindowHandle::Win32(h)) => h.hwnd.get(),
            _ => {
                eprintln!("nitrate: no Win32 window handle");
                event_loop.exit();
                return;
            }
        };
        let size = window.inner_size();
        match Presenter::start(hwnd, (size.width, size.height), self.mpv.clone()) {
            Ok((presenter, started)) => {
                self.presenter = Some(presenter);
                self.started = Some(started);
            }
            Err(e) => self.state.message = Some(format!("Video output failed: {e}")),
        }
        self.window = Some(window);
        if let Some(file) = self.options.file.clone() {
            self.open(&file);
        }
        self.refresh();
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::MpvWakeup => self.drain_mpv(event_loop),
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(p) = &self.presenter {
                    p.resize(size.width, size.height);
                }
            }
            WindowEvent::ModifiersChanged(m) => self.modifiers = m.state(),
            WindowEvent::CursorMoved { position, .. } => {
                let (x, y) = (position.x as f32, position.y as f32);
                self.state.pointer = Some((x, y));
                self.activity();
                if let Some(layout) = self.layout() {
                    match self.state.dragging {
                        Some(Target::Timeline(_)) => {
                            let f = layout.timeline_fraction(x);
                            self.state.dragging = Some(Target::Timeline(f));
                            self.state.position = f * self.state.duration;
                            self.seek_fraction(f, false);
                        }
                        Some(Target::Volume(_)) => {
                            let f = layout.volume_fraction(x);
                            self.state.dragging = Some(Target::Volume(f));
                            self.set_volume(f * 100.0);
                        }
                        _ => {}
                    }
                }
            }
            WindowEvent::CursorLeft { .. } => self.state.pointer = None,
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. } => {
                self.activity();
                let hit = match (self.layout(), self.state.pointer) {
                    (Some(l), Some((x, y))) => l.hit(x, y),
                    _ => None,
                };
                let controls_shown = self.state.controls_visible && self.state.has_file;
                match hit.filter(|_| controls_shown) {
                    Some(Target::PlayPause) => self.command(&["cycle", "pause"]),
                    Some(Target::Timeline(f)) => {
                        self.state.dragging = Some(Target::Timeline(f));
                        self.state.position = f * self.state.duration;
                        self.seek_fraction(f, false);
                    }
                    Some(Target::Volume(f)) => {
                        self.state.dragging = Some(Target::Volume(f));
                        self.set_volume(f * 100.0);
                    }
                    Some(Target::Mute) => self.command(&["cycle", "mute"]),
                    Some(Target::Fullscreen) => self.toggle_fullscreen(),
                    None => {
                        if !self.state.has_file {
                            self.open_dialog();
                        } else if self.pending_click.take().is_some_and(|t| t.elapsed() < DOUBLE_CLICK) {
                            self.toggle_fullscreen();
                        } else {
                            self.pending_click = Some(Instant::now());
                        }
                    }
                }
            }
            WindowEvent::MouseInput { state: ElementState::Released, button: MouseButton::Left, .. } => {
                if let Some(Target::Timeline(f)) = self.state.dragging.take() {
                    self.seek_fraction(f, true);
                }
                self.state.dragging = None;
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let steps = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y as f64,
                    MouseScrollDelta::PixelDelta(p) => p.y / 40.0,
                };
                self.activity();
                self.set_volume(self.state.volume + steps * 5.0);
            }
            WindowEvent::DroppedFile(path) => self.open(&path.to_string_lossy()),
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                self.activity();
                let ctrl = self.modifiers.control_key();
                let shift = self.modifiers.shift_key();
                match event.logical_key.as_ref() {
                    Key::Named(NamedKey::Space) => self.command(&["cycle", "pause"]),
                    Key::Named(NamedKey::ArrowLeft) => {
                        self.command(&["seek", if shift { "-1" } else { "-5" }, "relative+exact"])
                    }
                    Key::Named(NamedKey::ArrowRight) => {
                        self.command(&["seek", if shift { "1" } else { "5" }, "relative+exact"])
                    }
                    Key::Named(NamedKey::ArrowUp) => self.set_volume(self.state.volume + 5.0),
                    Key::Named(NamedKey::ArrowDown) => self.set_volume(self.state.volume - 5.0),
                    Key::Named(NamedKey::Escape) if self.state.fullscreen => self.toggle_fullscreen(),
                    Key::Named(NamedKey::Enter) if self.modifiers.alt_key() => self.toggle_fullscreen(),
                    Key::Character(c) => match (c.to_ascii_lowercase().as_str(), ctrl) {
                        ("o", true) => self.open_dialog(),
                        ("f", false) => self.toggle_fullscreen(),
                        ("m", false) => self.command(&["cycle", "mute"]),
                        ("k", false) => self.command(&["cycle", "pause"]),
                        ("j", false) => self.command(&["seek", "-10", "relative+exact"]),
                        ("l", false) => self.command(&["seek", "10", "relative+exact"]),
                        (".", false) => self.command(&["frame-step"]),
                        (",", false) => self.command(&["frame-back-step"]),
                        _ => {}
                    },
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(t) = self.pending_click
            && t.elapsed() >= DOUBLE_CLICK
        {
            self.pending_click = None;
            self.command(&["cycle", "pause"]);
        }
        if self.smoke_until.is_some_and(|t| Instant::now() >= t) {
            self.finish_smoke(event_loop);
            return;
        }
        self.refresh();
        // Sleep until the next thing we asked for; otherwise wait for events.
        let mut next: Option<Instant> = self.smoke_until;
        let mut consider = |t: Instant| next = Some(next.map_or(t, |n| n.min(t)));
        if let Some(t) = self.pending_click {
            consider(t + DOUBLE_CLICK);
        }
        if self.state.controls_visible && self.state.has_file && !self.state.paused {
            consider(self.last_activity + HIDE_AFTER);
        }
        event_loop.set_control_flow(match next {
            Some(t) => ControlFlow::WaitUntil(t),
            None => ControlFlow::Wait,
        });
    }
}
