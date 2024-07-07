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
// It's currently effetively a function-by-function rewrite in Rust.

use core::slice;
use std::{cmp, mem, ptr};

use common::components::memory::MemoryMapper;

use crate::GameGirlAdv;

const MAX_CH: usize = 12;
const TOTAL_FRAME_COUNT: u32 = 7;
const SAMPLE_RATE: u32 = 65536;
const SAMPLES_PER_FRAME: u32 = SAMPLE_RATE / 60 + 1;
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

const fn i8_to_float(value: i8) -> f32 {
    value as f32 / 127.
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

#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
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
    pub fn pc_match(gg: &mut GameGirlAdv) {
        let addr = gg.get(0x300_7FF0);
        if addr != 0 {
            gg.apu.mplayer.sound_main_ram(&mut gg.memory.mapper, addr);
        }
    }

    pub fn sound_main_ram(&mut self, bus: &mut MemoryMapper<8192>, info_addr: u32) {
        let Some(sound_info) = bus.get::<GameGirlAdv, SoundInfo>(info_addr) else {
            return;
        };
        if sound_info.magic != 0x68736D54 {
            return;
        }
        self.sound_info = sound_info;

        if !self.engaged {
            assert_ne!(self.sound_info.pcm_samples_per_vblank, 0);
            self.recreate_buffer();
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

    fn recreate_buffer(&mut self) {
        let capacity = (SAMPLES_PER_FRAME * TOTAL_FRAME_COUNT * 4) as usize;
        self.buffer.reserve_exact(capacity - self.buffer.len());
        unsafe {
            self.buffer.set_len(capacity);
        }
        self.buffer.fill(0.0);
    }

    fn render_frame(&mut self, bus: &mut MemoryMapper<8192>) {
        const DIFFERENTIAL_LUT: &[f32] = &[
            i8_to_float(0x00u8 as i8),
            i8_to_float(0x01u8 as i8),
            i8_to_float(0x04u8 as i8),
            i8_to_float(0x09u8 as i8),
            i8_to_float(0x10u8 as i8),
            i8_to_float(0x19u8 as i8),
            i8_to_float(0x24u8 as i8),
            i8_to_float(0x31u8 as i8),
            i8_to_float(0xC0u8 as i8),
            i8_to_float(0xCFu8 as i8),
            i8_to_float(0xDCu8 as i8),
            i8_to_float(0xE7u8 as i8),
            i8_to_float(0xF0u8 as i8),
            i8_to_float(0xF7u8 as i8),
            i8_to_float(0xFCu8 as i8),
            i8_to_float(0xFFu8 as i8),
        ];

        self.current_frame = (self.current_frame + 1) % TOTAL_FRAME_COUNT;
        self.recreate_buffer();

        let reverb_strength = if self.force_reverb {
            cmp::max(self.sound_info.reverb, 48)
        } else {
            self.sound_info.reverb
        };
        let max_channels = cmp::min(self.sound_info.max_channels as usize, MAX_CH);
        let destination = (self.current_frame * SAMPLES_PER_FRAME * 2) as usize;

        if reverb_strength > 0 {
            self.render_reverb(destination, reverb_strength);
        } else {
            self.buffer[destination..(destination + (SAMPLES_PER_FRAME as usize * 8))].fill(0.);
        }

        for i in 0..max_channels {
            let channel = &mut self.sound_info.channels[i];
            let sampler = &mut self.samplers[i];
            let envelope = &mut self.envelopes[i];

            if (channel.status & CHANNEL_ON) == 0 {
                continue;
            }

            let angular_step = if (channel.type_ & 8) != 0 {
                self.sound_info.pcm_sample_rate as f32 / SAMPLE_RATE as f32
            } else {
                channel.frequency as f32 / SAMPLE_RATE as f32
            };

            let compressed = (channel.type_ & 32) != 0;
            let sample_history = &mut sampler.sample_history;
            let wave_info = &sampler.wave_info;

            let mut wave_size = wave_info.number_of_samples;
            if sampler.compressed != compressed || sampler.wave_data.is_null() {
                if compressed {
                    wave_size *= 33;
                    wave_size = (wave_size + 63) / 64;
                }
                let wave_data_begin = channel.wave_address + mem::size_of::<WaveInfo>() as u32;
                sampler.wave_data = bus.page::<GameGirlAdv, false>(wave_data_begin);
                if (sampler.wave_data as usize) < 0x10_000 {
                    log::warn!(
                        "Mplayer: Channel {i} had invalid wave address 0x{:08X}",
                        channel.wave_address
                    );
                    channel.status = 0; // Disable channel, there is no good way to deal with this.
                    continue;
                }
                sampler.compressed = compressed;
            }

            dbg!(sampler.wave_data, wave_size);
            let wave_data =
                unsafe { slice::from_raw_parts_mut(sampler.wave_data, wave_size as usize) };
            for j in 0..SAMPLES_PER_FRAME {
                let t = j as f32 / SAMPLES_PER_FRAME as f32;

                let volume_l = envelope.volume_l[0] * (1. - t) + envelope.volume_l[1] * t;
                let volume_r = envelope.volume_r[0] * (1. - t) + envelope.volume_r[1] * t;

                if sampler.should_fetch_sample {
                    let mut sample;

                    if compressed {
                        let block_offset = sampler.current_position & 63;
                        let block_address = (sampler.current_position >> 6) * 33;

                        if block_offset == 0 {
                            sample = i8_to_float(wave_data[block_address as usize] as i8);
                        } else {
                            sample = sample_history[0];
                        }

                        let address = block_address + (block_offset >> 1) + 1;
                        let mut lut_index = wave_data[address as usize];

                        if (block_offset & 1) != 0 {
                            lut_index &= 15;
                        } else {
                            lut_index >>= 4;
                        }

                        sample += DIFFERENTIAL_LUT[lut_index as usize];
                    } else {
                        sample = i8_to_float(wave_data[sampler.current_position as usize] as i8);
                    }

                    if self.use_cubic_filter {
                        sample_history[3] = sample_history[2];
                        sample_history[2] = sample_history[1];
                    }
                    sample_history[1] = sample_history[0];
                    sample_history[0] = sample;

                    sampler.should_fetch_sample = false;
                }

                let sample;
                let mu = sampler.resample_phase;

                if self.use_cubic_filter {
                    // http://paulbourke.net/miscellaneous/interpolation/
                    let mu2 = mu * mu;
                    let a0 = sample_history[0] - sample_history[1] - sample_history[3]
                        + sample_history[2];
                    let a1 = sample_history[3] - sample_history[2] - a0;
                    let a2 = sample_history[1] - sample_history[3];
                    let a3 = sample_history[2];
                    sample = a0 * mu * mu2 + a1 * mu2 + a2 * mu + a3;
                } else {
                    sample = sample_history[0] * mu + sample_history[1] * (1.0 - mu);
                }

                self.buffer[destination + (j * 2 + 0) as usize] += sample * volume_r;
                self.buffer[destination + (j * 2 + 1) as usize] += sample * volume_l;

                sampler.resample_phase += angular_step;

                if sampler.resample_phase >= 1.0 {
                    let n = sampler.resample_phase as u32;
                    sampler.resample_phase -= n as f32;
                    sampler.current_position += n;
                    sampler.should_fetch_sample = true;

                    if sampler.current_position >= wave_info.number_of_samples {
                        if (channel.status & CHANNEL_LOOP) != 0 {
                            sampler.current_position = wave_info.loop_position + n - 1;
                        } else {
                            sampler.current_position = wave_info.number_of_samples;
                            sampler.should_fetch_sample = false;
                        }
                    }
                }
            }
        }
    }

    fn render_reverb(&mut self, destination: usize, strength: u8) {
        const EARLY_COEFFICIENT: f32 = 0.0015;
        const LATE_COEFFICIENTS: [[f32; 2]; 3] = [[1.0, 0.1], [0.6, 0.25], [0.35, 0.35]];
        const NORMALIZE_COEFFICIENTS: f32 = {
            let mut sum = 0.0;
            sum += LATE_COEFFICIENTS[0][0];
            sum += LATE_COEFFICIENTS[0][1];
            sum += LATE_COEFFICIENTS[1][0];
            sum += LATE_COEFFICIENTS[1][1];
            sum += LATE_COEFFICIENTS[2][0];
            sum += LATE_COEFFICIENTS[2][1];
            1.0 / sum
        };

        let early_buffer = (((self.current_frame + TOTAL_FRAME_COUNT - 1) % TOTAL_FRAME_COUNT)
            * SAMPLES_PER_FRAME
            * 2) as usize;
        let late_buffers = [
            (((self.current_frame + 2) % TOTAL_FRAME_COUNT) * SAMPLES_PER_FRAME * 2) as usize,
            (((self.current_frame + 1) % TOTAL_FRAME_COUNT) * SAMPLES_PER_FRAME * 2) as usize,
            destination,
        ];

        let factor = strength as f32 / 128.0;
        for l in (0..SAMPLES_PER_FRAME * 2).step_by(2) {
            let r = l + 1;
            let early_reflection_l = self.buffer[early_buffer + l as usize] * EARLY_COEFFICIENT;
            let early_reflection_r = self.buffer[early_buffer + r as usize] * EARLY_COEFFICIENT;
            let mut late_reflection_l = 0.0;
            let mut late_reflection_r = 0.0;

            for j in 0..3 {
                let sample_l = self.buffer[late_buffers[j] + l as usize];
                let sample_r = self.buffer[late_buffers[j] + r as usize];
                late_reflection_l +=
                    sample_l * LATE_COEFFICIENTS[j][0] + sample_r * LATE_COEFFICIENTS[j][1];
                late_reflection_r +=
                    sample_l * LATE_COEFFICIENTS[j][1] + sample_r * LATE_COEFFICIENTS[j][0];
            }

            late_reflection_l *= NORMALIZE_COEFFICIENTS;
            late_reflection_r *= NORMALIZE_COEFFICIENTS;
            self.buffer[destination + l as usize] =
                (early_reflection_l + late_reflection_l) * factor;
            self.buffer[destination + r as usize] =
                (early_reflection_r + late_reflection_r) * factor;
        }
    }

    pub fn read_sample(&mut self, bus: &mut MemoryMapper<8192>) -> [f32; 2] {
        if self.buffer_read_index == 0 {
            self.render_frame(bus);
        }

        let sample = &self.buffer[((self.current_frame * SAMPLES_PER_FRAME
            + self.buffer_read_index as u32)
            * 2) as usize..];
        self.buffer_read_index += 1;
        if self.buffer_read_index == SAMPLES_PER_FRAME as usize {
            self.buffer_read_index = 0;
        }

        return [sample[0], sample[1]];
    }
}

#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
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
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
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

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
struct Sampler {
    compressed: bool,
    should_fetch_sample: bool,
    current_position: u32,
    resample_phase: f32,
    sample_history: [f32; 4],
    wave_info: WaveInfo,
    #[cfg_attr(feature = "serde", serde(skip))]
    #[cfg_attr(feature = "serde", serde(default = "null"))]
    wave_data: *mut u8,
}

unsafe impl Send for Sampler {}
unsafe impl Sync for Sampler {}

fn null() -> *mut u8 {
    ptr::null::<u8>() as *mut u8
}

impl Default for Sampler {
    fn default() -> Self {
        Self {
            compressed: Default::default(),
            should_fetch_sample: true,
            current_position: Default::default(),
            resample_phase: Default::default(),
            sample_history: Default::default(),
            wave_info: Default::default(),
            wave_data: ptr::null::<u8>() as *mut u8,
        }
    }
}

#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
struct WaveInfo {
    type_: u16,
    status: u16,
    frequency: u32,
    loop_position: u32,
    number_of_samples: u32,
}

#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
struct Envelope {
    volume: f32,
    volume_l: [f32; 2],
    volume_r: [f32; 2],
}
