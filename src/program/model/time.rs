//! A moment as the list writes it: the month and the day.

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// "Nov 9" of "2026-11-09T14:20:00.000Z"; what is not a date is left as it came.
#[must_use]
pub fn short_date(stamp: &str) -> String {
    let mut parts = stamp.get(..10).unwrap_or(stamp).split('-');
    let _year = parts.next();
    let month = parts
        .next()
        .and_then(|m| m.parse::<usize>().ok())
        .and_then(|m| MONTHS.get(m.wrapping_sub(1)));
    let day = parts.next().and_then(|d| d.parse::<u32>().ok());
    match (month, day) {
        (Some(month), Some(day)) => format!("{month} {day}"),
        _ => stamp.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_are_the_month_and_the_day() {
        assert_eq!(short_date("2026-11-09T14:20:00.000Z"), "Nov 9");
        assert_eq!(short_date("2026-01-31T00:00:00Z"), "Jan 31");
        assert_eq!(short_date("soon"), "soon");
        assert_eq!(short_date("2026-13-01T00:00:00Z"), "2026-13-01T00:00:00Z");
    }
}
