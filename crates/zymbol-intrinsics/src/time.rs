//! Civil-calendar arithmetic for `std/time`.
//!
//! Everything here is pure and works on primitives, which is the point: the
//! tree-walker and the register VM each keep their own `Value` type, so a
//! shared table of `(name, arity)` is not enough to stop them drifting — the
//! *answers* have to come from one place. `std/term` is duplicated between the
//! two engines and stays in step by inspection; a calendar cannot be kept in
//! step by inspection.
//!
//! The one impure pair — [`now_ms`] and [`local_offset_minutes_at`] — lives
//! here for the same reason: a clock read is primitive (it returns an `i64`),
//! and putting it anywhere else would mean writing the local-zone lookup twice.
//!
//! # The instant and the calendar are different things
//!
//! An epoch is an **instant**: milliseconds since 1970-01-01T00:00:00Z, always
//! UTC, with no zone attached. A year/month/day is a **calendar reading** of an
//! instant, and there is no such reading without saying in which zone. Every
//! function that produces or consumes civil fields therefore takes an offset in
//! minutes east of UTC.
//!
//! Milliseconds and not nanoseconds: an epoch in nanoseconds is ~1.7e18 and the
//! integer is ±(2⁵³−1) ≈ 9.007e15, so it would not fit. In milliseconds it is
//! ~1.7e12, with room until the year 287396.
//!
//! # Below a day it is duration, from a day up it is calendar
//!
//! The rule that governs [`add`] and [`diff`]. A minute is always 60 000 ms; a
//! *day* is not always 86 400 000, because a zone that observes daylight saving
//! has one 23-hour day and one 25-hour day every year. "Tomorrow at the same
//! time" and "24 hours from now" are different questions, and only one of them
//! is what a person means by "a day".

// ── The civil calendar ───────────────────────────────────────────────────────
//
// Howard Hinnant's `days_from_civil` / `civil_from_days` (public domain, the
// algorithms behind C++20's <chrono>). Exact over the proleptic Gregorian
// calendar for any year that fits, with no tables and no leap-year special
// casing beyond the era arithmetic.

/// Days since 1970-01-01 for a civil date. `month` is 1–12, `day` 1–31.
pub fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400; // [0, 399]
    let mp = (month + 9) % 12; // March = 0
    let doy = (153 * mp + 2) / 5 + day - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146_097 + doe - 719_468
}

/// The civil date `(year, month, day)` for a day count since 1970-01-01.
pub fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11], March = 0
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Days in a month, honouring leap years.
pub fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap(year) => 29,
        2 => 28,
        _ => 0,
    }
}

/// Whether `year` is a Gregorian leap year.
pub fn is_leap(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

// ── Units of time ────────────────────────────────────────────────────────────

pub const MS_PER_SECOND: i64 = 1_000;
pub const MS_PER_MINUTE: i64 = 60 * MS_PER_SECOND;
pub const MS_PER_HOUR: i64 = 60 * MS_PER_MINUTE;
pub const MS_PER_DAY: i64 = 24 * MS_PER_HOUR;

/// The largest integer the language carries, ±(2⁵³−1). Kept here rather than
/// imported so this crate stays dependency-free apart from the clock.
const MAX_SAFE: i64 = 9_007_199_254_740_991;

/// What a unit name means to [`add`] and [`diff`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    /// A fixed number of milliseconds — below a day, time is duration.
    Duration(i64),
    /// Whole days on the calendar (1 or 7 of them).
    Days(i64),
    /// Whole months on the calendar (1 or 12 of them).
    Months(i64),
}

/// The unit a name stands for. One spelling each, in full: a stdlib that takes
/// both `"second"` and `"s"` is two ways to write one thing.
pub fn parse_unit(name: &str) -> Result<Unit, String> {
    Ok(match name {
        "millisecond" => Unit::Duration(1),
        "second" => Unit::Duration(MS_PER_SECOND),
        "minute" => Unit::Duration(MS_PER_MINUTE),
        "hour" => Unit::Duration(MS_PER_HOUR),
        "day" => Unit::Days(1),
        "week" => Unit::Days(7),
        "month" => Unit::Months(1),
        "year" => Unit::Months(12),
        other => {
            return Err(format!(
                "unknown unit '{other}': one of millisecond, second, minute, hour, day, week, month, year"
            ))
        }
    })
}

