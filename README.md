# RP2040 entropy source

This is a random number generator for a RP2040. It requires one hardware component,
a capacitor between a GPIO pin and ground. It should be treated as a proof of concept,
it requires further analysis before use in applications with consequences.

This is based on Peter Allan’s [twocent](https://github.com/alwynallan/twocents), with
half the component and pin count.

An end program would use [getrandom](https://docs.rs/getrandom) with `custom` feature

```rust
    caprand::setup(&mut p.PIN_10).unwrap();
    getrandom::register_custom_getrandom!(caprand::random);
```

## Operation

The capacitor is discharged briefly (currently 1 cycle)
then the RP2040 internal pullup brings the capacitor high again.
The time taken to reach “high” level is used as the random sample value.

The time is measured to an exact clock cycle, in bursts of 5 bits (limited by number of registers).
That low order part of the pullup time is output as a noise sample. Samples are hashed together
to form a seed, which seeds a [ChaCha20](https://docs.rs/rand_chacha/latest/rand_chacha/struct.ChaCha20Rng.html)
cryptographic DRNG.

## Security

The noise source has not been thoroughly quantified. Empirical testing seems
to show 1-2 bits per sample. As a workaround it takes 100 noise samples per bit of output,
hashing 256100 input noise samples to seed the DRNG.

Online health tests are yet to be implemented.

The hardware scheme has no protection against local interference (similar to the RP2040 itself).

## Hardware

Testing was performed with a 10nF Y5V SMD chip capacitor soldered between
GP10 pad and the adjacent GND pad, on a Pico W board.
Other capacitor values should also work OK - 100nF was tested, 1nF is likely to work.

## Examples

[rand](examples/rand.rs) uses getrandom as a normal program would.

[usbnoise](examples/usbnoise.rs) outputs raw samples as hex values, as a USB serial device

[bitplot](examples/bitplot.rs) outputs ascii art representations of samples, as a USB serial device

[cyclecount](examples/cyclecount.rs) prints the whole pullup time (not just the low order bits). This
uses the `SYST` timer register.

These should be compiled with `--feature defmt` (or modified to avoid it).
