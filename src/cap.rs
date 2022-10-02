use defmt::{debug, info, panic};

use cortex_m::peripheral::SYST;
use embassy_rp::gpio::{Flex, Pin, Pull};
use embassy_rp::pac;

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
Experimental values:
0.1uF "monolithic" perhaps from futurlec?
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

/// Extra iterations prior to taking output, so that `overshoot` is nice and noisy.
const WARMUP: usize = 8;

// Range of cycle counts for overshoot. These values are somewhat dependent on the
// cpu frequency and capacitor values.

// Lower limit is necessary because we don't want to hit the threshold immediately
// on reading.
const LOW_OVER: u32 = 1000;
// Power of two for faster modulo
const HIGH_OVER: u32 = LOW_OVER + 16384;

struct Timer<'t> {
    syst: &'t mut SYST,
    t1: u32,
}

impl<'t> Timer<'t> {
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
            return Err(());
        }
        self.syst.disable_counter();
        Ok(self.t1 - t2)
    }
}

// `f()` is called on each output `u32`.
pub fn cap_rand<'d, P: Pin, F>(
    pin: &mut Flex<'d, P>,
    pin_num: usize,
    syst: &mut SYST,
    n_out: usize,
    mut f: F,
) -> Result<(), ()>
where
    F: FnMut(u32),
{
    syst.set_clock_source(cortex_m::peripheral::syst::SystClkSource::Core);
    // prescribed sequence
    syst.set_reload(10_000_000 - 1);
    syst.clear_current();
    syst.enable_counter();

    // Seemed to reduce noise (for investigating thermal noise vs other interference).
    // XXX Somehow disabling ROSC breaks SWD, perhaps signal integrity issues?
    // unsafe{ pac::ROSC.ctrl().modify(|s| s.set_enable(pac::rosc::vals::Enable::DISABLE)) };

    // Not necessary, but output has a cleaner more understandable linear response
    // with Schmitt Trigger disabled.
    unsafe {
        pac::PADS_BANK0
            .gpio(pin_num)
            .modify(|s| s.set_schmitt(false))
    };

    // Measure pulldown time from vcc as a sanity check
    pin.set_as_output();
    pin.set_high();
    // Long enough to drive the capacitor high
    cortex_m::asm::delay(10000);
    pin.set_as_input();
    pin.set_pull(Pull::Down);
    syst.clear_current();
    let t = Timer::new(syst);
    // Get it near the threshold to begin.
    while pin.is_high() {}
    let del = t.done()?;
    info!("initial pulldown del is {}", del);

    // The main loop
    let mut overshoot = 1u32;
    let n_iter = WARMUP + 2 * n_out;
    for i in 0..n_iter {
        // Pull up until hit logical high
        pin.set_pull(Pull::Up);
        while pin.is_low() {}
        // Keep pulling up for `overshoot` cycles
        cortex_m::asm::delay(overshoot);

        // Pull down, time how long to reach threshold
        pin.set_pull(Pull::Down);
        let t = Timer::new(syst);
        while pin.is_high() {}
        let meas = t.done()?;

        if i > WARMUP && (i - WARMUP) % 2 == 1 {
            // produce output, don't use internally
            f(meas)
        } else {
            // don't produce output, mix measured in
            overshoot = overshoot * 2 + meas;
            // modulo to sensible range
            if overshoot > HIGH_OVER {
                overshoot = LOW_OVER + (overshoot % (HIGH_OVER - LOW_OVER + 1))
            }
        }
    }
    Ok(())
}
