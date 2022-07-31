// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use super::scheduling::AdvEvent;
use crate::{
    common::SAMPLE_RATE,
    components::{
        apu_psg::{Channel, ChannelsControl, ChannelsSelection, GenericApu, ScheduleFn},
        scheduler::Scheduler,
    },
    gga::{
        addr::{FIFO_A_L, SOUNDBIAS, SOUNDCNT_H},
        dma::Dmas,
        scheduling::ApuEvent,
        GameGirlAdv, CPU_CLOCK,
    },
    numutil::{NumExt, U16Ext},
};

pub const SAMPLE_EVERY_N_CLOCKS: i32 = (CPU_CLOCK / SAMPLE_RATE as f32) as i32;
const GG_OFFS: i32 = 4;

/// APU of the GGA, which is a GG APU in addition to 2 DMA channels.
#[derive(Default, Deserialize, Serialize)]
pub struct Apu {
    // 4 channels found on GG(C)
    pub(crate) cgb_chans: GenericApu,
    // DMA channels
    buffers: [VecDeque<i8>; 2],
    current_samples: [i8; 2],
    // Output buffer
    pub buffer: Vec<f32>,
}

impl Apu {
    /// Handle event. Since all APU events reschedule themselves, this
    /// function returns the time after which the event should repeat.
    pub fn handle_event(gg: &mut GameGirlAdv, event: ApuEvent, late_by: i32) -> i32 {
        match event {
            // We multiply the time by 4 since the generic APU expects GG t-cycles,
            // which are 1/4th of GGA CPU clock
            ApuEvent::Gen(gen) => (gen.dispatch(&mut gg.apu.cgb_chans) * GG_OFFS) - late_by,

            ApuEvent::Sequencer => {
                gg.apu.cgb_chans.tick_sequencer();
                0x8000 - late_by
            }

            ApuEvent::PushSample => {
                Self::push_output(gg);
                SAMPLE_EVERY_N_CLOCKS - late_by
            }
        }
    }

    pub fn init_scheduler(gg: &mut GameGirlAdv) {
        GenericApu::init_scheduler(&mut shed(&mut gg.scheduler));
    }

    fn push_output(gg: &mut GameGirlAdv) {
        if !gg.apu.cgb_chans.power {
            // Master enable, also applies to DMA channels
            gg.apu.buffer.push(0.);
            gg.apu.buffer.push(0.);
            return;
        }
        let mut left = 0;
        let mut right = 0;

        let cnt = gg[SOUNDCNT_H];
        let a_vol_mul = 1 + cnt.bit(2) as i16;
        let b_vol_mul = 1 + cnt.bit(3) as i16;
        let a = gg.apu.current_samples[0] as i16 * a_vol_mul * 2;
        let b = gg.apu.current_samples[1] as i16 * b_vol_mul * 2;

        if cnt.is_bit(8) {
            right += a;
        }
        if cnt.is_bit(9) {
            left += a;
        }
        if cnt.is_bit(12) {
            right += b;
        }
        if cnt.is_bit(13) {
            left += b;
        }

        let cgb_sample = gg.apu.cgb_chans.make_sample();
        let cgb_mul = match cnt & 3 {
            0 => 512.,  // 25%
            1 => 1024., // 50%
            2 => 2048., // 100%
            _ => 0.,    // 3: prohibited
        };
        right += (cgb_sample[0] * cgb_mul * 0.8) as i16;
        left += (cgb_sample[1] * cgb_mul * 0.8) as i16;

        let bias = gg[SOUNDBIAS].bits(0, 10) as i16;
        gg.apu.buffer.push(Self::bias(right, bias) as f32 / 1024.0);
        gg.apu.buffer.push(Self::bias(left, bias) as f32 / 1024.0);
    }

    fn bias(mut sample: i16, bias: i16) -> i16 {
        sample += bias;
        if sample > 0x3ff {
            sample = 0x3ff;
        } else if sample < 0 {
            sample = 0;
        }
        sample -= bias;
        sample
    }
}

// Impl block for DMA channels
impl Apu {
    /// Timer handling this channel overflowed, go to next sample and request
    /// more samples if needed
    pub fn timer_overflow<const CH: usize>(gg: &mut GameGirlAdv) {
        if let Some(next) = gg.apu.buffers[CH].pop_front() {
            gg.apu.current_samples[CH] = next;
        }

        if gg.apu.buffers[CH].len() <= 16 {
            let dest = 0x400_0000 | (FIFO_A_L + CH.u32() * 4);
            for dma in 1..=2 {
                if Dmas::get_dest(gg, dma) == dest {
                    Dmas::try_fifo_transfer(gg, dma);
                }
            }
        }
    }

    pub fn push_samples<const CH: usize>(&mut self, samples: u16) {
        self.buffers[CH].push_back(samples.low() as i8);
        self.buffers[CH].push_back(samples.high() as i8);
    }

    pub fn push_sample<const CH: usize>(&mut self, samples: u8) {
        self.buffers[CH].push_back(samples as i8);
    }
}

