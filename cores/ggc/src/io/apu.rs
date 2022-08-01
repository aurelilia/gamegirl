// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use common::{
    components::{
        apu_psg::{Channel, ChannelsControl, ChannelsSelection, GenericApu, ScheduleFn},
        scheduler::Scheduler,
    },
    SAMPLE_RATE,
};
use serde::{Deserialize, Serialize};

use crate::{
    io::scheduling::{ApuEvent, GGEvent},
    GameGirl, T_CLOCK_HZ,
};

pub const SAMPLE_EVERY_N_CLOCKS: i32 = (T_CLOCK_HZ / SAMPLE_RATE) as i32;

/// APU variant used by DMG/CGB.
#[derive(Deserialize, Serialize)]
pub struct Apu {
    pub(super) inner: GenericApu,
    pub buffer: Vec<f32>,
}

impl Apu {
    pub fn handle_event(gg: &mut GameGirl, event: ApuEvent, late_by: i32) {
        match event {
            ApuEvent::PushSample => {
                let sample = gg.apu.inner.make_sample();
                gg.apu.buffer.push(sample[0]);
                gg.apu.buffer.push(sample[1]);
                gg.scheduler
                    .schedule(GGEvent::ApuEvent(event), SAMPLE_EVERY_N_CLOCKS - late_by);
            }

            ApuEvent::TickSequencer => {
                gg.apu.inner.tick_sequencer();
                gg.scheduler
                    .schedule(GGEvent::ApuEvent(event), 0x2000 - late_by);
            }

            ApuEvent::Gen(gen) => {
                let next = gen.dispatch(&mut gg.apu.inner);
                gg.scheduler
                    .schedule(GGEvent::ApuEvent(event), next - late_by);
            }
        }
    }

    pub fn write(gg: &mut GameGirl, addr: u16, value: u8) {
        Self::write_register(&mut gg.apu.inner, addr, value, &mut shed(&mut gg.scheduler));
    }

    pub fn init_scheduler(gg: &mut GameGirl) {
        GenericApu::init_scheduler(&mut shed(&mut gg.scheduler));
        gg.scheduler.schedule(
            GGEvent::ApuEvent(ApuEvent::PushSample),
            SAMPLE_EVERY_N_CLOCKS,
        );
        gg.scheduler
            .schedule(GGEvent::ApuEvent(ApuEvent::TickSequencer), 0x2000);
    }

    pub fn new(cgb: bool) -> Self {
        Self {
            inner: GenericApu::new(cgb),
            buffer: Vec::with_capacity(5000),
        }
    }
}

impl Apu {
    pub fn read_register(apu: &GenericApu, addr: u16) -> u8 {
        match addr {
            0xFF10 => 0x80 | apu.pulse1.channel().read_sweep_register(),
            0xFF11 => 0x3F | (apu.pulse1.channel().read_pattern_duty() << 6),
            0xFF12 => apu.pulse1.channel().envelope().read_envelope_register(),
            0xFF14 => 0xBF | ((apu.pulse1.read_length_enable() as u8) << 6),

            0xFF16 => 0x3F | (apu.pulse2.channel().read_pattern_duty() << 6),
            0xFF17 => apu.pulse2.channel().envelope().read_envelope_register(),
            0xFF19 => 0xBF | ((apu.pulse2.read_length_enable() as u8) << 6),

            0xFF1A => 0x7F | ((apu.wave.dac_enabled() as u8) << 7),
            0xFF1C => 0x9F | ((apu.wave.channel().read_volume()) << 5),
            0xFF1E => 0xBF | ((apu.wave.read_length_enable() as u8) << 6),

            0xFF21 => apu.noise.channel().envelope().read_envelope_register(),
            0xFF22 => apu.noise.channel().read_noise_register(),
            0xFF23 => 0xBF | ((apu.noise.read_length_enable() as u8) << 6),

            0xFF24 => apu.channels_control.bits(),
            0xFF25 => apu.channels_selection.bits(),
            0xFF26 => {
                0x70 | ((apu.power as u8) << 7)
                    | ((apu.noise.enabled() as u8) << 3)
                    | ((apu.wave.enabled() as u8) << 2)
                    | ((apu.pulse2.enabled() as u8) << 1)
                    | apu.pulse1.enabled() as u8
            }

            0xFF30..=0xFF3F => apu.wave.channel().read_buffer((addr & 0xF) as u8),
            _ => 0xFF,
        }
    }

