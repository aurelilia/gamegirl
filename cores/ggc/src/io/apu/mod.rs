// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

//! This audio implementation is slightly abridged from mizu:
//! https://github.com/Amjad50/mizu/tree/master/mizu-core/src/apu
//! Thank you to it's authors!

mod channel;
mod envelope;
mod noise_channel;
mod pulse_channel;
mod wave_channel;

use bitflags::bitflags;
use channel::{Channel, Dac, LengthCountedChannel};
use noise_channel::NoiseChannel;
use pulse_channel::PulseChannel;
use wave_channel::WaveChannel;

bitflags! {
    #[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
    #[derive(Copy, Clone)]
    struct ChannelsControl: u8 {
        const VIN_LEFT  = 1 << 7;
        const VOL_LEFT  = 7 << 4;
        const VIN_RIGHT = 1 << 3;
        const VOL_RIGHT = 7;
    }
}

impl ChannelsControl {
    fn vol_left(&self) -> u8 {
        (self.bits() >> 4) & 7
    }

    fn vol_right(&self) -> u8 {
        self.bits() & 7
    }
}

bitflags! {
    #[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
    #[derive(Copy, Clone)]
    struct ChannelsSelection: u8 {
        const NOISE_LEFT   = 1 << 7;
        const WAVE_LEFT    = 1 << 6;
        const PULSE2_LEFT  = 1 << 5;
        const PULSE1_LEFT  = 1 << 4;
        const NOISE_RIGHT  = 1 << 3;
        const WAVE_RIGHT   = 1 << 2;
        const PULSE2_RIGHT = 1 << 1;
        const PULSE1_RIGHT = 1 << 0;
    }
}

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Apu {
    pulse1: Dac<LengthCountedChannel<PulseChannel>>,
    pulse2: Dac<LengthCountedChannel<PulseChannel>>,
    wave: Dac<LengthCountedChannel<WaveChannel>>,
    noise: Dac<LengthCountedChannel<NoiseChannel>>,

    channels_control: ChannelsControl,
    channels_selection: ChannelsSelection,

    power: bool,

    sample_counter: f64,

    /// Stores the value of the 4th bit (5th in double speed mode) of the
    /// divider as sequencer clocks are controlled by the divider
    divider_sequencer_clock_bit: bool,

    /// The frame sequencer position, the frame sequencer has 8 positions
    /// from 1 to 8 in this emulator (as it is incremented before use)
    /// In each position some components are clocked.
    /// Length counters are clocked in positions 1, 3, 5, 7
    /// Volume Envelops are clocked in positions 8
    /// Sweeps          are clocked in positions 3, 7
    sequencer_position: i8,

    // Keep track when to clock the APU, it should be clocked every 4 tcycles
    // this is to keep working normally even in CPU double speed mode
    clocks_counter: u8,

    is_dmg: bool,
}

impl Apu {
    pub fn new(is_dmg: bool) -> Self {
        Self {
            channels_control: ChannelsControl::from_bits_truncate(0),
            channels_selection: ChannelsSelection::from_bits_truncate(0),
            power: false,

            sample_counter: 0.,
            pulse1: Dac::new(LengthCountedChannel::new(PulseChannel::default(), 64)),
            pulse2: Dac::new(LengthCountedChannel::new(PulseChannel::default(), 64)),
            wave: Dac::new(LengthCountedChannel::new(WaveChannel::new(is_dmg), 256)),
            noise: Dac::new(LengthCountedChannel::new(NoiseChannel::default(), 64)),
            divider_sequencer_clock_bit: false,
            sequencer_position: 0,
            clocks_counter: 0,

            is_dmg,
        }
    }

    pub fn new_skip_boot_rom(is_dmg: bool) -> Self {
        let mut apu = Self::new(is_dmg);

        // after boot_rom state
        apu.pulse1.channel_mut().write_pattern_duty(2);
        apu.pulse1
            .channel_mut()
            .envelope_mut()
            .write_envelope_register(0xF3);
        apu.noise.write_sound_length(0x3F);
        apu.channels_control = ChannelsControl::from_bits_truncate(0x77);
        apu.channels_selection = ChannelsSelection::from_bits_truncate(0xF3);
        apu.pulse1.set_enable(true);
        apu.wave.set_dac_enable(false);
        apu.power = true;

        apu
    }

