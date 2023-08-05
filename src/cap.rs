//! The raw noise source using a capacitor on a GPIO pin.
//!
//! Most users should use [`caprand::setup`](crate::setup) and [`caprand::getrandom`](crate::getrandom) instead.
//! This module is accessible for health testing and analysis.
use cortex_m::peripheral::SYST;
#[cfg(not(feature = "defmt"))]
#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

#[cfg(feature = "defmt")]
#[allow(unused_imports)]
use defmt::{debug, error, info, trace};

use core::arch::asm;

use embassy_rp::gpio::Pin;
use rp_pac as pac;

/// Drives a pin low for an exact number of cycles.
///
/// Will be called with the pin output disabled.
/// Call with interrupts disabled if it's important.
fn exact_low(pin: &impl Pin, low_cycles: u32) {
    let pin_num = pin.pin() as usize;
    let mask = 1u32 << pin_num;
    // set pin value low. not out enabled yet
    let so = pac::SIO.gpio_out(0);
    so.value_clr().write_value(1 << pin_num);

    // get output-enable set/clear registers
    let soe = pac::SIO.gpio_oe(0);
    let soe_set = soe.value_set().as_ptr();
    let soe_clr = soe.value_clr().as_ptr();

    // We set output enable, wait a number of cycles, then clear output enable.
    // Modulo 3 because the subs/bne loops take 3 cycles.
    match (low_cycles, low_cycles % 3) {
        (0, _) => {
            // no drive low
        }
        (1, _) => unsafe {
            asm!(
            // one cycle for set
            "str {mask}, [{soe_set}]",
            "str {mask}, [{soe_clr}]",

            mask = in(reg) mask,
            soe_set = in(reg) soe_set,
            soe_clr = in(reg) soe_clr,
            options(nostack, readonly),
            );
        },
        (2, _) => unsafe {
            asm!(
            // one cycle for set
            "str {mask}, [{soe_set}]",
            // one cycle
            "nop",
            "str {mask}, [{soe_clr}]",

            mask = in(reg) mask,
            soe_set = in(reg) soe_set,
            soe_clr = in(reg) soe_clr,
            options(nostack, readonly),
            );
        },
        (_, 0) => unsafe {
            // min low time of 3
            asm!(
            // one cycle for set
            "str {mask}, [{soe_set}]",
            "000:",
            // one cycle
            "subs {d}, 3",
            // two cycles bne taken
            // one cycle for bne not taken
            "bne 000b",
            "str {mask}, [{soe_clr}]",

            mask = in(reg) mask,
            soe_set = in(reg) soe_set,
            soe_clr = in(reg) soe_clr,
            d = in(reg) low_cycles,
            options(nostack, readonly),
            );
        },
        (_, 1) => unsafe {
            // min low time of 4
            asm!(
            // one cycle for set
            "str {mask}, [{soe_set}]",
            // one extra cycle
            "subs {d}, 1",
            "000:",
            // one cycle
            "subs {d}, 3",
            // two cycles bne taken
            // one cycle for bne not taken
            "bne 000b",
            "str {mask}, [{soe_clr}]",

            mask = in(reg) mask,
            soe_set = in(reg) soe_set,
            soe_clr = in(reg) soe_clr,
            d = in(reg) low_cycles,
            options(nostack, readonly),
            );
        },
        (_, 2) => unsafe {
            // min low time of 5
            asm!(
            // one cycle for set
            "str {mask}, [{soe_set}]",
            // two extra cycles
            "subs {d}, 1",
            "subs {d}, 1",
            "000:",
            // one cycle
            "subs {d}, 3",
            // two cycles bne taken
            // one cycle for bne not taken
            "bne 000b",
            "str {mask}, [{soe_clr}]",

            mask = in(reg) mask,
            soe_set = in(reg) soe_set,
            soe_clr = in(reg) soe_clr,
            d = in(reg) low_cycles,
            options(nostack, readonly),
            );
        },
        // `match` doesn't understand modulo
        (_, 3..) => unreachable!(),
    }
}

