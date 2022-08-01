// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

#![feature(trait_alias)]

//! This implementation is abridged from mizu: https://github.com/Amjad50/mizu
//! It is under the MIT license. See the linked repository for more info.
//! Thank you to Amjad50 for mizu!

use bitflags::bitflags;
pub use channel::Channel;
use channel::{Dac, LengthCountedChannel};
use noise_channel::NoiseChannel;
use pulse_channel::PulseChannel;
use wave_channel::WaveChannel;

mod channel;
mod envelope;
mod noise_channel;
mod pulse_channel;
mod wave_channel;

bitflags! {
    #[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
    pub struct ChannelsControl: u8 {
        const VIN_LEFT  = 1 << 7;
        const VOL_LEFT  = 7 << 4;
        const VIN_RIGHT = 1 << 3;
        const VOL_RIGHT = 7;
    }
}

impl ChannelsControl {
    fn vol_left(self) -> u8 {
        (self.bits() >> 4) & 7
    }

    fn vol_right(self) -> u8 {
        self.bits() & 7
    }
}

bitflags! {
    #[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
    pub struct ChannelsSelection: u8 {
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
pub struct GenericApu {
    pub pulse1: Dac<LengthCountedChannel<PulseChannel<false>>>,
    pub pulse2: Dac<LengthCountedChannel<PulseChannel<true>>>,
    pub wave: Dac<LengthCountedChannel<WaveChannel>>,
    pub noise: Dac<LengthCountedChannel<NoiseChannel>>,

    pub channels_control: ChannelsControl,
    pub channels_selection: ChannelsSelection,

    pub power: bool,

    /// The frame sequencer position, the frame sequencer has 8 positions
    /// from 1 to 8 in this emulator (as it is incremented before use)
    /// In each position some components are clocked.
    /// Length counters are clocked in positions 1, 3, 5, 7
    /// Volume Envelops are clocked in positions 8
    /// Sweeps          are clocked in positions 3, 7
    pub sequencer_position: i8,

    pub cgb: bool,
}

impl GenericApu {
    pub fn new(cgb: bool) -> Self {
        Self {
            channels_control: ChannelsControl::from_bits_truncate(0),
            channels_selection: ChannelsSelection::from_bits_truncate(0),
            power: false,

            pulse1: Dac::new(LengthCountedChannel::new(PulseChannel::default(), 64)),
            pulse2: Dac::new(LengthCountedChannel::new(PulseChannel::default(), 64)),
            wave: Dac::new(LengthCountedChannel::new(WaveChannel::default(), 256)),
            noise: Dac::new(LengthCountedChannel::new(NoiseChannel::default(), 64)),
            sequencer_position: 0,

            cgb,
        }
    }

    #[inline]
    pub fn tick_sequencer(&mut self) {
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

impl GenericApu {
    #[inline]
    pub fn make_sample(&mut self) -> [f32; 2] {
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
        [right_sample / 8.0, left_sample / 8.0]
    }

    /// Power off, does not handle writing 0 to all disabled regs.
    pub fn power_off(&mut self) {
        self.pulse1.set_enable(false);
        self.pulse2.set_enable(false);
        self.wave.set_enable(false);
        self.noise.set_enable(false);
    }

    pub fn power_on(&mut self) {
        self.sequencer_position = 0;

        self.pulse1.channel_mut().reset_sequencer();
        self.pulse2.channel_mut().reset_sequencer();
        self.wave.channel_mut().reset_buffer_index();

        if self.cgb {
            // reset length counters in CGB
            self.pulse1.reset_length_counter();
            self.pulse2.reset_length_counter();
            self.wave.reset_length_counter();
            self.noise.reset_length_counter();
        }
    }

    /// determines if the next frame sequencer clock is going to include
    /// clocking the length counter
    pub fn is_length_clock_next(&self) -> bool {
        (self.sequencer_position + 1) % 2 != 0
    }

    /// write the top 2 bits of NRx4 registers and runs the obsecure
    /// behaviour of clocking the length counter
    pub fn write_channel_length_enable_and_trigger<C: Channel>(
        channel: &mut LengthCountedChannel<C>,
        is_length_clock_next: bool,
        data: u8,
        sched: &mut impl ScheduleFn,
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
            channel.trigger_length(!is_length_clock_next, sched);
        }
    }

    pub fn init_scheduler(sched: &mut impl ScheduleFn) {
        sched(GenApuEvent::NoiseReload, 4);
    }
}

impl Default for GenericApu {
    fn default() -> Self {
        Self::new(true)
    }
}

pub trait ScheduleFn = FnMut(GenApuEvent, i32);

#[derive(Copy, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum GenApuEvent {
    Pulse1Reload,
    Pulse2Reload,
    WaveReload,
    NoiseReload,
}

impl GenApuEvent {
    #[inline]
    pub fn dispatch(&self, apu: &mut GenericApu) -> i32 {
        if !apu.power {
            // Just wait a while.
            return 0xFF;
        }

        match self {
            Self::Pulse1Reload => apu.pulse1.channel_mut().clock(),
            Self::Pulse2Reload => apu.pulse2.channel_mut().clock(),
            Self::WaveReload => apu.wave.channel_mut().clock(),
            Self::NoiseReload => apu.noise.channel_mut().clock(),
        }
        .max(1) as i32
    }
}
