use crate::market;
use crate::monitor::engine::evaluate;
use crate::monitor::session::is_trading_session;
use crate::monitor::shared::DEFAULT_INTERVAL_SECS;
use crate::monitor::SharedMonitor;
use async_trait::async_trait;
use chrono::{Local, Utc};
use serde::Serialize;
use std::collections::HashMap;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_background_service::{BackgroundService, ServiceContext, ServiceError};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorQuoteEvent {
    pub code: String,
    pub price: Option<f64>,
    pub change_pct: Option<f64>,
}

pub struct MonitorBackgroundService {
    shared: Option<SharedMonitor>,
}

impl MonitorBackgroundService {
    pub fn new() -> Self {
        Self { shared: None }
    }
}

#[async_trait]
impl<R: Runtime> BackgroundService<R> for MonitorBackgroundService {
    async fn init(&mut self, ctx: &ServiceContext<R>) -> Result<(), ServiceError> {
        let shared = ctx
            .app
            .try_state::<SharedMonitor>()
            .map(|s| s.inner().clone())
            .ok_or_else(|| ServiceError::Init("MonitorShared 未注册".into()))?;
        {
            let mut guard = shared.lock().await;
            guard.enabled = true;
            guard.consecutive_failures = 0;
        }
        self.shared = Some(shared);
        Ok(())
    }

    async fn run(&mut self, ctx: &ServiceContext<R>) -> Result<(), ServiceError> {
        let shared = self
            .shared
            .clone()
            .ok_or_else(|| ServiceError::Init("shared missing".into()))?;

        // 启动后立即扫一次，之后按间隔休眠
        loop {
            if let Err(e) = tick_once(&ctx.app, &shared, ctx).await {
                eprintln!("[monitor] tick error: {e}");
            }

            let secs = {
                let guard = shared.lock().await;
                if guard.interval_secs == 0 {
                    DEFAULT_INTERVAL_SECS
                } else {
                    guard.interval_secs
                }
            };

            tokio::select! {
                _ = ctx.shutdown.cancelled() => {
                    let mut guard = shared.lock().await;
                    guard.enabled = false;
                    break;
                }
                _ = tokio::time::sleep(Duration::from_secs(secs)) => {}
            }
        }

        Ok(())
    }
}

async fn tick_once<R: Runtime>(
    app: &AppHandle<R>,
    shared: &SharedMonitor,
    ctx: &ServiceContext<R>,
) -> Result<(), String> {
    if !is_trading_session(Local::now()) {
        return Ok(());
    }

    let (stocks, rules) = {
        let guard = shared.lock().await;
        if !guard.enabled || guard.stocks.is_empty() {
            return Ok(());
        }
        (guard.stocks.clone(), guard.rules.clone())
    };

    let quote_map = match market::fetch_stock_quotes(&stocks).await {
        Ok(m) => {
            let mut guard = shared.lock().await;
            guard.consecutive_failures = 0;
            m
        }
        Err(e) => {
            let mut guard = shared.lock().await;
            guard.consecutive_failures = guard.consecutive_failures.saturating_add(1);
            return Err(e);
        }
    };

    let mut quote_events = Vec::new();
    {
        let mut guard = shared.lock().await;
        for stock in &mut guard.stocks {
            if let Some(q) = quote_map.get(&stock.code) {
                crate::ashare::apply_quote(stock, q);
                quote_events.push(MonitorQuoteEvent {
                    code: stock.code.clone(),
                    price: q.price,
                    change_pct: q.change_pct,
                });
            }
        }
    }
    let _ = app.emit("monitor-quotes", &quote_events);

    let names: HashMap<String, String> = stocks
        .iter()
        .map(|s| (s.code.clone(), s.name.clone()))
        .collect();

    let fired = {
        let mut guard = shared.lock().await;
        evaluate(&rules, &quote_map, &names, &mut guard.engine, Utc::now())
    };

    for item in fired {
        ctx.notifier.show(&item.title, &item.body);
        let _ = app.emit("monitor-alert", &item.alert);
    }

    Ok(())
}