#[allow(dead_code)]
fn time_rise_noasm(pin: &mut impl Pin, _low_cycles: u32) -> u8 {
    let pin_num = pin.pin() as usize;
    let mask = 1u32 << pin_num;

    let pad = pac::PADS_BANK0.gpio(pin_num);
    // bank 0 single cycle IO in
    let gpio_in = pac::SIO.gpio_in(0);
    let gpio_out = pac::SIO.gpio_out(0);
    let gpio_oe = pac::SIO.gpio_oe(0);

    // enable pullup
    pad.modify(|s| s.set_pue(true));

    // low for a short period of time
    gpio_out.value_clr().write_value(mask);
    gpio_oe.value_set().write_value(mask);
    gpio_oe.value_clr().write_value(mask);

    // count for it to reach logic high
    let mut c = 0u8;
    while gpio_in.read() & mask == 0 {
        c = c.wrapping_add(1);
    }
    // disable pullup after finished
    pad.modify(|s| s.set_pue(false));
    c
}


/// Drives a pin low then times how long it takes to rise to logic high.
///
/// `low_cycles` is the amount of time to hold the pin low to discharge
/// the capacitor. As the pullup charges the capacitor, it samples on every
/// clock cycles in bursts of 6 bits. When the final bit of a burst is high,
/// it returns that 5-bit burst as the value (ignoring last bit which is always
/// high)
fn time_rise(pin: &mut impl Pin, low_cycles: u32) -> u8 {
    let pin_num = pin.pin() as usize;
    let mask = 1u32 << pin_num;

    let pad = pac::PADS_BANK0.gpio(pin_num);
    // bank 0 single cycle IO in
    let gpio_in = pac::SIO.gpio_in(0).as_ptr();

    // enable pullup
    pad.modify(|s| s.set_pue(true));

    // Drive low for a number of cycles
    exact_low(pin, low_cycles);

    let x0: u32;
    let x1: u32;
    let x2: u32;
    let x3: u32;
    let x4: u32;
    // Time how long it takes for the pullup to reach high signal level
    unsafe {
        asm!(
            // save (rust asm doesn't handle frame pointer r7)
            "mov r10, r7",
            "nop",
            "nop",
            "nop",

            "222:",
            // read gpio_in register, 6 cycles
            "ldr {x0}, [{gpio_in}]",
            "ldr {x1}, [{gpio_in}]",
            "ldr {x2}, [{gpio_in}]",
            "ldr {x3}, [{gpio_in}]",
            "ldr {x4}, [{gpio_in}]",
            "ldr r7,   [{gpio_in}]",
            // only test the most recent sample. 1 cycle
            "ands r7, {mask}",
            // Loop if bit set, 2 cycles
            "beq 222b",

            // restore
            "mov r7, r10",

            mask = in(reg) mask,
            gpio_in = in(reg) gpio_in,
            x0 = out(reg) x0,
            x1 = out(reg) x1,
            x2 = out(reg) x2,
            x3 = out(reg) x3,
            x4 = out(reg) x4,
            out("r10") _,
            options(nostack, readonly),
        );
    }

    // A loop takes 9 cycles, so we would expect the distribution of
    // first-bit-set to be:
    // 0 4/9
    // 1 1/9
    // 2 1/9
    // 3 1/9
    // 4 1/9
    // 5 1/9

    // Combine all measurements in a constant-time way
    let result = (x0 & mask)
        | (x1 & mask).rotate_left(1)
        | (x2 & mask).rotate_left(2)
        | (x3 & mask).rotate_left(3)
        | (x4 & mask).rotate_left(4)
        // r7 register is always set on exit from the loop. 
        | mask.rotate_left(5);

    let result = result.rotate_right(pin_num as u32) as u8;

    // Disable pullup until next run. Preserved capacitor charge
    // carried over to the next iteration helps improve noise.
    pad.modify(|s| s.set_pue(false));

    result
}

/// Returns the least significant bit set, or 8 if 0.
///
/// Is neither constant time nor efficient, for display purposes only.
pub fn lsb(v: u8) -> u8 {
    for i in 0..u8::BITS {
        if v & 1 << i != 0 {
            return i as u8;
        }
    }
    8
}

/// Wraps timing with SYST. The clock source must already be configured.
struct SyTi<'t> {
    syst: &'t mut SYST,
    t1: u32,
}

impl<'t> SyTi<'t> {
    /// panics if `syst` is not using the core clock.
    fn new(syst: &'t mut SYST) -> Self {
        assert!(syst.get_clock_source() == cortex_m::peripheral::syst::SystClkSource::Core);
        syst.clear_current();
        syst.enable_counter();
        Self {
            syst,
            t1: SYST::get_reload(),
        }
    }

