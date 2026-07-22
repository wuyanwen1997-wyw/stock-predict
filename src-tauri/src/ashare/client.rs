/// 共享 HTTP 客户端、重试与解析工具。

pub(crate) const QUOTE_URLS: &[&str] = &[
    "https://push2delay.eastmoney.com/api/qt/ulist.np/get",
    "https://push2.eastmoney.com/api/qt/ulist.np/get",
    "https://82.push2.eastmoney.com/api/qt/ulist.np/get",
    "https://push2delay.eastmoney.com/api/qt/ulist/get",
];

pub(crate) const HTTP_RETRY: u32 = 2;
pub(crate) const HTTP_RETRY_DELAY_MS: u64 = 400;

const USER_AGENT_DESKTOP: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
const USER_AGENT_MOBILE: &str = "Mozilla/5.0 (Linux; Android 13; Mobile) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Mobile Safari/537.36";

fn user_agent() -> &'static str {
    if cfg!(target_os = "android") || cfg!(target_os = "ios") {
        USER_AGENT_MOBILE
    } else {
        USER_AGENT_DESKTOP
    }
}

pub(crate) fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .connect_timeout(std::time::Duration::from_secs(6))
        .pool_idle_timeout(std::time::Duration::from_secs(30))
        .user_agent(user_agent())
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {e}"))
}

pub(crate) fn apply_browser_headers(builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
    builder
        .header("Accept", "application/json, text/plain, */*")
        .header("Accept-Language", "zh-CN,zh;q=0.9")
        .header("Referer", "https://quote.eastmoney.com/")
}

pub(crate) fn log_warn(scope: &str, msg: &str) {
    eprintln!("[stock-predict:{scope}] {msg}");
}

pub(crate) async fn sleep_retry(attempt: u32) {
    if attempt == 0 {
        return;
    }
    tokio::time::sleep(std::time::Duration::from_millis(
        HTTP_RETRY_DELAY_MS * u64::from(attempt),
    ))
    .await;
}

pub(crate) fn parse_f64(v: &serde_json::Value) -> Option<f64> {
    match v {
        serde_json::Value::Number(n) => n.as_f64(),
        serde_json::Value::String(s) => {
            let t = s.trim();
            if t.is_empty() || t == "-" {
                None
            } else {
                t.parse().ok()
            }
        }
        _ => None,
    }
}

pub(crate) fn parse_json_f64(v: &serde_json::Value) -> f64 {
    parse_f64(v).unwrap_or(0.0)
}
