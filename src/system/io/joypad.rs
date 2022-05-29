use eframe::egui::Key;

#[derive(Default)]
pub struct Joypad {
    key_states: [bool; 8],
}

impl Joypad {
    pub fn read(&self, joyp: u8) -> u8 {
        let row_start = match joyp & 0x30 {
            0x10 => 0,
            0x20 => 4,
            _ => return 0xCF,
        };
        let mut res = 0;
        for key in self.key_states.iter().rev().skip(row_start).take(4) {
            res <<= 1;
            if !key {
                res += 1;
            }
        }
        res | (joyp & 0x30) | 0b1100_0000
    }
}

#[derive(Copy, Clone)]
pub enum Button {
    A,
    B,
    Select,
    Start,
    Right,
    Left,
    Up,
    Down,
}

impl Button {
    const BTNS: [Self; 8] = [
        Self::A,
        Self::B,
        Self::Select,
        Self::Start,
        Self::Right,
        Self::Left,
        Self::Up,
        Self::Down,
    ];
    const KEYS: [Key; 8] = [
        Key::X,
        Key::Z,
        Key::Space,
        Key::Enter,
        Key::ArrowRight,
        Key::ArrowLeft,
        Key::ArrowUp,
        Key::ArrowDown,
    ];

    fn to_key(&self) -> Key {
        Self::KEYS[*self as usize]
    }
}