    pub fn write_register(apu: &mut GenericApu, addr: u16, data: u8, sched: &mut impl ScheduleFn) {
        // `addr % 5 != 2` will be true if its not a length counter register,
        // as these are not affected by power off, but `addr % 5 != 2` also
        // includes `0xFF25` and we don't want to be able to write to it
        if !apu.power && addr <= 0xFF25 && (addr % 5 != 2 || addr == 0xFF25) {
            return;
        }

        let is_length_clock_next = apu.is_length_clock_next();

        match addr {
            0xFF10 => apu.pulse1.channel_mut().write_sweep_register(data),
            0xFF11 => {
                if apu.power {
                    apu.pulse1.channel_mut().write_pattern_duty(data >> 6);
                }

                apu.pulse1.write_sound_length(data & 0x3F);
            }
            0xFF12 => {
                apu.pulse1
                    .channel_mut()
                    .envelope_mut()
                    .write_envelope_register(data);

                apu.pulse1.set_dac_enable(data & 0xF8 != 0);
            }
            0xFF13 => {
                let freq = (apu.pulse1.channel().frequency() & 0xFF00) | data as u16;
                apu.pulse1.channel_mut().write_frequency(freq);
            }
            0xFF14 => {
                let freq = (apu.pulse1.channel().frequency() & 0xFF) | (((data as u16) & 0x7) << 8);
                apu.pulse1.channel_mut().write_frequency(freq);

                GenericApu::write_channel_length_enable_and_trigger(
                    &mut *apu.pulse1,
                    is_length_clock_next,
                    data,
                    sched,
                );
            }

            0xFF16 => {
                if apu.power {
                    apu.pulse2.channel_mut().write_pattern_duty(data >> 6);
                }

                apu.pulse2.write_sound_length(data & 0x3F);
            }
            0xFF17 => {
                apu.pulse2
                    .channel_mut()
                    .envelope_mut()
                    .write_envelope_register(data);

                apu.pulse2.set_dac_enable(data & 0xF8 != 0);
            }
            0xFF18 => {
                let freq = (apu.pulse2.channel().frequency() & 0xFF00) | data as u16;
                apu.pulse2.channel_mut().write_frequency(freq);
            }
            0xFF19 => {
                let freq = (apu.pulse2.channel().frequency() & 0xFF) | (((data as u16) & 0x7) << 8);
                apu.pulse2.channel_mut().write_frequency(freq);

                GenericApu::write_channel_length_enable_and_trigger(
                    &mut *apu.pulse2,
                    is_length_clock_next,
                    data,
                    sched,
                );
            }

            0xFF1A => {
                apu.wave.set_dac_enable(data & 0x80 != 0);
            }
            0xFF1B => {
                apu.wave.write_sound_length(data);
            }
            0xFF1C => apu.wave.channel_mut().write_volume((data >> 5) & 3),
            0xFF1D => {
                let freq = (apu.wave.channel().frequency() & 0xFF00) | data as u16;
                apu.wave.channel_mut().write_frequency(freq);
            }
            0xFF1E => {
                let freq = (apu.wave.channel().frequency() & 0xFF) | (((data as u16) & 0x7) << 8);
                apu.wave.channel_mut().write_frequency(freq);

                GenericApu::write_channel_length_enable_and_trigger(
                    &mut *apu.wave,
                    is_length_clock_next,
                    data,
                    sched,
                );
            }

            0xFF20 => apu.noise.write_sound_length(data & 0x3F),
            0xFF21 => {
                apu.noise
                    .channel_mut()
                    .envelope_mut()
                    .write_envelope_register(data);

                apu.noise.set_dac_enable(data & 0xF8 != 0);
            }
            0xFF22 => apu.noise.channel_mut().write_noise_register(data),
            0xFF23 => {
                GenericApu::write_channel_length_enable_and_trigger(
                    &mut *apu.noise,
                    is_length_clock_next,
                    data,
                    sched,
                );
            }

            0xFF24 => apu
                .channels_control
                .clone_from(&ChannelsControl::from_bits_truncate(data)),
            0xFF25 => apu
                .channels_selection
                .clone_from(&ChannelsSelection::from_bits_truncate(data)),

            0xFF26 => {
                let new_power = data & 0x80 != 0;
                if apu.power && !new_power {
                    for i in 0xFF10..=0xFF25 {
                        Apu::write_register(apu, i, 0, sched);
                    }
                    apu.power_off();
                } else if !apu.power && new_power {
                    apu.power_on();
                }

                // update `apu.power` after `power_off`, because we
                // need to be able to write zeros to registers normally
                apu.power = new_power;
            }

            0xFF30..=0xFF3F => {
                apu.wave
                    .channel_mut()
                    .write_buffer((addr & 0xF) as u8, data);
            }
            _ => (),
        }
    }

    pub fn read_pcm12(apu: &GenericApu) -> u8 {
        let p1 = apu.pulse1.output() & 0xF;
        let p2 = apu.pulse2.output() & 0xF;

        (p2 << 4) | p1
    }

    pub fn read_pcm34(apu: &GenericApu) -> u8 {
        let p1 = apu.wave.output() & 0xF;
        let p2 = apu.noise.output() & 0xF;

        (p2 << 4) | p1
    }
}

fn shed(sched: &mut Scheduler<GGEvent>) -> impl ScheduleFn + '_ {
    |e, t| {
        let evt = GGEvent::ApuEvent(ApuEvent::Gen(e));
        sched.cancel(evt);
        sched.schedule(evt, t);
    }
}
