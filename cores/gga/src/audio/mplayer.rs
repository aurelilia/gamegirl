// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// This file is released and thus subject to the terms of the
// GNU General Public License Version 3 (GPL-3).
// If a copy was not distributed with this file, you can
// obtain one at http://www.gnu.org/licenses/.

// This HLE implementation of the MusicPlayer2000 is heavily based on the work
// of fleroviux and her emulator NBA:
// https://github.com/nba-emu/NanoBoyAdvance/blob/master/src/nba/src/hw/apu/hle
// Thank you!

use std::ptr;

use common::components::memory::MemoryMapper;

use crate::GameGirlAdv;

const MAX_CH: usize = 12;
const CHANNEL_START: u8 = 0x80;
const CHANNEL_STOP: u8 = 0x40;
const CHANNEL_LOOP: u8 = 0x10;
const CHANNEL_ECHO: u8 = 0x04;
const CHANNEL_ENV_MASK: u8 = 0x03;
const CHANNEL_ENV_ATTACK: u8 = 0x03;
const CHANNEL_ENV_DECAY: u8 = 0x02;
const CHANNEL_ENV_SUSTAIN: u8 = 0x01;
const CHANNEL_ENV_RELEASE: u8 = 0x00;
const CHANNEL_ON: u8 = CHANNEL_START | CHANNEL_STOP | CHANNEL_ECHO | CHANNEL_ENV_MASK;

fn u8_to_float(value: u8) -> f32 {
    value as f32 / 256.
}

pub fn find_mp2k(rom: &[u8]) -> Option<u32> {
    fn crc32(dat: &[u8]) -> u32 {
        let mut crc = u32::MAX;

        for byte in dat {
            let mut byte = *byte;
            for _ in 0..8 {
                if ((crc ^ byte as u32) & 1) != 0 {
                    crc = (crc >> 1) ^ 0xEDB88320;
                } else {
                    crc >>= 1;
                }

                byte >>= 1;
            }
        }

        return !crc;
    }

    const CRC32: u32 = 0x27EA7FCF;
    const LEN: usize = 48;

    if rom.len() < 48 {
        return None;
    }

    for addr in (0..(rom.len() - LEN)).step_by(2) {
        let crc = crc32(&rom[addr..(addr + LEN)]);
        if CRC32 == crc {
            println!("SoundMain at 0x{addr:X}");
            let mut addr =
                u32::from_le_bytes(rom[(addr + 0x74)..(addr + 0x78)].try_into().unwrap());
            if (addr & 1) == 0 {
                addr &= !1;
                addr += 4;
            } else {
                addr &= !3;
                addr += 8;
            }
            println!("SoundMainRAM at 0x{addr:X}");
            return Some(addr);
        }
    }

    None
}

pub struct MusicPlayer {
    samplers: [Sampler; MAX_CH],
    envelopes: [Envelope; MAX_CH],
    sound_info: SoundInfo,

    engaged: bool,
    use_cubic_filter: bool,
    force_reverb: bool,
    buffer: Vec<f32>,
    current_frame: u32,
    buffer_read_index: usize,
}

