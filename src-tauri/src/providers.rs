use crate::core::{unix_now, DataCard, Location, Widget};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs,
    io::{BufRead, BufReader, Read, Write},
    path::PathBuf,
    process::{Command, Stdio},
    sync::{mpsc, Arc, Mutex},
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageWindow {
    pub bucket: String,
    pub slot: String,
    pub label: String,
    pub used_percent: f64,
    pub minutes: u64,
    pub resets_at: u64,
}
#[derive(Clone)]
struct CodexSnapshot {
    fetched: u64,
    scope: String,
    windows: Vec<UsageWindow>,
}
#[derive(Default)]
struct Cache {
    cards: HashMap<String, (String, u64, DataCard)>,
    failures: HashMap<String, u8>,
}
pub struct Providers {
    client: Client,
    directory: PathBuf,
    cache: Mutex<Cache>,
    codex: Mutex<HashMap<String, CodexSnapshot>>,
    in_flight: Mutex<HashMap<String, Arc<Mutex<()>>>>,
}
fn error_state(error: &str) -> &'static str {
    let text = error.to_ascii_lowercase();
    if text.contains("rate limit") || text.contains("http 429") {
        "rate_limited"
    } else if text.contains("sign-in")
        || text.contains("sign in")
        || text.contains("credential")
        || text.contains("connect your claude")
    {
        "authentication_required"
    } else {
        "unavailable"
    }
}
fn retry_seconds(state: &str, attempt: u8, key: &str) -> u64 {
    if state == "invalid_configuration" {
        return u64::MAX;
    }
    // A client outside DroidFun App can renew the shared local sign-in. Recheck
    // occasionally so an authentication failure is not cached forever.
    if state == "authentication_required" {
        return 300;
    }
    if state == "rate_limited" {
        return 600;
    }
    let base = match attempt {
        0 | 1 => 30,
        2 => 120,
        _ => 600,
    };
    let jitter = Sha256::digest(key.as_bytes())[0] as u64 % (base / 5 + 1);
    base + jitter
}
fn failure(source: &str, state: &str, detail: &str) -> DataCard {
    DataCard {
        source_id: source.into(),
        scope_id: String::new(),
        fetched_at: unix_now(),
        observed_at: 0,
        stale_at: 0,
        expires_at: 0,
        state: state.into(),
        headline: "WAITING".into(),
        footer: match state {
            "authentication_required" => "CONNECT ACCOUNT",
            "invalid_configuration" => "SETUP NEEDED",
            "rate_limited" => "RATE LIMITED",
            _ => "UNAVAILABLE",
        }
        .into(),
        detail: detail.into(),
        values: Value::Null,
    }
}
fn http_json(client: &Client, url: &str, query: &[(&str, String)]) -> Result<Value, String> {
    crate::network::json(client, url, query, None, &[], 262144)
}
fn finite(v: &Value) -> Option<f64> {
    v.as_f64().filter(|n| n.is_finite())
}
fn window(
    v: &Value,
    bucket: &str,
    slot: &str,
    label: &str,
    minutes: Option<u64>,
) -> Option<UsageWindow> {
    let used = finite(v.get("usedPercent").or_else(|| v.get("used_percentage"))?)?;
    let resets = v.get("resetsAt").or_else(|| v.get("resets_at"))?.as_u64()?;
    let minutes = minutes.or_else(|| v.get("windowDurationMins")?.as_u64())?;
    if minutes == 0 || resets == 0 {
        return None;
    }
    Some(UsageWindow {
        bucket: bucket.into(),
        slot: slot.into(),
        label: label.into(),
        used_percent: used,
        minutes,
        resets_at: resets,
    })
}
pub fn duration_label(minutes: u64) -> String {
    if minutes % 1440 == 0 {
        format!("{}D", minutes / 1440)
    } else if minutes % 60 == 0 {
        format!("{}H", minutes / 60)
    } else {
        format!("{minutes}M")
    }
}
pub fn normalize_codex(v: &Value) -> Vec<UsageWindow> {
    let mut windows = Vec::new();
    let buckets: Vec<(String, &Value)> = match v
        .get("rateLimitsByLimitId")
        .and_then(Value::as_object)
        .filter(|m| !m.is_empty())
    {
        Some(m) => m.iter().map(|(id, b)| (id.clone(), b)).collect(),
        None => v
            .get("rateLimits")
            .filter(|b| b.is_object())
            .map(|b| {
                vec![(
                    b.get("limitId")
                        .and_then(Value::as_str)
                        .unwrap_or("codex")
                        .into(),
                    b,
                )]
            })
            .unwrap_or_default(),
    };
    for (id, bucket) in buckets {
        let label = bucket
            .get("limitName")
            .and_then(Value::as_str)
            .unwrap_or(&id);
        for slot in ["primary", "secondary"] {
            if let Some(w) = bucket
                .get(slot)
                .and_then(|v| window(v, &id, slot, label, None))
            {
                windows.push(w);
            }
        }
    }
    windows
}
pub fn usage_card(source: &str, scope: &str, w: &UsageWindow, observed: u64) -> DataCard {
    let remaining = (100.0 - w.used_percent).clamp(0.0, 100.0);
    let expiry = (observed + 3600).min(w.resets_at);
    let now = unix_now();
    let remaining_display = if remaining > 0.0 && remaining < 1.0 {
        "<1%".into()
    } else {
        format!("{:.0}%", remaining.floor())
    };
    DataCard {
        source_id: source.into(),
        scope_id: scope.into(),
        fetched_at: now,
        observed_at: observed,
        stale_at: observed + 600,
        expires_at: expiry,
        state: "fresh".into(),
        headline: remaining_display.clone(),
        footer: format!("{} ALLOWANCE", duration_label(w.minutes)),
        detail: format!(
            "{} · {} left · {} window · resets {} · {}",
            w.label,
            remaining_display,
            duration_label(w.minutes),
            chrono::DateTime::from_timestamp(w.resets_at as i64, 0)
                .map(|t| t
                    .with_timezone(&chrono::Local)
                    .format("%b %-d at %-I:%M %p %Z")
                    .to_string())
                .unwrap_or_default(),
            if source == "claude-account" {
                "Claude account allowance (desktop + Code)"
            } else if source == "claude-code" {
                "Last reported by Claude Code"
            } else {
                "Codex account allowance"
            }
        ),
        values: json!({"usedPercent": w.used_percent, "remainingPercent": remaining, "windowDurationMins": w.minutes, "resetsAt": w.resets_at, "bucket": w.bucket, "slot": w.slot}),
    }
}
fn weather_condition(code: u64) -> &'static str {
    match code {
        0 => "CLEAR",
        1 | 2 => "PARTLY CLOUDY",
        3 => "OVERCAST",
        45 | 48 => "FOG",
        51..=57 => "DRIZZLE",
        61..=67 | 80..=82 => "RAIN",
        71..=77 | 85 | 86 => "SNOW",
        95..=99 => "THUNDERSTORM",
        _ => "WEATHER",
    }
}
pub fn normalize_weather(v: &Value, widget: &Widget, now: u64) -> Result<DataCard, String> {
    let current = &v["current"];
    let temp = finite(&current["temperature_2m"]).ok_or("Weather temperature was absent")?;
    let observed = current["time"]
        .as_u64()
        .ok_or("Weather timestamp was absent")?;
    if observed > now + 900 || now.saturating_sub(observed) >= 7200 {
        return Err("Weather report is too old or has an invalid timestamp".into());
    }
    let condition = weather_condition(
        current["weather_code"]
            .as_u64()
            .ok_or("Weather condition was absent")?,
    );
    let location = widget.config.location.as_ref().ok_or("Choose a location")?;
    let unit = if widget.config.unit == "fahrenheit" {
        "F"
    } else {
        "C"
    };
    if v["current_units"]["temperature_2m"].as_str()
        != Some(if unit == "F" { "°F" } else { "°C" })
    {
        return Err("Provider returned unexpected temperature units".into());
    }
    let high = finite(&v["daily"]["temperature_2m_max"][0]);
    let low = finite(&v["daily"]["temperature_2m_min"][0]);
    let footer = if widget.config.weather_layout == "high_low" {
        match (high, low) {
            (Some(h), Some(l)) => format!("H{h:.0} L{l:.0} {unit}"),
            _ => return Err("Daily high/low was absent".into()),
        }
    } else {
        condition.into()
    };
    if footer.len() > 15 {
        return Err("Daily temperature range is too wide for the OLED".into());
    }
    Ok(DataCard {
        source_id: "open-meteo".into(),
        scope_id: format!("{},{}:{}", location.latitude, location.longitude, unit),
        fetched_at: now,
        observed_at: observed,
        stale_at: observed + 1800,
        expires_at: observed + 7200,
        state: "fresh".into(),
        headline: format!("{temp:.0} {unit}"),
        footer,
        detail: format!(
            "{} · {temp:.0} {unit} · {} · Open-Meteo",
            location.name, condition
        ),
        values: json!({"temperature": temp, "unit": unit, "condition": condition, "location": location.name, "high": v["daily"]["temperature_2m_max"][0], "low": v["daily"]["temperature_2m_min"][0]}),
    })
}
pub fn normalize_stock(v: &Value, widget: &Widget, now: u64) -> Result<DataCard, String> {
    let meta = &v["chart"]["result"][0]["meta"];
    let symbol = meta["symbol"]
        .as_str()
        .ok_or("This symbol has no public quote")?;
    if !symbol.eq_ignore_ascii_case(&widget.config.symbol) {
        return Err("Provider quote symbol did not match".into());
    }
    let price = finite(&meta["regularMarketPrice"])
        .filter(|p| *p >= 0.0)
        .ok_or("Quote price was absent or invalid")?;
    let observed = meta["regularMarketTime"]
        .as_u64()
        .ok_or("Quote time was absent")?;
    if observed > now + 300 || now.saturating_sub(observed) > 7 * 86400 {
        return Err("Quote is more than seven days old or has an invalid time".into());
    }
    let currency = meta["currency"]
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 8 && s.bytes().all(|c| c.is_ascii_alphabetic()))
        .ok_or("Quote currency was absent")?;
    let display_currency = if currency == "GBp" { "GBX" } else { currency };
    let offset = meta["gmtoffset"]
        .as_i64()
        .filter(|o| (-86400..=86400).contains(o))
        .unwrap_or(0);
    let time = chrono::DateTime::from_timestamp(observed as i64 + offset, 0)
        .ok_or("Quote time was invalid")?;
    let old_session = now.saturating_sub(observed) > 12 * 3600;
    let status = if old_session { "LAST" } else { "QUOTE" };
    let date = time.format("%m-%d").to_string();
    if format!("{price:.2}").len() > 15 {
        return Err("Quote is too wide for this OLED layout".into());
    }
    let change = finite(&meta["previousClose"])
        .or_else(|| finite(&meta["chartPreviousClose"]))
        .filter(|p| *p > 0.0)
        .map(|previous| (price - previous) / previous * 100.0)
        .filter(|v| v.is_finite());
    let footer = if widget.config.show_change {
        change
            .map(|n| format!("{display_currency} {n:+.1}%{date}"))
            .filter(|s| s.len() <= 15)
            .unwrap_or_else(|| format!("{display_currency} {status} {date}"))
    } else {
        format!("{display_currency} {status} {date}")
    };
    Ok(DataCard {
        source_id: "yahoo-finance".into(), scope_id: symbol.into(),
        fetched_at: now, observed_at: observed, stale_at: now + 1800,
        expires_at: (now + 3 * 86400).min(observed + 7 * 86400),
        state: "fresh".into(), headline: format!("{price:.2}"),
        footer,
        detail: format!("{symbol} · {price:.2} {currency} · last regular-session quote {} · Yahoo Finance · may be delayed", time.format("%b %-d, %-I:%M %p")),
        values: json!({"price":price,"currency":currency,"symbol":symbol,"quoteTime":observed,"exchange":meta["exchangeName"],"changePercent":change,"delay":"not guaranteed; may be delayed"}),
    })
}

