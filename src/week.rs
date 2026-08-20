use std::time::{SystemTime, UNIX_EPOCH};

// unix time starts at 00:00:00 UTC on 1 January 1970

pub type UnixTime = i64;

pub const DAY_SECS: UnixTime = 24 * 60 * 60;
pub const WEEKDAY_OFFSET_7_DAY_WEEK: i64 = 3; // Unix epoch started on Thursday; weekdays are 0-indexed
pub const WEEKDAY_OFFSET_8_DAY_WEEK: i64 = 0; // Unix epoch started on Monday in an 8-day week; weekdays are 0-indexed

/// Get current Unix time
pub fn now() -> UnixTime {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Set your computer time to after 1970")
        .as_secs() as i64
}

/// Get Unix time for "today" (aka today midnight)
fn today_tz(tz: i64) -> UnixTime {
    return day_num_tz(tz) * DAY_SECS - tz;
}

/// Get current day number (aka number of days since Unix epoch)
fn day_num() -> i64 {
    return now() / DAY_SECS;
}

/// Get current day number (aka number of days since Unix epoch)
fn day_num_tz(tz: i64) -> i64 {
    return (now() + tz) / DAY_SECS;
}

/// Given the length of the week, return what day of the week it is (0-indexed from Monday)
fn weekday_tz(week_len: i64, tz: i64) -> i64 {
    let offset = if week_len == 7 { WEEKDAY_OFFSET_7_DAY_WEEK } else { assert_eq!(week_len, 8); WEEKDAY_OFFSET_8_DAY_WEEK };
    return (day_num_tz(tz) + offset) % week_len;
}

/// Same as `is_logsday`, but use `tz` (time zone), which is a time difference (in seconds) between user time zone and UTC
pub fn is_logsday_tz(week_len: i64, logsday_weekday: i64, tz: i64) -> bool {
    return weekday_tz(week_len, tz) == logsday_weekday;
}

pub fn time_left_today_tz(tz: i64) -> i64 {
    return today_tz(tz) + DAY_SECS - now();
}

pub fn time_until_next_logsday_tz(week_len: i64, logsday_weekday: i64, tz: i64) -> UnixTime {
    let mut days = logsday_weekday - weekday_tz(week_len, tz);
    if days < 0 { days += week_len; }
    return days * DAY_SECS - (now() - today_tz(tz));
}

/// return the number of days that have passed since the given day (or some time in the day)
pub fn days_since(time: UnixTime) -> i64 {
    let today = day_num();
    let given_day = time / DAY_SECS;
    assert!(given_day <= today);
    return today - given_day;
}

pub fn ensure_tz_valid(tz: i64) -> i64 {
    if tz < -12 * 60 * 60 || tz > 12 * 60 * 60 {
        return 0; // invalid local_today, so just return utc time
    }
    return tz;
}