#[cfg(not(feature = "defmt"))]
#[allow(unused_imports)]
use log::{debug, info, warn, error, trace};

#[cfg(feature = "defmt")]
#[allow(unused_imports)]
use defmt::{debug, info, warn, panic, error, trace};

use core::cell::RefCell;
use core::num::NonZeroU32;
use core::ops::DerefMut;

use critical_section::Mutex;
use rand_chacha::ChaCha20Rng;
use sha2::{Digest, Sha256};

use embassy_rp::gpio::Pin;

use rand_chacha::rand_core::{RngCore, SeedableRng};


// arbitrary constant
pub const CAPRAND_ERR: u32 = getrandom::Error::CUSTOM_START + 510132368;

pub fn error() -> getrandom::Error {
    // OK unwrap: const value is nonzero
    let c: NonZeroU32 = CAPRAND_ERR.try_into().unwrap();
    c.into()
}

static RNG: Mutex<RefCell<Option<CapRng>>> = Mutex::new(RefCell::new(None));

/// A random byte generator for use with `register_custom_getrandom`
///
/// [`setup()`](setup) must be called prior to using this function.
///
/// See documentation of [`getrandom::register_custom_getrandom`](getrandom::register_custom_getrandom).
///
/// # Examples
///
/// ```
/// fn main() {
///     getrandom::register_custom_getrandom!(caprand::getrandom);
/// }
/// ```
pub fn getrandom(buf: &mut [u8]) -> Result<(), getrandom::Error> {
    critical_section::with(|cs| {
        let mut rng = RNG.borrow_ref_mut(cs);
        let rng = rng.deref_mut();
        if let Some(rng) = rng {
            rng.0.fill_bytes(buf);
            Ok(())
        } else {
            error!("setup() not called");
            Err(error())
        }
    })
}

/// Seed the random generator from a capacitor noise source.
///
/// Call this at early startup.
///
/// # Arguments
///
/// * pin - The GPIO pin with a capacitor attached. This will be driven low and pulled high,
/// with timing used as a random source. The `Pin` may be used for other purposes once
/// `setup()` completes.
///
/// # Examples
///
/// ```
/// fn main() {
///     let mut p = embassy_rp::init(Default::default());
///
///     caprand::setup(&mut p.PIN_10).unwrap();
///     getrandom::register_custom_getrandom!(caprand::random);
///
///     let mut mystery = [0u8; 10];
///     getrandom::getrandom(&mut mystery).unwrap();
/// }
/// ```
pub fn setup(
    pin: &mut impl Pin,
) -> Result<(), getrandom::Error> {
    let r = CapRng::new(pin)?;

    critical_section::with(|cs| {
        let mut rng = RNG.borrow_ref_mut(cs);
        let _ = rng.insert(r);
    });
    Ok(())
}

// TODO: this is another impl of chacha20, can it use chacha20 crate instead? Is the size much?
// TODO: have some kind of fast erasure RNG instead?
/// A cryptographic PRNG seeded by the capacitor noise source.
pub struct CapRng(ChaCha20Rng);

impl rand::CryptoRng for CapRng {
}

impl CapRng {
    /// The number of noise samples to use for seeding.
    ///
    /// We have 256 bits output seed, and assume a noise source entropy rate of at least 0.01
    pub const SEED_SAMPLES: usize = 256 * 100;

    /// Call this at early startup. If noisy interrupts or time slicing is happening the caller
    /// should disable interrupts.
    fn new(pin: &mut impl Pin,
    ) -> Result<Self, getrandom::Error> {
        // let low_cycles = crate::cap::best_low_time(pin, 0..=100).unwrap();
        let low_cycles = 1;

        let mut h = Sha256::new();
        let mut noise = crate::cap::RawNoise::new(pin, low_cycles)
            .map_err(|_| error())?;

        let mut buf = [0u8; 100];
        for _ in 0..256 {
            for b in buf.iter_mut() {
                let v = noise.next()
                    // OK unwrap, iterator doesn't end
                    .unwrap()
                    .map_err(|_| error())?;
                *b = v;
            }

            h.update(buf);
        }
        let seed: [u8; 32] = h.finalize().into();
        Ok(Self(ChaCha20Rng::from_seed(seed)))
    }
}
