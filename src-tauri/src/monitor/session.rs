use chrono::{Datelike, NaiveTime, Weekday};

/// A 股交易时段（含集合竞价缓冲）：工作日 09:15–11:30、13:00–15:00。
/// `local` 使用本地墙钟时间（中国境内设备通常为 CST）。
pub fn is_trading_session(local: chrono::DateTime<chrono::Local>) -> bool {
    match local.weekday() {
        Weekday::Sat | Weekday::Sun => return false,
        _ => {}
    }

    let t = local.time();
    let morning_start = NaiveTime::from_hms_opt(9, 15, 0).unwrap();
    let morning_end = NaiveTime::from_hms_opt(11, 30, 0).unwrap();
    let afternoon_start = NaiveTime::from_hms_opt(13, 0, 0).unwrap();
    let afternoon_end = NaiveTime::from_hms_opt(15, 0, 0).unwrap();

    (t >= morning_start && t <= morning_end) || (t >= afternoon_start && t <= afternoon_end)
}

/// 便于单测：用「时:分」构造是否在时段内（假定周一）。
#[cfg(test)]
pub fn is_trading_hm(hour: u32, minute: u32) -> bool {
    use chrono::{Local, TimeZone};
    let dt = Local
        .with_ymd_and_hms(2026, 7, 20, hour, minute, 0) // 周一
        .single()
        .expect("valid datetime");
    assert_eq!(dt.weekday(), Weekday::Mon);
    is_trading_session(dt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Local, TimeZone};

    #[test]
    fn weekday_morning_open() {
        assert!(is_trading_hm(9, 15));
        assert!(is_trading_hm(10, 0));
        assert!(is_trading_hm(11, 30));
    }

    #[test]
    fn weekday_lunch_closed() {
        assert!(!is_trading_hm(11, 31));
        assert!(!is_trading_hm(12, 0));
        assert!(!is_trading_hm(12, 59));
    }

    #[test]
    fn weekday_afternoon() {
        assert!(is_trading_hm(13, 0));
        assert!(is_trading_hm(14, 30));
        assert!(is_trading_hm(15, 0));
        assert!(!is_trading_hm(15, 1));
    }

    #[test]
    fn weekend_closed() {
        let sat = Local
            .with_ymd_and_hms(2026, 7, 25, 10, 0, 0) // 周六
            .single()
            .unwrap();
        assert_eq!(sat.weekday(), Weekday::Sat);
        assert!(!is_trading_session(sat));
    }

    #[test]
    fn before_open_closed() {
        assert!(!is_trading_hm(9, 14));
        assert!(!is_trading_hm(8, 0));
    }
}