// ── Zones ────────────────────────────────────────────────────────────────────

/// Which reading of an instant a call asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zone {
    /// The default. An instant read as UTC needs nothing from the machine and
    /// gives the same answer everywhere, which is what a stored date wants.
    Utc,
    /// The machine's own zone at that instant — daylight saving included, so
    /// the same wall clock the person sitting at it sees.
    Local,
    /// A fixed offset in minutes east of UTC, written `+HHMM` or `-HHMM`.
    Fixed(i64),
}

/// Read a zone argument: `"UTC"`, `"local"`, or `±HHMM` such as `+1000`.
///
/// `±HH:MM` is deliberately not a second spelling; `±HHMM` is what `date +%z`
/// prints and what `%z` writes back.
pub fn parse_zone(spec: &str) -> Result<Zone, String> {
    match spec {
        "UTC" => return Ok(Zone::Utc),
        "local" => return Ok(Zone::Local),
        _ => {}
    }
    let bytes = spec.as_bytes();
    let sign = match bytes.first() {
        Some(b'+') => 1,
        Some(b'-') => -1,
        _ => return Err(bad_zone(spec)),
    };
    if bytes.len() != 5 || !bytes[1..].iter().all(u8::is_ascii_digit) {
        return Err(bad_zone(spec));
    }
    let hours: i64 = spec[1..3].parse().map_err(|_| bad_zone(spec))?;
    let minutes: i64 = spec[3..5].parse().map_err(|_| bad_zone(spec))?;
    if hours > 23 || minutes > 59 {
        return Err(bad_zone(spec));
    }
    Ok(Zone::Fixed(sign * (hours * 60 + minutes)))
}

fn bad_zone(spec: &str) -> String {
    format!("unknown zone '{spec}': use \"UTC\", \"local\", or an offset like \"+1000\" or \"-0400\"")
}

/// The offset in minutes a zone means at a given instant.
///
/// The instant matters: a zone that observes daylight saving is two different
/// offsets in the same year, so asking "what is the local offset" without
/// saying when has no single answer.
pub fn offset_of(zone: Zone, epoch_ms: i64) -> Result<i64, String> {
    match zone {
        Zone::Utc => Ok(0),
        Zone::Fixed(m) => Ok(m),
        Zone::Local => local_offset_minutes_at(epoch_ms),
    }
}

// ── Reading an instant ───────────────────────────────────────────────────────

/// An instant read on a calendar, in some zone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Parts {
    pub year: i64,
    pub month: i64,
    pub day: i64,
    pub hour: i64,
    pub minute: i64,
    pub second: i64,
    pub millisecond: i64,
    /// 1 = Monday … 7 = Sunday, as ISO 8601 numbers them.
    pub weekday: i64,
    /// Minutes east of UTC for this reading.
    pub offset: i64,
}

/// Read `epoch_ms` on the calendar, at `offset_min` minutes east of UTC.
pub fn parts_at(epoch_ms: i64, offset_min: i64) -> Parts {
    let shifted = epoch_ms + offset_min * MS_PER_MINUTE;
    // Floor division, so instants before 1970 land on the right day.
    let days = shifted.div_euclid(MS_PER_DAY);
    let in_day = shifted.rem_euclid(MS_PER_DAY);
    let (year, month, day) = civil_from_days(days);
    Parts {
        year,
        month,
        day,
        hour: in_day / MS_PER_HOUR,
        minute: in_day / MS_PER_MINUTE % 60,
        second: in_day / MS_PER_SECOND % 60,
        millisecond: in_day % MS_PER_SECOND,
        // 1970-01-01 was a Thursday, so day 0 is weekday 4.
        weekday: (days + 3).rem_euclid(7) + 1,
        offset: offset_min,
    }
}

