use super::*;

pub(super) fn resolve_period(
    query: &UsageQuery,
) -> (
    Option<DateTime<Utc>>,
    Option<DateTime<Utc>>,
    PeriodDescriptor,
) {
    resolve_period_at(query, Utc::now())
}

pub(super) fn resolve_period_at(
    query: &UsageQuery,
    now_utc: DateTime<Utc>,
) -> (
    Option<DateTime<Utc>>,
    Option<DateTime<Utc>>,
    PeriodDescriptor,
) {
    let timezone_name = query.timezone.as_deref().unwrap_or("Asia/Shanghai");
    let timezone = Tz::from_str(timezone_name).unwrap_or(chrono_tz::Asia::Shanghai);
    let now_local = now_utc.with_timezone(&timezone);
    let period_label = query.period.as_deref().unwrap_or("rolling30");
    let today = now_local.date_naive();
    let this_week = today - ChronoDuration::days(now_local.weekday().num_days_from_monday() as i64);
    let this_month = today.with_day(1).unwrap_or(today);
    let (start, default_grain, partial) = match period_label {
        "today" => (local_midnight_utc(today, timezone), "hour", true),
        "week" => (local_midnight_utc(this_week, timezone), "day", true),
        "rolling7" => (Some(now_utc - ChronoDuration::days(7)), "day", false),
        "month" => (local_midnight_utc(this_month, timezone), "day", true),
        "rolling30" => (Some(now_utc - ChronoDuration::days(30)), "day", false),
        "weeks12" => (
            local_midnight_utc(this_week - ChronoDuration::weeks(11), timezone),
            "week",
            true,
        ),
        "months12" => (
            local_midnight_utc(shift_month_start(this_month, -11), timezone),
            "month",
            true,
        ),
        "lifetime" => (None, "month", false),
        _ => (None, "month", false),
    };
    let elapsed = start.map(|start| now_utc.signed_duration_since(start));
    let comparison_start = match period_label {
        "today" => local_midnight_utc(today - ChronoDuration::days(1), timezone),
        "week" => local_midnight_utc(this_week - ChronoDuration::weeks(1), timezone),
        "month" => local_midnight_utc(shift_month_start(this_month, -1), timezone),
        "rolling7" => start.map(|start| start - ChronoDuration::days(7)),
        "rolling30" => start.map(|start| start - ChronoDuration::days(30)),
        "weeks12" => start.map(|start| start - ChronoDuration::weeks(12)),
        "months12" => local_midnight_utc(shift_month_start(this_month, -23), timezone),
        _ => None,
    };
    let comparison_end = match period_label {
        "today" | "week" | "month" => comparison_start
            .zip(elapsed)
            .map(|(previous, elapsed)| previous + elapsed)
            .zip(start)
            .map(|(candidate, boundary)| candidate.min(boundary)),
        "rolling7" | "rolling30" | "weeks12" | "months12" => start,
        _ => None,
    };

    (
        start,
        Some(now_utc),
        PeriodDescriptor {
            label: period_label.into(),
            start,
            end: Some(now_utc),
            timezone: timezone.name().into(),
            comparison_start,
            comparison_end,
            default_grain: default_grain.into(),
            partial,
        },
    )
}

pub(super) fn local_midnight_utc(date: NaiveDate, timezone: Tz) -> Option<DateTime<Utc>> {
    let local = date.and_time(NaiveTime::MIN);
    match timezone.from_local_datetime(&local) {
        LocalResult::Single(value) | LocalResult::Ambiguous(value, _) => {
            Some(value.with_timezone(&Utc))
        }
        LocalResult::None => None,
    }
}

fn shift_month_start(date: NaiveDate, offset: i32) -> NaiveDate {
    let month_index = date.year().saturating_mul(12) + date.month0() as i32 + offset;
    let year = month_index.div_euclid(12);
    let month = month_index.rem_euclid(12) as u32 + 1;
    NaiveDate::from_ymd_opt(year, month, 1).unwrap_or(date)
}
