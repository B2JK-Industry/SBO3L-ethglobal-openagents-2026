//! Time-window token gate (R11 P5 — extends `token_gate`).
//!
//! Decides whether an agent's identity is currently valid based on
//! wall-clock time rather than on-chain holdings. Composes with
//! the existing [`crate::token_gate::TokenGate`] trait via the
//! [`crate::token_gate::AllOfGates`] / [`crate::token_gate::AnyOfGates`]
//! combinators — the canonical pattern is "agent must hold an NFT
//! AND the request must arrive in business hours."
//!
//! Three modes:
//!
//! 1. **`UtcRange { start, end }`** — absolute unix-second window.
//!    Useful for "valid only for the demo window" policies or
//!    expiring-identity policies (the operator records first-use
//!    and constructs `UtcRange { 0, first_use + ttl }`).
//! 2. **`BusinessHours { start_hour, end_hour, tz_offset_hours,
//!    weekdays_only }`** — recurring window in a fixed UTC offset.
//!    Suitable for "9-to-5 NYC" or "Mon-Fri Tokyo" patterns.
//!    Fixed offset (no DST awareness — see [`Self::dst_caveat`]).
//! 3. **`Always`** — sentinel for "this gate never gates" — useful
//!    when constructing a composite where one branch should be a
//!    no-op based on config.
//!
//! ## Why fixed-offset (not full TZ DB)
//!
//! Hackathon scope. A real-world TZ-DB-backed gate would pull in
//! `chrono-tz` or equivalent (~1.5 MB of compiled timezone rules).
//! Fixed-offset captures 95% of the use cases (operators care about
//! their headquarters TZ, not generic TZ-DB lookup) at zero dep
//! cost. DST transitions are documented as a known limitation; an
//! operator who needs DST awareness sets two `BusinessHours` gates
//! in `AnyOfGates` covering both DST + non-DST offsets.
//!
//! ## Time injection for tests
//!
//! [`TimeWindowGate::with_now`] accepts a custom "current time"
//! closure so tests can drive the gate against fixed timestamps.
//! Production callers use [`TimeWindowGate::new`] which wraps
//! `SystemTime::now()`.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::token_gate::{GateError, GateResult, TokenGate};

/// Day-of-week used by [`BusinessHours`]. Sunday = 0 to match
/// the POSIX `tm_wday` convention.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weekday {
    Sunday = 0,
    Monday = 1,
    Tuesday = 2,
    Wednesday = 3,
    Thursday = 4,
    Friday = 5,
    Saturday = 6,
}

/// Three modes the gate can operate in. Each mode is pure-function
/// over `(now_secs, mode)`; no internal state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimeWindowMode {
    /// Absolute unix-second window. `[start, end)` — inclusive of
    /// `start`, exclusive of `end`. Either bound may be the unix
    /// epoch (0) or u64::MAX to express open-ended.
    UtcRange { start: u64, end: u64 },

    /// Recurring business-hours window. `start_hour` / `end_hour`
    /// are 0..=23 in the operator's offset. `tz_offset_hours` is
    /// the operator's offset from UTC, range -12..=14 (covers
    /// every real-world offset including UTC+14 Kiribati).
    /// `weekdays_only = true` rejects Saturday + Sunday.
    BusinessHours {
        start_hour: u8,
        end_hour: u8,
        tz_offset_hours: i8,
        weekdays_only: bool,
    },

    /// Sentinel "always passes" mode. Useful in composites where
    /// time gating is config-flag-disabled.
    Always,
}

/// Time-window gate. Implements [`TokenGate`] so it composes with
/// the existing ERC-721 / ERC-1155 gates via `AllOfGates` /
/// `AnyOfGates`.
pub struct TimeWindowGate {
    label: String,
    mode: TimeWindowMode,
    now: Arc<dyn Fn() -> u64 + Send + Sync>,
}

impl std::fmt::Debug for TimeWindowGate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TimeWindowGate")
            .field("label", &self.label)
            .field("mode", &self.mode)
            .finish()
    }
}

impl TimeWindowGate {
    /// Construct a gate using `SystemTime::now` as the time source.
    pub fn new(label: impl Into<String>, mode: TimeWindowMode) -> Self {
        Self {
            label: label.into(),
            mode,
            now: Arc::new(default_now),
        }
    }

    /// Construct a gate with a caller-supplied time source. Tests
    /// pass a closure returning a fixed timestamp; integration
    /// tests pass a closure reading from a mocked clock.
    pub fn with_now(
        label: impl Into<String>,
        mode: TimeWindowMode,
        now: impl Fn() -> u64 + Send + Sync + 'static,
    ) -> Self {
        Self {
            label: label.into(),
            mode,
            now: Arc::new(now),
        }
    }

