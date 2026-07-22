//! 回测口径：预测结果 vs 实际涨跌 → 命中统计（无 IO）。

/// 有效信号出手线：领先一侧概率 ≥ 该值才计入「整体/有效准确率」
pub const ACTIONABLE_LEAD: f64 = 55.0;

#[derive(Debug, Clone, Default)]
pub struct HitCounters {
    pub correct_all: u32,
    pub total_all: u32,
    pub correct_act: u32,
    pub total_act: u32,
    pub up_hits: u32,
    pub up_total: u32,
    pub down_hits: u32,
    pub down_total: u32,
    pub up_hits_act: u32,
    pub up_total_act: u32,
    pub down_hits_act: u32,
    pub down_total_act: u32,
    pub hc_correct: u32,
    pub hc_total: u32,
}

impl HitCounters {
    /// 登记一条预测结果。
    pub fn observe(
        &mut self,
        predicted: &str,
        actual: &str,
        up_probability: f64,
        down_probability: f64,
        high_confidence: bool,
    ) {
        let is_correct = predicted == actual;
        let lead = up_probability.max(down_probability);
        let actionable = lead + 1e-9 >= ACTIONABLE_LEAD;

        self.total_all += 1;
        if is_correct {
            self.correct_all += 1;
        }

        if predicted == "up" {
            self.up_total += 1;
            if actual == "up" {
                self.up_hits += 1;
            }
        } else {
            self.down_total += 1;
            if actual == "down" {
                self.down_hits += 1;
            }
        }

        if actionable {
            self.total_act += 1;
            if is_correct {
                self.correct_act += 1;
            }
            if predicted == "up" {
                self.up_total_act += 1;
                if actual == "up" {
                    self.up_hits_act += 1;
                }
            } else {
                self.down_total_act += 1;
                if actual == "down" {
                    self.down_hits_act += 1;
                }
            }
        }

        if high_confidence {
            self.hc_total += 1;
            if is_correct {
                self.hc_correct += 1;
            }
        }
    }
}

pub fn classify_change(change_pct: f64) -> &'static str {
    if change_pct > 0.0 {
        "up"
    } else {
        "down"
    }
}

/// 命中率百分比，保留一位小数（与历史回测展示口径一致）
pub fn pct(hits: u32, total: u32) -> f64 {
    if total == 0 {
        0.0
    } else {
        (hits as f64 / total as f64 * 1000.0).round() / 10.0
    }
}

pub fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

pub fn is_actionable(up_probability: f64, down_probability: f64) -> bool {
    up_probability.max(down_probability) + 1e-9 >= ACTIONABLE_LEAD
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actionable_threshold() {
        assert!(!is_actionable(54.0, 46.0));
        assert!(is_actionable(55.0, 45.0));
    }

    #[test]
    fn counters_track_actionable() {
        let mut c = HitCounters::default();
        c.observe("up", "up", 60.0, 40.0, true);
        c.observe("down", "up", 52.0, 48.0, false);
        assert_eq!(c.total_all, 2);
        assert_eq!(c.correct_all, 1);
        assert_eq!(c.total_act, 1);
        assert_eq!(c.correct_act, 1);
        assert_eq!(c.hc_total, 1);
    }
}
