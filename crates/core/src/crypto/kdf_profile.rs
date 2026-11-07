//! How much a password costs to try: the Argon2 parameters of a vault, the
//! limits a file may ask for and the choice this device makes for the crypto
//! level.
//!
//! The parameters live in the file's header, so a vault opens with what it
//! was sealed with, whatever the current defaults are. The header is read
//! *before* anything authenticates it, so [`KdfParams::check`] runs before a
//! single block is allocated: a changed byte must not be able to ask for
//! 4 TiB of memory or a month of iterations.

use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use crate::crypto::header::VaultHeader;
use crate::crypto::{self, KDF_ITERS, KDF_MEM_KIB, KDF_PAR, SALT_LEN};
use crate::error::{Error, Result};

const MIB: u32 = 1024;
const GIB: u32 = 1024 * MIB;

/// No vault is ever sealed cheaper than this.
pub const KDF_MEM_KIB_MIN: u32 = 64 * MIB;
/// The desktop profile; nothing above it was ever written.
pub const KDF_MEM_KIB_MAX: u32 = GIB;
pub const KDF_ITERS_MIN: u32 = 1;
/// Keeps a changed header from hanging the unlock: no profile ever wrote more
/// than 3 iterations, so a header asking for 5 is refused outright.
pub const KDF_ITERS_MAX: u32 = 4;
pub const KDF_PAR_MIN: u8 = 1;
pub const KDF_PAR_MAX: u8 = 4;

/// A rung that takes longer than this on the device is too heavy for it.
pub const KDF_TARGET_MAX: Duration = Duration::from_secs(3);

/// An explicit cap on how much memory a key derivation may take in this
/// process (tests, or a host that knows better); 1 GiB unless set.
static KDF_MEM_OVERRIDE_KIB: AtomicU32 = AtomicU32::new(KDF_MEM_KIB_MAX);

/// Caps every later derivation in the process. Never below
/// [`KDF_MEM_KIB_MIN`]: a standard vault must open on every device.
pub fn set_kdf_memory_ceiling_kib(kib: u32) {
    KDF_MEM_OVERRIDE_KIB.store(clamp_ceiling(kib), Ordering::Relaxed);
}

/// How much a derivation may take right now. A header inside the
/// [`KdfParams::check`] limits can still ask for more than the device has:
/// on Linux and Android an allocation that size "succeeds" under overcommit
/// and the kernel kills the app once Argon2 touches the pages. So the free
/// memory is read at the moment of the derivation (open, raise, calibration:
/// rare, and the read is cheap), not once at startup when a browser happened
/// to hold it. A vault above the ceiling is [`Error::VaultTooHeavy`], refused
/// before a single block is allocated.
pub fn kdf_memory_ceiling_kib() -> u32 {
    let available = std::fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|text| mem_available_kib(&text));
    effective_ceiling_kib(KDF_MEM_OVERRIDE_KIB.load(Ordering::Relaxed), available)
}

/// The ceiling for an override and the `MemAvailable` of the moment: the
/// lower of the two, or the override alone where there is no `/proc/meminfo`.
pub fn effective_ceiling_kib(override_kib: u32, available_kib: Option<u64>) -> u32 {
    let device = available_kib.map_or(KDF_MEM_KIB_MAX, device_limit_kib);
    clamp_ceiling(override_kib.min(device))
}

/// What a device with `available_kib` free may give a derivation: three
/// quarters, the rest left for the app and the system; within the limits.
pub fn device_limit_kib(available_kib: u64) -> u32 {
    let share = available_kib / 4 * 3;
    clamp_ceiling(u32::try_from(share).unwrap_or(u32::MAX))
}

/// `MemAvailable` in KiB from the text of a `/proc/meminfo`.
pub fn mem_available_kib(text: &str) -> Option<u64> {
    text.lines().find_map(|line| {
        let rest = line.strip_prefix("MemAvailable:")?;
        let mut fields = rest.split_whitespace();
        let value = fields.next()?.parse().ok()?;
        (fields.next() == Some("kB")).then_some(value)
    })
}

fn clamp_ceiling(kib: u32) -> u32 {
    kib.clamp(KDF_MEM_KIB_MIN, KDF_MEM_KIB_MAX)
}