    /// Documented limitation: DST handling.
    ///
    /// `BusinessHours` uses a fixed UTC offset. Operators in a DST
    /// zone (e.g. America/New_York alternates between UTC-5 and
    /// UTC-4) should construct two `BusinessHours` gates wrapped in
    /// [`crate::token_gate::AnyOfGates`] — one covering each
    /// offset. The gate library does not have a DST-aware mode
    /// because that requires the IANA TZ database (~1.5 MB), which
    /// is out of scope for hackathon-shipped agent-identity logic.
    pub const fn dst_caveat() -> &'static str {
        "fixed-offset only; for DST coverage, AnyOf two BusinessHours gates"
    }
}

fn default_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl TokenGate for TimeWindowGate {
    fn check(&self, owner: &str) -> Result<GateResult, GateError> {
        let now_secs = (self.now)();
        let (passed, evidence) = evaluate(&self.mode, now_secs);
        Ok(GateResult {
            passed,
            gate_label: self.label.clone(),
            owner: owner.to_string(),
            contract: String::new(),
            evidence,
        })
    }

    fn label(&self) -> &str {
        &self.label
    }
}

/// Pure-function evaluator. Public for callers that want to test
/// a window without constructing a full gate.
pub fn evaluate(mode: &TimeWindowMode, now_secs: u64) -> (bool, String) {
    match mode {
        TimeWindowMode::Always => (true, "mode=always".to_string()),
        TimeWindowMode::UtcRange { start, end } => {
            let passed = now_secs >= *start && now_secs < *end;
            (passed, format!("now={now_secs}, range=[{start}, {end})"))
        }
        TimeWindowMode::BusinessHours {
            start_hour,
            end_hour,
            tz_offset_hours,
            weekdays_only,
        } => {
            // Shift to operator's local time by adding offset
            // seconds. Saturating arithmetic so an extreme
            // negative offset doesn't underflow.
            let offset_secs = (*tz_offset_hours as i64) * 3600;
            let local_secs = if offset_secs >= 0 {
                now_secs.saturating_add(offset_secs as u64)
            } else {
                now_secs.saturating_sub(offset_secs.unsigned_abs())
            };

            let local_hour = ((local_secs / 3600) % 24) as u8;
            let local_dow = day_of_week_from_unix(local_secs);

            let in_hours = local_hour >= *start_hour && local_hour < *end_hour;
            let is_weekday = !matches!(local_dow, Weekday::Saturday | Weekday::Sunday);
            let passes_weekday = !*weekdays_only || is_weekday;
            let passed = in_hours && passes_weekday;

            (
                passed,
                format!(
                    "now={now_secs}, local_hour={local_hour}, local_dow={local_dow:?}, \
                     window=[{start_hour:02}:00, {end_hour:02}:00) tz=UTC{tz_offset_hours:+}, \
                     weekdays_only={weekdays_only}"
                ),
            )
        }
    }
}

