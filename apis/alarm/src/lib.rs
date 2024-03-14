#![no_std]

use core::cell::Cell;
use core::fmt;
use core::ops::{Add, Sub};
use libtock_platform as platform;
use libtock_platform::share;
use libtock_platform::{DefaultConfig, ErrorCode, Syscalls};

/// The alarm driver
///
/// # Example
/// ```ignore
/// use libtock2::Alarm;
///
/// // Wait for timeout
/// Alarm::sleep(Alarm::Milliseconds(2500));
/// ```

pub struct Alarm<S: Syscalls, C: platform::subscribe::Config = DefaultConfig>(S, C);

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Hz(pub u32);

pub trait Convert {
    /// Converts a time unit by rounding up.
    fn to_ticks(self, freq: Hz) -> Ticks;
}

#[derive(Copy, Clone, Debug)]
pub struct Ticks(pub u32);

impl Convert for Ticks {
    fn to_ticks(self, _freq: Hz) -> Ticks {
        self
    }
}

impl Add for Ticks {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Ticks(self.0.wrapping_add(other.0))
    }
}

impl Sub for Ticks {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Ticks(self.0.wrapping_sub(other.0))
    }
}

impl fmt::Display for Ticks {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Copy, Clone)]
pub struct Milliseconds(pub u32);

impl Convert for Milliseconds {
    fn to_ticks(self, freq: Hz) -> Ticks {
        // Saturating multiplication will top out at about 1 hour at 1MHz.
        // It's large enough for an alarm, and much simpler than failing
        // or losing precision for short sleeps.

        /// u32::div_ceil is still unstable.
        fn div_ceil(a: u32, other: u32) -> u32 {
            let d = a / other;
            let m = a % other;
            if m == 0 {
                d
            } else {
                d + 1
            }
        }
        Ticks(div_ceil(self.0.saturating_mul(freq.0), 1000))
    }
}

impl<S: Syscalls, C: platform::subscribe::Config> Alarm<S, C> {
    /// Run a check against the console capsule to ensure it is present.
    #[inline(always)]
    pub fn exists() -> Result<(), ErrorCode> {
        S::command(DRIVER_NUM, command::EXISTS, 0, 0).to_result()
    }

    pub fn get_frequency() -> Result<Hz, ErrorCode> {
        S::command(DRIVER_NUM, command::FREQUENCY, 0, 0)
            .to_result()
            .map(Hz)
    }

    pub fn get_time() -> Result<Ticks, ErrorCode> {
        S::command(DRIVER_NUM, command::TIME, 0, 0)
            .to_result()
            .map(Ticks)
    }

    pub fn sleep_for<T: Convert>(time: T) -> Result<(), ErrorCode> {
        let freq = Self::get_frequency()?;
        let ticks = time.to_ticks(freq);

        let called: Cell<Option<(u32, u32)>> = Cell::new(None);
        share::scope(|subscribe| {
            S::subscribe::<_, _, C, DRIVER_NUM, { subscribe::CALLBACK }>(subscribe, &called)?;

            S::command(DRIVER_NUM, command::SET_RELATIVE, ticks.0, 0)
                .to_result()
                .map(|_when: u32| ())?;

            loop {
                S::yield_wait();
                if let Some((_when, _ref)) = called.get() {
                    return Ok(());
                }
            }
        })
    }
}

#[cfg(test)]
mod tests;

// -----------------------------------------------------------------------------
// Driver number and command IDs
// -----------------------------------------------------------------------------

const DRIVER_NUM: u32 = 0;

// Command IDs
#[allow(unused)]
mod command {
    pub const EXISTS: u32 = 0;
    pub const FREQUENCY: u32 = 1;
    pub const TIME: u32 = 2;
    pub const STOP: u32 = 3;

    pub const SET_RELATIVE: u32 = 5;
    pub const SET_ABSOLUTE: u32 = 6;
}

#[allow(unused)]
mod subscribe {
    pub const CALLBACK: u32 = 0;
}
