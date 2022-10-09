#[cfg(not(feature = "defmt"))]
use log::{debug, info, warn, error, trace};

#[cfg(feature = "defmt")]
use defmt::{error, debug, info, panic, trace};

use core::arch::asm;

use cortex_m::peripheral::SYST;
use embassy_rp::gpio::{Flex, Pin, Pull, AnyPin};
use embassy_rp::{pac, Peripheral, PeripheralRef};

///                                         _ still pullup, =up_x
///         t3      t4       t1      t2    /
/// |prev    |pulldn |still  | pullup|    /
/// |iter    |active |pulldn,| active|   / | pulldn active, etc
/// |        |=t_down|=down_x| =t_up |  o  | measuring next t_down, etc...
/// |        .       |       |       |     |
///        .   .     |       |       |     .
///      .       .   |       |       |   .   .
///    .           . |       |       | .       .
///  .---------------.---------------.-----------.------ GPIO threshold. Probably isn't really flat.
///                    .           .               .
///                      .       .
///                        .   .
///                          .
///  The GPIO pin is attached to a capacitor to ground. 0.1uF used for experiment.
///  The GPIO pin is left as input and is driven low/high with pulldown/pullup
///  resistors. The time taken to reach logical low/high is measured. The pulldown/pullup
///  resistor is left on for a further time down_x/up_x, which intends to drive
///  the voltage further so that a useful time can be measured for the next t_up/t_down
///  cycle.
///  The down_x/up_x overshoot cycle times are doubled each iteration and add in the measured time,
///  amplifying noise captured in t_down/t_up measurements.
///  down_x/up_x time is kept within a sensible range with modulo.
///  The random output is t_down and t_up time.

/*
Experimental capacitor values:
0.1uF "monolithic" through hole, perhaps from futurlec?
GPIO6
0.003717 INFO  initial pulldown del is 346730
0.003710 INFO  initial pulldown del is 345770
less than ~13000 overshoot had lots of no-delays

Altronics R8617 • 0.01uF 50V Y5V 0805 SMD Chip Capacitor PK 10
GPIO10
0.001292 INFO  initial pulldown del is 43234
0.001294 INFO  initial pulldown del is 43598


Altronics R8629 • 0.047uF 50V X7R 0805 SMD Chip Capacitor PK 10
GPIO13
0.002497 INFO  initial pulldown del is 193878
0.002547 INFO  initial pulldown del is 200170
*/


// Range of cycle counts for overshoot. These values are somewhat dependent on the
// cpu frequency and capacitor values.

// Lower limit is necessary because we don't want to hit the threshold immediately
// on reading.
// const LOW_OVER: u32 = 1000;
// // Power of two for faster modulo
// const HIGH_OVER: u32 = LOW_OVER + 1023;
const LOW_OVER: u32 = 100;
const HIGH_OVER: u32 = 100;

// Assume worst case from rp2040 datasheet.
// 3.3v vdd, 2v logical high voltage, 50kohm pullup, 0.01uF capacitor, 125Mhz clock.
// 0.5 * 50e3 * 0.01e-6 * 125e6 = 31250.0
// Then allow 50% leeway for tolerances.
const MIN_CAPACITOR_DEL: u32 = 0;

/// Extra iterations prior to taking output, so that `overshoot` is nice and noisy.
const WARMUP: usize = 16;

/// Wraps timing with SYST. The clock source must already be configured.
struct SyTi<'t> {
    syst: &'t mut SYST,
    t1: u32,
}

impl<'t> SyTi<'t> {
    fn new(syst: &'t mut SYST) -> Self {
        syst.clear_current();
        syst.enable_counter();
        Self {
            syst,
            t1: SYST::get_current(),
        }
    }