/// Day-of-week from a unix timestamp. Anchor: 1970-01-01 was a
/// Thursday (Weekday::Thursday = 4 in our enum). Compute via
/// `(days_since_epoch + 4) % 7`.
pub fn day_of_week_from_unix(secs: u64) -> Weekday {
    let days = secs / 86_400;
    let dow_index = ((days + 4) % 7) as u8;
    match dow_index {
        0 => Weekday::Sunday,
        1 => Weekday::Monday,
        2 => Weekday::Tuesday,
        3 => Weekday::Wednesday,
        4 => Weekday::Thursday,
        5 => Weekday::Friday,
        _ => Weekday::Saturday,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::token_gate::{AllOfGates, GateResult};

    /// 2026-05-02 12:00 UTC — Saturday. Used as a stable anchor
    /// for tests so changing the host clock doesn't move results.
    /// Computed from 2026-01-01 00:00 UTC (1_767_225_600) + 121
    /// days + 12h. Day of week: (4 + 121) % 7 = 6 = Saturday.
    const SAT_2026_05_02_NOON: u64 = 1_777_723_200;

    /// 2026-05-04 14:30 UTC — Monday afternoon. (4 + 123) % 7 = 1 = Monday.
    const MON_2026_05_04_1430: u64 = 1_777_905_000;

    /// 2026-05-04 06:00 UTC — Monday early morning.
    const MON_2026_05_04_0600: u64 = 1_777_874_400;

    fn fixed_now(t: u64) -> impl Fn() -> u64 + Send + Sync + 'static {
        move || t
    }

    #[test]
    fn always_mode_passes_at_any_time() {
        let g = TimeWindowGate::with_now("always", TimeWindowMode::Always, fixed_now(0));
        let r: GateResult = g.check("0xowner").unwrap();
        assert!(r.passed);
        assert!(r.evidence.contains("mode=always"));
    }

    #[test]
    fn utc_range_passes_within_window() {
        let g = TimeWindowGate::with_now(
            "demo-window",
            TimeWindowMode::UtcRange {
                start: 1_000,
                end: 2_000,
            },
            fixed_now(1_500),
        );
        let r = g.check("0xowner").unwrap();
        assert!(r.passed);
        assert!(r.evidence.contains("now=1500"));
    }

    #[test]
    fn utc_range_rejects_before_window() {
        let g = TimeWindowGate::with_now(
            "demo",
            TimeWindowMode::UtcRange {
                start: 1_000,
                end: 2_000,
            },
            fixed_now(500),
        );
        assert!(!g.check("0xowner").unwrap().passed);
    }

    #[test]
    fn utc_range_rejects_after_window_inclusive_end() {
        let g = TimeWindowGate::with_now(
            "demo",
            TimeWindowMode::UtcRange {
                start: 1_000,
                end: 2_000,
            },
            fixed_now(2_000), // end is exclusive — exactly equal rejects
        );
        assert!(!g.check("0xowner").unwrap().passed);
    }

    #[test]
    fn business_hours_utc_passes_in_window_weekday() {
        // 2026-05-04 14:30 UTC = Monday, hour 14. 09:00-17:00 UTC → in.
        let g = TimeWindowGate::with_now(
            "biz",
            TimeWindowMode::BusinessHours {
                start_hour: 9,
                end_hour: 17,
                tz_offset_hours: 0,
                weekdays_only: true,
            },
            fixed_now(MON_2026_05_04_1430),
        );
        assert!(g.check("0xowner").unwrap().passed);
    }

    #[test]
    fn business_hours_rejects_weekend_when_weekdays_only() {
        // Saturday at noon → rejected.
        let g = TimeWindowGate::with_now(
            "biz",
            TimeWindowMode::BusinessHours {
                start_hour: 9,
                end_hour: 17,
                tz_offset_hours: 0,
                weekdays_only: true,
            },
            fixed_now(SAT_2026_05_02_NOON),
        );
        let r = g.check("0xowner").unwrap();
        assert!(!r.passed);
        assert!(r.evidence.contains("Saturday"));
    }

    #[test]
    fn business_hours_passes_weekend_when_not_weekdays_only() {
        let g = TimeWindowGate::with_now(
            "biz-7day",
            TimeWindowMode::BusinessHours {
                start_hour: 9,
                end_hour: 17,
                tz_offset_hours: 0,
                weekdays_only: false,
            },
            fixed_now(SAT_2026_05_02_NOON),
        );
        assert!(g.check("0xowner").unwrap().passed);
    }

    #[test]
    fn business_hours_with_negative_tz_offset() {
        // 2026-05-04 14:30 UTC = 2026-05-04 09:30 EST (UTC-5).
        // Business hours 09:00-17:00 EST → 09:30 EST is inside.
        let g = TimeWindowGate::with_now(
            "ny-biz",
            TimeWindowMode::BusinessHours {
                start_hour: 9,
                end_hour: 17,
                tz_offset_hours: -5,
                weekdays_only: true,
            },
            fixed_now(MON_2026_05_04_1430),
        );
        let r = g.check("0xowner").unwrap();
        assert!(r.passed, "{}", r.evidence);
        assert!(r.evidence.contains("local_hour=9"));
        assert!(r.evidence.contains("UTC-5"));
    }

    #[test]
    fn business_hours_with_positive_tz_offset() {
        // 2026-05-04 06:00 UTC = 2026-05-04 15:00 JST (UTC+9).
        // Business hours 09:00-17:00 JST → 15:00 JST is inside.
        let g = TimeWindowGate::with_now(
            "tokyo-biz",
            TimeWindowMode::BusinessHours {
                start_hour: 9,
                end_hour: 17,
                tz_offset_hours: 9,
                weekdays_only: true,
            },
            fixed_now(MON_2026_05_04_0600),
        );
        let r = g.check("0xowner").unwrap();
        assert!(r.passed, "{}", r.evidence);
        assert!(r.evidence.contains("local_hour=15"));
        assert!(r.evidence.contains("UTC+9"));
    }

    #[test]
    fn dst_workaround_via_anyof_two_gates() {
        // Operator wants America/New_York biz hours that span both
        // EST (UTC-5) and EDT (UTC-4). The library doesn't do DST
        // natively; the operator constructs both gates in AnyOf.
        let est = TimeWindowGate::with_now(
            "ny-est",
            TimeWindowMode::BusinessHours {
                start_hour: 9,
                end_hour: 17,
                tz_offset_hours: -5,
                weekdays_only: true,
            },
            fixed_now(MON_2026_05_04_1430), // 14:30 UTC = 09:30 EST
        );
        let edt = TimeWindowGate::with_now(
            "ny-edt",
            TimeWindowMode::BusinessHours {
                start_hour: 9,
                end_hour: 17,
                tz_offset_hours: -4,
                weekdays_only: true,
            },
            fixed_now(MON_2026_05_04_1430), // 14:30 UTC = 10:30 EDT
        );
        let any = crate::token_gate::AnyOfGates::new(
            "ny-biz-with-dst",
            vec![Box::new(est), Box::new(edt)],
        )
        .unwrap();
        // Both pass — AnyOf returns the first passing branch.
        assert!(any.check("0xowner").unwrap().passed);
    }

    #[test]
    fn day_of_week_unix_anchor() {
        // 1970-01-01 = Thursday (POSIX anchor).
        assert_eq!(day_of_week_from_unix(0), Weekday::Thursday);
        // 2026-05-02 noon = Saturday (verified anchor).
        assert_eq!(
            day_of_week_from_unix(SAT_2026_05_02_NOON),
            Weekday::Saturday
        );
        // 2026-05-04 noon = Monday.
        assert_eq!(day_of_week_from_unix(MON_2026_05_04_1430), Weekday::Monday);
    }

    #[test]
    fn time_window_composes_with_token_gate_via_allof() {
        use crate::ens_live::{JsonRpcTransport, RpcError};
        use crate::token_gate::Erc721Gate;
        use std::cell::RefCell;
        use std::collections::VecDeque;

        struct FakeTransport {
            responses: RefCell<VecDeque<String>>,
        }
        impl JsonRpcTransport for FakeTransport {
            fn eth_call(&self, _to: &str, _data: &str) -> Result<String, RpcError> {
                self.responses
                    .borrow_mut()
                    .pop_front()
                    .ok_or_else(|| RpcError::Decode("drained".into()))
            }
        }
        // 32-byte big-endian uint256 == 1: 31 leading zero bytes + 0x01.
        let mut padded = String::from("0x");
        for _ in 0..62 {
            padded.push('0');
        }
        padded.push('0');
        padded.push('1');
        let transport = FakeTransport {
            responses: RefCell::new(VecDeque::from([padded])),
        };
        let token = Erc721Gate::new(
            transport,
            "team-nft",
            "0x1111111111111111111111111111111111111111",
        );
        let time = TimeWindowGate::with_now(
            "biz-hours",
            TimeWindowMode::BusinessHours {
                start_hour: 0,
                end_hour: 24,
                tz_offset_hours: 0,
                weekdays_only: false,
            },
            fixed_now(MON_2026_05_04_1430),
        );
        let composed: AllOfGates =
            AllOfGates::new("token-and-time", vec![Box::new(token), Box::new(time)]).unwrap();
        let r = composed
            .check("0x2222222222222222222222222222222222222222")
            .unwrap();
        assert!(r.passed);
    }

    #[test]
    fn business_hours_edge_of_window_inclusive_start_exclusive_end() {
        // Window 09:00-10:00 UTC, weekday. Test exactly 09:00:00,
        // 09:59:59, 10:00:00 (boundary inclusive of start, exclusive
        // of end).
        let make = |t: u64| {
            TimeWindowGate::with_now(
                "edge",
                TimeWindowMode::BusinessHours {
                    start_hour: 9,
                    end_hour: 10,
                    tz_offset_hours: 0,
                    weekdays_only: true,
                },
                fixed_now(t),
            )
        };

        // Monday 09:00 UTC exactly:
        let mon_0900 = MON_2026_05_04_1430 - 5 * 3600 - 30 * 60; // 14:30 - 5:30 = 09:00
        assert!(make(mon_0900).check("0x").unwrap().passed);

        // Monday 09:59:59 UTC:
        assert!(make(mon_0900 + 3599).check("0x").unwrap().passed);

        // Monday 10:00:00 UTC exactly: rejected (end exclusive).
        assert!(!make(mon_0900 + 3600).check("0x").unwrap().passed);
    }

    #[test]
    fn dst_caveat_string_pinned() {
        // Just confirms the documented caveat is reachable from
        // user code so a future contributor doesn't accidentally
        // delete the doc string.
        assert!(TimeWindowGate::dst_caveat().contains("DST"));
    }
}