/// The KDF fields of a vault header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KdfParams {
    pub id: u8,
    pub mem_kib: u32,
    pub iters: u32,
    pub par: u8,
}

impl KdfParams {
    /// What every vault below the crypto level is sealed with.
    pub const STANDARD: Self = Self {
        id: VaultHeader::KDF_ARGON2ID,
        mem_kib: KDF_MEM_KIB,
        iters: KDF_ITERS,
        par: KDF_PAR,
    };

    /// The crypto level: an Argon2id cost old builds refuse to touch.
    pub const fn hardened(mem_kib: u32, iters: u32, par: u8) -> Self {
        Self {
            id: VaultHeader::KDF_ARGON2ID_HARDENED,
            mem_kib,
            iters,
            par,
        }
    }

    /// Whether these values may be run at all. A failure is
    /// [`Error::Secret`]: to the user a changed header is one more file that
    /// does not open with this password, and nothing tells them apart.
    pub fn check(&self) -> Result<()> {
        let known = matches!(
            self.id,
            VaultHeader::KDF_ARGON2ID | VaultHeader::KDF_ARGON2ID_HARDENED
        );
        if known
            && (KDF_MEM_KIB_MIN..=KDF_MEM_KIB_MAX).contains(&self.mem_kib)
            && (KDF_ITERS_MIN..=KDF_ITERS_MAX).contains(&self.iters)
            && (KDF_PAR_MIN..=KDF_PAR_MAX).contains(&self.par)
        {
            Ok(())
        } else {
            Err(Error::Secret)
        }
    }
}

/// What a kind of device may spend on the crypto level: rungs from the
/// heaviest down, and a floor taken when none of them fits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KdfProfile {
    rungs: &'static [KdfParams],
    floor: KdfParams,
}

impl KdfProfile {
    pub const DESKTOP: Self = Self {
        rungs: &[],
        floor: KdfParams::hardened(GIB, 3, 4),
    };

    /// `par` stays 1: the argon2 crate runs lanes one after another, so more
    /// of them would help an attacker's GPU and not this phone.
    pub const MOBILE: Self = Self {
        rungs: &[
            KdfParams::hardened(256 * MIB, 3, 1),
            KdfParams::hardened(128 * MIB, 3, 1),
        ],
        floor: KdfParams::hardened(KDF_MEM_KIB_MIN, 3, 1),
    };

    pub const fn for_this_device() -> Self {
        if cfg!(any(target_os = "android", target_os = "ios")) {
            Self::MOBILE
        } else {
            Self::DESKTOP
        }
    }

    /// The heaviest rung `measure` finds fits [`KDF_TARGET_MAX`]; a rung above
    /// `ceiling_kib` is never tried, and one that could not run at all counts
    /// as too heavy. The floor is not measured: there is nothing left to step
    /// down to. A floor above the ceiling (the desktop's gibibyte on a machine
    /// with little free memory) comes down to the ceiling, so the vault this
    /// sets up still opens here.
    pub fn calibrate(
        &self,
        ceiling_kib: u32,
        mut measure: impl FnMut(&KdfParams) -> Result<Duration>,
    ) -> KdfParams {
        self.rungs
            .iter()
            .filter(|rung| rung.mem_kib <= ceiling_kib)
            .find(|rung| measure(rung).is_ok_and(|spent| spent <= KDF_TARGET_MAX))
            .copied()
            .unwrap_or_else(|| self.floor_within(ceiling_kib))
    }

    fn floor_within(&self, ceiling_kib: u32) -> KdfParams {
        if self.floor.mem_kib <= ceiling_kib {
            return self.floor;
        }
        // Whole mebibytes, never below the minimum every vault may carry.
        let mem_kib = (clamp_ceiling(ceiling_kib) / MIB * MIB).max(KDF_MEM_KIB_MIN);
        KdfParams {
            mem_kib,
            ..self.floor
        }
    }

    /// [`Self::calibrate`] against the real KDF under the process ceiling.
    /// Allocates each rung it tries.
    pub fn calibrate_here(&self) -> KdfParams {
        self.calibrate(kdf_memory_ceiling_kib(), time_one_derivation)
    }
}