impl GenericApu {
    pub fn read_register_gga(&self, addr: u16) -> u8 {
        match addr {
            0x60 => self.pulse1.channel().read_sweep_register(),
            0x62 => (self.pulse1.channel().read_pattern_duty() << 6),
            0x63 => self.pulse1.channel().envelope().read_envelope_register(),
            0x65 => ((self.pulse1.read_length_enable() as u8) << 6),

            0x68 => (self.pulse2.channel().read_pattern_duty() << 6),
            0x69 => self.pulse2.channel().envelope().read_envelope_register(),
            0x6D => ((self.pulse2.read_length_enable() as u8) << 6),

            0x70 => 0xE0 | ((self.wave.dac_enabled() as u8) << 7),
            0x73 => 0x80 | ((self.wave.channel().read_volume()) << 5),
            0x75 => ((self.wave.read_length_enable() as u8) << 6),

            0x79 => self.noise.channel().envelope().read_envelope_register(),
            0x7C => self.noise.channel().read_noise_register(),
            0x7D => ((self.noise.read_length_enable() as u8) << 6),

            0x80 => 0x77 & self.channels_control.bits(),
            0x81 => self.channels_selection.bits(),
            0x84 => {
                ((self.power as u8) << 7)
                    | ((self.noise.enabled() as u8) << 3)
                    | ((self.pulse2.enabled() as u8) << 1)
                    | self.pulse1.enabled() as u8
            }

            0x90..=0x9F => self.wave.channel().read_buffer((addr & 0xF) as u8),
            _ => 0,
        }
    }

    pub fn write_register_gga(&mut self, addr: u16, data: u8, sched: &mut impl ScheduleFn) {
        // `addr % 5 != 2` will be true if its not a length counter register,
        // as these are not affected by power off, but `addr % 5 != 2` also
        // includes `0x81` and we don't want to be able to write to it
        if !self.power && addr <= 0x81 && (addr % 5 != 2 || addr == 0x81) {
            return;
        }

        let is_length_clock_next = self.is_length_clock_next();

        match addr {
            0x60 => self.pulse1.channel_mut().write_sweep_register(data),
            0x62 => {
                if self.power {
                    self.pulse1.channel_mut().write_pattern_duty(data >> 6);
                }

                self.pulse1.write_sound_length(data & 0x3F);
            }
            0x63 => {
                self.pulse1
                    .channel_mut()
                    .envelope_mut()
                    .write_envelope_register(data);

                self.pulse1.set_dac_enable(data & 0xF8 != 0);
            }
            0x64 => {
                let freq = (self.pulse1.channel().frequency() & 0xFF00) | data as u16;
                self.pulse1.channel_mut().write_frequency(freq);
            }
            0x65 => {
                let freq =
                    (self.pulse1.channel().frequency() & 0xFF) | (((data as u16) & 0x7) << 8);
                self.pulse1.channel_mut().write_frequency(freq);

                Self::write_channel_length_enable_and_trigger(
                    &mut *self.pulse1,
                    is_length_clock_next,
                    data,
                    sched,
                );
            }

            0x68 => {
                if self.power {
                    self.pulse2.channel_mut().write_pattern_duty(data >> 6);
                }

                self.pulse2.write_sound_length(data & 0x3F);
            }
            0x69 => {
                self.pulse2
                    .channel_mut()
                    .envelope_mut()
                    .write_envelope_register(data);

                self.pulse2.set_dac_enable(data & 0xF8 != 0);
            }
            0x6C => {
                let freq = (self.pulse2.channel().frequency() & 0xFF00) | data as u16;
                self.pulse2.channel_mut().write_frequency(freq);
            }
            0x6D => {
                let freq =
                    (self.pulse2.channel().frequency() & 0xFF) | (((data as u16) & 0x7) << 8);
                self.pulse2.channel_mut().write_frequency(freq);

                Self::write_channel_length_enable_and_trigger(
                    &mut *self.pulse2,
                    is_length_clock_next,
                    data,
                    sched,
                );
            }

            0x70 => {
                self.wave.set_dac_enable(data & 0x80 != 0);
            }
            0x72 => {
                self.wave.write_sound_length(data);
            }
            0x73 => self.wave.channel_mut().write_volume((data >> 5) & 3),
            0x74 => {
                let freq = (self.wave.channel().frequency() & 0xFF00) | data as u16;
                self.wave.channel_mut().write_frequency(freq);
            }
            0x75 => {
                let freq = (self.wave.channel().frequency() & 0xFF) | (((data as u16) & 0x7) << 8);
                self.wave.channel_mut().write_frequency(freq);

                Self::write_channel_length_enable_and_trigger(
                    &mut *self.wave,
                    is_length_clock_next,
                    data,
                    sched,
                );
            }

            0x78 => self.noise.write_sound_length(data & 0x3F),
            0x79 => {
                self.noise
                    .channel_mut()
                    .envelope_mut()
                    .write_envelope_register(data);

                self.noise.set_dac_enable(data & 0xF8 != 0);
            }
            0x7C => self.noise.channel_mut().write_noise_register(data),
            0x7D => {
                Self::write_channel_length_enable_and_trigger(
                    &mut *self.noise,
                    is_length_clock_next,
                    data,
                    sched,
                );
            }

            0x80 => self
                .channels_control
                .clone_from(&ChannelsControl::from_bits_truncate(data)),
            0x81 => self
                .channels_selection
                .clone_from(&ChannelsSelection::from_bits_truncate(data)),

            0x84 => {
                let new_power = data & 0x80 != 0;
                if self.power && !new_power {
                    for i in 0x60..=0x81 {
                        self.write_register_gga(i, 0, sched);
                    }
                    self.power_off();
                } else if !self.power && new_power {
                    self.power_on();
                }

                // update `self.power` after `power_off`, because we
                // need to be able to write zeros to registers normally
                self.power = new_power;
            }

            0x90..=0x9F => {
                self.wave
                    .channel_mut()
                    .write_buffer((addr & 0xF) as u8, data);
            }

            _ => (),
        }
    }
}

#[inline]
pub fn shed(sched: &mut Scheduler<AdvEvent>) -> impl ScheduleFn + '_ {
    |e, t| {
        let evt = AdvEvent::ApuEvent(ApuEvent::Gen(e));
        sched.cancel(evt);
        sched.schedule(evt, t * GG_OFFS);
    }
}