/// The instant a civil reading names, at `offset_min` minutes east of UTC.
///
/// Every field is checked: a wrong *type* is the caller's bug and belongs to
/// the engine, but 2026-13-01 is data, and data that is out of range gets a
/// soft error rather than a silently rolled-over date.
#[allow(clippy::too_many_arguments)]
pub fn epoch_from_civil(
    year: i64,
    month: i64,
    day: i64,
    hour: i64,
    minute: i64,
    second: i64,
    millisecond: i64,
    offset_min: i64,
) -> Result<i64, String> {
    if !(-9999..=9999).contains(&year) {
        return Err(format!("year {year} is outside -9999..9999"));
    }
    if !(1..=12).contains(&month) {
        return Err(format!("month {month} is outside 1..12"));
    }
    let last = days_in_month(year, month);
    if !(1..=last).contains(&day) {
        return Err(format!("day {day} is outside 1..{last} for {year}-{month:02}"));
    }
    if !(0..=23).contains(&hour) {
        return Err(format!("hour {hour} is outside 0..23"));
    }
    if !(0..=59).contains(&minute) {
        return Err(format!("minute {minute} is outside 0..59"));
    }
    if !(0..=59).contains(&second) {
        return Err(format!("second {second} is outside 0..59"));
    }
    if !(0..=999).contains(&millisecond) {
        return Err(format!("millisecond {millisecond} is outside 0..999"));
    }
    let ms = days_from_civil(year, month, day) * MS_PER_DAY
        + hour * MS_PER_HOUR
        + minute * MS_PER_MINUTE
        + second * MS_PER_SECOND
        + millisecond
        - offset_min * MS_PER_MINUTE;
    guard(ms)
}

/// The instant a civil reading names in a zone, resolving the zone against the
/// instant it is about to produce.
///
/// A local reading is circular by nature — the offset depends on the instant
/// and the instant depends on the offset — so it is resolved twice: once with
/// the offset that holds at the UTC-shaped guess, then again at the corrected
/// instant. Around a daylight-saving change the second pass is what moves the
/// answer onto the right side of the jump.
#[allow(clippy::too_many_arguments)]
pub fn epoch_from_civil_in(
    year: i64,
    month: i64,
    day: i64,
    hour: i64,
    minute: i64,
    second: i64,
    millisecond: i64,
    zone: Zone,
) -> Result<i64, String> {
    let mut offset = match zone {
        Zone::Utc => 0,
        Zone::Fixed(m) => m,
        Zone::Local => {
            let guess = epoch_from_civil(year, month, day, hour, minute, second, millisecond, 0)?;
            local_offset_minutes_at(guess)?
        }
    };
    let mut ms = epoch_from_civil(year, month, day, hour, minute, second, millisecond, offset)?;
    if zone == Zone::Local {
        let corrected = local_offset_minutes_at(ms)?;
        if corrected != offset {
            offset = corrected;
            ms = epoch_from_civil(year, month, day, hour, minute, second, millisecond, offset)?;
        }
    }
    Ok(ms)
}

// ── Arithmetic ───────────────────────────────────────────────────────────────

/// `count` units after `epoch_ms`, read in `zone`.
///
/// Calendar units land on the same wall clock: one month after 15:00 on the
/// 31st of January is 15:00 on the 28th of February, because there is no 31st.
/// Clamping is what every calendar does — the alternative is rolling into
/// March, which turns "next month" into "the month after next".
pub fn add(epoch_ms: i64, count: i64, unit: Unit, zone: Zone) -> Result<i64, String> {
    match unit {
        Unit::Duration(step) => guard(
            epoch_ms
                .checked_add(count.checked_mul(step).ok_or_else(overflow)?)
                .ok_or_else(overflow)?,
        ),
        Unit::Days(per) => {
            let offset = offset_of(zone, epoch_ms)?;
            let p = parts_at(epoch_ms, offset);
            let days = days_from_civil(p.year, p.month, p.day)
                .checked_add(count.checked_mul(per).ok_or_else(overflow)?)
                .ok_or_else(overflow)?;
            let (year, month, day) = civil_from_days(days);
            rebuild(year, month, day, &p, zone)
        }
        Unit::Months(per) => {
            let offset = offset_of(zone, epoch_ms)?;
            let p = parts_at(epoch_ms, offset);
            let total = (p.year * 12 + (p.month - 1))
                .checked_add(count.checked_mul(per).ok_or_else(overflow)?)
                .ok_or_else(overflow)?;
            let year = total.div_euclid(12);
            let month = total.rem_euclid(12) + 1;
            let day = p.day.min(days_in_month(year, month));
            rebuild(year, month, day, &p, zone)
        }
    }
}