    fn reset(&mut self) {
        self.syst.clear_current();
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

/// Pulls a pin low for an exact number of cycles.
/// Call with interrupts disabled if it's important.
fn exact_low(pin: PeripheralRef<AnyPin>, low_cycles: u32) {
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
            // no pull low
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
                // one cycle
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
                // one cycle
                "subs {d}, 1",
                // one cycle
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

fn time_rise(pin: PeripheralRef<AnyPin>, low_cycles: u32) -> Result<u32, ()> {
    let pin_num = pin.pin() as usize;
    let mask = 1u32 << pin_num;

    // Pull low for a number of cycles
    exact_low(pin, low_cycles);

    // bank 0 single cycle IO in
    let gpio_in = pac::SIO.gpio_in(0).ptr();
    let x0: u32;
    let x1: u32;
    let x2: u32;
    let x3: u32;
    let x4: u32;
    let x5: u32;
    // Time how long it takes to pull up
    unsafe {
        asm!(
            // save
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

            // return last sample in x5
            "mov {mask}, r7",
            // restore
            "mov r7, r10",

            mask = inlateout(reg) mask => x5,
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

    // let tick = t.done()?;

    // let pos = if x0 & mask != 0 {
    //     0
    // } else if x1 & mask != 0 {
    //     1
    // } else if x2 & mask != 0 {
    //     2
    // } else if x3 & mask != 0 {
    //     3
    // } else if x4 & mask != 0 {
    //     4
    // } else if x5 & mask != 0 {
    //     5
    // } else {
    //     6
    // };
    // let tick = tick + pos;
    // // first measurement is less precise.
    // let precise = pos != 0;

    // Combine all measurements in a constant-time way
    // TODO: Check it really is constant time - seems like it should be.
    let result = (x0 & mask)
        | (x1 & mask).rotate_left(1)
        | (x2 & mask).rotate_left(2)
        | (x3 & mask).rotate_left(3)
        | (x4 & mask).rotate_left(4)
        | (x5 & mask).rotate_left(5);
    let result = result >> pin_num;

    Ok(result)
}

// `f()` is called on each output `u32`.
pub fn noise<'d, F>(
    mut pin: PeripheralRef<AnyPin>,
    low_cycles: u32,
    mut f: F,
) -> Result<(), ()>
where
    F: FnMut(u32) -> bool,
{
    let pin_num = pin.pin() as usize;

    // Seemed to reduce noise (for investigating thermal noise vs other interference).
    // XXX Somehow disabling ROSC breaks SWD, perhaps signal integrity issues?
    // unsafe{ pac::ROSC.ctrl().modify(|s| s.set_enable(pac::rosc::vals::Enable::DISABLE)) };

    let mut gpio = Flex::<AnyPin>::new(pin.reborrow());
    // // Measure pullup time from 0 as a sanity check
    // gpio.set_as_output();
    // gpio.set_low();
    // // Long enough to drive the capacitor low
    // cortex_m::asm::delay(10000);
    // gpio.set_as_input();
    // // let del = critical_section::with(|_cs| {
    // //     gpio.set_pull(Pull::Up);
    // //     let t = SyTi::new(syst);
    // //     // Get it near the threshold to begin.
    // //     while gpio.is_low() {}
    // //     t.done()
    // // })?;
    drop(gpio);
    // // info!("Initial pullup del is {}", del);

    // // if del < MIN_CAPACITOR_DEL {
    // //     error!("Capacitor seems small or missing?");
    // //     return Err(())
    // // }


    let mut warming = WARMUP;

    let _p = PinSetup::new(pin_num as u8);

    for (i, _) in core::iter::repeat(()).enumerate() {
        let r = critical_section::with(|_cs| {

            let r = time_rise(pin.reborrow(), low_cycles)?;
            Ok(r)
        })?;

        // real output
        // if precise && warming == 0 {
        if warming == 0 {
            if !f(r) {
                // no more output wanted
                break
            }
        }
        warming = warming.saturating_sub(1);
    }
    Ok(())
}

pub fn best_low_time(mut pin: PeripheralRef<AnyPin>, max_time: u32) -> Result<u32, ()> {
    const ITERS: usize = 1000;
    let mut best = None;
    for t in 1..=max_time {
        let mut hist = [0u32; 64];
        let mut n = 0;
        noise(pin.reborrow(), t,
            |v| {
                hist[v as usize] += 1;
                n += 1;
                n < ITERS
            })?;
        let hmax = *hist.iter().max().unwrap();
        // trace!("t {} hmax {}", t, hmax);
        if let Some((_, best_hmax)) = best {
            if hmax < best_hmax {
                best = Some((t, hmax));
            }
        } else {
            best = Some((t, hmax));
        }
    }
    // TODO unwrap
    Ok(best.unwrap().0)
}

struct PinSetup(u8);

impl PinSetup {
    fn new(pin_num: u8) -> Self {
        unsafe {
            pac::PADS_BANK0
                .gpio(pin_num as usize)
                .modify(|s| {
                    // // Disabling the Schmitt Trigger gives a clearer correlation between "overshoot"
                    // // and measured values.
                    // s.set_schmitt(false);
                    // Input enable
                    s.set_ie(true);
                    // Pullup
                    s.set_pue(true);
                    s.set_pde(false);
                });

            // Use SIO
            pac::IO_BANK0
                .gpio(pin_num as usize)
                .ctrl()
                .modify(|s| {
                    s.set_funcsel(pac::io::vals::Gpio0ctrlFuncsel::SIO_0.0)
                });

            pac::SYSCFG
                .proc_in_sync_bypass()
                .modify(|s| {
                    let val = s.proc_in_sync_bypass();
                    s.set_proc_in_sync_bypass(val | 1 << pin_num);
                });

        }
        PinSetup(pin_num)
    }
}

impl Drop for PinSetup {
    fn drop(&mut self) {
        let pin_num = self.0;
        unsafe {
            pac::PADS_BANK0
                .gpio(pin_num as usize)
                .modify(|s| {
                    // Pullup
                    s.set_pue(false);
                });

            pac::SYSCFG
                .proc_in_sync_bypass()
                .modify(|s| {
                    let val = s.proc_in_sync_bypass();
                    s.set_proc_in_sync_bypass(val & !(1 << pin_num));
                });

            // Use SIO
            pac::IO_BANK0
                .gpio(pin_num as usize)
                .ctrl()
                .modify(|s| {
                    s.set_funcsel(pac::io::vals::Gpio0ctrlFuncsel::NULL.0);
                });
        }
    }
}
