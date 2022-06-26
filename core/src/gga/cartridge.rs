use std::iter;

use serde::{Deserialize, Serialize};
use FlashCmdStage::*;
use SaveType::*;

use crate::{gga::memory::KB, storage::GameSave};

const SAVE_TYPES: &[(SaveType, &str)] = &[
    (
        Flash128 {
            state: FlashState::new(),
            bank: 0,
        },
        "FLASH1M_V",
    ),
    (Flash64(FlashState::new()), "FLASH_V"),
    (Flash64(FlashState::new()), "FLASH512_V"),
    (Sram, "SRAM_V"),
    (Eeprom, "EEPROM_V"),
];
// Both Macronix.
const FLASH64_ID: [u8; 2] = [0xC2, 0x1C];
const FLASH128_ID: [u8; 2] = [0xC2, 0x09];

#[derive(Default, Deserialize, Serialize)]
pub struct Cartridge {
    #[serde(skip)]
    #[serde(default)]
    pub rom: Vec<u8>,
    pub ram: Vec<u8>,
    pub save_type: SaveType,
}

impl Cartridge {
    pub fn read_ram(&self, addr: usize) -> u8 {
        match self.save_type {
            Flash64(state) if state.mode == FlashMode::Id => FLASH64_ID[addr & 1],
            Flash128 { state, .. } if state.mode == FlashMode::Id => FLASH128_ID[addr & 1],

            Flash128 { bank: 1, .. } => self.ram[addr | 0x10000],
            Nothing => 0,
            _ => self.ram[addr],
        }
    }

    pub fn write_ram(&mut self, addr: usize, value: u8) {
        match &mut self.save_type {
            Flash64(state) => state.write(addr, value, &mut self.ram, None),
            Flash128 { state, bank } => state.write(addr, value, &mut self.ram, Some(bank)),
            _ => (),
        }
    }

    pub fn load_rom(&mut self, rom: Vec<u8>) {
        self.rom = rom;
        self.save_type = self.detect_save();

        let zero_iter = iter::repeat(0);
        let len = self.ram.len();
        match self.save_type {
            Nothing => {}
            Eeprom => self.ram.extend(zero_iter.take((8 * KB) - len)),
            Sram => self.ram.extend(zero_iter.take((32 * KB) - len)),
            Flash64(_) => self.ram.extend(zero_iter.take((64 * KB) - len)),
            Flash128 { .. } => self.ram.extend(zero_iter.take((128 * KB) - len)),
        }
    }

    pub fn make_save(&self) -> Option<GameSave> {
        match self.save_type {
            Nothing => None,
            _ => Some(GameSave {
                ram: self.ram.clone(),
                rtc: None,
                title: self.title(),
            }),
        }
    }

    pub fn load_save(&mut self, save: GameSave) {
        self.ram = save.ram;
    }

    pub fn title(&self) -> String {
        self.read_string(0x0A0, 12)
    }

    pub fn game_code(&self) -> String {
        self.read_string(0x0AC, 4)
    }

    fn detect_save(&self) -> SaveType {
        // This is not efficient
        let self_str = String::from_utf8_lossy(&self.rom);
        for (ty, str) in SAVE_TYPES {
            if self_str.contains(str) {
                return *ty;
            }
        }
        Nothing
    }

    fn read_string(&self, base: usize, max: usize) -> String {
        let mut buf = String::new();
        for idx in 0..max {
            let ch = self.rom[base + idx] as char;
            if ch == '\0' {
                break;
            }
            buf.push(ch);
        }
        buf
    }
}

#[derive(Debug, Copy, Clone, Deserialize, Serialize)]
pub enum SaveType {
    Nothing,
    Eeprom,
    Sram,
    Flash64(FlashState),
    Flash128 { state: FlashState, bank: u8 },
}

impl Default for SaveType {
    fn default() -> Self {
        Nothing
    }
}

#[derive(Debug, Copy, Clone, Deserialize, Serialize)]
pub struct FlashState {
    command_stage: Option<FlashCmdStage>,
    mode: FlashMode,
}

impl FlashState {
    fn write(&mut self, addr: usize, value: u8, ram: &mut [u8], bank: Option<&mut u8>) {
        match (addr, value, self.command_stage) {
            (0x0, _, _) if self.mode == FlashMode::BankSelect => {
                self.mode = FlashMode::Regular;
                *bank.unwrap() = value & 1;
            }

            (_, _, _) if self.mode == FlashMode::Write => {
                self.mode = FlashMode::Regular;
                if bank.cloned() == Some(1) {
                    ram[addr | 0x10000] = value;
                } else {
                    ram[addr] = value;
                }
            }

            (0x5555, 0xAA, None) => self.command_stage = Some(FirstWritten),
            (0x2AAA, 0x55, Some(FirstWritten)) => self.command_stage = Some(SecondWritten),

            // Erase 4K sector
            (_, 0x30, Some(SecondWritten)) => {
                if self.mode == FlashMode::Erase {
                    let addr = if bank.cloned() == Some(1) {
                        (addr & 0xF000) | 0x10000
                    } else {
                        addr & 0xF000
                    };
                    for mem in ram.iter_mut().skip(addr).take(0x1000) {
                        *mem = 0xFF;
                    }
                }
                self.mode = FlashMode::Regular;
                self.command_stage = None;
            }

            (0x5555, _, Some(SecondWritten)) => {
                match value {
                    // Enter Erase mode
                    0x80 => self.mode = FlashMode::Erase,
                    // Erase entire chip
                    0x10 => {
                        if self.mode == FlashMode::Erase {
                            for mem in ram {
                                *mem = 0xFF;
                            }
                        }
                        self.mode = FlashMode::Regular;
                    }

                    // Enter write mode
                    0xA0 => self.mode = FlashMode::Write,
                    // Enter bank select, if banked chip
                    0xB0 if bank.is_some() => self.mode = FlashMode::BankSelect,

                    // Enter ID mode
                    0x90 => self.mode = FlashMode::Id,
                    // Exit ID mode
                    0xF0 => self.mode = FlashMode::Regular,

                    _ => (),
                }
                self.command_stage = None;
            }

            _ => (),
        }
    }

    const fn new() -> Self {
        // Why is Default not const...
        Self {
            command_stage: None,
            mode: FlashMode::Regular,
        }
    }
}

#[derive(Debug, Copy, Clone, Deserialize, Serialize)]
pub enum FlashCmdStage {
    FirstWritten,
    SecondWritten,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Deserialize, Serialize)]
pub enum FlashMode {
    Regular,
    Write,
    Id,
    Erase,
    BankSelect,
}
