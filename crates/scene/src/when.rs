//! When a page was seen, as the history view says it: "Today 14:32",
//! "Yesterday 09:05", "Mon 14:32" within the week, then "10 Nov", or
//! "10 Nov 2025" in another year. Times are local milliseconds (UTC plus the
//! zone's offset), so this stays pure; the app adds the offset.

const DAY_MS: i64 = 86_400_000;
const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
/// Day 0 (1970-01-01) was a Thursday.
const WEEKDAYS: [&str; 7] = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"];

/// (year, month 1–12, day 1–31) of `days` since 1970-01-01 (proleptic
/// Gregorian; Howard Hinnant's `civil_from_days`).
pub fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

/// How long ago `then` was, both in local ms.
pub fn label(then: i64, now: i64) -> String {
    let (day, today) = (then.div_euclid(DAY_MS), now.div_euclid(DAY_MS));
    let mins = then.rem_euclid(DAY_MS) / 60_000;
    let clock = format!("{:02}:{:02}", mins / 60, mins % 60);
    let ago = today - day;
    match ago {
        0 => format!("Today {clock}"),
        1 => format!("Yesterday {clock}"),
        2..=6 => format!("{} {clock}", WEEKDAYS[day.rem_euclid(7) as usize]),
        _ => {
            let (y, m, d) = civil(day);
            let month = MONTHS[m as usize - 1];
            if y == civil(today).0 { format!("{d} {month}") } else { format!("{d} {month} {y}") }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Local ms of a date and time.
    fn at(y: i64, m: u32, d: u32, h: i64, min: i64) -> i64 {
        let days = (0..30_000).find(|&n| civil(n) == (y, m, d)).expect("date after 1970");
        days * DAY_MS + (h * 60 + min) * 60_000
    }

    #[test]
    fn civil_dates() {
        assert_eq!(civil(0), (1970, 1, 1));
        assert_eq!(civil(-1), (1969, 12, 31));
        assert_eq!(civil(59), (1970, 3, 1));
        assert_eq!(civil(11_016), (2000, 2, 29));
        assert_eq!(civil(20_406), (2025, 11, 14));
    }

    #[test]
    fn labels() {
        let now = at(2026, 11, 14, 10, 0); // a Saturday
        assert_eq!(label(at(2026, 11, 14, 9, 5), now), "Today 09:05");
        assert_eq!(label(at(2026, 11, 14, 0, 0), now), "Today 00:00");
        assert_eq!(label(at(2026, 11, 13, 23, 59), now), "Yesterday 23:59");
        assert_eq!(label(at(2026, 11, 9, 14, 32), now), "Mon 14:32");
        assert_eq!(label(at(2026, 11, 8, 14, 32), now), "Sun 14:32");
        assert_eq!(label(at(2026, 11, 7, 14, 32), now), "7 Nov");
        assert_eq!(label(at(2026, 1, 1, 1, 0), now), "1 Jan");
        assert_eq!(label(at(2025, 12, 31, 1, 0), now), "31 Dec 2025");
        // A clock that went back: a date, not "Today".
        assert_eq!(label(at(2026, 11, 15, 8, 0), now), "15 Nov");
    }
}