/// Rebuild an instant from a moved date, keeping the original wall clock.
fn rebuild(year: i64, month: i64, day: i64, p: &Parts, zone: Zone) -> Result<i64, String> {
    epoch_from_civil_in(
        year,
        month,
        day,
        p.hour,
        p.minute,
        p.second,
        p.millisecond,
        zone,
    )
}

/// Whole units from `b` to `a`, read in `zone`. Negative when `a` is earlier.
///
/// Below a day this is elapsed duration, truncated toward zero. From a day up
/// it is calendar distance: `diff` between two instants one minute apart across
/// midnight is one day, because they are on different days — and across a
/// daylight-saving change a day is still a day, which dividing by 86 400 000
/// would get wrong once a year.
pub fn diff(a: i64, b: i64, unit: Unit, zone: Zone) -> Result<i64, String> {
    match unit {
        Unit::Duration(step) => Ok((a - b) / step),
        Unit::Days(per) => {
            let pa = parts_at(a, offset_of(zone, a)?);
            let pb = parts_at(b, offset_of(zone, b)?);
            let days = days_from_civil(pa.year, pa.month, pa.day)
                - days_from_civil(pb.year, pb.month, pb.day);
            Ok(days / per)
        }
        Unit::Months(per) => {
            let pa = parts_at(a, offset_of(zone, a)?);
            let pb = parts_at(b, offset_of(zone, b)?);
            let mut months = (pa.year - pb.year) * 12 + (pa.month - pb.month);
            // A whole month has not passed until the day-of-month is reached.
            if months > 0 && day_time(&pa) < day_time(&pb) {
                months -= 1;
            } else if months < 0 && day_time(&pa) > day_time(&pb) {
                months += 1;
            }
            Ok(months / per)
        }
    }
}

/// Where a reading sits inside its month, for comparing two of them.
fn day_time(p: &Parts) -> (i64, i64, i64, i64, i64) {
    (p.day, p.hour, p.minute, p.second, p.millisecond)
}

// ── Formatting ───────────────────────────────────────────────────────────────

/// Render an instant with a POSIX `date`-derived pattern.
///
/// The digits are **always ASCII**, whatever numeral mode the program is in.
/// A date is the one piece of text a program writes for a machine to read
/// back — a filename, a database column, an ISO 8601 field — and `२०२६-०८-२३`
/// is not ISO 8601. Text for a person is built from [`parts_at`] instead, whose
/// numbers print in whatever script the mode selects.
pub fn format(epoch_ms: i64, pattern: &str, offset_min: i64) -> Result<String, String> {
    let p = parts_at(epoch_ms, offset_min);
    let mut out = String::with_capacity(pattern.len() + 8);
    let mut chars = pattern.chars();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('Y') => out.push_str(&year_text(p.year)),
            Some('m') => pad2(&mut out, p.month),
            Some('d') => pad2(&mut out, p.day),
            Some('H') => pad2(&mut out, p.hour),
            Some('M') => pad2(&mut out, p.minute),
            Some('S') => pad2(&mut out, p.second),
            Some('L') => out.push_str(&format!("{:03}", p.millisecond)),
            Some('j') => out.push_str(&format!("{:03}", day_of_year(&p))),
            Some('u') => out.push_str(&p.weekday.to_string()),
            Some('z') => out.push_str(&offset_text(p.offset)),
            Some('F') => {
                out.push_str(&year_text(p.year));
                out.push('-');
                pad2(&mut out, p.month);
                out.push('-');
                pad2(&mut out, p.day);
            }
            Some('T') => {
                pad2(&mut out, p.hour);
                out.push(':');
                pad2(&mut out, p.minute);
                out.push(':');
                pad2(&mut out, p.second);
            }
            Some('%') => out.push('%'),
            Some(other) => {
                return Err(format!(
                    "unknown pattern '%{other}': one of %Y %m %d %H %M %S %L %j %u %z %F %T %%"
                ))
            }
            None => return Err("pattern ends in a lone '%'".to_string()),
        }
    }
    Ok(out)
}

