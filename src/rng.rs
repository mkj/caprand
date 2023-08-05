#[cfg(not(feature = "defmt"))]
#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

#[cfg(feature = "defmt")]
#[allow(unused_imports)]
use defmt::{debug, error, info, panic, trace, warn};

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
/// getrandom::register_custom_getrandom!(caprand::getrandom);
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
/// let mut p = embassy_rp::init(Default::default());
///
/// caprand::setup(&mut p.PIN_10).unwrap();
/// getrandom::register_custom_getrandom!(caprand::random);
///
/// let mut mystery = [0u8; 10];
/// getrandom::getrandom(&mut mystery).unwrap();
/// ```
pub fn setup(pin: &mut impl Pin) -> Result<(), getrandom::Error> {
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

impl rand::CryptoRng for CapRng {}

impl CapRng {
    /// The number of noise samples to use for seeding.
    ///
    /// We need to produce a 256 bit output seed.
    pub const SEED_SAMPLES: usize = 256 * 100;

    const MAX_FAILURES: usize = 3;

    pub fn new(pin: &mut impl Pin) -> Result<Self, getrandom::Error> {
        let low_cycles = 1;
        let mut noise = crate::cap::RawNoise::new(pin, low_cycles);

        let mut valid_samples = 0;
        let mut h = Sha256::new();

        let mut health = crate::health::TotalHealth::new();
        let mut failures = 0;

        while valid_samples < Self::SEED_SAMPLES {
            let (v, valid) = noise
                .next()
                // OK unwrap, iterator doesn't end
                .unwrap();
            if valid {
                valid_samples += 1;
                if health.test(v).is_err() {
                    valid_samples = 0;
                    failures += 1;
                    if failures > Self::MAX_FAILURES {
                        error!("Health tests failed after {} retries", Self::MAX_FAILURES);
                        return Err(error())
                    }
                }
            }

            // even "invalid" samples are included in the hash
            h.update([v]);
        }

        let seed: [u8; 32] = h.finalize().into();
        Ok(Self(ChaCha20Rng::from_seed(seed)))
    }
}

impl rand::RngCore for CapRng {
    fn next_u32(&mut self) -> u32 {
        self.0.next_u32()
    }

    fn next_u64(&mut self) -> u64 {
        self.0.next_u64()
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        self.0.fill_bytes(dest)
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand::Error> {
        self.0.try_fill_bytes(dest)
    }
}
