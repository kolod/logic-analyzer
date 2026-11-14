//! Civil dates and times from epoch seconds, written the way each language writes them.
//!
//! The time zone is a fixed UTC offset supplied by the caller (research R9); years 1 to 9999 are
//! supported and anything else is not shown.

use crate::locale_data::{DateOrder, LocaleData};

/// The earliest instant that is shown: 0001-01-01T00:00:00 (in the offset's local time).
const MIN_LOCAL: i64 = -62_135_596_800;
/// The latest instant that is shown: 9999-12-31T23:59:59.
const MAX_LOCAL: i64 = 253_402_300_799;
const SECONDS_PER_DAY: i64 = 86_400;

/// A date and a time of day.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Civil {
    pub(crate) year: u32,
    pub(crate) month: u32,
    pub(crate) day: u32,
    pub(crate) hour: u32,
    pub(crate) minute: u32,
    pub(crate) second: u32,
}

/// The proleptic Gregorian date of a day number counted from 1970-01-01 (Howard Hinnant's
/// `civil_from_days`).
#[allow(
    clippy::arithmetic_side_effects,
    reason = "the day number is bounded by the callers to about ±3.7 million, far from overflow"
)]
pub(crate) fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (
        year,
        u32::try_from(month).unwrap_or(1),
        u32::try_from(day).unwrap_or(1),
    )
}

/// The local civil date and time of `epoch_seconds` at `offset_seconds` from UTC; `None` outside
/// the years 1 to 9999.
pub(crate) fn civil_from_epoch(epoch_seconds: i64, offset_seconds: i32) -> Option<Civil> {
    let local = epoch_seconds.checked_add(i64::from(offset_seconds))?;
    if !(MIN_LOCAL..=MAX_LOCAL).contains(&local) {
        return None;
    }
    let days = local.div_euclid(SECONDS_PER_DAY);
    let time = u32::try_from(local.rem_euclid(SECONDS_PER_DAY)).ok()?;
    let (year, month, day) = civil_from_days(days);
    Some(Civil {
        year: u32::try_from(year).ok()?,
        month,
        day,
        hour: time.checked_div(3_600)?,
        minute: time.checked_rem(3_600)?.checked_div(60)?,
        second: time.checked_rem(60)?,
    })
}

fn two_digits_or_plain(value: u32, pad: bool) -> String {
    if pad {
        format!("{value:02}")
    } else {
        value.to_string()
    }
}

/// The date in the numeric short form of the language.
pub(crate) fn format_date(civil: &Civil, data: &LocaleData) -> String {
    let year = format!("{:04}", civil.year);
    let month = two_digits_or_plain(civil.month, data.pad_day_month);
    let day = two_digits_or_plain(civil.day, data.pad_day_month);
    let sep = data.date_separator;
    match data.date_order {
        DateOrder::Mdy => format!("{month}{sep}{day}{sep}{year}"),
        DateOrder::Dmy => format!("{day}{sep}{month}{sep}{year}"),
        DateOrder::Ymd => format!("{year}{sep}{month}{sep}{day}"),
    }
}

/// The time of day with seconds, on the clock of the language.
pub(crate) fn format_time(civil: &Civil, data: &LocaleData) -> String {
    let (minute, second) = (civil.minute, civil.second);
    if data.clock_24 {
        let hour = two_digits_or_plain(civil.hour, data.pad_hour);
        return format!("{hour}:{minute:02}:{second:02}");
    }
    let hour = match civil.hour.checked_rem(12).unwrap_or(0) {
        0 => 12,
        h => h,
    };
    let marker = if civil.hour < 12 { data.am } else { data.pm };
    format!("{hour}:{minute:02}:{second:02} {marker}")
}

