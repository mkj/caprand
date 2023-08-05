#[cfg(not(feature = "defmt"))]
#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

#[cfg(feature = "defmt")]
#[allow(unused_imports)]
use defmt::{debug, error, info, panic, trace, warn};

struct Repetition {
    prev: u8,
    count: usize,
}

impl Repetition {
    fn new() -> Self {
        // Initialising with count = 0 means
        // the fixed initial `prev` value doesn't matter.
        Self {
            prev: 0u8,
            count: 0,
        }
    }

    /// Returns the running repetition count for the given value
    fn feed(&mut self, val: u8) -> usize {
        if val == self.prev {
            self.count += 1;
        } else {
            self.count = 1;
            self.prev = val;
        }
        self.count
    }
}

/// Repetition Count Test
///
/// Ref NIST SP 800-90B 4.4.1
pub struct RepetitionTest {
    r: Repetition,
    cutoff: usize,
}

impl RepetitionTest {
    pub fn new(cutoff: usize) -> Self {
        Self {
            r: Repetition::new(),
            cutoff,
        }
    }

    pub fn test(&mut self, val: u8) -> Result<(), ()> {
        if self.r.feed(val) < self.cutoff {
            Ok(())
        } else {
            warn!("Repetition test failed for value {}", val);
            Err(())
        }
    }
}

/// Repetition Count Test
///
/// Ref NIST SP 800-90B 4.4.1
pub struct AdaptiveProportionTest {
    // A, value to compare
    val: u8,
    // B, count of matches in the window
    matches: usize,
    // i, iterator in the window
    i: usize,

    // W, window size. constant across iterations
    window: usize,
    // C cutoff. failure occurs if matches >= cutoff
    cutoff: usize,
}

impl AdaptiveProportionTest {
    pub fn new(window: usize, cutoff: usize) -> Self {
        Self {
            val: 0,
            matches: 0,
            i: 0,
            window,
            cutoff,
        }
    }

    pub fn test(&mut self, val: u8) -> Result<(), ()> {
        if self.i == 0 {
            // new iteration
            self.val = val;
            self.matches = 0;
            self.i = 1;
            Ok(())
        } else {
            if self.val == val {
                self.matches += 1
            }
            let result = self.matches < self.cutoff;
            self.i += 1;

            if self.i == self.window {
                self.i = 0;
                self.matches = 0;
            }

            if result {
                Ok(())
            } else {
                warn!("Adaptive proportion test failed for value {}", self.val);
                Err(())
            }
        }
    }
}

pub struct TotalHealth {
    adaptive: AdaptiveProportionTest,
    repetition: RepetitionTest,
}

impl TotalHealth {
    pub fn new() -> Self {
        Self {
            adaptive: AdaptiveProportionTest::new(512, 410),
            // 201 for H = 0.1, alpha = 2**-20
            repetition: RepetitionTest::new(201),
        }
    }

    pub fn test(&mut self, val: u8) -> Result<(), ()> {
        self.adaptive.test(val)?;
        self.repetition.test(val)?;
        Ok(())
    }
}
