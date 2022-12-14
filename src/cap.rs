//! The raw noise source using a capacitor on a GPIO pin.
//!
//! Most users should use [`caprand::setup`](crate::setup) and [`caprand::getrandom`](crate::getrandom) instead.
//! This module is accessible for health testing and analysis.
use cortex_m::peripheral::SYST;
#[cfg(not(feature = "defmt"))]
#[allow(unused_imports)]
use log::{debug, info, warn, error, trace};

#[cfg(feature = "defmt")]
#[allow(unused_imports)]
use defmt::{error, debug, info, panic, trace};

use core::arch::asm;

use embassy_rp::gpio::Pin;
use embassy_rp::pac;

/// Drives a pin low for an exact number of cycles.
/// Call with interrupts disabled if it's important.
fn exact_low(pin: &impl Pin, low_cycles: u32) {
    let pin_num = pin.pin() as usize;
    let mask = 1u32 << pin_num;
    // pin low
    let so = pac::SIO.gpio_out(0);
    unsafe {
        so.value_clr().write_value(1 << pin_num);
    }

    // output enable set/clear registers
    let soe = pac::SIO.gpio_oe(0);
    let soe_set = soe.value_set().ptr();
    let soe_clr = soe.value_clr().ptr();

    // We set output enable, wait a number of cycles, then clear output enable.
    // Modulo 3 because the subs/bne loops take 3 cycles.
    match (low_cycles, low_cycles % 3) {
        (0, _) => {
            // no drive low
        },
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
        (_, 3..) => unreachable!()
    }
}

/// Drives a pin low then times how long it takes to rise to logic high.
///
/// `low_cycles` is the amount of time to hold the pin low to discharge
/// the capacitor. As the pullup charges the capacitor, it samples on every
/// clock cycles in bursts of 6 bits. When the final bit of a burst is high,
/// it returns that 6-bit burst as the value.
fn time_rise(pin: &mut impl Pin, low_cycles: u32, syst: Option<&mut SYST>) -> Result<u8, ()> {
    let pin_num = pin.pin() as usize;
    let mask = 1u32 << pin_num;

    let pad = pac::PADS_BANK0.gpio(pin_num);
    // bank 0 single cycle IO in
    let gpio_in = pac::SIO.gpio_in(0).ptr();

    // enable pullup
    unsafe { pad.modify(|s| s.set_pue(true)); }

    // Drive low for a number of cycles
    let t = syst.map(SyTi::new);
    exact_low(pin, low_cycles);
    if let Some(t) = t {
        let t = t.done()?;
        trace!("exact {}", t);
    }

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

    // Combine all measurements in a constant-time way
    let result = (x0 & mask)
        | (x1 & mask).rotate_left(1)
        | (x2 & mask).rotate_left(2)
        | (x3 & mask).rotate_left(3)
        | (x4 & mask).rotate_left(4)
        ;

    let result = result.rotate_right(pin_num as u32);
    let result = result as u8;

    // Disable pullup until next run. Preserved capacitor charge
    // carried over to the next iteration helps improve noise.
    unsafe { pad.modify(|s| s.set_pue(false)); }

    Ok(result)
}

/// Returns the least significant bit set, or 8 if 0.
/// Is neither constant time nor efficient, for display purposes only.
pub fn lsb(v: u8) -> u8 {
    for i in 0..u8::BITS {
        if v & 1<<i != 0 {
            return i as u8
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
/// For each output sample it drives a pin low then times
/// how long it takes to rise to logic high. `low_cycles` is the amount of
/// time to hold the pin low to discharge the capacitor.
/// As the pullup charges the capacitor, it samples on every
/// clock cycles in bursts of 6 bits. When the final bit of a burst is high,
/// it outputs that 6-bit burst as the value.
///
/// Samples are correlated and biased, so must be processed before
/// further use, using a cryptographic extractor or similar scheme.
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
            let r = time_rise(self.pin, self.low_cycles, None)?;
            let t = t.done()?;
            let t = t + lsb(r) as u32;
            Ok(t)
        })
    }
}

