//! UTC timestamps as the state files hold them: `isoSeconds` (`date -u
//! +%Y-%m-%dT%H:%M:%SZ`, `Date#toISOString` cut to the second) and its parse.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

const DAY: i64 = 86_400;

/// `t` as `2026-09-28T12:34:56Z`, truncated to the second.
pub fn iso_seconds(t: SystemTime) -> String {
  let secs = epoch_seconds(t);
  let (days, rest) = (secs.div_euclid(DAY), secs.rem_euclid(DAY));
  let (year, month, day) = civil_from_days(days);
  format!(
    "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
    rest / 3600,
    rest % 3600 / 60,
    rest % 60
  )
}

/// `t` as `Date#toISOString` prints it, with milliseconds: `2026-09-28T12:34:56.789Z`.
/// A time before the epoch keeps whole seconds.
pub fn iso_millis(t: SystemTime) -> String {
  let millis = t
    .duration_since(UNIX_EPOCH)
    .map_or(0, |after| after.subsec_millis());
  let seconds = iso_seconds(t);
  format!("{}.{millis:03}Z", seconds.trim_end_matches('Z'))
}

/// `Date.now()` for `t`: milliseconds since the epoch, 0 before it.
pub fn epoch_millis(t: SystemTime) -> u64 {
  t.duration_since(UNIX_EPOCH).map_or(0, |after| {
    u64::try_from(after.as_millis()).unwrap_or(u64::MAX)
  })
}

/// Whole seconds since the epoch, rounded down (before it, too).
fn epoch_seconds(t: SystemTime) -> i64 {
  match t.duration_since(UNIX_EPOCH) {
    Ok(after) => i64::try_from(after.as_secs()).unwrap_or(i64::MAX),
    Err(before) => {
      let before = before.duration();
      let whole = i64::try_from(before.as_secs()).unwrap_or(i64::MAX);
      -whole - i64::from(before.subsec_nanos() > 0)
    }
  }
}

/// Howard Hinnant's `civil_from_days`: the proleptic Gregorian date of `days`.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
  let z = days + 719_468;
  let era = z.div_euclid(146_097);
  let doe = z.rem_euclid(146_097);
  let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
  let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
  let mp = (5 * doy + 2) / 153;
  let day = doy - (153 * mp + 2) / 5 + 1;
  let month = if mp < 10 { mp + 3 } else { mp - 9 };
  let year = yoe + era * 400 + i64::from(month <= 2);
  (year, month, day)
}

/// Howard Hinnant's `days_from_civil`.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
  let year = year - i64::from(month <= 2);
  let era = year.div_euclid(400);
  let yoe = year.rem_euclid(400);
  let mp = (month + 9) % 12;
  let doy = (153 * mp + 2) / 5 + day - 1;
  let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
  era * 146_097 + doe - 719_468
}

/// `YYYY-MM-DDTHH:MM:SS[.fff]Z` as a time, or `None` for any other text.
pub fn parse_iso(text: &str) -> Option<SystemTime> {
  let text = text.strip_suffix('Z')?;
  let (date, clock) = text.split_once('T')?;
  let (clock, fraction) = clock.split_once('.').unwrap_or((clock, ""));
  let [year, month, day] = fields(date, '-')?;
  let [hour, minute, second] = fields(clock, ':')?;
  let in_range = (1..=12).contains(&month)
    && (1..=31).contains(&day)
    && (0..24).contains(&hour)
    && (0..60).contains(&minute)
    && (0..60).contains(&second)
    && fraction.bytes().all(|byte| byte.is_ascii_digit());
  if !in_range {
    return None;
  }
  let millis = match fraction {
    "" => 0,
    digits => format!("{digits:0<3}").get(..3)?.parse::<u64>().ok()?,
  };
  let secs = days_from_civil(year, month, day) * DAY + hour * 3600 + minute * 60 + second;
  let millis = secs
    .checked_mul(1000)?
    .checked_add(i64::try_from(millis).ok()?)?;
  let offset = Duration::from_millis(millis.unsigned_abs());
  if millis >= 0 {
    UNIX_EPOCH.checked_add(offset)
  } else {
    UNIX_EPOCH.checked_sub(offset)
  }
}

/// Three `sep`-separated decimal fields.
fn fields(text: &str, sep: char) -> Option<[i64; 3]> {
  let mut parts = text.split(sep).map(|part| part.parse::<i64>().ok());
  let fields = [parts.next()??, parts.next()??, parts.next()??];
  parts.next().is_none().then_some(fields)
}

#[cfg(test)]
#[path = "tests/time_test.rs"]
mod tests;