/// `%F` on its own — the ISO 8601 date, which is what most callers want.
pub fn iso_date(epoch_ms: i64, offset_min: i64) -> String {
    let p = parts_at(epoch_ms, offset_min);
    let mut out = year_text(p.year);
    out.push('-');
    pad2(&mut out, p.month);
    out.push('-');
    pad2(&mut out, p.day);
    out
}

fn pad2(out: &mut String, n: i64) {
    if n < 10 {
        out.push('0');
    }
    out.push_str(&n.to_string());
}

/// Four digits, with a leading `-` for years before 1, as ISO 8601 does.
fn year_text(year: i64) -> String {
    if year < 0 {
        format!("-{:04}", -year)
    } else {
        format!("{year:04}")
    }
}

fn offset_text(minutes: i64) -> String {
    let sign = if minutes < 0 { '-' } else { '+' };
    let abs = minutes.abs();
    format!("{sign}{:02}{:02}", abs / 60, abs % 60)
}

fn day_of_year(p: &Parts) -> i64 {
    days_from_civil(p.year, p.month, p.day) - days_from_civil(p.year, 1, 1) + 1
}

// ── Range ────────────────────────────────────────────────────────────────────

fn overflow() -> String {
    "the result leaves the integer range, ±(2^53 − 1)".to_string()
}

/// Keep every produced instant inside the language's integer range, so an
/// overflow is a soft `##Time` rather than a number that wraps.
fn guard(ms: i64) -> Result<i64, String> {
    if ms.abs() > MAX_SAFE {
        Err(overflow())
    } else {
        Ok(ms)
    }
}

// ── The clock ────────────────────────────────────────────────────────────────

/// Now, as milliseconds since the epoch.
pub fn now_ms() -> i64 {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => d.as_millis() as i64,
        // Before 1970 on a machine whose clock is set that way.
        Err(e) => -(e.duration().as_millis() as i64),
    }
}

/// The machine's offset from UTC, in minutes, at a given instant.
///
/// Fails rather than guessing. The `time` crate refuses to read the zone from a
/// process that has other threads running, because another thread may be in
/// `setenv` at that moment and the lookup reads `TZ` — a real data race that
/// has produced real crashes. A wrong date is worse than a caught error, and
/// `"UTC"` and a fixed offset never take this path at all.
pub fn local_offset_minutes_at(epoch_ms: i64) -> Result<i64, String> {
    let seconds = epoch_ms.div_euclid(1000);
    let at = time::OffsetDateTime::from_unix_timestamp(seconds)
        .map_err(|_| format!("instant {epoch_ms} is outside the range a zone can be read at"))?;
    time::UtcOffset::local_offset_at(at)
        .map(|o| i64::from(o.whole_minutes()))
        .map_err(|_| {
            "the machine's local zone could not be read; use \"UTC\" or an explicit offset like \"-0400\""
                .to_string()
        })
}

// ── What `std/time` exports ──────────────────────────────────────────────────
//
// One layer above the calendar and one below the engines. Everything an engine
// does is unbox its own `Value` into these primitives and box the answer, so a
// difference between the tree-walker and the register VM can only be a
// difference in *unboxing* — never in what a month is.
//
// The zone is the last argument everywhere and always optional, defaulting to
// UTC: an instant read as UTC needs nothing from the machine and gives the same
// answer on every one of them, which is what a stored date wants.

/// Resolve an optional zone argument, defaulting to UTC.
fn zone_or_utc(spec: Option<&str>) -> Result<Zone, String> {
    match spec {
        None => Ok(Zone::Utc),
        Some(s) => parse_zone(s),
    }
}

/// `time::today([zone])` — the current date as `YYYY-MM-DD`.
pub fn call_today(zone: Option<&str>) -> Result<String, String> {
    let now = now_ms();
    Ok(iso_date(now, offset_of(zone_or_utc(zone)?, now)?))
}

/// `time::parts(epoch [, zone])` — an instant read on the calendar.
pub fn call_parts(epoch_ms: i64, zone: Option<&str>) -> Result<Parts, String> {
    Ok(parts_at(epoch_ms, offset_of(zone_or_utc(zone)?, epoch_ms)?))
}

