//! The on-screen TPS key panel (desktop TPS roadmap D6): every key of the
//! main block with the glyph it types under TPS and its Shift-layer glyph,
//! up while TPS is typed and the user asked for it (`tpsKeyboardShown`).
//! Show-only — a click does nothing yet (P6).
//!
//! One per service activation, owned by no context: the stored key is all
//! that crosses activations and processes, and only the activation with
//! keyboard focus shows it. Whether it should be up comes from a source the
//! text service installs, so the posted sync a focus callback sends (W3) reads
//! the focus and the settings as they are when the message arrives, with no
//! hold on the service. Placed at the bottom centre of the work area of the monitor
//! the foreground window is on — where an on-screen keyboard sits, clear of
//! most of the text being typed; the hotkey and focus paths have no caret to
//! anchor to.

use super::render::{DWriteMeasurer, RenderFactory, Surface};
use super::theme::{SystemTheme, Theme};
use super::window::{
    frame_in_work_area, monitor_at, monitor_of_window, MonitorArea, PopupWindow, WindowHandler,
    WindowRef, BASE_DPI,
};
use std::cell::RefCell;
use std::rc::Rc;
use taigi_desktop_core::candidates::{FontSpec, TextMeasurer};
use taigi_desktop_core::composing::ContextToken;
use taigi_desktop_core::keys::{tps_keyboard_rows, TpsKeyCap, TpsKeyboardRow};
use taigi_desktop_core::settings::{AppearanceMode, CandidateFontChoice, CandidateFontSelection};
use windows::Win32::Foundation::{POINT, RECT};
use windows::Win32::Graphics::Direct2D::Common::{D2D1_COLOR_F, D2D_RECT_F, D2D_SIZE_U};
use windows::Win32::Graphics::Direct2D::{
    ID2D1HwndRenderTarget, ID2D1SolidColorBrush, D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT,
    D2D1_ROUNDED_RECT,
};
use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
use windows_numerics::Vector2;

const WINDOW_CLASS: &str = "TaigiKeyboardTpsKeyboard";
/// A key cap's side and the gap between caps, in DIPs. Mirrors
/// `TpsKeyboardPanel.capSize` / `capGap` on macOS.
const CAP_SIZE: f32 = 48.0;
const CAP_GAP: f32 = 6.0;
const CAP_CORNER_RADIUS: f32 = 6.0;
const CAP_STROKE: f32 = 1.0;
/// The inset of the cap's corner labels from its edges.
const CAP_INSET: f32 = 4.0;
const PADDING: f32 = 12.0;
/// How far the panel sits above the bottom of the work area.
const BOTTOM_MARGIN: f32 = 24.0;
const GLYPH_FONT_SIZE: f32 = 22.0;
const SHIFT_GLYPH_FONT_SIZE: f32 = 13.0;
const LABEL_FONT_SIZE: f32 = 11.0;
/// The glyphs draw in a bundled face that holds every glyph the layout
/// types, ㆻ (U+31BB, Unicode 13) included — the system face lacks it (a
/// tofu box on the dev box, 2026-10-04) and jf open 粉圓 lacks ˪ ˫. Iansui,
/// whose family name `render.rs` `bundled_family_name` resolves.
const GLYPH_FONT: CandidateFontChoice = CandidateFontChoice::Iansui;

/// The appearance to draw the panel in as focus and the settings stand now,
/// or `None` when it should not be up (`SettingsDocument::is_tps_keyboard_wanted`).
pub type TpsKeyboardSource = Box<dyn Fn() -> Option<AppearanceMode>>;

/// The panel's window; its content lives in the window's handler.
pub struct TpsKeyboard {
    window: Option<PopupWindow>,
}

/// What one frame draws with (`Surface::frame`'s pair).
struct Pen<'a> {
    target: &'a ID2D1HwndRenderTarget,
    brush: &'a ID2D1SolidColorBrush,
}

struct PanelContent {
    factory: Rc<RenderFactory>,
    source: TpsKeyboardSource,
    /// The core's rows, fixed for the life of the process.
    rows: Vec<TpsKeyboardRow>,
    appearance: AppearanceMode,
    theme: Theme,
    dpi: f32,
    surface: Option<Surface>,
}

impl TpsKeyboard {
    /// Made with its (hidden) window, at activation: every later move is a
    /// posted sync, and a focus callback may neither create nor show a window
    /// (W3) — one hidden popup per activation is the price of a panel that
    /// can come up on focus alone.
    pub fn new(factory: Rc<RenderFactory>, source: TpsKeyboardSource) -> Self {
        let appearance = AppearanceMode::Auto;
        let panel = Rc::new(RefCell::new(PanelContent {
            factory,
            source,
            rows: tps_keyboard_rows(),
            appearance,
            theme: Theme::resolve(appearance, &SystemTheme::read()),
            dpi: BASE_DPI,
            surface: None,
        }));
        let window = PopupWindow::create(WINDOW_CLASS, panel)
            .inspect_err(|error| log::error!("ui.tps_keyboard_window_failed error={error}"))
            .ok();
        Self { window }
    }