fn time_one_derivation(kdf: &KdfParams) -> Result<Duration> {
    let started = Instant::now();
    crypto::derive_key(b"calibration", &[0u8; SALT_LEN], kdf)?;
    Ok(started.elapsed())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[test]
    fn the_standard_cost_is_the_one_every_old_file_carries() {
        assert_eq!(
            (
                KdfParams::STANDARD.id,
                KdfParams::STANDARD.mem_kib,
                KdfParams::STANDARD.iters,
                KdfParams::STANDARD.par
            ),
            (0, 64 * 1024, 3, 1)
        );
        KdfParams::STANDARD.check().unwrap();
    }

    #[test]
    fn the_desktop_spends_a_gibibyte_and_is_never_measured() {
        let picked =
            KdfProfile::DESKTOP.calibrate(KDF_MEM_KIB_MAX, |_| panic!("nothing to choose from"));
        assert_eq!(picked, KdfParams::hardened(1024 * 1024, 3, 4));
        picked.check().unwrap();
    }

    #[test]
    fn a_phone_keeps_the_heaviest_rung_that_fits() {
        let picked = KdfProfile::MOBILE.calibrate(KDF_MEM_KIB_MAX, |_| Ok(Duration::from_secs(2)));
        assert_eq!(picked, KdfParams::hardened(256 * 1024, 3, 1));
    }

    #[test]
    fn a_slow_phone_steps_down_a_rung_at_a_time() {
        let tried = RefCell::new(Vec::new());
        let picked = KdfProfile::MOBILE.calibrate(KDF_MEM_KIB_MAX, |rung| {
            tried.borrow_mut().push(rung.mem_kib);
            Ok(Duration::from_secs(if rung.mem_kib > 128 * 1024 {
                5
            } else {
                3
            }))
        });
        assert_eq!(picked.mem_kib, 128 * 1024);
        assert_eq!(*tried.borrow(), [256 * 1024, 128 * 1024]);
    }

    #[test]
    fn exactly_the_limit_still_fits_and_just_over_it_does_not() {
        let at = KdfProfile::MOBILE.calibrate(KDF_MEM_KIB_MAX, |_| Ok(KDF_TARGET_MAX));
        assert_eq!(at.mem_kib, 256 * 1024);
        let over = KdfProfile::MOBILE.calibrate(KDF_MEM_KIB_MAX, |_| {
            Ok(KDF_TARGET_MAX + Duration::from_millis(1))
        });
        assert_eq!(over.mem_kib, 64 * 1024);
    }

    #[test]
    fn the_floor_holds_however_slow_the_phone_is() {
        let picked = KdfProfile::MOBILE.calibrate(KDF_MEM_KIB_MAX, |_| Ok(Duration::from_secs(60)));
        assert_eq!(picked, KdfParams::hardened(64 * 1024, 3, 1));
        picked.check().unwrap();
    }

    #[test]
    fn a_rung_that_cannot_run_is_stepped_over() {
        let picked = KdfProfile::MOBILE.calibrate(KDF_MEM_KIB_MAX, |rung| {
            if rung.mem_kib == 256 * 1024 {
                Err(Error::Crypto("no memory".into()))
            } else {
                Ok(Duration::from_secs(1))
            }
        });
        assert_eq!(picked.mem_kib, 128 * 1024);
    }

    #[test]
    fn a_rung_above_the_memory_ceiling_is_never_tried() {
        let tried = RefCell::new(Vec::new());
        let picked = KdfProfile::MOBILE.calibrate(200 * 1024, |rung| {
            tried.borrow_mut().push(rung.mem_kib);
            Ok(Duration::from_secs(1))
        });
        assert_eq!(picked.mem_kib, 128 * 1024);
        assert_eq!(*tried.borrow(), [128 * 1024]);
    }

    #[test]
    fn a_rung_exactly_at_the_ceiling_is_tried() {
        let picked = KdfProfile::MOBILE.calibrate(256 * 1024, |_| Ok(Duration::from_secs(1)));
        assert_eq!(picked.mem_kib, 256 * 1024);
    }

    #[test]
    fn a_desktop_short_of_memory_comes_down_to_the_ceiling() {
        let picked =
            KdfProfile::DESKTOP.calibrate(300 * 1024 + 17, |_| panic!("nothing to choose from"));
        assert_eq!(picked, KdfParams::hardened(300 * 1024, 3, 4));
        picked.check().unwrap();
        let tiny = KdfProfile::DESKTOP.calibrate(1, |_| panic!("nothing to choose from"));
        assert_eq!(tiny.mem_kib, KDF_MEM_KIB_MIN);
    }

    #[test]
    fn a_device_gives_three_quarters_of_its_available_memory_within_the_limits() {
        assert_eq!(device_limit_kib(400_000), 300_000);
        assert_eq!(device_limit_kib(50_000), KDF_MEM_KIB_MIN);
        assert_eq!(device_limit_kib(0), KDF_MEM_KIB_MIN);
        assert_eq!(device_limit_kib(64_000_000), KDF_MEM_KIB_MAX);
        assert_eq!(device_limit_kib(u64::MAX), KDF_MEM_KIB_MAX);
        // 8 GB machine with a browser open: a 1 GiB vault still opens.
        assert_eq!(device_limit_kib(1_500_000), KDF_MEM_KIB_MAX);
    }

    #[test]
    fn the_ceiling_is_the_lower_of_the_override_and_the_device() {
        assert_eq!(
            effective_ceiling_kib(KDF_MEM_KIB_MAX, Some(400_000)),
            300_000
        );
        assert_eq!(
            effective_ceiling_kib(100 * 1024, Some(64_000_000)),
            100 * 1024
        );
        assert_eq!(effective_ceiling_kib(100 * 1024, Some(400_000)), 100 * 1024);
        assert_eq!(effective_ceiling_kib(100 * 1024, None), 100 * 1024);
        assert_eq!(
            effective_ceiling_kib(KDF_MEM_KIB_MAX, None),
            KDF_MEM_KIB_MAX
        );
        assert_eq!(effective_ceiling_kib(0, Some(0)), KDF_MEM_KIB_MIN);
    }

    #[test]
    fn mem_available_is_read_from_meminfo() {
        let text = "MemTotal:       16000000 kB\nMemFree:  1 kB\nMemAvailable:   600000 kB\n";
        assert_eq!(mem_available_kib(text), Some(600_000));
    }

    #[test]
    fn a_meminfo_without_a_readable_available_line_gives_nothing() {
        for text in [
            "",
            "MemTotal: 16000000 kB\n",
            "MemAvailable: lots kB\n",
            "MemAvailable: 600000 MB\n",
            "MemAvailable:\n",
        ] {
            assert_eq!(mem_available_kib(text), None, "{text:?}");
        }
    }

    #[test]
    fn the_ceiling_never_drops_below_what_a_standard_vault_needs() {
        assert_eq!(clamp_ceiling(0), KDF_MEM_KIB_MIN);
        assert_eq!(clamp_ceiling(u32::MAX), KDF_MEM_KIB_MAX);
        assert_eq!(clamp_ceiling(100 * 1024), 100 * 1024);
    }

    #[test]
    fn every_rung_of_every_profile_is_inside_the_limits() {
        for profile in [KdfProfile::DESKTOP, KdfProfile::MOBILE] {
            for rung in profile.rungs.iter().chain([&profile.floor]) {
                rung.check().unwrap();
                assert_eq!(rung.id, VaultHeader::KDF_ARGON2ID_HARDENED);
            }
        }
    }

    #[test]
    fn values_outside_the_limits_are_refused() {
        let good = KdfParams::hardened(64 * 1024, 3, 1);
        let refused = [
            KdfParams { mem_kib: 0, ..good },
            KdfParams {
                mem_kib: KDF_MEM_KIB_MIN - 1,
                ..good
            },
            KdfParams {
                mem_kib: KDF_MEM_KIB_MAX + 1,
                ..good
            },
            KdfParams {
                mem_kib: u32::MAX,
                ..good
            },
            KdfParams { iters: 0, ..good },
            KdfParams {
                iters: KDF_ITERS_MAX + 1,
                ..good
            },
            KdfParams {
                iters: u32::MAX,
                ..good
            },
            KdfParams { par: 0, ..good },
            KdfParams {
                par: KDF_PAR_MAX + 1,
                ..good
            },
            KdfParams { par: 255, ..good },
            KdfParams {
                id: VaultHeader::KDF_NONE,
                ..good
            },
            KdfParams { id: 9, ..good },
        ];
        for kdf in refused {
            assert!(matches!(kdf.check(), Err(Error::Secret)), "{kdf:?}");
        }
    }
}