/// `time::of(year, month, day [, hour, minute, second] [, zone])`.
///
/// Three numbers or six; anything else is a call the caller got wrong. The
/// missing clock fields are midnight, because a date without a time is the
/// start of that day in every calendar there is.
pub fn call_of(fields: &[i64], zone: Option<&str>) -> Result<i64, String> {
    let (y, mo, d, h, mi, s) = match fields {
        [y, mo, d] => (*y, *mo, *d, 0, 0, 0),
        [y, mo, d, h, mi, s] => (*y, *mo, *d, *h, *mi, *s),
        _ => {
            return Err(format!(
                "expected 3 numbers (year, month, day) or 6 (…, hour, minute, second), got {}",
                fields.len()
            ))
        }
    };
    epoch_from_civil_in(y, mo, d, h, mi, s, 0, zone_or_utc(zone)?)
}

/// `time::format(epoch, pattern [, zone])`.
pub fn call_format(epoch_ms: i64, pattern: &str, zone: Option<&str>) -> Result<String, String> {
    format(epoch_ms, pattern, offset_of(zone_or_utc(zone)?, epoch_ms)?)
}

/// `time::add(epoch, count, unit [, zone])`.
pub fn call_add(epoch_ms: i64, count: i64, unit: &str, zone: Option<&str>) -> Result<i64, String> {
    add(epoch_ms, count, parse_unit(unit)?, zone_or_utc(zone)?)
}

/// `time::diff(a, b, unit [, zone])` — whole units from `b` to `a`.
pub fn call_diff(a: i64, b: i64, unit: &str, zone: Option<&str>) -> Result<i64, String> {
    diff(a, b, parse_unit(unit)?, zone_or_utc(zone)?)
}

