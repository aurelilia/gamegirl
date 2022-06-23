use crate::{
    ggc::io::{
        addr::{DIV, KEY1},
        apu::{
            apu::{ChannelsControl, ChannelsSelection},
            channel::ApuChannel,
        },
        Mmu,
    },
    numutil::NumExt,
};
use serde::{Deserialize, Serialize};

pub use apu::GenericApu;

mod apu;
mod channel;
mod envelope;
mod noise_channel;
mod pulse_channel;
mod wave_channel;

/// APU variant used by DMG/CGB.
#[derive(Deserialize, Serialize)]
pub struct GGApu {
    pub(super) inner: GenericApu,

    /// Counts time until next sample.
    sample_counter: f64,
    /// Stores the value of the 4th bit (5th in double speed mode) of the
    /// divider as sequencer clocks are controlled by the divider
    pub divider_sequencer_clock_bit: bool,

    pub buffer: Vec<f32>,
}

impl GGApu {
    pub fn step(mmu: &mut Mmu, cycles: u16) {
        let ds = mmu.cgb && mmu[KEY1].is_bit(7);
        let div = mmu.timer.read(DIV);
        mmu.apu.clock(cycles, ds, div);
    }

    /// The APU is clocked by the divider, on the falling edge of the bit 12
    /// of the divider, this is needed since the divider can be clocked manually
    /// by resetting it to 0 on write
    fn clock(&mut self, cycles: u16, double_speed: bool, divider: u8) {
        // 2 in normal speed, 1 in double speed
        let clocks = (!double_speed) as u16 + 1;
        let div_bit = 4 + double_speed as u8;

        let adj_clocks = clocks * cycles;

        const SAMPLE_RATE: f64 = 44100.;
        const SAMPLE_EVERY_N_CLOCKS: f64 = (((16384 * 256) / 2) as f64) / SAMPLE_RATE;
        self.sample_counter += adj_clocks as f64;
        if self.sample_counter >= SAMPLE_EVERY_N_CLOCKS {
            let sample = self.inner.make_sample();
            self.buffer.push(sample[0]);
            self.buffer.push(sample[1]);
            self.sample_counter -= SAMPLE_EVERY_N_CLOCKS;
        }

        self.inner.clock(adj_clocks);
        if self.inner.power {
            let old_div_sequencer_bit = self.divider_sequencer_clock_bit;
            self.divider_sequencer_clock_bit = (divider >> div_bit) & 1 == 1;
            if old_div_sequencer_bit && !self.divider_sequencer_clock_bit {
                self.inner.tick_sequencer();
            }
        }
    }

    pub fn write(&mut self, addr: u16, value: u8) {
        self.inner
            .write_register_gg(addr, value, self.divider_sequencer_clock_bit);
    }

    pub fn new(cgb: bool) -> Self {
        Self {
            inner: GenericApu::new(cgb),
            sample_counter: 0.,
            divider_sequencer_clock_bit: false,
            buffer: Vec::new(),
        }
    }
}

impl GenericApu {
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

    pub fn write_register_gg(&mut self, addr: u16, data: u8, clock_bit: bool) {
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
                    for i in 0xFF10..=0xFF25 {
                        self.write_register_gg(i, 0, clock_bit);
                    }
                    self.power_off();
                } else if !self.power && new_power {
                    self.power_on(clock_bit);
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
}