impl Providers {
    pub fn new(directory: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(&directory)
            .map_err(|_| "Could not create provider directory".to_string())?;
        let provider = Self {
            directory,
            client: Client::builder()
                .timeout(Duration::from_secs(10))
                .connect_timeout(Duration::from_secs(5))
                .user_agent(concat!("DroidFunApp/", env!("CARGO_PKG_VERSION")))
                .build()
                .map_err(|_| "Could not initialize network client".to_string())?,
            cache: Mutex::new(Cache::default()),
            codex: Mutex::new(HashMap::new()),
            in_flight: Mutex::new(HashMap::new()),
        };
        provider.load_public_cache();
        Ok(provider)
    }
    pub fn search_locations(&self, name: &str) -> Result<Vec<Location>, String> {
        if !(2..=100).contains(&name.trim().len()) {
            return Err("Enter at least two characters of a city name".into());
        }
        let v = http_json(
            &self.client,
            "https://geocoding-api.open-meteo.com/v1/search",
            &[
                ("name", name.trim().into()),
                ("count", "8".into()),
                ("language", "en".into()),
                ("format", "json".into()),
            ],
        )?;
        let Some(rows) = v["results"].as_array() else {
            return Ok(Vec::new());
        };
        Ok(rows
            .iter()
            .filter_map(|r| {
                Some(Location {
                    name: r["name"].as_str()?.into(),
                    latitude: finite(&r["latitude"])?,
                    longitude: finite(&r["longitude"])?,
                    country: r["country"].as_str().unwrap_or("").into(),
                    region: r["admin1"].as_str().unwrap_or("").into(),
                })
            })
            .collect())
    }
    fn load_public_cache(&self) {
        let path = self.directory.join("public-cache.json");
        if fs::metadata(&path)
            .map(|m| m.len() <= 262144)
            .unwrap_or(false)
        {
            if let Ok(bytes) = fs::read(path) {
                if let Ok(entries) =
                    serde_json::from_slice::<HashMap<String, (String, u64, DataCard)>>(&bytes)
                {
                    let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
                    for (key, (sig, next, card)) in entries.into_iter().take(60) {
                        if ["open-meteo", "yahoo-finance"].contains(&card.source_id.as_str())
                            && card.usable(unix_now())
                        {
                            // Startup refresh is due; stored observations keep their own age/expiry.
                            cache.cards.insert(key, (sig, next, card));
                        }
                    }
                }
            }
        }
    }
    fn save_public_cache(&self, cache: &Cache) {
        let entries: HashMap<_, _> = cache
            .cards
            .iter()
            .filter(|(_, (_, _, card))| {
                ["open-meteo", "yahoo-finance"].contains(&card.source_id.as_str())
                    && card.usable(unix_now())
            })
            .take(60)
            .collect();
        if let Ok(bytes) = serde_json::to_vec(&entries) {
            let path = self.directory.join("public-cache.json");
            let temp = path.with_extension("tmp");
            if fs::write(&temp, bytes).is_ok() {
                let _ = fs::rename(temp, path);
            }
        }
    }
    pub fn startup_cards(&self, widgets: &[Widget]) -> HashMap<String, DataCard> {
        let cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        widgets
            .iter()
            .filter_map(|w| {
                if w.config.mode != "live" {
                    return None;
                }
                let sig = format!("{:x}", Sha256::digest(serde_json::to_vec(&w.config).ok()?));
                let key = format!("{}:{}", w.kind, sig);
                cache
                    .cards
                    .get(&key)
                    .filter(|(_, _, d)| d.usable(unix_now()))
                    .map(|(_, _, d)| (w.id.clone(), d.clone()))
            })
            .collect()
    }
    pub fn retry_authentication(&self, widget: &Widget) {
        let sig = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&widget.config).unwrap_or_default())
        );
        let key = format!("{}:{}", widget.kind, sig);
        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        if cache
            .cards
            .get(&key)
            .is_some_and(|(_, _, card)| card.state == "authentication_required")
        {
            cache.cards.remove(&key);
            cache.failures.remove(&key);
        }
    }
    pub fn invalidate_claude(&self) {
        if let Ok(mut cache) = self.cache.lock() {
            cache
                .cards
                .retain(|_, (_, _, card)| !card.source_id.starts_with("claude"));
        }
    }
    pub fn codex_windows(&self, path: &str) -> Result<Vec<UsageWindow>, String> {
        self.codex_read(path).map(|v| v.windows)
    }
    fn codex_read(&self, path: &str) -> Result<CodexSnapshot, String> {
        let mut cache = self
            .codex
            .lock()
            .map_err(|_| "Account reader unavailable")?;
        let now = unix_now();
        if let Some(v) = cache
            .get(path)
            .filter(|v| now.saturating_sub(v.fetched) < 300)
        {
            return Ok(v.clone());
        }
        let (account, limits) = read_codex(path)?;
        if account["account"]["type"].as_str() != Some("chatgpt") {
            return Err(
                "Sign in to the Codex CLI with a ChatGPT account to read allowance windows".into(),
            );
        }
        let scope = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&account["account"]).unwrap_or_default())
        );
        let snapshot = CodexSnapshot {
            fetched: now,
            scope,
            windows: normalize_codex(&limits),
        };
        cache.insert(path.into(), snapshot.clone());
        Ok(snapshot)
    }
    pub fn fetch_cancelable(
        &self,
        widget: &Widget,
        cancel: Arc<std::sync::atomic::AtomicBool>,
    ) -> DataCard {
        let _guard = crate::network::RequestGuard::new(cancel);
        self.fetch(widget)
    }
    pub fn fetch(&self, widget: &Widget) -> DataCard {
        let now = unix_now();
        let signature = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&widget.config).unwrap_or_default())
        );
        let cache_key = format!("{}:{}", widget.kind, signature);
        let key_lock = {
            let mut locks = self.in_flight.lock().unwrap_or_else(|e| e.into_inner());
            if locks.len() >= 128 {
                locks.retain(|_, lock| Arc::strong_count(lock) > 1);
            }
            locks
                .entry(cache_key.clone())
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone()
        };
        let _in_flight = match crate::network::lock_cancellable(&key_lock) {
            Ok(guard) => guard,
            Err(e) => return failure("canceled", "unavailable", &e),
        };
        let cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((sig, next, card)) = cache.cards.get(&cache_key) {
            if *sig == signature && now < *next {
                return card.clone();
            }
        }
        let previous = cache.cards.get(&cache_key).cloned();
        drop(cache);
        let mut card = match widget.kind.as_str() {
            "weather" => {
                if let Some(l) = &widget.config.location {
                    let query = [("latitude",l.latitude.to_string()),("longitude",l.longitude.to_string()),("temperature_unit",widget.config.unit.clone()),("current","temperature_2m,weather_code".into()),("daily","temperature_2m_max,temperature_2m_min".into()),("timezone","auto".into()),("timeformat","unixtime".into()),("forecast_days","1".into())];
                    match http_json(&self.client,"https://api.open-meteo.com/v1/forecast",&query).and_then(|v|normalize_weather(&v,widget,now)) {
                        Ok(d)=>d,Err(e)=>failure("open-meteo",error_state(&e),&e)
                    }
                } else { failure("open-meteo","invalid_configuration","Choose a weather location") }
            },
            "stock" => self.fetch_stock(widget, now),
            "codex" => match self.codex_read(&widget.config.cli_path) {
                Ok(v) => match v.windows.iter().find(|w| w.bucket == widget.config.limit_id && w.slot == widget.config.window) {
                    Some(w) => usage_card("codex",&v.scope,w,v.fetched),
                    None => failure("codex","unavailable","The selected allowance window was not returned. Choose an available window in Edit.")
                }, Err(e) => failure("codex",error_state(&e),&e)
            },
            "claude" => if widget.config.claude_source == "account" {
                match crate::claude_account::read(&self.client, &widget.config.window, &widget.config.claude_profile) {
                    Ok(card) => card, Err(e) => failure("claude-account", error_state(&e), &e)
                }
            } else { self.fetch_claude(widget) },
            _ => failure("unknown","unavailable","Unknown provider"),
        };
        if crate::network::canceled() {
            return failure(
                "canceled",
                "unavailable",
                "Source changed; outdated request canceled",
            );
        }
        let failed = !card.usable(now);
        let failure_state = card.state.clone();
        let retry_after = card
            .detail
            .split("retry after ")
            .nth(1)
            .and_then(|s| s.trim_end_matches('s').parse::<u64>().ok())
            .unwrap_or(0);
        if failed && card.state != "authentication_required" {
            if let Some((sig, _, previous)) = &previous {
                if *sig == signature
                    && previous.usable(now)
                    && ["weather", "stock"].contains(&widget.kind.as_str())
                    && ["unavailable", "rate_limited"].contains(&card.state.as_str())
                {
                    let mut saved = previous.clone();
                    saved.state = "stale".into();
                    saved.detail = format!("Cached report: {}", card.detail);
                    card = saved;
                }
            }
        }
        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        let mut wait = if failed {
            let n = cache.failures.entry(cache_key.clone()).or_default();
            *n = n.saturating_add(1);
            retry_seconds(&failure_state, *n, &cache_key).max(retry_after)
        } else {
            cache.failures.remove(&cache_key);
            match widget.kind.as_str() {
                "weather" => {
                    if card.usable(now) {
                        900
                    } else {
                        120
                    }
                }
                "stock" => {
                    if card.usable(now) {
                        900
                    } else {
                        300
                    }
                }
                "codex" => 300,
                "claude" => {
                    if widget.config.claude_source == "account" {
                        300
                    } else {
                        2
                    }
                }
                _ => 300,
            }
        };
        // A known account-window reset can precede the five-minute poll.
        // Re-fetch at its expiry instead of caching an unusable percentage.
        if !failed && widget.kind == "claude" && widget.config.claude_source == "account" {
            wait = wait.min(card.expires_at.saturating_sub(now).max(1));
        }
        if cache.cards.len() >= 128 {
            if let Some(key) = cache
                .cards
                .iter()
                .min_by_key(|(_, (_, _, card))| card.fetched_at)
                .map(|(key, _)| key.clone())
            {
                cache.cards.remove(&key);
                cache.failures.remove(&key);
            }
        }
        cache.cards.insert(
            cache_key,
            (signature, now.saturating_add(wait), card.clone()),
        );
        if ["weather", "stock"].contains(&widget.kind.as_str()) {
            self.save_public_cache(&cache);
        }
        card
    }
    fn fetch_stock(&self, widget: &Widget, now: u64) -> DataCard {
        let mut url =
            reqwest::Url::parse("https://query1.finance.yahoo.com/v8/finance/chart/").unwrap();
        url.path_segments_mut()
            .unwrap()
            .pop_if_empty()
            .push(&widget.config.symbol);
        match http_json(
            &self.client,
            url.as_str(),
            &[("interval", "1d".into()), ("range", "1d".into())],
        ) {
            Ok(v) => normalize_stock(&v, widget, now)
                .unwrap_or_else(|e| failure("yahoo-finance", "unavailable", &e)),
            Err(e) => failure("yahoo-finance", error_state(&e), &e),
        }
    }
    fn fetch_claude(&self, widget: &Widget) -> DataCard {
        let directory = if widget.config.claude_report_directory.is_empty() {
            self.directory.join("claude/default")
        } else {
            PathBuf::from(&widget.config.claude_report_directory)
        };
        let mut reports: Vec<ClaudeReport> = fs::read_dir(directory)
            .ok()
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .take(256)
            .filter_map(|e| {
                if e.file_type().ok()?.is_symlink() {
                    return None;
                }
                let m = e.metadata().ok()?;
                if !m.is_file() || m.len() > 65536 || e.path().extension()?.to_str()? != "json" {
                    return None;
                }
                let report: ClaudeReport =
                    serde_json::from_slice(&fs::read(e.path()).ok()?).ok()?;
                if report.version != 1
                    || report.profile != "default"
                    || report.session_id.len() > 128
                    || report.observed_at > unix_now() + 60
                {
                    return None;
                }
                if !widget.config.session_id.is_empty()
                    && report.session_id != widget.config.session_id
                {
                    return None;
                }
                Some(report)
            })
            .collect();
        reports.sort_by_key(|r| r.observed_at);
        let Some(r) = reports.last() else {
            return failure("claude-code","unavailable","Waiting for a Claude Code status-line report. Enable the bridge, then use Claude Code.");
        };
        let v = if widget.config.window == "seven_day" {
            &r.seven_day
        } else {
            &r.five_hour
        };
        match v.as_ref() {Some(w)=>usage_card("claude-code",&r.session_id,&UsageWindow::from(w),r.observed_at),None=>failure("claude-code","unavailable","Claude Code did not supply this allowance window; eligible sessions report it after an API response")}
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeReport {
    pub version: u8,
    pub profile: String,
    pub session_id: String,
    pub observed_at: u64,
    pub five_hour: Option<UsageWindowStored>,
    pub seven_day: Option<UsageWindowStored>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageWindowStored {
    pub used_percent: f64,
    pub resets_at: u64,
    pub minutes: u64,
}
impl From<&UsageWindowStored> for UsageWindow {
    fn from(w: &UsageWindowStored) -> Self {
        Self {
            bucket: "claude".into(),
            slot: if w.minutes == 300 {
                "five_hour"
            } else {
                "seven_day"
            }
            .into(),
            label: "Claude Code".into(),
            used_percent: w.used_percent,
            minutes: w.minutes,
            resets_at: w.resets_at,
        }
    }
}
fn codex_path(explicit: &str) -> Result<PathBuf, String> {
    if !explicit.is_empty() {
        let p = PathBuf::from(explicit);
        return if p.is_file() {
            Ok(p)
        } else {
            Err("The selected Codex CLI path does not exist".into())
        };
    }
    let name = if cfg!(windows) { "codex.exe" } else { "codex" };
    let mut paths: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|v| std::env::split_paths(&v).map(|p| p.join(name)).collect())
        .unwrap_or_default();
    if let Some(home) = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }) {
        paths.push(PathBuf::from(home).join(".local/bin").join(name));
    }
    if cfg!(target_os = "macos") {
        paths.extend([PathBuf::from("/opt/homebrew/bin/codex"),PathBuf::from("/usr/local/bin/codex"),PathBuf::from("/Applications/ChatGPT.app/Contents/Resources/codex-cli/CodexCLI.app/Contents/MacOS/codex")]);
    }
    paths
        .into_iter()
        .find(|p| p.is_file())
        .ok_or("Install the Codex CLI or choose its path in the editor".into())
}
struct ChildGuard(std::process::Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn read_codex(path: &str) -> Result<(Value, Value), String> {
    let exe = codex_path(path)?;
    let mut child = ChildGuard(
        Command::new(exe)
            .args(["app-server", "--listen", "stdio://"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| "Could not start the Codex CLI".to_string())?,
    );
    let mut input = child.0.stdin.take().ok_or("Codex input unavailable")?;
    let output = child.0.stdout.take().ok_or("Codex output unavailable")?;
    let (tx, rx) = mpsc::sync_channel(8);
    std::thread::spawn(move || {
        let mut reader = BufReader::new(output);
        loop {
            let mut bytes = Vec::new();
            match (&mut reader).take(262145).read_until(b'\n', &mut bytes) {
                Ok(0) | Err(_) => break,
                _ => {}
            }
            if bytes.len() > 262144 {
                break;
            }
            if let Ok(v) = serde_json::from_slice::<Value>(&bytes) {
                if v.get("id").is_some() {
                    let _ = tx.try_send(v);
                }
            }
        }
    });
    let request_end = Instant::now() + Duration::from_secs(10);
    let mut rpc = |id: u64, method: &str, params: Value| -> Result<Value, String> {
        // This adapter has no turn, tool, login, reset-credit, or mutation methods.
        if !["initialize", "account/read", "account/rateLimits/read"].contains(&method) {
            return Err("Disallowed Codex method".into());
        }
        writeln!(
            input,
            "{}",
            json!({"id":id,"method":method,"params":params})
        )
        .and_then(|_| input.flush())
        .map_err(|_| "Codex request failed".to_string())?;
        let end = request_end;
        loop {
            let remaining = end.saturating_duration_since(Instant::now());
            if crate::network::canceled() {
                return Err("Source changed; Codex read canceled".into());
            }
            if remaining.is_zero() {
                return Err("Codex account read timed out".into());
            }
            let v = match rx.recv_timeout(remaining.min(Duration::from_millis(50))) {
                Ok(v) => v,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(_) => return Err("Codex server exited".into()),
            };
            if v["id"].as_u64() == Some(id) {
                if v.get("error").is_some() {
                    return Err(
                        "Codex could not read account allowances; check CLI sign-in and version"
                            .into(),
                    );
                }
                return Ok(v["result"].clone());
            }
        }
    };
    rpc(
        1,
        "initialize",
        json!({"clientInfo":{"name":"deskbot","title":"DroidFun App","version":env!("CARGO_PKG_VERSION")},"capabilities":{"experimentalApi":false}}),
    )?;
    // initialize notification must precede subsequent requests.
    drop(rpc);
    writeln!(input, "{}", json!({"method":"initialized","params":{}}))
        .and_then(|_| input.flush())
        .map_err(|_| "Codex initialization failed".to_string())?;
    let read = |input: &mut std::process::ChildStdin,
                id: u64,
                method: &str,
                params: Value|
     -> Result<Value, String> {
        writeln!(
            input,
            "{}",
            json!({"id":id,"method":method,"params":params})
        )
        .and_then(|_| input.flush())
        .map_err(|_| "Codex request failed".to_string())?;
        let end = request_end;
        loop {
            let remain = end.saturating_duration_since(Instant::now());
            if crate::network::canceled() {
                return Err("Source changed; Codex read canceled".into());
            }
            if remain.is_zero() {
                return Err("Codex account read timed out".into());
            }
            let v = match rx.recv_timeout(remain.min(Duration::from_millis(50))) {
                Ok(v) => v,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(_) => return Err("Codex server exited".into()),
            };
            if v["id"].as_u64() == Some(id) {
                if v.get("error").is_some() {
                    return Err("Codex allowance read failed; check CLI sign-in".into());
                }
                return Ok(v["result"].clone());
            }
        }
    };
    let account = read(&mut input, 2, "account/read", json!({"refreshToken":false}))?;
    let limits = read(&mut input, 3, "account/rateLimits/read", json!({}))?;
    Ok((account, limits))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Settings, SourceConfig};
    #[test]
    fn codex_windows_keep_buckets_nulls_and_actual_durations_separate() {
        let v = json!({"rateLimits":{"primary":{"usedPercent":99,"windowDurationMins":300,"resetsAt":2000}},"rateLimitsByLimitId":{"codex":{"primary":{"usedPercent":25,"windowDurationMins":10080,"resetsAt":2000},"secondary":null},"other":{"limitName":"Other","primary":{"usedPercent":42,"windowDurationMins":60,"resetsAt":3000},"secondary":{"usedPercent":null,"windowDurationMins":300,"resetsAt":3000}}}});
        let windows = normalize_codex(&v);
        assert_eq!(windows.len(), 2);
        let w = windows.iter().find(|w| w.bucket == "codex").unwrap();
        let d = usage_card("codex", "account", w, 1000);
        assert_eq!(d.headline, "75%");
        assert_eq!(d.footer, "7D ALLOWANCE");
        assert_eq!(d.state_at(1600), "stale");
        assert_eq!(d.state_at(2000), "unavailable");
        assert_eq!(normalize_codex(&json!({"rateLimits":null})).len(), 0);
    }
    #[test]
    fn weather_retains_provider_unit_and_rejects_unit_mismatch() {
        let mut w = Settings::default().widgets[1].clone();
        w.config = SourceConfig {
            mode: "live".into(),
            location: Some(Location {
                name: "Oslo".into(),
                latitude: 59.9,
                longitude: 10.7,
                country: "Norway".into(),
                region: String::new(),
            }),
            unit: "celsius".into(),
            ..SourceConfig::default()
        };
        let v = json!({"current":{"temperature_2m":12.4,"weather_code":61,"time":1000},"current_units":{"temperature_2m":"°C"},"daily":{"temperature_2m_max":[15],"temperature_2m_min":[6]}});
        let d = normalize_weather(&v, &w, 1100).unwrap();
        assert_eq!(d.headline, "12 C");
        assert_eq!(d.footer, "RAIN");
        assert_eq!(d.state_at(2800), "stale");
        assert_eq!(d.state_at(8200), "unavailable");
        w.config.unit = "fahrenheit".into();
        assert!(normalize_weather(&v, &w, 1100).is_err());
    }
    #[test]
    fn public_cache_survives_restart_but_not_configuration_or_expiry() {
        let dir = std::env::temp_dir().join(format!(
            "deskbot-cache-{}-{}",
            std::process::id(),
            unix_now()
        ));
        let mut widget = Settings::default().widgets[2].clone();
        widget.config.mode = "live".into();
        widget.config.symbol = "MSFT".into();
        let now = unix_now();
        let quote = json!({"chart":{"result":[{"meta":{"symbol":"MSFT","regularMarketPrice":535.07,"regularMarketTime":now-60,"currency":"USD","gmtoffset":0}}]}});
        let card = normalize_stock(&quote, &widget, now).unwrap();
        let sig = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&widget.config).unwrap())
        );
        let key = format!("stock:{sig}");
        {
            let providers = Providers::new(dir.clone()).unwrap();
            let mut cache = providers.cache.lock().unwrap();
            cache.cards.insert(key, (sig, now + 900, card));
            providers.save_public_cache(&cache);
        }
        let providers = Providers::new(dir.clone()).unwrap();
        assert_eq!(
            providers.startup_cards(&[widget.clone()])[&widget.id].headline,
            "535.07"
        );
        widget.config.symbol = "AAPL".into();
        assert!(providers.startup_cards(&[widget]).is_empty());
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn alternate_layouts_use_only_verified_values_and_bounded_retries() {
        let mut weather = Settings::default().widgets[1].clone();
        weather.config.location = Some(Location {
            name: "Oslo".into(),
            latitude: 59.9,
            longitude: 10.7,
            country: String::new(),
            region: String::new(),
        });
        weather.config.unit = "celsius".into();
        weather.config.weather_layout = "high_low".into();
        let v = json!({"current":{"temperature_2m":12.4,"weather_code":61,"time":1000},"current_units":{"temperature_2m":"°C"},"daily":{"temperature_2m_max":[15],"temperature_2m_min":[6]}});
        assert_eq!(
            normalize_weather(&v, &weather, 1100).unwrap().footer,
            "H15 L6 C"
        );
        let mut missing = v.clone();
        missing["daily"] = Value::Null;
        assert!(normalize_weather(&missing, &weather, 1100).is_err());
        let mut stock = Settings::default().widgets[2].clone();
        stock.config.symbol = "MSFT".into();
        stock.config.show_change = true;
        let v = json!({"chart":{"result":[{"meta":{"symbol":"MSFT","regularMarketPrice":110,"previousClose":100,"regularMarketTime":1791576001,"currency":"USD","gmtoffset":-14400}}]}});
        assert_eq!(
            normalize_stock(&v, &stock, 1791763200).unwrap().footer,
            "USD +10.0%10-09"
        );
        let mut missing = v.clone();
        missing["chart"]["result"][0]["meta"]["previousClose"] = Value::Null;
        assert_eq!(
            normalize_stock(&missing, &stock, 1791763200)
                .unwrap()
                .footer,
            "USD LAST 10-09"
        );
        assert!((30..=36).contains(&retry_seconds("unavailable", 1, "a")));
        assert!((120..=144).contains(&retry_seconds("unavailable", 2, "a")));
        assert_eq!(retry_seconds("authentication_required", 1, "a"), 300);
        assert_eq!(retry_seconds("rate_limited", 1, "a"), 600);
    }

    #[test]
    fn public_quote_requires_matching_symbol_time_price_and_currency() {
        let mut w = Settings::default().widgets[2].clone();
        w.config.symbol = "MSFT".into();
        let now = 1791763200;
        let v = json!({"chart":{"result":[{"meta":{"symbol":"MSFT","regularMarketPrice":535.07,"regularMarketTime":1791576001,"currency":"USD","gmtoffset":-14400}}]}});
        let d = normalize_stock(&v, &w, now).unwrap();
        assert_eq!(d.headline, "535.07");
        assert_eq!(d.footer, "USD LAST 10-09");
        assert_eq!(d.values["currency"], "USD");
        w.config.symbol = "AAPL".into();
        assert!(normalize_stock(&v, &w, now).is_err());
        assert!(normalize_stock(&json!({"chart":{"error":{}}}), &w, now).is_err());
        w.config.symbol = "MSFT".into();
        assert!(normalize_stock(&v, &w, now + 8 * 86400).is_err());
        let mut missing = v.clone();
        missing["chart"]["result"][0]["meta"]["currency"] = Value::Null;
        assert!(normalize_stock(&missing, &w, now).is_err());
        missing = v.clone();
        missing["chart"]["result"][0]["meta"]["regularMarketPrice"] = Value::Null;
        assert!(normalize_stock(&missing, &w, now).is_err());
        let mut pence = v.clone();
        pence["chart"]["result"][0]["meta"]["currency"] = json!("GBp");
        let d = normalize_stock(&pence, &w, now).unwrap();
        assert_eq!(d.footer, "GBX LAST 10-09");
        assert_eq!(d.values["currency"], "GBp");
    }
}