    /// Queues a sync for the message loop: the panel shows or hides as its
    /// source says when the message runs, so whatever focus or setting moved
    /// in between, the last sync settles it. Every path asks this way — a
    /// focus callback must (W3), and a synchronous show could re-enter a
    /// focus callback that then finds the panel busy and is lost.
    pub fn request_sync(&self) {
        if let Some(window) = self.window.as_deref() {
            window.post_sync_request();
        }
    }

    pub fn destroy(&mut self) {
        if let Some(window) = self.window.take() {
            window.destroy();
        }
    }
}

impl PanelContent {
    /// Up at the bottom of the foreground window's monitor when the source
    /// answers content; down otherwise. With no foreground window to place
    /// it by, nothing changes until the next sync.
    fn sync(&mut self, window: &WindowRef) {
        let Some(appearance) = (self.source)() else {
            window.hide();
            return;
        };
        let Some(monitor) = foreground_monitor() else {
            return;
        };
        self.appearance = appearance;
        self.theme = Theme::resolve(appearance, &SystemTheme::read());
        // The surface is sized in pixels: kept unless the scale moved.
        if monitor.dpi != self.dpi {
            self.dpi = monitor.dpi;
            self.surface = None;
        }
        window.show_at(bottom_frame(&monitor, panel_size(&self.rows)));
    }

    fn draw_text(
        &self,
        pen: &Pen<'_>,
        text: &str,
        font: FontSpec,
        frame: (f32, f32, f32, f32),
        centered: bool,
        colour: D2D1_COLOR_F,
    ) {
        let (x, y, width, height) = frame;
        let layout = if centered {
            self.factory.layout_centered(text, font, width, height)
        } else {
            self.factory.layout(text, font, width, height)
        };
        let Ok(layout) = layout else {
            return;
        };
        // SAFETY: drawing on our own target between Begin/EndDraw.
        unsafe {
            pen.brush.SetColor(&colour);
            pen.target.DrawTextLayout(
                Vector2 { X: x, Y: y },
                &layout,
                pen.brush,
                D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT,
            );
        }
    }

    /// One cap at `(x, y)`: its outline, the key's label top left, the Shift
    /// glyph top right, the glyph large in the middle.
    fn draw_cap(&self, pen: &Pen<'_>, cap: &TpsKeyCap, x: f32, y: f32) {
        let theme = self.theme;
        let rounded = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: x + CAP_STROKE / 2.0,
                top: y + CAP_STROKE / 2.0,
                right: x + CAP_SIZE - CAP_STROKE / 2.0,
                bottom: y + CAP_SIZE - CAP_STROKE / 2.0,
            },
            radiusX: CAP_CORNER_RADIUS,
            radiusY: CAP_CORNER_RADIUS,
        };
        // SAFETY: drawing on our own target between Begin/EndDraw.
        unsafe {
            pen.brush.SetColor(&theme.border);
            pen.target
                .DrawRoundedRectangle(&rounded, pen.brush, CAP_STROKE, None);
        }
        let corner_height = SHIFT_GLYPH_FONT_SIZE * 1.4;
        self.draw_text(
            pen,
            &cap.label.to_string(),
            font(CandidateFontChoice::System, LABEL_FONT_SIZE),
            (x + CAP_INSET, y + CAP_INSET, CAP_SIZE / 2.0, corner_height),
            false,
            theme.tertiary_text,
        );
        if let Some(shift_glyph) = cap.shift_glyph {
            let shift_font = font(GLYPH_FONT, SHIFT_GLYPH_FONT_SIZE);
            let width = DWriteMeasurer {
                factory: &self.factory,
            }
            .width(shift_glyph, shift_font);
            self.draw_text(
                pen,
                shift_glyph,
                shift_font,
                (
                    x + CAP_SIZE - CAP_INSET - width,
                    y + CAP_INSET,
                    width.max(1.0),
                    corner_height,
                ),
                false,
                theme.secondary_text,
            );
        }
        let glyph_top = y + CAP_INSET + corner_height / 2.0;
        self.draw_text(
            pen,
            cap.glyph,
            font(GLYPH_FONT, GLYPH_FONT_SIZE),
            (x, glyph_top, CAP_SIZE, y + CAP_SIZE - glyph_top),
            true,
            theme.text,
        );
    }
}

fn font(choice: CandidateFontChoice, size: f32) -> FontSpec {
    FontSpec {
        selection: CandidateFontSelection::BuiltIn(choice),
        size,
    }
}

/// The panel's size in DIPs: the widest row (its indent and caps) and four
/// rows of caps, padded.
fn panel_size(rows: &[TpsKeyboardRow]) -> (f32, f32) {
    let pitch = CAP_SIZE + CAP_GAP;
    let width = rows
        .iter()
        .map(|row| (row.indent + row.caps.len() as f32) * pitch - CAP_GAP)
        .fold(0.0f32, f32::max);
    let count = rows.len() as f32;
    let height = count * pitch - CAP_GAP;
    (width + 2.0 * PADDING, height.max(0.0) + 2.0 * PADDING)
}