/// Date and time joined by one space.
pub(crate) fn format_datetime(civil: &Civil, data: &LocaleData) -> String {
    format!("{} {}", format_date(civil, data), format_time(civil, data))
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::*;
    use crate::language::Language;

    fn is_leap(year: i64) -> bool {
        (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
    }

    fn days_in_month(year: i64, month: u32) -> u32 {
        match month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            _ if is_leap(year) => 29,
            _ => 28,
        }
    }

    #[test]
    fn every_day_from_year_1_to_9999_matches_a_straightforward_calendar() {
        // Day number of 0001-01-01 counted from 1970-01-01.
        let first = MIN_LOCAL / SECONDS_PER_DAY;
        let last = MAX_LOCAL / SECONDS_PER_DAY;
        let (mut year, mut month, mut day) = (1_i64, 1_u32, 1_u32);
        for days in first..=last {
            assert_eq!(
                civil_from_days(days),
                (year, month, day),
                "day number {days}"
            );
            day += 1;
            if day > days_in_month(year, month) {
                day = 1;
                month += 1;
                if month > 12 {
                    month = 1;
                    year += 1;
                }
            }
        }
        assert_eq!((year, month, day), (10_000, 1, 1));
    }

    #[test]
    fn the_range_limits_and_overflow_are_handled() {
        assert!(civil_from_epoch(MIN_LOCAL, 0).is_some());
        assert!(civil_from_epoch(MIN_LOCAL - 1, 0).is_none());
        assert!(civil_from_epoch(MAX_LOCAL, 0).is_some());
        assert!(civil_from_epoch(MAX_LOCAL + 1, 0).is_none());
        // The offset moves the instant across the limit.
        assert!(civil_from_epoch(MAX_LOCAL, 3_600).is_none());
        assert!(civil_from_epoch(MAX_LOCAL + 3_600, -3_600).is_some());
        for epoch in [i64::MIN, i64::MAX, 0, -1] {
            for offset in [i32::MIN, i32::MAX, 0, 64_800, -64_800] {
                let _ = civil_from_epoch(epoch, offset);
            }
        }
    }

    fn at(epoch: i64, offset: i32, language: Language) -> String {
        let civil = civil_from_epoch(epoch, offset).unwrap();
        format_datetime(&civil, LocaleData::for_language(language))
    }

    #[test]
    fn the_fixed_instant_is_written_the_way_each_language_writes_it() {
        let epoch = 1_794_672_309; // 2026-11-14T16:05:09Z
        let table = [
            (Language::En, "11/14/2026 4:05:09 PM"),
            (Language::De, "14.11.2026 16:05:09"),
            (Language::Fr, "14/11/2026 16:05:09"),
            (Language::Es, "14/11/2026 16:05:09"),
            (Language::Ja, "2026/11/14 16:05:09"),
            (Language::Uk, "14.11.2026 16:05:09"),
        ];
        for (language, expected) in table {
            assert_eq!(at(epoch, 0, language), expected, "{language:?}");
        }
        assert_eq!(at(epoch, 7_200, Language::En), "11/14/2026 6:05:09 PM");
        assert_eq!(at(epoch, -18_000, Language::En), "11/14/2026 11:05:09 AM");
        assert_eq!(at(epoch, -18_000, Language::De), "14.11.2026 11:05:09");
    }

    #[test]
    fn small_hours_midnight_noon_and_day_changes() {
        let nine = 1_794_614_400 + 9 * 3_600 + 5 * 60 + 9; // 2026-11-14T09:05:09Z
        assert_eq!(at(nine, 0, Language::En), "11/14/2026 9:05:09 AM");
        assert_eq!(at(nine, 0, Language::De), "14.11.2026 09:05:09");
        assert_eq!(at(nine, 0, Language::Fr), "14/11/2026 09:05:09");
        assert_eq!(at(nine, 0, Language::Es), "14/11/2026 9:05:09");
        assert_eq!(at(nine, 0, Language::Ja), "2026/11/14 9:05:09");
        assert_eq!(at(nine, 0, Language::Uk), "14.11.2026 09:05:09");
        let midnight = 1_794_614_400;
        assert_eq!(at(midnight, 0, Language::En), "11/14/2026 12:00:00 AM");
        assert_eq!(
            at(midnight + 12 * 3_600, 0, Language::En),
            "11/14/2026 12:00:00 PM"
        );
        // 23:30 UTC at +01:00 is 00:30 the next day.
        assert_eq!(
            at(midnight + 84_600, 3_600, Language::De),
            "15.11.2026 00:30:00"
        );
    }

    #[test]
    fn dates_around_the_epoch_leap_days_and_the_ends_of_the_range() {
        assert_eq!(at(-1, 0, Language::En), "12/31/1969 11:59:59 PM");
        assert_eq!(at(0, 0, Language::De), "01.01.1970 00:00:00");
        assert_eq!(at(MIN_LOCAL, 0, Language::En), "1/1/0001 12:00:00 AM");
        assert_eq!(at(MAX_LOCAL, 0, Language::En), "12/31/9999 11:59:59 PM");
        assert_eq!(at(1_709_164_800, 0, Language::En), "2/29/2024 12:00:00 AM");
        assert_eq!(at(1_709_164_800, 0, Language::De), "29.02.2024 00:00:00");
        assert_eq!(at(1_709_164_800, 0, Language::Es), "29/2/2024 0:00:00");
        assert_eq!(at(1_709_164_800, 0, Language::Ja), "2024/02/29 0:00:00");
        assert_eq!(at(4_107_542_400, 0, Language::En), "3/1/2100 12:00:00 AM");
    }
}
