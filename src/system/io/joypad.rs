use crate::system::cpu::Interrupt;
use crate::system::io::addr::JOYP;
use crate::system::GameGirl;
use eframe::egui::Key;
use eframe::egui::Key::J;

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
        for key in self.key_states.iter().skip(row_start).take(4).rev() {
            res <<= 1;
            res += (!key) as u8;
        }
        res | (joyp & 0x30) | 0b1100_0000
    }

    pub fn set(gg: &mut GameGirl, button: Button, state: bool) {
        gg.mmu.joypad.key_states[button as usize] = state;
        let read = gg.mmu.joypad.read(gg.mmu[JOYP]);
        if read & 0x0F != 0x0F {
            gg.request_interrupt(Interrupt::Joypad);
        }
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

    pub fn from_key(key: Key) -> Option<Self> {
        Self::KEYS
            .iter()
            .position(|k| *k == key)
            .map(|i| Self::BTNS[i])
    }

    fn to_key(&self) -> Key {
        Self::KEYS[*self as usize]
    }
}
