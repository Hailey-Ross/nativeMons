//! Controller and keyboard in the C# launcher's terms: the XInput button bits (Pad()) and Windows virtual keys.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseButton { Left, Right, Middle }

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Quit,
    /// A virtual key (real presses through SDL, stable-check's posted WM_KEYDOWN through the message hook).
    Key(u32),
    /// Client position, and the cursor's place on the desktop.
    MouseMove { x: i32, y: i32, gx: i32, gy: i32 },
    MouseDown { x: i32, y: i32, b: MouseButton },
    MouseUp { x: i32, y: i32, b: MouseButton },
    /// WinForms' MouseClick: released on the button pressed, with no window drag in between.
    Click { x: i32, y: i32, b: MouseButton },
    Wheel(i32),
    Focus(bool),
    /// WM_REQUEST rang: state\request.txt waits.
    Request,
    Exposed,
    /// Files or folders dropped on the window (DragDrop), together.
    Drop(Vec<String>),
}
pub const UP: u16 = 0x0001;
pub const DOWN: u16 = 0x0002;
pub const LEFT: u16 = 0x0004;
pub const RIGHT: u16 = 0x0008;
pub const A: u16 = 0x1000;
pub const B: u16 = 0x2000;
pub const X: u16 = 0x4000;
pub const Y: u16 = 0x8000;
pub const LB: u16 = 0x0100;
pub const RB: u16 = 0x0200;
pub const START: u16 = 0x0010;
pub const BACK: u16 = 0x0020;

/// One controller's state as SDL reports it (SDL's stick y grows downward, XInput's upward).
#[derive(Clone, Copy, Debug, Default)]
pub struct PadState {
    pub up: bool, pub down: bool, pub left: bool, pub right: bool,
    pub south: bool, pub east: bool, pub west: bool, pub north: bool,
    pub lb: bool, pub rb: bool, pub start: bool, pub back: bool,
    pub lx: i16, pub ly: i16,
}

/// Pad(): the buttons, with the left stick past 16000 counting as the d-pad.
pub fn pad_bits(s: &PadState) -> u16 {
    let mut b = 0u16;
    if s.up { b |= UP; } if s.down { b |= DOWN; } if s.left { b |= LEFT; } if s.right { b |= RIGHT; }
    if s.south { b |= A; } if s.east { b |= B; } if s.west { b |= X; } if s.north { b |= Y; }
    if s.lb { b |= LB; } if s.rb { b |= RB; } if s.start { b |= START; } if s.back { b |= BACK; }
    let ly = -(s.ly as i32);
    if ly > 16000 { b |= UP; } if ly < -16000 { b |= DOWN; }
    if (s.lx as i32) < -16000 { b |= LEFT; } if s.lx as i32 > 16000 { b |= RIGHT; }
    b
}

/// The first controller that reports anything, as Pad() returns the first connected slot.
pub fn first_pad(all: &[PadState]) -> u16 { all.first().map_or(0, pad_bits) }

/// SDL scancodes (USB HID usage ids) to the virtual keys ProcessCmdKey reads; 0 for keys it ignores.
pub fn scancode_to_vk(sc: u32) -> u32 {
    match sc {
        82 => 0x26, 81 => 0x28, 80 => 0x25, 79 => 0x27,           // arrows
        26 => 0x57, 4 => 0x41, 22 => 0x53, 7 => 0x44,             // W A S D
        40 | 88 => 0x0D, 41 => 0x1B, 42 => 0x08, 44 => 0x20, 43 => 0x09,
        27 => 0x58, 20 => 0x51, 8 => 0x45, 75 => 0x21, 78 => 0x22, // X Q E PageUp PageDown
        _ => 0,
    }
}

/// WinForms' wheel delta: 120 per notch, positive away from the user.
pub fn wheel_delta(sdl_y: f32) -> i32 { (sdl_y * 120.0).round() as i32 }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stick_and_buttons() {
        let mut s = PadState::default();
        assert_eq!(pad_bits(&s), 0);
        s.ly = -20000; // pushed up in SDL's terms
        assert_eq!(pad_bits(&s), UP);
        s.ly = 16000; // exactly the threshold does not count
        assert_eq!(pad_bits(&s), 0);
        s.ly = 16001;
        assert_eq!(pad_bits(&s), DOWN);
        s = PadState { lx: -32768, south: true, east: true, west: true, lb: true, rb: true, ..Default::default() };
        assert_eq!(pad_bits(&s), LEFT | A | B | X | LB | RB);
        assert_eq!(first_pad(&[]), 0);
    }
    #[test]
    fn keys() {
        assert_eq!(scancode_to_vk(82), 0x26);
        assert_eq!(scancode_to_vk(40), 0x0D);
        assert_eq!(scancode_to_vk(88), 0x0D);
        assert_eq!(scancode_to_vk(41), 0x1B);
        assert_eq!(scancode_to_vk(75), 0x21);
        assert_eq!(scancode_to_vk(30), 0);
        assert_eq!(wheel_delta(1.0), 120);
        assert_eq!(wheel_delta(-2.0), -240);
    }
}