impl<P: Pin> Iterator for RawNoise<'_, P> {
    type Item = Result<u8, ()>;

    fn next(&mut self) -> Option<Self::Item> {
        let r = critical_section::with(|_cs| {
            let r = time_rise(self.pin, self.low_cycles, None)?;
            Ok(r)
        });
        Some(r)
    }
}

/// Determines a good pull-low time to use for the capacitor
///
/// This returns the time having the highest min-entropy measured from
/// a number of iterations. That corresponds to the time giving the lowest
/// likelihood (or histogram count) for the most likely value in the histogram.
pub fn best_low_time(pin: &mut impl Pin, times: impl IntoIterator<Item = u32>) -> Result<u32, ()> {
    const ITERS: usize = 200;
    const WARMUP: usize = 5;
    let mut best = None;
    for t in times {
        let mut hist = [0u32; 64];
        let mut hd = [0u32; 9];
        let mut noise = RawNoise::new(pin, t);
        // warmup
        for _ in 0..WARMUP {
            let _ = noise.next();
        }
        for v in noise.take(ITERS) {
            let v = v?;
            hist[v as usize] += 1;
            hd[lsb(v) as usize] += 1;
        }

        let hmax = *hist.iter().max().unwrap();
        // let hmax = *hd.iter().max().unwrap();
        // let hmax = hist.iter().enumerate().filter_map(|(i, &v)| {
        //     if i&1 == 0 { Some(v) } else { None }
        // }).max().unwrap_or(0);
        trace!("t {}:  {}  {}  {}  {}  {}  {}  {}  hmax {}", t,
            hd[0], hd[1], hd[2], hd[3], hd[4], hd[5], hd[6],
            hmax);
        // trace!("{:?}", hist);
        if let Some((_, best_hmax)) = best {
            if hmax < best_hmax {
                best = Some((t, hmax));
            }
        } else {
            best = Some((t, hmax));
        }
    }
    // OK unwrap: loop has >0 iterations, best is always set
    Ok(best.unwrap().0)
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
    bypass: bool,
}

impl PinSetup {
    fn new(pin_num: u8) -> Self {
        let (schmitt, ie, pde, pue, func, bypass);
        unsafe {
            (schmitt, ie, pde, pue) = pac::PADS_BANK0
                .gpio(pin_num as usize)
                .modify(|s| {
                    // Disabling the Schmitt Trigger simplifies analysis.
                    let prev = (s.schmitt(), s.ie(), s.pde(), s.pue());
                    s.set_schmitt(false);
                    // Input enable
                    s.set_ie(true);
                    // No pulldown
                    s.set_pde(false);
                    prev
                });

            // Use SIO
            func = pac::IO_BANK0
                .gpio(pin_num as usize)
                .ctrl()
                .modify(|s| {
                    let func = s.funcsel();
                    s.set_funcsel(pac::io::vals::Gpio0ctrlFuncsel::SIO_0.0);
                    func
                });

            bypass = pac::SYSCFG
                .proc_in_sync_bypass()
                .modify(|s| {
                    let bypass = s.proc_in_sync_bypass();
                    s.set_proc_in_sync_bypass(bypass | 1 << pin_num);
                    bypass & 1 << pin_num != 0
                });

        }
        PinSetup {
            pin: pin_num,
            schmitt,
            ie,
            pde,
            pue,
            func,
            bypass,
        }
    }
}

impl Drop for PinSetup {
    fn drop(&mut self) {
        unsafe {
            pac::PADS_BANK0
                .gpio(self.pin as usize)
                .modify(|s| {
                    s.set_ie(self.ie);
                    s.set_schmitt(self.schmitt);
                    s.set_pde(self.pde);
                    s.set_pue(self.pue);
                });

            pac::SYSCFG
                .proc_in_sync_bypass()
                .modify(|s| {
                    let val = s.proc_in_sync_bypass();
                    let b = self.bypass as u32;
                    s.set_proc_in_sync_bypass(val & !(b << self.pin));
                });

            // Use SIO
            pac::IO_BANK0
                .gpio(self.pin as usize)
                .ctrl()
                .modify(|s| {
                    s.set_funcsel(self.func)
                });
        }
    }
}