    pub fn read_register_gg(&self, addr: u16) -> u8 {
        match addr {
            0xFF10 => 0x80 | self.pulse1.channel().read_sweep_register(),
            0xFF11 => 0x3F | (self.pulse1.channel().read_pattern_duty() << 6),
            0xFF12 => self.pulse1.channel().envelope().read_envelope_register(),
            0xFF13 => 0xFF,
            0xFF14 => 0xBF | ((self.pulse1.read_length_enable() as u8) << 6),

            0xFF15 => 0xFF,
            0xFF16 => 0x3F | (self.pulse2.channel().read_pattern_duty() << 6),
            0xFF17 => self.pulse2.channel().envelope().read_envelope_register(),
            0xFF18 => 0xFF,
            0xFF19 => 0xBF | ((self.pulse2.read_length_enable() as u8) << 6),

            0xFF1A => 0x7F | ((self.wave.dac_enabled() as u8) << 7),
            0xFF1B => 0xFF,
            0xFF1C => 0x9F | ((self.wave.channel().read_volume()) << 5),
            0xFF1D => 0xFF,
            0xFF1E => 0xBF | ((self.wave.read_length_enable() as u8) << 6),

            0xFF1F => 0xFF,
            0xFF20 => 0xFF,
            0xFF21 => self.noise.channel().envelope().read_envelope_register(),
            0xFF22 => self.noise.channel().read_noise_register(),
            0xFF23 => 0xBF | ((self.noise.read_length_enable() as u8) << 6),

            0xFF24 => self.channels_control.bits(),
            0xFF25 => self.channels_selection.bits(),
            0xFF26 => {
                0x70 | ((self.power as u8) << 7)
                    | ((self.noise.enabled() as u8) << 3)
                    | ((self.wave.enabled() as u8) << 2)
                    | ((self.pulse2.enabled() as u8) << 1)
                    | self.pulse1.enabled() as u8
            }

            0xFF27..=0xFF2F => 0xFF,

            0xFF30..=0xFF3F => self.wave.channel().read_buffer((addr & 0xF) as u8),
            _ => unreachable!(),
        }
    }

    pub fn write_register_gg(&mut self, addr: u16, data: u8) {
        // `addr % 5 != 2` will be true if its not a length counter register,
        // as these are not affected by power off, but `addr % 5 != 2` also
        // includes `0xFF25` and we don't want to be able to write to it
        if !self.power && addr <= 0xFF25 && (addr % 5 != 2 || addr == 0xFF25) {
            return;
        }

        let is_length_clock_next = self.is_length_clock_next();

        match addr {
            0xFF10 => self.pulse1.channel_mut().write_sweep_register(data),
            0xFF11 => {
                if self.power {
                    self.pulse1.channel_mut().write_pattern_duty(data >> 6);
                }

                self.pulse1.write_sound_length(data & 0x3F);
            }
            0xFF12 => {
                self.pulse1
                    .channel_mut()
                    .envelope_mut()
                    .write_envelope_register(data);

                self.pulse1.set_dac_enable(data & 0xF8 != 0);
            }
            0xFF13 => {
                let freq = (self.pulse1.channel().frequency() & 0xFF00) | data as u16;
                self.pulse1.channel_mut().write_frequency(freq);
            }
            0xFF14 => {
                let freq =
                    (self.pulse1.channel().frequency() & 0xFF) | (((data as u16) & 0x7) << 8);
                self.pulse1.channel_mut().write_frequency(freq);

                Self::write_channel_length_enable_and_trigger(
                    &mut *self.pulse1,
                    is_length_clock_next,
                    data,
                );
            }

            0xFF15 => {}
            0xFF16 => {
                if self.power {
                    self.pulse2.channel_mut().write_pattern_duty(data >> 6);
                }

                self.pulse2.write_sound_length(data & 0x3F);
            }
            0xFF17 => {
                self.pulse2
                    .channel_mut()
                    .envelope_mut()
                    .write_envelope_register(data);

                self.pulse2.set_dac_enable(data & 0xF8 != 0);
            }
            0xFF18 => {
                let freq = (self.pulse2.channel().frequency() & 0xFF00) | data as u16;
                self.pulse2.channel_mut().write_frequency(freq);
            }
            0xFF19 => {
                let freq =
                    (self.pulse2.channel().frequency() & 0xFF) | (((data as u16) & 0x7) << 8);
                self.pulse2.channel_mut().write_frequency(freq);

                Self::write_channel_length_enable_and_trigger(
                    &mut *self.pulse2,
                    is_length_clock_next,
                    data,
                );
            }

            0xFF1A => {
                self.wave.set_dac_enable(data & 0x80 != 0);
            }
            0xFF1B => {
                self.wave.write_sound_length(data);
            }
            0xFF1C => self.wave.channel_mut().write_volume((data >> 5) & 3),
            0xFF1D => {
                let freq = (self.wave.channel().frequency() & 0xFF00) | data as u16;
                self.wave.channel_mut().write_frequency(freq);
            }
            0xFF1E => {
                let freq = (self.wave.channel().frequency() & 0xFF) | (((data as u16) & 0x7) << 8);
                self.wave.channel_mut().write_frequency(freq);

                Self::write_channel_length_enable_and_trigger(
                    &mut *self.wave,
                    is_length_clock_next,
                    data,
                );
            }

            0xFF1F => {}
            0xFF20 => self.noise.write_sound_length(data & 0x3F),
            0xFF21 => {
                self.noise
                    .channel_mut()
                    .envelope_mut()
                    .write_envelope_register(data);

                self.noise.set_dac_enable(data & 0xF8 != 0);
            }
            0xFF22 => self.noise.channel_mut().write_noise_register(data),
            0xFF23 => {
                Self::write_channel_length_enable_and_trigger(
                    &mut *self.noise,
                    is_length_clock_next,
                    data,
                );
            }

            0xFF24 => self
                .channels_control
                .clone_from(&ChannelsControl::from_bits_truncate(data)),
            0xFF25 => self
                .channels_selection
                .clone_from(&ChannelsSelection::from_bits_truncate(data)),

            0xFF26 => {
                let new_power = data & 0x80 != 0;
                if self.power && !new_power {
                    self.power_off();
                } else if !self.power && new_power {
                    self.power_on();
                }

                // update `self.power` after `power_off`, because we
                // need to be able to write zeros to registers normally
                self.power = new_power;
            }

            0xFF27..=0xFF2F => {
                // unused
            }

            0xFF30..=0xFF3F => {
                self.wave
                    .channel_mut()
                    .write_buffer((addr & 0xF) as u8, data);
            }
            _ => unreachable!(),
        }
    }

