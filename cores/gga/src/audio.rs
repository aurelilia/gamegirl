// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

//! TODO: Resampling is not actually being used properly.
//! for the time being: broken!

use std::collections::VecDeque;

use common::{
    components::scheduler::Scheduler,
    numutil::{NumExt, U16Ext},
    TimeS,
};
use modular_bitfield::{bitfield, specifiers::*};
use psg_apu::{Channel, ChannelsControl, ChannelsSelection, GenericApu, ScheduleFn};

use super::scheduling::AdvEvent;
use crate::{addr::FIFO_A_L, dma::Dmas, scheduling::ApuEvent, GameGirlAdv, CPU_CLOCK};

const SAMPLE_EVERY_N_CLOCKS: TimeS = CPU_CLOCK as TimeS / 2i64.pow(16);
const GG_OFFS: TimeS = 4;

#[bitfield]
#[repr(u16)]
#[derive(Default, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct SoundControl {
    cgb_vol: B2,
    a_vol: B1,
    b_vol: B1,
    #[skip]
    __: B4,
    a_right_en: bool,
    a_left_en: bool,
    pub a_timer: B1,
    #[skip]
    a_reset_fifo: bool,
    b_right_en: bool,
    b_left_en: bool,
    pub b_timer: B1,
    #[skip]
    b_reset_fifo: bool,
}

#[bitfield]
#[repr(u16)]
#[derive(Default, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct SoundBias {
    bias: B10,
    #[skip]
    __: B4,
    #[skip]
    amplitude: B2,
}