impl MusicPlayer {
    pub fn sound_main_ram(&mut self, bus: MemoryMapper<8192>, info_addr: u32) {
        let Some(sound_info) = bus.get::<GameGirlAdv, SoundInfo>(info_addr) else {
            return;
        };
        if sound_info.magic != 0x68736D54 {
            return;
        }
        self.sound_info = sound_info;

        if !self.engaged {
            assert_ne!(self.sound_info.pcm_samples_per_vblank, 0);
            self.buffer.clear();
            self.engaged = true;
        }

        let max_channels = self.sound_info.max_channels.min(MAX_CH as u8);

        for i in 0..(max_channels as usize) {
            let channel = &mut self.sound_info.channels[i];
            if (channel.status & CHANNEL_ON) == 0 {
                continue;
            }

            let sampler = &mut self.samplers[i];
            let mut envelope_volume = channel.envelope_volume as u32;
            let envelope_phase = channel.status & CHANNEL_ENV_MASK;

            let mut hq_envelope_volume = [0.0f32; 2];
            hq_envelope_volume[0] = self.envelopes[i].volume;

            if (channel.status & CHANNEL_START) != 0 {
                if (channel.status & CHANNEL_STOP) != 0 {
                    channel.status = 0;
                    continue;
                }

                envelope_volume = channel.envelope_attack as u32;
                if envelope_volume == 0xFF {
                    channel.status = CHANNEL_ENV_DECAY;
                } else {
                    channel.status = CHANNEL_ENV_ATTACK;
                }
                hq_envelope_volume[0] = u8_to_float(channel.envelope_attack);

                let Some(wave_info) = bus.get::<GameGirlAdv, WaveInfo>(channel.wave_address) else {
                    log::warn!(
                        "Mplayer: Channel {i} had invalid wave address 0x{:08X}",
                        channel.wave_address
                    );
                    channel.status = 0;
                    continue;
                };

                *sampler = Sampler::default();
                sampler.wave_info = wave_info;
                if (sampler.wave_info.status & 0xC000) != 0 {
                    channel.status |= CHANNEL_LOOP;
                }
            } else if (channel.status & CHANNEL_ECHO) != 0 {
                let len = channel.echo_length;
                channel.echo_length -= 1;
                if len == 0 {
                    channel.status = 0;
                    continue;
                }
            } else if (channel.status & CHANNEL_STOP) != 0 {
                envelope_volume = (envelope_volume * channel.envelope_release as u32) >> 8;
                hq_envelope_volume[0] *= u8_to_float(channel.envelope_release);

                if envelope_volume <= channel.echo_volume as u32 {
                    if channel.echo_volume == 0 {
                        channel.status = 0;
                        continue;
                    }

                    channel.status |= CHANNEL_ECHO;
                    envelope_volume = channel.echo_volume as u32;
                    hq_envelope_volume[0] = u8_to_float(channel.echo_volume);
                }
            } else if envelope_phase == CHANNEL_ENV_ATTACK {
                envelope_volume += channel.envelope_attack as u32;
                hq_envelope_volume[0] =
                    1.0f32.min(hq_envelope_volume[0] + u8_to_float(channel.envelope_attack));

                if envelope_volume > 0xFE {
                    channel.status = (channel.status & !CHANNEL_ENV_MASK) | CHANNEL_ENV_DECAY;
                    envelope_volume = 0xFF;
                }
            } else if envelope_phase == CHANNEL_ENV_DECAY {
                envelope_volume = (envelope_volume * channel.envelope_decay as u32) >> 8;
                hq_envelope_volume[0] *= u8_to_float(channel.envelope_decay);

                let envelope_sustain = channel.envelope_sustain;
                if envelope_volume <= envelope_sustain as u32 {
                    if envelope_sustain == 0 && channel.echo_volume == 0 {
                        channel.status = 0;
                        continue;
                    }

                    channel.status = (channel.status & !CHANNEL_ENV_MASK) | CHANNEL_ENV_SUSTAIN;
                    envelope_volume = envelope_sustain as u32;
                    hq_envelope_volume[0] = u8_to_float(envelope_sustain);
                }
            }

            channel.envelope_volume = envelope_volume as u8;
            envelope_volume = (envelope_volume * (self.sound_info.master_volume as u32 + 1)) >> 4;
            channel.envelope_volume_r = ((envelope_volume * channel.volume_r as u32) >> 8) as u8;
            channel.envelope_volume_l = ((envelope_volume * channel.volume_l as u32) >> 8) as u8;

            // Try to predict the envelope's value at the start of the next audio frame,
            // so that we can linearly interpolate the envelope between the current and next
            // frame.
            if (channel.status & CHANNEL_STOP) != 0 {
                if ((envelope_volume * channel.envelope_release as u32) >> 8)
                    <= channel.echo_volume as u32
                {
                    hq_envelope_volume[1] = u8_to_float(channel.echo_volume);
                } else {
                    hq_envelope_volume[1] =
                        hq_envelope_volume[0] * u8_to_float(channel.envelope_release);
                }
            } else if (channel.status & CHANNEL_ENV_MASK) == CHANNEL_ENV_ATTACK {
                hq_envelope_volume[1] =
                    1.0f32.min(hq_envelope_volume[0] + u8_to_float(channel.envelope_attack));
            } else if (channel.status & CHANNEL_ENV_MASK) == CHANNEL_ENV_DECAY {
                if ((envelope_volume * channel.envelope_decay as u32) >> 8)
                    <= channel.envelope_sustain as u32
                {
                    hq_envelope_volume[1] = u8_to_float(channel.envelope_sustain);
                } else {
                    hq_envelope_volume[1] =
                        hq_envelope_volume[0] * u8_to_float(channel.envelope_decay);
                }
            } else {
                hq_envelope_volume[1] = hq_envelope_volume[0];
            }

            let hq_master_volume = (self.sound_info.master_volume + 1) as f32 / 16.0;
            let hq_volume_r = hq_master_volume * u8_to_float(channel.volume_r);
            let hq_volume_l = hq_master_volume * u8_to_float(channel.volume_l);

            self.envelopes[i].volume = hq_envelope_volume[0];

            for j in 0..2 {
                self.envelopes[i].volume_r[j] = hq_envelope_volume[j] * hq_volume_r;
                self.envelopes[i].volume_l[j] = hq_envelope_volume[j] * hq_volume_l;
            }
        }
    }
}

#[repr(C)]
struct SoundInfo {
    magic: u32,
    pcm_dma_counter: u8,
    reverb: u8,
    max_channels: u8,
    master_volume: u8,
    _unknown1: [u8; 8],
    pcm_samples_per_vblank: i32,
    pcm_sample_rate: i32,
    _unknown2: [u32; 14],
    channels: [SoundChannel; MAX_CH],
}

#[repr(C)]
#[derive(Default)]
struct SoundChannel {
    status: u8,
    type_: u8,
    volume_r: u8,
    volume_l: u8,
    envelope_attack: u8,
    envelope_decay: u8,
    envelope_sustain: u8,
    envelope_release: u8,
    unknown0: u8,
    envelope_volume: u8,
    envelope_volume_r: u8,
    envelope_volume_l: u8,
    echo_volume: u8,
    echo_length: u8,
    _unknown1: [u8; 18],
    frequency: u32,
    wave_address: u32,
    _unknown2: [u32; 6],
}

struct Sampler {
    compressed: bool,
    should_fetch_sample: bool,
    current_pos: u32,
    resample_phase: f32,
    sample_history: [f32; 4],
    wave_info: WaveInfo,
    wave_data: *const u8,
}

impl Default for Sampler {
    fn default() -> Self {
        Self {
            compressed: Default::default(),
            should_fetch_sample: true,
            current_pos: Default::default(),
            resample_phase: Default::default(),
            sample_history: Default::default(),
            wave_info: Default::default(),
            wave_data: ptr::null(),
        }
    }
}

#[derive(Default)]
struct WaveInfo {
    type_: u16,
    status: u16,
    frequency: u32,
    loop_position: u32,
    number_of_samples: u32,
}

#[derive(Default)]
struct Envelope {
    volume: f32,
    volume_l: [f32; 2],
    volume_r: [f32; 2],
}