    pub fn read_pcm12(&self) -> u8 {
        let p1 = self.pulse1.output() & 0xF;
        let p2 = self.pulse2.output() & 0xF;

        (p2 << 4) | p1
    }

    pub fn read_pcm34(&self) -> u8 {
        let p1 = self.wave.output() & 0xF;
        let p2 = self.noise.output() & 0xF;

        (p2 << 4) | p1
    }

    /// The APU is clocked by the divider, on the falling edge of the bit 12
    /// of the divider, this is needed since the divider can be clocked manually
    /// by resetting it to 0 on write
    pub fn clock(&mut self, double_speed: bool, divider: u8, buf: &mut [Vec<f32>; 2]) {
        // 2 in normal speed, 1 in double speed
        let clocks = (!double_speed) as u8 + 1;

        self.clocks_counter += clocks;
        if self.clocks_counter >= 2 {
            self.clocks_counter -= 2;
        } else {
            // don't do anything, wait for the next cycle
            return;
        }

        const SAMPLE_RATE: f64 = 48000.;
        const SAMPLE_EVERY_N_CLOCKS: f64 = (((16384 * 256) / 4) as f64) / SAMPLE_RATE;

        self.sample_counter += 1.;
        if self.sample_counter >= SAMPLE_EVERY_N_CLOCKS {
            self.push_output(buf);
            self.sample_counter -= SAMPLE_EVERY_N_CLOCKS;
        }

        if !self.power {
            return;
        }

        self.pulse1.channel_mut().clock();
        self.pulse2.channel_mut().clock();
        self.wave.channel_mut().clock();
        self.noise.channel_mut().clock();

        let old_div_sequencer_bit = self.divider_sequencer_clock_bit;
        let bit = if double_speed { 5 } else { 4 };
        let new_div_sequencer_bit = (divider >> bit) & 1 == 1;

        self.divider_sequencer_clock_bit = new_div_sequencer_bit;

        if old_div_sequencer_bit && !new_div_sequencer_bit {
            self.sequencer_position += 1;

            match self.sequencer_position {
                1 | 5 => {
                    self.pulse1.clock_length_counter();
                    self.pulse2.clock_length_counter();
                    self.wave.clock_length_counter();
                    self.noise.clock_length_counter();
                }
                3 | 7 => {
                    self.pulse1.channel_mut().clock_sweeper();
                    self.pulse1.clock_length_counter();
                    self.pulse2.clock_length_counter();
                    self.wave.clock_length_counter();
                    self.noise.clock_length_counter();
                }
                8 => {
                    self.pulse1.channel_mut().envelope_mut().clock();
                    self.pulse2.channel_mut().envelope_mut().clock();
                    self.noise.channel_mut().envelope_mut().clock();
                    self.sequencer_position = 0;
                }
                0 | 2 | 4 | 6 => {}
                _ => unreachable!(),
            }
        }
    }
}