    /// returns the duration, or failure on overflow
    fn done(self) -> Result<u32, ()> {
        let t2 = SYST::get_current();
        if self.syst.has_wrapped() {
            error!("SYST wrapped");
            return Err(());
        }
        self.syst.disable_counter();
        Ok(self.t1 - t2)
    }
}

/// A noise source iterator using a capacitor on a GPIO pin.
///
/// For each output sample it drives  pin low, then times
/// how long it takes to rise to logic high. `low_cycles` is the amount of
/// time to hold the pin low to discharge the capacitor.
/// As the pullup charges the capacitor, it samples on every
/// clock cycle in bursts of 6 cycles. When the final bit of a burst is high,
/// it outputs that 6-bit burst as the value.
///
/// Samples are correlated and biased, so must be processed before
/// further use, using a cryptographic extractor or similar scheme
/// (see [`CapRng`](crate::rng::CapRng))
///
/// It is advisable to collect output from RawNoise into a buffer and discard
/// the first couple of samples - time will vary due to XIP cache loads from flash,
/// as well as charge time varying for the capacitor's first cycle.
pub struct RawNoise<'a, P: Pin> {
    pin: &'a mut P,
    low_cycles: u32,
    _setup: PinSetup,
}

impl<'a, P: Pin> RawNoise<'a, P> {
    pub fn new(pin: &'a mut P, low_cycles: u32) -> Self {
        let setup = PinSetup::new(pin.pin());
        Self {
            pin,
            low_cycles,
            _setup: setup,
        }
    }

    /// Returns the next sample as a total cycle count.
    ///
    /// This cycle count is only relative for comparison between samples.
    pub fn next_with_systick(&mut self, syst: &mut SYST) -> Result<u32, ()> {
        critical_section::with(|_cs| {
            let t = SyTi::new(syst);
            let r = time_rise(self.pin, self.low_cycles);
            let t = t.done()?;
            let t = t + lsb(r) as u32;
            Ok(t)
        })
    }
}

impl<P: Pin> Iterator for RawNoise<'_, P> {
    /// (`value, valid)`. `valid` is false for samples that are the first
    /// of a sequence, to simplify health checks.
    type Item = (u8, bool);

    fn next(&mut self) -> Option<Self::Item> {
        let r = critical_section::with(|_cs| {
            time_rise(self.pin, self.low_cycles)
        });
        let valid = (r & 1) == 0;
        Some((r, valid))
    }
}

/// Configures a GPIO pin and cleans up afterwards
///
/// This is equivalent to setup performed by embassy-rp HAL, but
/// works with a borrowed PAC pin that can be re-used later by the application.
struct PinSetup {
    pin: u8,
    // previous values to restore
    schmitt: bool,
    ie: bool,
    pde: bool,
    pue: bool,
    func: u8,
}

impl PinSetup {
    fn new(pin_num: u8) -> Self {
        let (schmitt, ie, pde, pue) = pac::PADS_BANK0.gpio(pin_num as usize).modify(|s| {
            let prev = (s.schmitt(), s.ie(), s.pde(), s.pue());
            // Disabling the Schmitt Trigger seems sensible.
            s.set_schmitt(false);
            // Input enable
            s.set_ie(true);
            // No pulldown
            s.set_pde(false);
            prev
        });

        // Use SIO, single cycle IO
        let func = pac::IO_BANK0.gpio(pin_num as usize).ctrl().modify(|s| {
            let func = s.funcsel();
            s.set_funcsel(pac::io::vals::Gpio0ctrlFuncsel::SIO_0.into());
            func
        });

        PinSetup {
            pin: pin_num,
            schmitt,
            ie,
            pde,
            pue,
            func,
        }
    }
}

impl Drop for PinSetup {
    fn drop(&mut self) {
        pac::PADS_BANK0.gpio(self.pin as usize).modify(|s| {
            s.set_ie(self.ie);
            s.set_schmitt(self.schmitt);
            s.set_pde(self.pde);
            s.set_pue(self.pue);
        });

        pac::IO_BANK0
            .gpio(self.pin as usize)
            .ctrl()
            .modify(|s| s.set_funcsel(self.func));
    }
}
