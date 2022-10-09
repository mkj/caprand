#[cfg(not(feature = "defmt"))]
use log::{debug, info, warn, error};

#[cfg(feature = "defmt")]
use defmt::{error, debug, info, panic};

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
// Returns (ticks: u32, precise: bool)
fn time_rise(pin: PeripheralRef<AnyPin>, wait: &mut u32, rpos: &mut u32, syst: &mut SYST) -> Result<(u32, bool), ()> {

    let pin_num = pin.pin() as usize;
    let mask = 1u32 << pin_num;

    // bank 0 single cycle IO in
    let gpio_in = pac::SIO.gpio_in(0).ptr();
    let so = pac::SIO.gpio_out(0);
    let soe = pac::SIO.gpio_oe(0);
    unsafe {
        so.value_clr().write_value(1 << pin_num);
        soe.value_set().write_value(1 << pin_num);
    }
    // // unsafe {
    // //     asm!(
    // //         "nop",
    // //     )
    // // }
    // cortex_m::asm::delay(20);
    // cortex_m::asm::delay(1000);
    unsafe {
        soe.value_clr().write_value(1 << pin_num);
    }

    unsafe {
        pac::PADS_BANK0
            .gpio(pin_num)
            .modify(|s| {
                // Pullup
                s.set_pue(true);
            });
    };

    // for testing with logic analyzer
    // let mut out = Flex::new(unsafe { embassy_rp::peripherals::PIN_16::steal() });
    // out.set_as_output();
    // so.value_clr().write_value(1<<16);

    // pin.set_pull(Pull::Down);
    // cortex_m::asm::delay(900);
    // pin.set_pull(Pull::None);

    let t = SyTi::new(syst);

    let x0: u32;
    let x1: u32;
    let x2: u32;
    let x3: u32;
    let x4: u32;
    let x5: u32;

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

    let tick = t.done()?;

    let pos = if x0 & mask != 0 {
        0
    } else if x1 & mask != 0 {
        1
    } else if x2 & mask != 0 {
        2
    } else if x3 & mask != 0 {
        3
    } else if x4 & mask != 0 {
        4
    } else if x5 & mask != 0 {
        5
    } else {
        6
    };
    let tick = tick + pos;
    // first measurement is less precise.
    let precise = pos != 0;

    if tick > 600 {
        *wait = wait.saturating_sub(1);
    } else if tick > 600 {
        *wait = (*wait+1).max(10000);
    }

    unsafe {
        pac::PADS_BANK0
            .gpio(pin_num)
            .modify(|s| {
                // No pullup
                s.set_pue(false);
            });
    };

    *rpos = pos;
    Ok((tick, precise))
}

// `f()` is called on each output `u32`.
pub fn noise<'d, F>(
    mut pin: PeripheralRef<AnyPin>,
    syst: &mut SYST,
    mut f: F,
) -> Result<(), ()>
where
    F: FnMut(u32, u32) -> bool,
{
    let pin_num = pin.pin() as usize;
    syst.set_clock_source(cortex_m::peripheral::syst::SystClkSource::Core);
    // prescribed sequence for setup
    syst.set_reload(10_000_000 - 1);
    syst.clear_current();
    syst.enable_counter();

    // Seemed to reduce noise (for investigating thermal noise vs other interference).
    // XXX Somehow disabling ROSC breaks SWD, perhaps signal integrity issues?
    // unsafe{ pac::ROSC.ctrl().modify(|s| s.set_enable(pac::rosc::vals::Enable::DISABLE)) };


    let mut gpio = Flex::<AnyPin>::new(pin.reborrow());
    // Measure pullup time from 0 as a sanity check
    gpio.set_as_output();
    gpio.set_low();
    // Long enough to drive the capacitor low
    cortex_m::asm::delay(10000);
    gpio.set_as_input();
    let del = critical_section::with(|_cs| {
        gpio.set_pull(Pull::Up);
        let t = SyTi::new(syst);
        // Get it near the threshold to begin.
        while gpio.is_low() {}
        t.done()
    })?;
    drop(gpio);
    info!("Initial pullup del is {}", del);

    if del < MIN_CAPACITOR_DEL {
        error!("Capacitor seems small or missing?");
        return Err(())
    }


    // The main loop
    let mut overshoot = 1u32;

    let mut warming = WARMUP;
    let mut wait = 0;
    let mut pos = 0;

    unsafe {
        pac::PADS_BANK0
            .gpio(pin_num)
            .modify(|s| {
                // Disabling the Schmitt Trigger gives a clearer correlation between "overshoot"
                // and measured values.
                s.set_schmitt(false);
                // Input enable
                s.set_ie(true);
            });

        // Use SIO
        pac::IO_BANK0
            .gpio(pin_num)
            .ctrl()
            .modify(|s| {
                s.set_funcsel(pac::io::vals::Gpio0ctrlFuncsel::SIO_0.0)
            });
    }

    // After warmup we sample twice at each "overshoot" value.
    // One sample is returned as random output, the other is mixed
    // in to the overshoot value.
    for (i, _) in core::iter::repeat(()).enumerate() {

        let (meas, precise) = critical_section::with(|_cs| {
            // pin.set_pull(Pull::Down);
            // // let t = SyTi::new(syst);
            // while pin.is_high() {}
            // // let ticks = t.done()?;
            // // Keep pulling down for `overshoot` cycles
            // cortex_m::asm::delay(overshoot);

            // // Pull up, time how long to reach threshold
            // pin.set_pull(Pull::None);
            // debug!("pulldown {}", ticks);

            // let t = SyTi::new(syst);
            // while pin.is_high() {}
            // let r = t.done().map(|t| (t, true));

            let (r, precise) = time_rise(pin.reborrow(), &mut wait, &mut pos, syst)?;

            // let mut gpio = Flex::<AnyPin>::new(pin.reborrow());
            // gpio.set_pull(Pull::None);

            // if (r > )


            Ok((r, precise))
        })?;

        if i % 2 == 0 {
            // real output
            // if precise && warming == 0 {
            if warming == 0 {
                if !f(meas, pos) {
                    // no more output wanted
                    break
                }
            }
            warming = warming.saturating_sub(1);
        } else {
            // don't produce output, mix measured sample in
            overshoot = overshoot * 2 + meas;
            // modulo to sensible range
            if overshoot > HIGH_OVER {
                overshoot = LOW_OVER + (overshoot % (HIGH_OVER - LOW_OVER + 1))
            }
        }
    }
    Ok(())
}