impl Apu {
    fn push_output(&mut self, buf: &mut [Vec<f32>; 2]) {
        let right_vol = self.channels_control.vol_right() as f32 + 1.;
        let left_vol = self.channels_control.vol_left() as f32 + 1.;

        let pulse1 = self.pulse1.dac_output() / 8.;
        let pulse2 = self.pulse2.dac_output() / 8.;
        let wave = self.wave.dac_output() / 8.;
        let noise = self.noise.dac_output() / 8.;

        let right_pulse1 = if self
            .channels_selection
            .contains(ChannelsSelection::PULSE1_RIGHT)
        {
            pulse1 * right_vol
        } else {
            0.
        };

        let right_pulse2 = if self
            .channels_selection
            .contains(ChannelsSelection::PULSE2_RIGHT)
        {
            pulse2 * right_vol
        } else {
            0.
        };

        let right_wave = if self
            .channels_selection
            .contains(ChannelsSelection::WAVE_RIGHT)
        {
            wave * right_vol
        } else {
            0.
        };

        let right_noise = if self
            .channels_selection
            .contains(ChannelsSelection::NOISE_RIGHT)
        {
            noise * right_vol
        } else {
            0.
        };

        let left_pulse1 = if self
            .channels_selection
            .contains(ChannelsSelection::PULSE1_LEFT)
        {
            pulse1 * left_vol
        } else {
            0.
        };

        let left_pulse2 = if self
            .channels_selection
            .contains(ChannelsSelection::PULSE2_LEFT)
        {
            pulse2 * left_vol
        } else {
            0.
        };

        let left_wave = if self
            .channels_selection
            .contains(ChannelsSelection::WAVE_LEFT)
        {
            wave * left_vol
        } else {
            0.
        };

        let left_noise = if self
            .channels_selection
            .contains(ChannelsSelection::NOISE_LEFT)
        {
            noise * left_vol
        } else {
            0.
        };

        // one sample for the right, one for the left

        let right_sample = right_pulse1 + right_pulse2 + right_wave + right_noise;
        let left_sample = left_pulse1 + left_pulse2 + left_wave + left_noise;
        buf[0].push(right_sample);
        buf[1].push(left_sample);
    }

    fn power_off(&mut self) {
        for i in 0xFF10..=0xFF25 {
            self.write_register_gg(i, 0);
        }

        self.pulse1.set_enable(false);
        self.pulse2.set_enable(false);
        self.wave.set_enable(false);
        self.noise.set_enable(false);
    }

    fn power_on(&mut self) {
        self.sequencer_position = 0;

        // Special case where if the APU is turned on and bit 4 (5 in double speed)
        // of the divider is set, the APU will delay the next clock, so it will take
        // 2 clocks to reach the first event in the sequencer instead of 1
        //
        // See: SameSuite test apu/div_write_trigger_10, Note: In the test it describes
        // that the APU `skips` the first event and not delay it which is wrong
        if self.divider_sequencer_clock_bit {
            self.sequencer_position = -1;
        }

        self.pulse1.channel_mut().reset_sequencer();
        self.pulse2.channel_mut().reset_sequencer();
        self.wave.channel_mut().reset_buffer_index();

        if !self.is_dmg {
            // reset length counters in CGB
            self.pulse1.reset_length_counter();
            self.pulse2.reset_length_counter();
            self.wave.reset_length_counter();
            self.noise.reset_length_counter();
        }
    }

    /// determines if the next frame sequencer clock is going to include
    /// clocking the length counter
    fn is_length_clock_next(&self) -> bool {
        (self.sequencer_position + 1) % 2 != 0
    }

    /// write the top 2 bits of NRx4 registers and runs the obsecure
    /// behaviour of clocking the length counter
    fn write_channel_length_enable_and_trigger<C: Channel>(
        channel: &mut LengthCountedChannel<C>,
        is_length_clock_next: bool,
        data: u8,
    ) {
        let old_length_enable = channel.read_length_enable();
        let new_length_enable = (data >> 6) & 1 == 1;
        channel.write_length_enable(new_length_enable);

        // obsecure behaviour: if the length decrement is enabled now (was not),
        // and the sequencer will not clock length, then clock it now
        if !is_length_clock_next && !old_length_enable && new_length_enable {
            channel.clock_length_counter();
        }

        if data & 0x80 != 0 {
            // trigger length, which would trigger the channel inside
            channel.trigger_length(!is_length_clock_next);
        }
    }
}