/// APU of the GGA, which is a GG APU in addition to 2 DMA channels.
#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Apu {
    // Registers
    pub cnt: SoundControl,
    pub bias: SoundBias,

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
    pub fn handle_event(gg: &mut GameGirlAdv, event: ApuEvent, late_by: TimeS) -> TimeS {
        match event {
            // We multiply the time by 4 since the generic APU expects GG t-cycles,
            // which are 1/4th of GGA CPU clock
            ApuEvent::Gen(gen) => (gen.dispatch(&mut gg.apu.cgb_chans) * GG_OFFS) - late_by,

            ApuEvent::Sequencer => {
                gg.apu.cgb_chans.tick_sequencer();
                0x8000 - late_by
            }

            ApuEvent::PushSample => {
                gg.apu.push_output();
                SAMPLE_EVERY_N_CLOCKS - late_by
            }
        }
    }

    pub fn init_scheduler(gg: &mut GameGirlAdv) {
        GenericApu::init_scheduler(&mut shed(&mut gg.scheduler));
        gg.scheduler.schedule(
            AdvEvent::ApuEvent(ApuEvent::PushSample),
            SAMPLE_EVERY_N_CLOCKS,
        );
    }

    fn push_output(&mut self) {
        if !self.cgb_chans.power {
            // Master enable, also applies to DMA channels
            self.buffer.push(0.);
            self.buffer.push(0.);
            return;
        }
        let mut left = 0;
        let mut right = 0;

        let cnt = self.cnt;
        let a_vol_mul = 1 + cnt.a_vol() as i16;
        let b_vol_mul = 1 + cnt.b_vol() as i16;
        let a = self.current_samples[0] as i16 * a_vol_mul * 2;
        let b = self.current_samples[1] as i16 * b_vol_mul * 2;

        if cnt.a_right_en() {
            right += a;
        }
        if cnt.a_left_en() {
            left += a;
        }
        if cnt.b_right_en() {
            right += b;
        }
        if cnt.b_left_en() {
            left += b;
        }

        let cgb_sample = self.cgb_chans.make_sample();
        let cgb_mul = match cnt.cgb_vol() {
            0 => 512.,  // 25%
            1 => 1024., // 50%
            2 => 2048., // 100%
            _ => 0.,    // 3: prohibited
        };
        right += (cgb_sample[0] * cgb_mul * 0.8) as i16;
        left += (cgb_sample[1] * cgb_mul * 0.8) as i16;

        let bias = self.bias.bias() as i16;
        self.buffer.push(Self::bias(right, bias) as f32 / 1024.0);
        self.buffer.push(Self::bias(left, bias) as f32 / 1024.0);
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
                if gg.dma.channels[dma].dad == dest {
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

impl Apu {
    pub fn read_register_psg(apu: &GenericApu, addr: u16) -> u8 {
        match addr {
            0x60 => apu.pulse1.channel().read_sweep_register(),
            0x62 => apu.pulse1.channel().read_pattern_duty() << 6,
            0x63 => apu.pulse1.channel().envelope().read_envelope_register(),
            0x65 => (apu.pulse1.read_length_enable() as u8) << 6,

            0x68 => apu.pulse2.channel().read_pattern_duty() << 6,
            0x69 => apu.pulse2.channel().envelope().read_envelope_register(),
            0x6D => (apu.pulse2.read_length_enable() as u8) << 6,

            0x70 => 0xE0 | ((apu.wave.dac_enabled() as u8) << 7),
            0x73 => 0x80 | ((apu.wave.channel().read_volume()) << 5),
            0x75 => (apu.wave.read_length_enable() as u8) << 6,

            0x79 => apu.noise.channel().envelope().read_envelope_register(),
            0x7C => apu.noise.channel().read_noise_register(),
            0x7D => (apu.noise.read_length_enable() as u8) << 6,

            0x80 => 0x77 & apu.channels_control.bits(),
            0x81 => apu.channels_selection.bits(),
            0x84 => {
                ((apu.power as u8) << 7)
                    | ((apu.noise.enabled() as u8) << 3)
                    | ((apu.pulse2.enabled() as u8) << 1)
                    | apu.pulse1.enabled() as u8
            }

            0x90..=0x9F => apu.wave.channel().read_buffer((addr & 0xF) as u8),
            _ => 0,
        }
    }

    pub fn write_register_psg(
        apu: &mut GenericApu,
        addr: u16,
        data: u8,
        sched: &mut impl ScheduleFn,
    ) {
        // `addr % 5 != 2` will be true if its not a length counter register,
        // as these are not affected by power off, but `addr % 5 != 2` also
        // includes `0x81` and we don't want to be able to write to it
        if !apu.power && addr <= 0x81 && (addr % 5 != 2 || addr == 0x81) {
            return;
        }

        let is_length_clock_next = apu.is_length_clock_next();

        match addr {
            0x60 => apu.pulse1.channel_mut().write_sweep_register(data),
            0x62 => {
                if apu.power {
                    apu.pulse1.channel_mut().write_pattern_duty(data >> 6);
                }

                apu.pulse1.write_sound_length(data & 0x3F);
            }
            0x63 => {
                apu.pulse1
                    .channel_mut()
                    .envelope_mut()
                    .write_envelope_register(data);

                apu.pulse1.set_dac_enable(data & 0xF8 != 0);
            }
            0x64 => {
                let freq = (apu.pulse1.channel().frequency() & 0xFF00) | data as u16;
                apu.pulse1.channel_mut().write_frequency(freq);
            }
            0x65 => {
                let freq = (apu.pulse1.channel().frequency() & 0xFF) | (((data as u16) & 0x7) << 8);
                apu.pulse1.channel_mut().write_frequency(freq);

                GenericApu::write_channel_length_enable_and_trigger(
                    &mut *apu.pulse1,
                    is_length_clock_next,
                    data,
                    sched,
                );
            }

            0x68 => {
                if apu.power {
                    apu.pulse2.channel_mut().write_pattern_duty(data >> 6);
                }

                apu.pulse2.write_sound_length(data & 0x3F);
            }
            0x69 => {
                apu.pulse2
                    .channel_mut()
                    .envelope_mut()
                    .write_envelope_register(data);

                apu.pulse2.set_dac_enable(data & 0xF8 != 0);
            }
            0x6C => {
                let freq = (apu.pulse2.channel().frequency() & 0xFF00) | data as u16;
                apu.pulse2.channel_mut().write_frequency(freq);
            }
            0x6D => {
                let freq = (apu.pulse2.channel().frequency() & 0xFF) | (((data as u16) & 0x7) << 8);
                apu.pulse2.channel_mut().write_frequency(freq);

                GenericApu::write_channel_length_enable_and_trigger(
                    &mut *apu.pulse2,
                    is_length_clock_next,
                    data,
                    sched,
                );
            }

            0x70 => {
                apu.wave.set_dac_enable(data & 0x80 != 0);
            }
            0x72 => {
                apu.wave.write_sound_length(data);
            }
            0x73 => apu.wave.channel_mut().write_volume((data >> 5) & 3),
            0x74 => {
                let freq = (apu.wave.channel().frequency() & 0xFF00) | data as u16;
                apu.wave.channel_mut().write_frequency(freq);
            }
            0x75 => {
                let freq = (apu.wave.channel().frequency() & 0xFF) | (((data as u16) & 0x7) << 8);
                apu.wave.channel_mut().write_frequency(freq);

                GenericApu::write_channel_length_enable_and_trigger(
                    &mut *apu.wave,
                    is_length_clock_next,
                    data,
                    sched,
                );
            }

            0x78 => apu.noise.write_sound_length(data & 0x3F),
            0x79 => {
                apu.noise
                    .channel_mut()
                    .envelope_mut()
                    .write_envelope_register(data);

                apu.noise.set_dac_enable(data & 0xF8 != 0);
            }
            0x7C => apu.noise.channel_mut().write_noise_register(data),
            0x7D => {
                GenericApu::write_channel_length_enable_and_trigger(
                    &mut *apu.noise,
                    is_length_clock_next,
                    data,
                    sched,
                );
            }

            0x80 => apu
                .channels_control
                .clone_from(&ChannelsControl::from_bits_truncate(data)),
            0x81 => apu
                .channels_selection
                .clone_from(&ChannelsSelection::from_bits_truncate(data)),

            0x84 => {
                let new_power = data & 0x80 != 0;
                if apu.power && !new_power {
                    for i in 0x60..=0x81 {
                        Apu::write_register_psg(apu, i, 0, sched);
                    }
                    apu.power_off();
                } else if !apu.power && new_power {
                    apu.power_on();
                }

                // update `apu.power` after `power_off`, because we
                // need to be able to write zeros to registers normally
                apu.power = new_power;
            }

            0x90..=0x9F => {
                apu.wave
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
