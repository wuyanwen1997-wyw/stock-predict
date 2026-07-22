use crate::models::StockQuote;
use crate::monitor::rules::{MonitorAlert, MonitorRule};
use chrono::{DateTime, Datelike, Utc};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

static ALERT_SEQ: AtomicU64 = AtomicU64::new(1);

fn new_id() -> String {
    let n = ALERT_SEQ.fetch_add(1, Ordering::Relaxed);
    format!("{}-{}", Utc::now().timestamp_millis(), n)
}

#[derive(Debug, Default, Clone)]
pub struct EngineState {
    /// rule_id -> last fire time
    pub last_fired: HashMap<String, DateTime<Utc>>,
    /// rule_id -> (ymd as i32 yyyymmdd, count)
    pub day_counts: HashMap<String, (i32, u32)>,
}

#[derive(Debug, Clone)]
pub struct FiredAlert {
    pub alert: MonitorAlert,
    pub title: String,
    pub body: String,
}

fn ymd_key(dt: DateTime<Utc>) -> i32 {
    let d = dt.date_naive();
    d.year() * 10_000 + d.month() as i32 * 100 + d.day() as i32
}

/// 对启用规则评估行情；更新冷却与日计数；返回本次触发列表。
pub fn evaluate(
    rules: &[MonitorRule],
    quotes: &HashMap<String, StockQuote>,
    names: &HashMap<String, String>,
    state: &mut EngineState,
    now: DateTime<Utc>,
) -> Vec<FiredAlert> {
    let today = ymd_key(now);
    let mut fired = Vec::new();

    for rule in rules.iter().filter(|r| r.enabled) {
        let quote = match quotes.get(&rule.code) {
            Some(q) => q,
            None => continue,
        };
        if !rule.condition.matches(quote.price, quote.change_pct) {
            continue;
        }

        // 日上限
        let (day, count) = state.day_counts.get(&rule.id).copied().unwrap_or((today, 0));
        let count = if day == today { count } else { 0 };
        if count >= rule.max_per_day {
            continue;
        }

        // 冷却
        if let Some(last) = state.last_fired.get(&rule.id) {
            let elapsed = (now - *last).num_seconds().max(0) as u64;
            if elapsed < rule.cooldown_sec {
                continue;
            }
        }

        let name = names
            .get(&rule.code)
            .cloned()
            .unwrap_or_else(|| rule.name.clone());
        let cond = rule.condition.label();
        let price_s = quote
            .price
            .map(|p| format!("{p:.2}"))
            .unwrap_or_else(|| "-".into());
        let pct_s = quote
            .change_pct
            .map(|p| format!("{p:.2}%"))
            .unwrap_or_else(|| "-".into());
        let message = format!("{cond} · 现价 {price_s} · 涨跌幅 {pct_s}");
        let title = format!("{name} 触发预警");
        let body = message.clone();

        let alert = MonitorAlert {
            id: new_id(),
            rule_id: rule.id.clone(),
            code: rule.code.clone(),
            name,
            message,
            price: quote.price,
            change_pct: quote.change_pct,
            fired_at: now,
        };

        state.last_fired.insert(rule.id.clone(), now);
        state.day_counts.insert(rule.id.clone(), (today, count + 1));

        fired.push(FiredAlert { alert, title, body });
    }

    fired
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monitor::rules::AlertCondition;
    use crate::models::StockQuote;
    use chrono::TimeZone;

    fn quote(price: f64, pct: f64) -> StockQuote {
        StockQuote {
            price: Some(price),
            change_pct: Some(pct),
            change_amt: None,
            open: None,
            high: None,
            low: None,
            prev_close: None,
            volume: None,
            turnover: None,
        }
    }

    fn rule(id: &str, code: &str, cond: AlertCondition) -> MonitorRule {
        MonitorRule {
            id: id.into(),
            code: code.into(),
            name: "测试".into(),
            enabled: true,
            condition: cond,
            cooldown_sec: 300,
            max_per_day: 5,
            created_at: String::new(),
        }
    }

    #[test]
    fn fires_on_change_pct() {
        let rules = vec![rule(
            "r1",
            "600519",
            AlertCondition::ChangePctAbove { value: 3.0 },
        )];
        let mut quotes = HashMap::new();
        quotes.insert("600519".into(), quote(100.0, 3.5));
        let names = HashMap::from([("600519".into(), "茅台".into())]);
        let mut state = EngineState::default();
        let now = Utc.with_ymd_and_hms(2026, 7, 23, 2, 0, 0).unwrap();
        let fired = evaluate(&rules, &quotes, &names, &mut state, now);
        assert_eq!(fired.len(), 1);
        assert!(fired[0].title.contains("茅台"));
    }

    #[test]
    fn cooldown_blocks_repeat() {
        let rules = vec![rule(
            "r1",
            "600519",
            AlertCondition::ChangePctAbove { value: 1.0 },
        )];
        let mut quotes = HashMap::new();
        quotes.insert("600519".into(), quote(100.0, 2.0));
        let names = HashMap::new();
        let mut state = EngineState::default();
        let t0 = Utc.with_ymd_and_hms(2026, 7, 23, 2, 0, 0).unwrap();
        assert_eq!(evaluate(&rules, &quotes, &names, &mut state, t0).len(), 1);
        let t1 = t0 + chrono::Duration::seconds(60);
        assert_eq!(evaluate(&rules, &quotes, &names, &mut state, t1).len(), 0);
        let t2 = t0 + chrono::Duration::seconds(301);
        assert_eq!(evaluate(&rules, &quotes, &names, &mut state, t2).len(), 1);
    }

    #[test]
    fn max_per_day() {
        let mut r = rule(
            "r1",
            "600519",
            AlertCondition::PriceAbove { value: 10.0 },
        );
        r.max_per_day = 2;
        r.cooldown_sec = 0;
        let rules = vec![r];
        let mut quotes = HashMap::new();
        quotes.insert("600519".into(), quote(20.0, 0.0));
        let names = HashMap::new();
        let mut state = EngineState::default();
        let t0 = Utc.with_ymd_and_hms(2026, 7, 23, 2, 0, 0).unwrap();
        assert_eq!(evaluate(&rules, &quotes, &names, &mut state, t0).len(), 1);
        assert_eq!(
            evaluate(&rules, &quotes, &names, &mut state, t0 + chrono::Duration::seconds(1)).len(),
            1
        );
        assert_eq!(
            evaluate(&rules, &quotes, &names, &mut state, t0 + chrono::Duration::seconds(2)).len(),
            0
        );
    }
}
