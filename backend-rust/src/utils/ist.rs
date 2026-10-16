//! The product runs on Indian Standard Time only (Asia/Kolkata, UTC+05:30,
//! no DST). Every "which day is this" decision (day filters, day buckets,
//! "today", export dates) goes through this module so the offset lives in
//! exactly one place. Wire formats stay as they are (UTC / RFC 3339 in JSON,
//! `YYYY-MM-DD` date parameters); only boundary computation and display use
//! IST.

use chrono::{DateTime, Duration, FixedOffset, NaiveDate, TimeZone, Utc};

/// IANA name used as the default timezone for recurring rules.
pub const IST_TIMEZONE_NAME: &str = "Asia/Kolkata";

/// Seconds east of UTC (5h30m).
const IST_OFFSET_SECS: i32 = 5 * 3600 + 30 * 60;

/// IST as a fixed offset (India has no DST, so a fixed offset is exact).
pub fn ist() -> FixedOffset {
    FixedOffset::east_opt(IST_OFFSET_SECS).expect("IST offset is in range")
}

/// Converts an instant to IST wall-clock time.
pub fn to_ist(dt: DateTime<Utc>) -> DateTime<FixedOffset> {
    dt.with_timezone(&ist())
}

/// The IST calendar date containing `dt`.
pub fn ist_date_of(dt: DateTime<Utc>) -> NaiveDate {
    to_ist(dt).date_naive()
}

/// Today's date in IST.
pub fn ist_today() -> NaiveDate {
    ist_date_of(Utc::now())
}

/// The IST day `date` as a half-open UTC interval
/// `[00:00 IST of date, 00:00 IST of date + 1)`. Filter with
/// `ts >= start AND ts < end` so the last second of the day is never lost.
pub fn ist_day_bounds_utc(date: NaiveDate) -> (DateTime<Utc>, DateTime<Utc>) {
    let start_local = date.and_hms_opt(0, 0, 0).expect("midnight is valid");
    let start = ist()
        .from_local_datetime(&start_local)
        .single()
        .expect("a fixed offset has no ambiguous local times")
        .with_timezone(&Utc);
    (start, start + Duration::days(1))
}

/// `dd MMM yyyy` in IST, e.g. `16 Oct 2026` (3-letter English month).
pub fn format_date(dt: DateTime<Utc>) -> String {
    to_ist(dt).format("%d %b %Y").to_string()
}

/// `dd MMM yyyy, hh:mm:ss AM` (12-hour, IST) for spreadsheet cells.
pub fn format_datetime(dt: DateTime<Utc>) -> String {
    to_ist(dt).format("%d %b %Y, %I:%M:%S %p").to_string()
}

/// `dd-MMM-yyyy_HHMMSS` (IST) for file names, which cannot contain slashes.
pub fn format_filename_stamp(dt: DateTime<Utc>) -> String {
    to_ist(dt).format("%d-%b-%Y_%H%M%S").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utc(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
    }

    #[test]
    fn day_bounds_are_half_open_ist_day() {
        let d = NaiveDate::from_ymd_opt(2026, 10, 16).unwrap();
        let (s, e) = ist_day_bounds_utc(d);
        assert_eq!(s, utc("2026-10-15T18:30:00Z"));
        assert_eq!(e, utc("2026-10-16T18:30:00Z"));
    }

    #[test]
    fn midnight_straddle_belongs_to_the_right_day() {
        let before = utc("2026-10-15T18:29:59Z");
        let at = utc("2026-10-15T18:30:00Z");
        assert_eq!(
            ist_date_of(before),
            NaiveDate::from_ymd_opt(2026, 10, 15).unwrap()
        );
        assert_eq!(
            ist_date_of(at),
            NaiveDate::from_ymd_opt(2026, 10, 16).unwrap()
        );
        let (s, e) = ist_day_bounds_utc(NaiveDate::from_ymd_opt(2026, 10, 16).unwrap());
        assert!(!(before >= s && before < e));
        assert!(at >= s && at < e);
        // the very last second of the day is included, the next midnight is not
        assert!(utc("2026-10-16T18:29:59Z") < e);
        assert!(!(utc("2026-10-16T18:30:00Z") < e));
    }

    #[test]
    fn bounds_of_consecutive_days_are_contiguous() {
        let d = NaiveDate::from_ymd_opt(2026, 12, 31).unwrap();
        let (_, e) = ist_day_bounds_utc(d);
        let (s2, _) = ist_day_bounds_utc(d.succ_opt().unwrap());
        assert_eq!(e, s2);
    }

    #[test]
    fn formats_are_ist_dd_mmm_yyyy() {
        let dt = utc("2026-10-15T21:49:00Z");
        assert_eq!(format_date(dt), "16 Oct 2026");
        assert_eq!(format_datetime(dt), "16 Oct 2026, 03:19:00 AM");
        assert_eq!(format_filename_stamp(dt), "16-Oct-2026_031900");
    }
}
