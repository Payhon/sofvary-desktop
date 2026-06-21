use crate::platform::{OsKind, WorkArea};

pub const COMMAND_WINDOW: WindowSize = WindowSize {
    width: 760,
    height: 620,
};
pub const MACOS_COMMAND_WINDOW: WindowSize = WindowSize {
    width: COMMAND_WINDOW.width * 2,
    height: COMMAND_WINDOW.height * 2,
};
pub const GLYPH_WINDOW: WindowSize = WindowSize {
    width: 52,
    height: 52,
};

const RIGHT_EDGE_MARGIN: i32 = 14;
const BOTTOM_EDGE_MARGIN: i32 = 18;
const SNAP_THRESHOLD: i32 = 52;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowSize {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowPosition {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutModifier {
    Alt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutKey {
    A,
    I,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlobalShortcutSequence {
    pub display: &'static str,
    pub first_accelerator: &'static str,
    pub second_accelerator: &'static str,
    pub modifier: ShortcutModifier,
    pub first_key: ShortcutKey,
    pub second_key: ShortcutKey,
    pub window_ms: u64,
}

pub fn global_shortcut_sequence(os: OsKind) -> GlobalShortcutSequence {
    let display = match os {
        OsKind::Macos => "Option+A+I",
        OsKind::Windows | OsKind::Linux => "Alt+A+I",
    };

    GlobalShortcutSequence {
        display,
        first_accelerator: "alt+a",
        second_accelerator: "alt+i",
        modifier: ShortcutModifier::Alt,
        first_key: ShortcutKey::A,
        second_key: ShortcutKey::I,
        window_ms: 1200,
    }
}

pub fn tray_or_menu_bar_available(os: OsKind) -> bool {
    matches!(os, OsKind::Windows | OsKind::Macos)
}

pub fn command_window_size(os: OsKind) -> WindowSize {
    match os {
        OsKind::Macos => MACOS_COMMAND_WINDOW,
        OsKind::Windows | OsKind::Linux => COMMAND_WINDOW,
    }
}

pub fn fit_window_size_to_work_area(work_area: WorkArea, desired: WindowSize) -> WindowSize {
    let horizontal_margin = (RIGHT_EDGE_MARGIN.max(0) as u32).saturating_mul(2);
    let vertical_margin = (BOTTOM_EDGE_MARGIN.max(0) as u32).saturating_mul(2);
    let max_width = work_area.width.saturating_sub(horizontal_margin).max(320);
    let max_height = work_area.height.saturating_sub(vertical_margin).max(320);

    WindowSize {
        width: desired.width.min(max_width),
        height: desired.height.min(max_height),
    }
}

pub fn command_window_position(work_area: WorkArea, size: WindowSize) -> WindowPosition {
    WindowPosition {
        x: right_edge_x(&work_area, size, RIGHT_EDGE_MARGIN),
        y: work_area.y + ((work_area.height as i32 - size.height as i32) / 2).max(0),
    }
}

pub fn glyph_window_position(work_area: WorkArea, size: WindowSize) -> WindowPosition {
    WindowPosition {
        x: right_edge_x(&work_area, size, RIGHT_EDGE_MARGIN),
        y: work_area.y + work_area.height as i32 - size.height as i32 - BOTTOM_EDGE_MARGIN,
    }
}

pub fn snap_command_window_position(
    work_area: WorkArea,
    size: WindowSize,
    current_position: WindowPosition,
) -> Option<WindowPosition> {
    let target_x = right_edge_x(&work_area, size, RIGHT_EDGE_MARGIN);
    if (current_position.x - target_x).abs() <= SNAP_THRESHOLD {
        return Some(WindowPosition {
            x: target_x,
            y: current_position.y,
        });
    }

    None
}

fn right_edge_x(work_area: &WorkArea, size: WindowSize, margin_right: i32) -> i32 {
    work_area.x + work_area.width as i32 - size.width as i32 - margin_right
}

#[cfg(test)]
mod tests {
    use super::*;

    fn work_area() -> WorkArea {
        WorkArea {
            x: 10,
            y: 20,
            width: 1200,
            height: 800,
        }
    }

    #[test]
    fn places_command_window_on_right_edge_centered_vertically() {
        assert_eq!(
            command_window_position(work_area(), COMMAND_WINDOW),
            WindowPosition { x: 436, y: 110 }
        );
    }

    #[test]
    fn doubles_command_window_size_on_macos_only() {
        assert_eq!(command_window_size(OsKind::Windows), COMMAND_WINDOW);
        assert_eq!(command_window_size(OsKind::Linux), COMMAND_WINDOW);
        assert_eq!(
            command_window_size(OsKind::Macos),
            WindowSize {
                width: 1520,
                height: 1240
            }
        );
    }

    #[test]
    fn fits_large_command_window_inside_work_area() {
        assert_eq!(
            fit_window_size_to_work_area(work_area(), MACOS_COMMAND_WINDOW),
            WindowSize {
                width: 1172,
                height: 764
            }
        );
    }

    #[test]
    fn places_glyph_bottom_right() {
        assert_eq!(
            glyph_window_position(work_area(), GLYPH_WINDOW),
            WindowPosition { x: 1144, y: 750 }
        );
    }

    #[test]
    fn snaps_command_window_only_near_right_edge() {
        assert_eq!(
            snap_command_window_position(
                work_area(),
                COMMAND_WINDOW,
                WindowPosition { x: 421, y: 42 },
            ),
            Some(WindowPosition { x: 436, y: 42 })
        );
        assert_eq!(
            snap_command_window_position(
                work_area(),
                COMMAND_WINDOW,
                WindowPosition { x: 300, y: 42 },
            ),
            None
        );
    }

    #[test]
    fn exposes_platform_shortcut_labels() {
        assert_eq!(global_shortcut_sequence(OsKind::Windows).display, "Alt+A+I");
        assert_eq!(
            global_shortcut_sequence(OsKind::Macos).display,
            "Option+A+I"
        );
        assert_eq!(global_shortcut_sequence(OsKind::Linux).display, "Alt+A+I");
    }

    #[test]
    fn keeps_linux_tray_experimental() {
        assert!(tray_or_menu_bar_available(OsKind::Windows));
        assert!(tray_or_menu_bar_available(OsKind::Macos));
        assert!(!tray_or_menu_bar_available(OsKind::Linux));
    }
}