/// The monitor the foreground window is on — the application the user is
/// typing in. `None` while no window is foreground (activation moving).
fn foreground_monitor() -> Option<MonitorArea> {
    // SAFETY: a plain window query.
    let foreground = unsafe { GetForegroundWindow() };
    if foreground.is_invalid() {
        return None;
    }
    monitor_of_window(foreground)
}

/// The panel at the bottom centre of `monitor`'s work area.
fn bottom_frame(monitor: &MonitorArea, size: (f32, f32)) -> RECT {
    frame_in_work_area(monitor, size, Some(BOTTOM_MARGIN))
}

impl WindowHandler for PanelContent {
    fn paint(&mut self, window: &WindowRef) {
        let size = panel_size(&self.rows);
        let scale = self.dpi / BASE_DPI;
        let pixel_size = D2D_SIZE_U {
            width: (size.0 * scale).ceil().max(1.0) as u32,
            height: (size.1 * scale).ceil().max(1.0) as u32,
        };
        if self.surface.is_none() {
            match Surface::create(&self.factory, window.hwnd(), pixel_size, self.dpi) {
                Ok(surface) => self.surface = Some(surface),
                Err(error) => {
                    log::error!("ui.tps_keyboard_surface_failed error={error}");
                    return;
                }
            }
        }
        let Some(surface) = &self.surface else { return };
        let theme = self.theme;
        let pitch = CAP_SIZE + CAP_GAP;
        let lost = surface
            .frame(|target, brush| {
                // SAFETY: drawing on our own target between Begin/EndDraw.
                unsafe { target.Clear(Some(&theme.background)) };
                let pen = Pen { target, brush };
                for (row_index, row) in self.rows.iter().enumerate() {
                    let y = PADDING + row_index as f32 * pitch;
                    for (cap_index, cap) in row.caps.iter().enumerate() {
                        let x = PADDING + (row.indent + cap_index as f32) * pitch;
                        self.draw_cap(&pen, cap, x, y);
                    }
                }
            })
            .is_err();
        if lost {
            self.surface = None;
        }
    }

    /// Show-only until the click path lands (desktop TPS roadmap P6).
    fn click(&mut self, _window: &WindowRef, _point: (f32, f32)) {}

    fn wheel(&mut self, _window: &WindowRef, _delta: f32) {}

    /// The panel moved to a monitor with another scale: placed again at the
    /// bottom of that monitor at the new DPI.
    fn dpi_changed(&mut self, window: &WindowRef, dpi: f32, suggested: RECT) {
        self.dpi = dpi;
        self.surface = None;
        let centre = POINT {
            x: (suggested.left + suggested.right) / 2,
            y: (suggested.top + suggested.bottom) / 2,
        };
        match monitor_at(centre) {
            Some(monitor) => window.show_at(bottom_frame(&monitor, panel_size(&self.rows))),
            None if suggested.right > suggested.left => window.show_at(suggested),
            None => {}
        }
    }

    fn timer(&mut self, _window: &WindowRef, _id: usize) {}

    /// Required by the trait; never posted to this window — focus events
    /// post a sync, which hides it when it should not be up.
    fn hide_requested(&mut self, _window: &WindowRef, _owner: Option<ContextToken>) {}

    /// Stays up across a theme change, so it is re-coloured in place (the
    /// window invalidates itself after this).
    fn system_theme_changed(&mut self) {
        self.theme = Theme::resolve(self.appearance, &SystemTheme::read());
    }

    fn sync_requested(&mut self, window: &WindowRef) {
        self.sync(window);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use taigi_desktop_core::keys::tps_keyboard_rows;

    #[test]
    fn the_panel_is_as_wide_as_its_widest_staggered_row() {
        // trace: rows 12 @0, 10 @0.5, 11 @0.75, 10 @1.25 key widths; pitch
        // 54 → widths 642, 561, 628.5, 601.5; the number row is widest.
        // Height: 4 × 54 − 6 = 210. Plus 12 padding each side.
        let (width, height) = panel_size(&tps_keyboard_rows());
        assert_eq!(width, 642.0 + 24.0);
        assert_eq!(height, 210.0 + 24.0);
    }

    #[test]
    fn the_frame_sits_centred_above_the_bottom_of_the_work_area() {
        // trace: work area 0..1920 × 0..1040 at 144 DPI (scale 1.5); a
        // 100 × 50 DIP panel is 150 × 75 px; margin 24 DIP = 36 px.
        let monitor = MonitorArea {
            work_area: RECT {
                left: 0,
                top: 0,
                right: 1920,
                bottom: 1040,
            },
            dpi: 144.0,
        };
        let frame = bottom_frame(&monitor, (100.0, 50.0));
        assert_eq!(
            (frame.left, frame.top, frame.right, frame.bottom),
            (885, 929, 1035, 1004)
        );
    }
}
