use core::event::Event;
use std::any::Any;
use std::sync::Arc;

pub enum RenderIntent {
    Window(WindowConfig),
    Overlay(OverlayConfig),
}

impl Clone for RenderIntent {
    fn clone(&self) -> Self {
        match self {
        RenderIntent::Window(cfg) => RenderIntent::Window(WindowConfig {
            width: cfg.width,
            height: cfg.height,
            position: cfg.position.clone(),
            auto_close: cfg.auto_close,
            draws: cfg.draws.clone(),
            selected_index: 0,
            on_hit: None,
            on_delete: None,
            on_close: None,
        }),
            RenderIntent::Overlay(_) => panic!("Overlay clone not implemented"),
        }
    }
}

pub struct RenderIntentEvent(pub RenderIntent);
impl Event for RenderIntentEvent {
    fn event_type(&self) -> &'static str { "render.intent" }
    fn as_any(&self) -> &dyn Any { self }
}

pub struct WindowConfig {
    pub width: u32,
    pub height: u32,
    pub position: WindowPosition,
    pub auto_close: bool,
    pub draws: Vec<DrawCmd>,
    pub selected_index: usize,
    pub on_hit: Option<Arc<dyn Fn(u64) + Send + Sync + 'static>>,
    pub on_delete: Option<Arc<dyn Fn(u64) + Send + Sync + 'static>>,
    pub on_close: Option<Arc<dyn Fn() + Send + Sync + 'static>>,
}

#[derive(Clone)]
pub enum WindowPosition {
    NearCursor,
    FollowFocus,
    ScreenCenter,
    At { x: i32, y: i32 },
}

pub struct OverlayConfig {
    pub bounds: OverlayBounds,
    pub transparent: bool,
    pub draws: Vec<DrawCmd>,
    pub on_mouse: Option<Arc<dyn Fn(OverlayMouseEvent) + Send + Sync + 'static>>,
}

#[derive(Clone)]
pub enum OverlayBounds {
    FullScreen,
    Region { x: i32, y: i32, width: u32, height: u32 },
    FollowCursor { width: u32, height: u32, offset_x: i32, offset_y: i32 },
}

#[derive(Clone)]
pub enum DrawCmd {
    Text(DrawText),
    Image(DrawImage),
    Separator(DrawSeparator),
    Rect(DrawRect),
    FillRect(DrawFillRect),
    Line(DrawLine),
    Polyline(DrawPolyline),
    HitArea(DrawHitArea),
    ClearAll,
}

#[derive(Clone)]
pub struct DrawText {
    pub x: i32,
    pub y: i32,
    pub text: String,
    pub font_size: u32,
    pub color: u32,
}

#[derive(Clone)]
pub struct DrawImage {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

#[derive(Clone)]
pub struct DrawSeparator {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub color: u32,
}

#[derive(Clone)]
pub struct DrawRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub color: u32,
    pub thickness: u32,
}

#[derive(Clone)]
pub struct DrawFillRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub color: u32,
    pub alpha: u8,
}

#[derive(Clone)]
pub struct DrawLine {
    pub x1: i32,
    pub y1: i32,
    pub x2: i32,
    pub y2: i32,
    pub color: u32,
    pub thickness: u32,
}

#[derive(Clone)]
pub struct DrawPolyline {
    pub points: Vec<(i32, i32)>,
    pub color: u32,
    pub thickness: u32,
}

#[derive(Clone)]
pub struct DrawHitArea {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub id: u64,
}

#[derive(Clone)]
pub struct OverlayMouseEvent {
    pub button: OverlayMouseButton,
    pub x: i32,
    pub y: i32,
    pub action: OverlayMouseAction,
}

#[derive(Clone, PartialEq)]
pub enum OverlayMouseButton {
    Left, Right, Middle, None,
}

#[derive(Clone)]
pub enum OverlayMouseAction {
    Down, Move, Up,
}