/// The dictionary `parts` answers with, as `(key, value)` in the order the
/// engines must build it. Insertion order is a dictionary's iteration order, so
/// the two engines have to agree on it here rather than each choosing.
pub fn parts_fields(p: &Parts) -> [(&'static str, i64); 9] {
    [
        ("year", p.year),
        ("month", p.month),
        ("day", p.day),
        ("hour", p.hour),
        ("minute", p.minute),
        ("second", p.second),
        ("millisecond", p.millisecond),
        ("weekday", p.weekday),
        ("offset", p.offset),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Round-tripping every day over four centuries is the only test that
    /// proves the era arithmetic rather than sampling it: 1600–2000 covers
    /// every leap rule, including 1700/1800/1900 (not leap) and 1600/2000 (leap).
    #[test]
    fn civil_round_trips_over_four_centuries() {
        let mut days = days_from_civil(1600, 1, 1);
        let end = days_from_civil(2000, 12, 31);
        let (mut y, mut m, mut d) = (1600, 1, 1);
        while days <= end {
            assert_eq!(civil_from_days(days), (y, m, d), "day {days}");
            assert_eq!(days_from_civil(y, m, d), days);
            days += 1;
            d += 1;
            if d > days_in_month(y, m) {
                d = 1;
                m += 1;
                if m > 12 {
                    m = 1;
                    y += 1;
                }
            }
        }
    }

    #[test]
    fn the_epoch_is_a_thursday() {
        let p = parts_at(0, 0);
        assert_eq!((p.year, p.month, p.day), (1970, 1, 1));
        assert_eq!(p.weekday, 4); // Thursday, ISO numbering
        assert_eq!((p.hour, p.minute, p.second, p.millisecond), (0, 0, 0, 0));
    }

    #[test]
    fn before_the_epoch_floors_instead_of_truncating() {
        // One millisecond before 1970 is 1969-12-31T23:59:59.999, not 1970-01-01.
        let p = parts_at(-1, 0);
        assert_eq!((p.year, p.month, p.day), (1969, 12, 31));
        assert_eq!((p.hour, p.minute, p.second, p.millisecond), (23, 59, 59, 999));
    }

    #[test]
    fn an_offset_moves_the_reading_not_the_instant() {
        // 2026-08-23T02:00Z read at -0400 is still the 22nd.
        let utc = epoch_from_civil(2026, 8, 23, 2, 0, 0, 0, 0).unwrap();
        let p = parts_at(utc, -240);
        assert_eq!((p.year, p.month, p.day, p.hour), (2026, 8, 22, 22));
        assert_eq!(p.offset, -240);
        // and building it back from that reading gives the same instant
        assert_eq!(epoch_from_civil(2026, 8, 22, 22, 0, 0, 0, -240).unwrap(), utc);
    }

    #[test]
    fn out_of_range_civil_fields_are_errors_not_rollovers() {
        assert!(epoch_from_civil(2026, 13, 1, 0, 0, 0, 0, 0).is_err());
        assert!(epoch_from_civil(2026, 2, 29, 0, 0, 0, 0, 0).is_err()); // 2026 is not leap
        assert!(epoch_from_civil(2024, 2, 29, 0, 0, 0, 0, 0).is_ok()); // 2024 is
        assert!(epoch_from_civil(2026, 1, 1, 24, 0, 0, 0, 0).is_err());
        assert!(epoch_from_civil(2026, 1, 1, 0, 0, 0, 1000, 0).is_err());
    }

    #[test]
    fn zones_read_the_three_forms_and_nothing_else() {
        assert_eq!(parse_zone("UTC").unwrap(), Zone::Utc);
        assert_eq!(parse_zone("local").unwrap(), Zone::Local);
        assert_eq!(parse_zone("+1000").unwrap(), Zone::Fixed(600));
        assert_eq!(parse_zone("-0400").unwrap(), Zone::Fixed(-240));
        assert_eq!(parse_zone("+0530").unwrap(), Zone::Fixed(330));
        for bad in ["+10:00", "1000", "+100", "+10000", "utc", "Z", "+2500", "+0060"] {
            assert!(parse_zone(bad).is_err(), "{bad} should be refused");
        }
    }

    #[test]
    fn adding_a_month_clamps_instead_of_rolling_over() {
        let jan31 = epoch_from_civil(2026, 1, 31, 15, 0, 0, 0, 0).unwrap();
        let feb = add(jan31, 1, Unit::Months(1), Zone::Utc).unwrap();
        let p = parts_at(feb, 0);
        assert_eq!((p.year, p.month, p.day, p.hour), (2026, 2, 28, 15));
        // and in a leap year it reaches the 29th
        let jan31_leap = epoch_from_civil(2024, 1, 31, 0, 0, 0, 0, 0).unwrap();
        let feb_leap = add(jan31_leap, 1, Unit::Months(1), Zone::Utc).unwrap();
        assert_eq!(parts_at(feb_leap, 0).day, 29);
    }

    #[test]
    fn adding_a_year_of_months_is_the_same_day_next_year() {
        let d = epoch_from_civil(2026, 8, 23, 9, 30, 0, 0, 0).unwrap();
        let next = add(d, 1, Unit::Months(12), Zone::Utc).unwrap();
        let p = parts_at(next, 0);
        assert_eq!((p.year, p.month, p.day, p.hour, p.minute), (2027, 8, 23, 9, 30));
    }

    #[test]
    fn days_are_calendar_and_hours_are_duration() {
        let d = epoch_from_civil(2026, 8, 23, 12, 0, 0, 0, 0).unwrap();
        assert_eq!(
            parts_at(add(d, 30, Unit::Days(1), Zone::Utc).unwrap(), 0).day,
            22
        );
        assert_eq!(add(d, 25, Unit::Duration(MS_PER_HOUR), Zone::Utc).unwrap(), d + 25 * MS_PER_HOUR);
        // a week is seven of them
        let w = add(d, 1, Unit::Days(7), Zone::Utc).unwrap();
        assert_eq!(w - d, 7 * MS_PER_DAY);
    }

    #[test]
    fn diff_counts_whole_units_toward_zero() {
        let a = epoch_from_civil(2026, 8, 23, 0, 0, 0, 0, 0).unwrap();
        let b = epoch_from_civil(2026, 7, 24, 0, 0, 0, 0, 0).unwrap();
        assert_eq!(diff(a, b, Unit::Days(1), Zone::Utc).unwrap(), 30);
        assert_eq!(diff(b, a, Unit::Days(1), Zone::Utc).unwrap(), -30);
        assert_eq!(diff(a, b, Unit::Days(7), Zone::Utc).unwrap(), 4);
        assert_eq!(diff(a, b, Unit::Duration(MS_PER_HOUR), Zone::Utc).unwrap(), 720);
    }

    #[test]
    fn a_day_apart_by_one_minute_is_still_a_day() {
        // Calendar distance, not elapsed time: 23:59 and 00:00 are one day apart.
        let a = epoch_from_civil(2026, 8, 22, 23, 59, 0, 0, 0).unwrap();
        let b = epoch_from_civil(2026, 8, 23, 0, 0, 0, 0, 0).unwrap();
        assert_eq!(diff(b, a, Unit::Days(1), Zone::Utc).unwrap(), 1);
        assert_eq!(diff(b, a, Unit::Duration(MS_PER_HOUR), Zone::Utc).unwrap(), 0);
    }

    #[test]
    fn a_whole_month_needs_the_day_of_month_to_arrive() {
        let start = epoch_from_civil(2026, 1, 15, 0, 0, 0, 0, 0).unwrap();
        let before = epoch_from_civil(2026, 2, 14, 0, 0, 0, 0, 0).unwrap();
        let on = epoch_from_civil(2026, 2, 15, 0, 0, 0, 0, 0).unwrap();
        assert_eq!(diff(before, start, Unit::Months(1), Zone::Utc).unwrap(), 0);
        assert_eq!(diff(on, start, Unit::Months(1), Zone::Utc).unwrap(), 1);
        assert_eq!(diff(start, on, Unit::Months(1), Zone::Utc).unwrap(), -1);
    }

    #[test]
    fn format_writes_ascii_digits_and_refuses_what_it_cannot_render() {
        let d = epoch_from_civil(2026, 8, 23, 14, 5, 9, 42, 0).unwrap();
        assert_eq!(format(d, "%F", 0).unwrap(), "2026-08-23");
        assert_eq!(format(d, "%T", 0).unwrap(), "14:05:09");
        assert_eq!(format(d, "%Y/%m/%d %H:%M:%S.%L", 0).unwrap(), "2026/08/23 14:05:09.042");
        assert_eq!(format(d, "%j", 0).unwrap(), "235");
        assert_eq!(format(d, "%u", 0).unwrap(), "7"); // a Sunday
        assert_eq!(format(d, "%z", 0).unwrap(), "+0000");
        assert_eq!(format(d, "%z", -240).unwrap(), "-0400");
        assert_eq!(format(d, "100%% seguro", 0).unwrap(), "100% seguro");
        assert!(format(d, "%Q", 0).is_err());
        assert!(format(d, "acaba en %", 0).is_err());
        assert_eq!(iso_date(d, 0), "2026-08-23");
    }

    #[test]
    fn day_of_year_counts_the_leap_day() {
        let dec31_leap = epoch_from_civil(2024, 12, 31, 0, 0, 0, 0, 0).unwrap();
        assert_eq!(format(dec31_leap, "%j", 0).unwrap(), "366");
        let dec31 = epoch_from_civil(2026, 12, 31, 0, 0, 0, 0, 0).unwrap();
        assert_eq!(format(dec31, "%j", 0).unwrap(), "365");
    }

    #[test]
    fn units_have_one_spelling_each() {
        assert_eq!(parse_unit("day").unwrap(), Unit::Days(1));
        assert_eq!(parse_unit("week").unwrap(), Unit::Days(7));
        assert_eq!(parse_unit("year").unwrap(), Unit::Months(12));
        assert_eq!(parse_unit("second").unwrap(), Unit::Duration(MS_PER_SECOND));
        for bad in ["s", "d", "days", "Day", "sec", "mo"] {
            assert!(parse_unit(bad).is_err(), "{bad} should be refused");
        }
    }

    #[test]
    fn the_clock_is_in_this_century() {
        let now = now_ms();
        // 2020-01-01 .. 2100-01-01, which is a real assertion about the unit:
        // seconds or nanoseconds would both fall outside it.
        assert!(now > 1_577_836_800_000 && now < 4_102_444_800_000, "now_ms = {now}");
    }
}
