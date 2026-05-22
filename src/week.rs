use std::time::{SystemTime, UNIX_EPOCH};

// unix time starts at 00:00:00 UTC on 1 January 1970

pub type UnixTime = i64;

const DAY_SECS: UnixTime = 24 * 60 * 60;
const WEEKDAY_OFFSET_7_DAY_WEEK: i64 = 3; // Unix epoch started on Thursday; weekdays are 0-indexed
const WEEKDAY_OFFSET_8_DAY_WEEK: i64 = 0; // Unix epoch started on Monday in an 8-day week; weekdays are 0-indexed

// return current time
pub fn now() -> UnixTime {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Set your computer time to after 1970")
        .as_secs() as i64
}

// return today's timestamp (today midnight)
pub fn today() -> UnixTime {
    return (now() / DAY_SECS) * DAY_SECS;
}

// return what day it is since Unix epoch (number of days passed)
pub fn day_num() -> i64 {
    return now() / DAY_SECS;
}

// given the length of the week, return what day of the week it is (0-indexed from Monday)
pub fn weekday(week_len: i64) -> i64 {
    if week_len == 7 {
        return (day_num() + WEEKDAY_OFFSET_7_DAY_WEEK) % 7;
    } else {
        assert_eq!(week_len, 8);
        return (day_num() + WEEKDAY_OFFSET_8_DAY_WEEK) % 8;
    }
}

// return true if today is logsday for the given settings
pub fn is_logsday(week_len: i64, logsday_weekday: i64) -> bool {
    return weekday(week_len) == logsday_weekday;
}

// return the number of days that have passed since the given day (or some time in the day)
pub fn days_since(time: UnixTime) -> i64 {
    let today = day_num();
    let given_day = time / DAY_SECS;
    assert!(given_day <= today);
    return today - given_day;
}