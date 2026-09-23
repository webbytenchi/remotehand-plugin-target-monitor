use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;
const PING_TIMEOUT: Duration = Duration::from_secs(2);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
const HTTP_TIMEOUT: Duration = Duration::from_secs(4);
const SAMPLE_INTERVAL: Duration = Duration::from_secs(2);
const HISTORY_LIMIT: usize = 1800;
const TOPBAR_HISTORY_LIMIT: usize = 30;
const STATE_CONFIRMATION_SAMPLES: u32 = 2;
const STORAGE_KEY: &str = "config";
const DEFAULT_TARGET_ID: &str = "target-8-8-8-8";
const DEFAULT_TARGET_NAME: &str = "Google DNS";
const DEFAULT_TARGET: &str = "8.8.8.8";
const MAX_SOUND_FILE_BYTES: u64 = 2 * 1024 * 1024;
const MAX_TOTAL_SOUND_BYTES: u64 = 12 * 1024 * 1024;
const MAX_SOUND_DURATION_MS: u64 = 10_000;
const DEFAULT_GRAPH_NORMAL_COLOR: &str = "#70a7e8";
const DEFAULT_GRAPH_HIGH_COLOR: &str = "#f1cf87";
const DEFAULT_GRAPH_OFFLINE_COLOR: &str = "#efaaaa";
const DEFAULT_TARGET_NAME_COLOR: &str = "#a8b4c2";
const DEFAULT_LATENCY_COLOR: &str = "#dce4ee";
const DEFAULT_BACKGROUND_COLOR: &str = "#16263a";
const DEFAULT_BORDER_COLOR: &str = "#3b5f8d";

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}
fn default_true() -> bool { true }
fn default_graph_normal_color() -> String { DEFAULT_GRAPH_NORMAL_COLOR.to_string() }
fn default_graph_high_color() -> String { DEFAULT_GRAPH_HIGH_COLOR.to_string() }
fn default_graph_offline_color() -> String { DEFAULT_GRAPH_OFFLINE_COLOR.to_string() }
fn default_target_name_color() -> String { DEFAULT_TARGET_NAME_COLOR.to_string() }
fn default_latency_color() -> String { DEFAULT_LATENCY_COLOR.to_string() }
fn default_background_color() -> String { DEFAULT_BACKGROUND_COLOR.to_string() }
fn default_border_color() -> String { DEFAULT_BORDER_COLOR.to_string() }
fn default_network_identity_topbar_source() -> String { "public".to_string() }

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HostReady {
    #[serde(rename = "type")]
    message_type: String,
    plugin_id: String,
    api_version: u32,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PluginHello<'a> {
    #[serde(rename = "type")]
    message_type: &'static str,
    plugin_id: &'a str,
    api_version: u32,
    plugin_version: &'a str,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HostRequest {
    #[serde(rename = "type")]
    message_type: String,
    request_id: u64,
    method: String,
    #[serde(default)]
    params: Value,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PluginResponse {
    #[serde(rename = "type")]
    message_type: &'static str,
    request_id: u64,
    ok: bool,
    result: Value,
    error: String,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PluginEvent {
    #[serde(rename = "type")]
    message_type: &'static str,
    event: &'static str,
    payload: Value,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PluginHostRequest {
    #[serde(rename = "type")]
    message_type: &'static str,
    request_id: u64,
    method: &'static str,
    params: Value,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HostResponse {
    #[serde(rename = "type")]
    _message_type: String,
    request_id: u64,
    #[serde(default)]
    ok: bool,
    #[serde(default)]
    result: Value,
    #[serde(default)]
    error: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SoundConfig {
    filename: String,
    mime: String,
    data_url: String,
    size_bytes: u64,
    duration_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyAlertRule {
    #[serde(default = "default_true")]
    enabled: bool,
    #[serde(default = "default_legacy_cooldown")]
    cooldown_seconds: u64,
    #[serde(default)]
    sound: Option<SoundConfig>,
}
fn default_legacy_cooldown() -> u64 { 60 }
fn legacy_rule(cooldown_seconds: u64) -> LegacyAlertRule { LegacyAlertRule { enabled: true, cooldown_seconds, sound: None } }
impl Default for LegacyAlertRule {
    fn default() -> Self { legacy_rule(default_legacy_cooldown()) }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyAlertConfig {
    #[serde(default = "default_true")]
    enabled: bool,
    #[serde(default)]
    high_latency: LegacyAlertRule,
    #[serde(default)]
    offline: LegacyAlertRule,
    #[serde(default)]
    recovery: LegacyAlertRule,
    #[serde(default)]
    reminders_enabled: bool,
    #[serde(default = "default_reminder_interval")]
    reminder_interval_seconds: u64,
}
fn default_reminder_interval() -> u64 { 300 }
impl Default for LegacyAlertConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            high_latency: legacy_rule(60),
            offline: legacy_rule(300),
            recovery: legacy_rule(5),
            reminders_enabled: false,
            reminder_interval_seconds: default_reminder_interval(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GenericAlertRule {
    id: String,
    name: String,
    #[serde(default = "default_true")]
    enabled: bool,
    operator: String,
    #[serde(default)]
    value: Option<Value>,
    #[serde(default)]
    sound: Option<SoundConfig>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TcpCheck {
    id: String,
    name: String,
    port: u16,
    #[serde(default = "default_true")]
    enabled: bool,
    #[serde(default)]
    alert_rule_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HttpCheck {
    id: String,
    name: String,
    url: String,
    #[serde(default = "default_true")]
    enabled: bool,
    #[serde(default)]
    response_mode: String,
    #[serde(default)]
    regex: String,
    #[serde(default)]
    monitor_whole_response: bool,
    #[serde(default)]
    detect_changes: bool,
    #[serde(default)]
    discovered_fields: BTreeMap<String, String>,
    #[serde(default)]
    field_alerts: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TopbarConfig {
    #[serde(default = "default_true")]
    enabled: bool,
    #[serde(default = "default_topbar_title")]
    title_source: String,
    #[serde(default = "default_topbar_value")]
    value_source: String,
    #[serde(default = "default_topbar_graph")]
    graph_source: String,
}
fn default_topbar_title() -> String { "target.name".to_string() }
fn default_topbar_value() -> String { "ping.latency".to_string() }
fn default_topbar_graph() -> String { "ping.latency".to_string() }
impl Default for TopbarConfig {
    fn default() -> Self {
        Self { enabled: true, title_source: default_topbar_title(), value_source: default_topbar_value(), graph_source: default_topbar_graph() }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MonitorTarget {
    id: String,
    name: String,
    address: String,
    #[serde(default = "default_true")]
    enabled: bool,
    #[serde(default = "default_true")]
    ping_enabled: bool,
    #[serde(default)]
    show_in_system_tray: bool,
    #[serde(default)]
    tcp_checks: Vec<TcpCheck>,
    #[serde(default)]
    http_checks: Vec<HttpCheck>,
    #[serde(default)]
    topbar: TopbarConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    graph_color: Option<String>,
    #[serde(default = "default_graph_normal_color")]
    graph_normal_color: String,
    #[serde(default = "default_graph_high_color")]
    graph_high_color: String,
    #[serde(default = "default_graph_offline_color")]
    graph_offline_color: String,
    #[serde(default = "default_target_name_color")]
    target_name_color: String,
    #[serde(default = "default_latency_color")]
    latency_color: String,
    #[serde(default = "default_background_color")]
    background_color: String,
    #[serde(default = "default_border_color")]
    border_color: String,
}
impl MonitorTarget {
    fn graph_color_for_state(&self, state: &str) -> &str {
        match state {
            "OFFLINE" | "ERROR" => &self.graph_offline_color,
            "HIGH LATENCY" | "DEGRADED" => &self.graph_high_color,
            _ => self.graph_color.as_deref().unwrap_or(&self.graph_normal_color),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredConfig {
    #[serde(default)]
    targets: Vec<MonitorTarget>,
    #[serde(default)]
    alerts: LegacyAlertConfig,
    #[serde(default)]
    alert_rules: Vec<GenericAlertRule>,
    #[serde(default)]
    show_network_identity_topbar: bool,
    #[serde(default = "default_network_identity_topbar_source")]
    network_identity_topbar_source: String,
}
impl Default for StoredConfig {
    fn default() -> Self { default_config() }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SampleRecord {
    timestamp: u64,
    latency_ms: Option<f64>,
    packet_loss_percent: f64,
    state: String,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MetricRecord {
    timestamp: u64,
    value: f64,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TcpResult {
    id: String,
    name: String,
    port: u16,
    reachable: bool,
    latency_ms: Option<f64>,
    error: Option<String>,
    timestamp: u64,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct HttpResult {
    id: String,
    name: String,
    url: String,
    status_code: Option<u16>,
    response_time_ms: Option<f64>,
    content_type: String,
    body: String,
    fields: BTreeMap<String, Value>,
    error: Option<String>,
    timestamp: u64,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ActiveAlert {
    key: String,
    target_id: String,
    rule_id: String,
    rule_name: String,
    source: String,
    current_value: Value,
    since: u64,
    acknowledged: bool,
}

#[derive(Debug, Clone)]
struct TargetMonitor {
    target: MonitorTarget,
    samples: Vec<SampleRecord>,
    last_state: String,
    pending_state: String,
    pending_count: u32,
    tcp_results: BTreeMap<String, TcpResult>,
    http_results: BTreeMap<String, HttpResult>,
    metric_history: BTreeMap<String, Vec<MetricRecord>>,
    previous_values: BTreeMap<String, Value>,
    active_alerts: BTreeMap<String, ActiveAlert>,
    last_legacy_alert_at: BTreeMap<String, u64>,
}
impl TargetMonitor {
    fn new(target: MonitorTarget) -> Self {
        Self {
            target,
            samples: Vec::new(),
            last_state: "READY".into(),
            pending_state: "READY".into(),
            pending_count: 0,
            tcp_results: BTreeMap::new(),
            http_results: BTreeMap::new(),
            metric_history: BTreeMap::new(),
            previous_values: BTreeMap::new(),
            active_alerts: BTreeMap::new(),
            last_legacy_alert_at: BTreeMap::new(),
        }
    }
    fn record_ping(&mut self, sample: &Value) -> Option<(String, String)> {
        let state = sample.get("state").and_then(Value::as_str).unwrap_or("OFFLINE").to_string();
        let previous_state = self.last_state.clone();
        if state == self.last_state {
            self.pending_state = state.clone();
            self.pending_count = 0;
        } else if state == self.pending_state {
            self.pending_count += 1;
            if self.pending_count >= STATE_CONFIRMATION_SAMPLES {
                self.last_state = state.clone();
                self.pending_count = 0;
            }
        } else {
            self.pending_state = state.clone();
            self.pending_count = 1;
        }
        let timestamp = now_ms();
        let latency = sample.get("latencyMs").and_then(Value::as_f64);
        self.samples.push(SampleRecord {
            timestamp,
            latency_ms: latency,
            packet_loss_percent: sample.get("packetLossPercent").and_then(Value::as_f64).unwrap_or(100.0),
            state,
        });
        trim_history(&mut self.samples, HISTORY_LIMIT);
        if let Some(v) = latency { self.record_metric("ping.latency", v, timestamp); }
        if previous_state != self.last_state { Some((previous_state, self.last_state.clone())) } else { None }
    }
    fn record_metric(&mut self, source: &str, value: f64, timestamp: u64) {
        let history = self.metric_history.entry(source.to_string()).or_default();
        history.push(MetricRecord { timestamp, value });
        trim_history(history, HISTORY_LIMIT);
    }
    fn unread_alerts(&self) -> usize { self.active_alerts.values().filter(|a| !a.acknowledged).count() }
    fn overall_state(&self) -> &'static str {
        if !self.target.enabled { return "DISABLED"; }
        if self.target.ping_enabled && self.last_state == "OFFLINE" { return "OFFLINE"; }
        let tcp_bad = self.target.tcp_checks.iter().filter(|c| c.enabled).any(|c| self.tcp_results.get(&c.id).map(|r| !r.reachable).unwrap_or(false));
        let http_bad = self.target.http_checks.iter().filter(|c| c.enabled).any(|c| self.http_results.get(&c.id).map(|r| r.error.is_some() || r.status_code.unwrap_or(599) >= 400).unwrap_or(false));
        if tcp_bad || http_bad || self.last_state == "HIGH LATENCY" || !self.active_alerts.is_empty() { "DEGRADED" } else { "ONLINE" }
    }
    fn summary(&self) -> Value {
        let latest = self.samples.last();
        let ping_history: Vec<Value> = self.samples.iter().map(|s| json!({
            "timestamp": s.timestamp, "latencyMs": s.latency_ms, "packetLossPercent": s.packet_loss_percent, "state": s.state
        })).collect();
        let metric_history: BTreeMap<String, Vec<Value>> = self.metric_history.iter().map(|(k,v)| {
            (k.clone(), v.iter().map(|m| json!({"timestamp":m.timestamp,"value":m.value})).collect())
        }).collect();
        json!({
            "id": self.target.id,
            "name": self.target.name,
            "target": self.target.address,
            "enabled": self.target.enabled,
            "pingEnabled": self.target.ping_enabled,
            "state": self.overall_state(),
            "pingState": self.last_state,
            "currentLatencyMs": latest.and_then(|s| s.latency_ms),
            "packetLossPercent": latest.map(|s| s.packet_loss_percent).unwrap_or(0.0),
            "history": ping_history,
            "tcpResults": self.tcp_results.values().collect::<Vec<_>>(),
            "httpResults": self.http_results.values().collect::<Vec<_>>(),
            "metricHistory": metric_history,
            "activeAlerts": self.active_alerts.values().collect::<Vec<_>>(),
            "unreadAlertCount": self.unread_alerts(),
            "config": self.target,
        })
    }
}
fn trim_history<T>(items: &mut Vec<T>, limit: usize) {
    if items.len() > limit { items.drain(..items.len()-limit); }
}

#[derive(Debug)]
struct MonitorStore {
    targets: BTreeMap<String, TargetMonitor>,
    config: StoredConfig,
}
impl MonitorStore {
    fn new() -> Self {
        let config = default_config();
        let targets = config.targets.iter().cloned().map(|t| (t.id.clone(), TargetMonitor::new(t))).collect();
        Self { targets, config }
    }
    fn replace_config(&mut self, config: StoredConfig) {
        let old = std::mem::take(&mut self.targets);
        let mut next = BTreeMap::new();
        for target in &config.targets {
            if let Some(existing) = old.get(&target.id) {
                if existing.target.address == target.address {
                    let mut monitor = existing.clone();
                    monitor.target = target.clone();
                    next.insert(target.id.clone(), monitor);
                    continue;
                }
            }
            next.insert(target.id.clone(), TargetMonitor::new(target.clone()));
        }
        self.targets = next;
        self.config = config;
    }
    fn summaries(&self) -> Vec<Value> { self.targets.values().map(TargetMonitor::summary).collect() }
}

fn default_target() -> MonitorTarget {
    MonitorTarget {
        id: DEFAULT_TARGET_ID.into(), name: DEFAULT_TARGET_NAME.into(), address: DEFAULT_TARGET.into(),
        enabled: true, ping_enabled: true, show_in_system_tray: false, tcp_checks: vec![], http_checks: vec![],
        topbar: TopbarConfig::default(), graph_color: None, graph_normal_color: default_graph_normal_color(),
        graph_high_color: default_graph_high_color(), graph_offline_color: default_graph_offline_color(),
        target_name_color: default_target_name_color(), latency_color: default_latency_color(),
        background_color: default_background_color(), border_color: default_border_color(),
    }
}
fn default_config() -> StoredConfig {
    StoredConfig {
        targets: vec![default_target()], alerts: LegacyAlertConfig::default(), alert_rules: vec![],
        show_network_identity_topbar: false, network_identity_topbar_source: default_network_identity_topbar_source(),
    }
}

fn valid_hex_color(value: &str) -> bool {
    value.len() == 7 && value.starts_with('#') && value.as_bytes()[1..].iter().all(|b| b.is_ascii_hexdigit())
}
fn validate_sound(sound: &SoundConfig) -> Result<(), String> {
    if sound.filename.trim().is_empty() || sound.filename.len() > 128 { return Err("sound filename is missing or too long".into()); }
    if !matches!(sound.mime.as_str(), "audio/mpeg"|"audio/mp3"|"audio/wav"|"audio/x-wav") { return Err("sound must be MP3 or WAV".into()); }
    if sound.size_bytes == 0 || sound.size_bytes > MAX_SOUND_FILE_BYTES { return Err("sound file must be 1 byte to 2 MiB".into()); }
    if sound.duration_ms == 0 || sound.duration_ms > MAX_SOUND_DURATION_MS { return Err("sound duration must be 1-10 seconds".into()); }
    Ok(())
}
fn validate_config(config: &StoredConfig) -> Result<(), String> {
    if config.targets.is_empty() || config.targets.len() > 64 { return Err("Target Monitor requires 1-64 targets".into()); }
    let mut target_ids = BTreeSet::new();
    let mut rule_ids = BTreeSet::new();
    let mut sound_total = 0u64;
    for rule in &config.alert_rules {
        if rule.id.trim().is_empty() || !rule_ids.insert(rule.id.clone()) { return Err("alert rule ids must be unique and non-empty".into()); }
        if rule.name.trim().is_empty() || rule.name.len() > 128 { return Err("alert rule name is missing or too long".into()); }
        if !matches!(rule.operator.as_str(), ">="|"<="|"="|"equals"|"changed") { return Err(format!("unsupported alert operator: {}", rule.operator)); }
        if rule.operator != "changed" && rule.value.is_none() { return Err(format!("alert rule {} requires a comparison value", rule.name)); }
        if let Some(sound) = &rule.sound { validate_sound(sound)?; sound_total += sound.size_bytes; }
    }
    for legacy in [&config.alerts.high_latency, &config.alerts.offline, &config.alerts.recovery] {
        if let Some(sound) = &legacy.sound { validate_sound(sound)?; sound_total += sound.size_bytes; }
    }
    if sound_total > MAX_TOTAL_SOUND_BYTES { return Err("total alert sounds exceed 12 MiB".into()); }
    for target in &config.targets {
        if target.id.trim().is_empty() || !target_ids.insert(target.id.clone()) { return Err("target ids must be unique and non-empty".into()); }
        if target.name.trim().is_empty() || target.name.len() > 128 { return Err("target name is missing or too long".into()); }
        resolve_target(&target.address)?;
        for color in [&target.graph_normal_color,&target.graph_high_color,&target.graph_offline_color,&target.target_name_color,&target.latency_color,&target.background_color,&target.border_color] {
            if !valid_hex_color(color) { return Err("target colors must use #RRGGBB".into()); }
        }
        if target.tcp_checks.len() > 32 || target.http_checks.len() > 32 { return Err("maximum 32 TCP and 32 HTTP checks per target".into()); }
        let mut check_ids = BTreeSet::new();
        for check in &target.tcp_checks {
            if check.id.trim().is_empty() || !check_ids.insert(check.id.clone()) { return Err("TCP check ids must be unique".into()); }
            if check.name.trim().is_empty() { return Err("TCP check name is required".into()); }
            for id in &check.alert_rule_ids { if !rule_ids.contains(id) { return Err(format!("unknown alert rule: {id}")); } }
        }
        for check in &target.http_checks {
            if check.id.trim().is_empty() || !check_ids.insert(check.id.clone()) { return Err("HTTP check ids must be unique".into()); }
            if check.name.trim().is_empty() || !(check.url.starts_with("http://") || check.url.starts_with("https://")) { return Err("HTTP checks require a name and http(s) URL".into()); }
            if !check.regex.is_empty() { Regex::new(&check.regex).map_err(|e| format!("invalid regex for {}: {e}", check.name))?; }
            for ids in check.field_alerts.values() { for id in ids { if !rule_ids.contains(id) { return Err(format!("unknown alert rule: {id}")); } } }
        }
    }
    Ok(())
}
fn normalize_config(mut config: StoredConfig) -> Result<StoredConfig, String> {
    if config.targets.is_empty() { config.targets.push(default_target()); }
    validate_config(&config)?;
    Ok(config)
}

fn read_frame<R: Read>(reader: &mut R) -> Result<Vec<u8>, String> {
    let mut length=[0u8;4]; reader.read_exact(&mut length).map_err(|e|format!("failed to read plugin frame length: {e}"))?;
    let length=u32::from_le_bytes(length) as usize;
    if length==0 || length>MAX_FRAME_BYTES { return Err(format!("invalid frame length: {length}")); }
    let mut payload=vec![0u8;length]; reader.read_exact(&mut payload).map_err(|e|format!("failed to read plugin frame payload: {e}"))?; Ok(payload)
}
fn write_frame_raw<W: Write,T:Serialize>(writer:&mut W,value:&T)->Result<(),String>{
    let payload=serde_json::to_vec(value).map_err(|e|format!("failed to encode frame: {e}"))?;
    writer.write_all(&(payload.len() as u32).to_le_bytes()).and_then(|_|writer.write_all(&payload)).and_then(|_|writer.flush()).map_err(|e|e.to_string())
}
fn send_outbound(output:&mpsc::Sender<Value>,value:impl Serialize)->Result<(),String>{
    output.send(serde_json::to_value(value).map_err(|e|e.to_string())?).map_err(|_|"plugin output writer stopped".into())
}
fn host_request(input:&mut impl Read, output:&mpsc::Sender<Value>, request_id:&mut u64, method:&'static str, params:Value)->Result<Value,String>{
    let id=*request_id; *request_id+=1;
    send_outbound(output,PluginHostRequest{message_type:"plugin.request",request_id:id,method,params})?;
    loop {
        let payload=read_frame(input)?;
        if let Ok(response)=serde_json::from_slice::<HostResponse>(&payload) {
            if response.request_id==id { return if response.ok {Ok(response.result)} else {Err(response.error)}; }
        }
    }
}
fn load_config(input:&mut impl Read,output:&mpsc::Sender<Value>,request_id:&mut u64)->Result<Option<StoredConfig>,String>{
    let result=host_request(input,output,request_id,"storage.get",json!({"key":STORAGE_KEY}))?;
    match result.get("value") { None|Some(Value::Null)=>Ok(None), Some(v)=>serde_json::from_value(v.clone()).map(Some).map_err(|e|format!("invalid stored Target Monitor config: {e}")) }
}
fn save_config(input:&mut impl Read,output:&mpsc::Sender<Value>,request_id:&mut u64,config:&StoredConfig)->Result<(),String>{
    host_request(input,output,request_id,"storage.set",json!({"key":STORAGE_KEY,"value":config}))?; Ok(())
}
fn ensure_config_loaded(store:&Arc<Mutex<MonitorStore>>,input:&mut impl Read,output:&mpsc::Sender<Value>,request_id:&mut u64)->Result<(),String>{
    let config=match load_config(input,output,request_id)? { Some(c)=>normalize_config(c)?, None=>default_config() };
    store.lock().map_err(|_|"monitor state lock poisoned".to_string())?.replace_config(config); Ok(())
}

fn resolve_target(target:&str)->Result<IpAddr,String>{
    let target=target.trim();
    if target.is_empty()||target.len()>255{return Err("target must be 1-255 characters".into());}
    if let Ok(ip)=target.parse(){return Ok(ip);}
    (target,0).to_socket_addrs().map_err(|e|format!("failed to resolve target: {e}"))?.next().map(|a|a.ip()).ok_or_else(||"target did not resolve".into())
}
fn sample_ping(target:&MonitorTarget)->Value{
    if !target.ping_enabled { return json!({"state":"DISABLED","latencyMs":Value::Null,"packetLossPercent":0.0}); }
    match resolve_target(&target.address) {
        Ok(ip)=>match ping::new(ip).timeout(PING_TIMEOUT).send() {
            Ok(reply)=>{let ms=reply.rtt.as_secs_f64()*1000.0; json!({"state":if ms>200.0{"HIGH LATENCY"}else{"NORMAL"},"latencyMs":(ms*1000.0).round()/1000.0,"packetLossPercent":0.0})}
            Err(e)=>json!({"state":"OFFLINE","latencyMs":Value::Null,"packetLossPercent":100.0,"error":e.to_string()})
        },
        Err(e)=>json!({"state":"OFFLINE","latencyMs":Value::Null,"packetLossPercent":100.0,"error":e})
    }
}
fn socket_for(address:&str,port:u16)->Result<SocketAddr,String>{
    (address,port).to_socket_addrs().map_err(|e|e.to_string())?.next().ok_or_else(||"service address did not resolve".into())
}
fn sample_tcp(target:&MonitorTarget,check:&TcpCheck)->TcpResult{
    let start=Instant::now();
    let (reachable,error)=match socket_for(&target.address,check.port) {
        Ok(addr)=>match TcpStream::connect_timeout(&addr,CONNECT_TIMEOUT){Ok(_)=>(true,None),Err(e)=>(false,Some(e.to_string()))},
        Err(e)=>(false,Some(e)),
    };
    TcpResult{id:check.id.clone(),name:check.name.clone(),port:check.port,reachable,latency_ms:if reachable{Some(start.elapsed().as_secs_f64()*1000.0)}else{None},error,timestamp:now_ms()}
}
fn flatten_json(prefix:&str,value:&Value,out:&mut BTreeMap<String,Value>){
    match value {
        Value::Object(map)=>for (k,v) in map { let p=if prefix.is_empty(){k.clone()}else{format!("{prefix}.{k}")}; flatten_json(&p,v,out); },
        Value::Array(values)=>for (i,v) in values.iter().enumerate().take(32){ let p=if prefix.is_empty(){i.to_string()}else{format!("{prefix}.{i}")}; flatten_json(&p,v,out); },
        _=>{ if !prefix.is_empty(){out.insert(prefix.to_string(),value.clone());} }
    }
}
fn extract_fields(mode:&str,pattern:&str,body:&str,content_type:&str)->Result<BTreeMap<String,Value>,String>{
    let mut out=BTreeMap::new();
    let mode=if mode.is_empty()||mode=="auto" { if content_type.contains("json"){"json"}else{"text"} } else { mode };
    if mode=="json" {
        let value:Value=serde_json::from_str(body).map_err(|e|format!("JSON parse failed: {e}"))?;
        flatten_json("",&value,&mut out);
    } else if mode=="regex" {
        let re=Regex::new(pattern).map_err(|e|format!("regex error: {e}"))?;
        let caps=re.captures(body).ok_or_else(||"regex did not match response".to_string())?;
        for name in re.capture_names().flatten() {
            if let Some(m)=caps.name(name) {
                let raw=m.as_str();
                out.insert(name.to_string(),raw.parse::<f64>().map(Value::from).unwrap_or_else(|_|Value::String(raw.to_string())));
            }
        }
        if out.is_empty() { return Err("regex must use named capture groups".into()); }
    } else {
        out.insert("response".into(),Value::String(body.to_string()));
    }
    Ok(out)
}
fn sample_http(check:&HttpCheck)->HttpResult{
    let start=Instant::now();
    let call=ureq::get(&check.url).timeout(HTTP_TIMEOUT).call();
    let (status,response)=match call { Ok(r)=>(Some(r.status()),Some(r)), Err(ureq::Error::Status(code,r))=>(Some(code),Some(r)), Err(e)=>return HttpResult{id:check.id.clone(),name:check.name.clone(),url:check.url.clone(),status_code:None,response_time_ms:Some(start.elapsed().as_secs_f64()*1000.0),content_type:String::new(),body:String::new(),fields:BTreeMap::new(),error:Some(e.to_string()),timestamp:now_ms()} };
    let content_type=response.as_ref().and_then(|r|r.header("content-type")).unwrap_or("").to_string();
    let body=response.unwrap().into_string().unwrap_or_default();
    let fields=extract_fields(&check.response_mode,&check.regex,&body,&content_type);
    HttpResult{id:check.id.clone(),name:check.name.clone(),url:check.url.clone(),status_code:status,response_time_ms:Some(start.elapsed().as_secs_f64()*1000.0),content_type,body:body.chars().take(65536).collect(),fields:fields.clone().unwrap_or_default(),error:fields.err(),timestamp:now_ms()}
}
fn numeric(v:&Value)->Option<f64>{v.as_f64().or_else(||v.as_str().and_then(|s|s.parse().ok()))}
fn rule_matches(rule:&GenericAlertRule,current:&Value,previous:Option<&Value>)->bool{
    if !rule.enabled{return false;}
    match rule.operator.as_str() {
        ">="=>numeric(current).zip(rule.value.as_ref().and_then(numeric)).map(|(a,b)|a>=b).unwrap_or(false),
        "<="=>numeric(current).zip(rule.value.as_ref().and_then(numeric)).map(|(a,b)|a<=b).unwrap_or(false),
        "="=>numeric(current).zip(rule.value.as_ref().and_then(numeric)).map(|(a,b)|(a-b).abs()<f64::EPSILON).unwrap_or(false),
        "equals"=>rule.value.as_ref().map(|v|v==current).unwrap_or(false),
        "changed"=>previous.map(|v|v!=current).unwrap_or(false),
        _=>false
    }
}
fn sound_payload(sound:&Option<SoundConfig>)->Value{
    match sound { Some(s)=>json!({"filename":s.filename,"mime":s.mime,"dataUrl":s.data_url,"durationMs":s.duration_ms}),None=>Value::Null }
}
fn evaluate_rules(output:&mpsc::Sender<Value>,monitor:&mut TargetMonitor,rules:&[GenericAlertRule],source:&str,current:Value,assigned:&[String]){
    let previous=monitor.previous_values.get(source).cloned();
    for rule_id in assigned {
        let Some(rule)=rules.iter().find(|r|&r.id==rule_id) else {continue};
        let key=format!("{source}:{rule_id}");
        let matches=rule_matches(rule,&current,previous.as_ref());
        if matches && !monitor.active_alerts.contains_key(&key) {
            let alert=ActiveAlert{key:key.clone(),target_id:monitor.target.id.clone(),rule_id:rule.id.clone(),rule_name:rule.name.clone(),source:source.into(),current_value:current.clone(),since:now_ms(),acknowledged:false};
            monitor.active_alerts.insert(key.clone(),alert.clone());
            let _=send_outbound(output,PluginEvent{message_type:"plugin.event",event:"monitor.alert",payload:json!({"targetId":monitor.target.id,"targetName":monitor.target.name,"alertType":"rule","ruleId":rule.id,"ruleName":rule.name,"source":source,"currentValue":current,"sound":sound_payload(&rule.sound),"fallbackBeep":rule.sound.is_none()})});
        } else if matches {
            if let Some(alert)=monitor.active_alerts.get_mut(&key){alert.current_value=current.clone();}
        } else {
            monitor.active_alerts.remove(&key);
        }
    }
    monitor.previous_values.insert(source.into(),current);
}
fn legacy_alert(output:&mpsc::Sender<Value>,monitor:&mut TargetMonitor,alerts:&LegacyAlertConfig,transition:Option<(String,String)>){
    if !alerts.enabled{return;}
    let current=monitor.last_state.clone();
    let selection=if let Some((previous,next))=transition {
        match (previous.as_str(),next.as_str()) {
            ("NORMAL","HIGH LATENCY")|("READY","HIGH LATENCY")=>Some(("highLatency",&alerts.high_latency,false)),
            ("NORMAL","OFFLINE")|("HIGH LATENCY","OFFLINE")|("READY","OFFLINE")=>Some(("offline",&alerts.offline,false)),
            ("OFFLINE","NORMAL")|("OFFLINE","HIGH LATENCY")|("HIGH LATENCY","NORMAL")=>Some(("recovery",&alerts.recovery,false)),
            _=>None
        }
    } else if alerts.reminders_enabled {
        match current.as_str() {
            "HIGH LATENCY"=>Some(("highLatency",&alerts.high_latency,true)),
            "OFFLINE"=>Some(("offline",&alerts.offline,true)),
            _=>None
        }
    } else { None };
    let Some((kind,rule,reminder))=selection else{return};
    if !rule.enabled{return;}
    let now=now_ms();
    let interval=if reminder{alerts.reminder_interval_seconds}else{rule.cooldown_seconds}.saturating_mul(1000);
    if monitor.last_legacy_alert_at.get(kind).map(|last|now.saturating_sub(*last)<interval).unwrap_or(false){return;}
    monitor.last_legacy_alert_at.insert(kind.to_string(),now);
    let target=&monitor.target;
    let _=send_outbound(output,PluginEvent{message_type:"plugin.event",event:"monitor.alert",payload:json!({"targetId":target.id,"targetName":target.name,"alertType":kind,"reminder":reminder,"sound":sound_payload(&rule.sound),"fallbackBeep":rule.sound.is_none()})});
}

fn sample_target(output:&mpsc::Sender<Value>,monitor:&mut TargetMonitor,rules:&[GenericAlertRule],legacy:&LegacyAlertConfig){
    let target=monitor.target.clone();
    let ping=sample_ping(&target);
    let transition=monitor.record_ping(&ping);
    legacy_alert(output,monitor,legacy,transition);
    for check in target.tcp_checks.iter().filter(|c|c.enabled) {
        let result=sample_tcp(&target,check);
        if let Some(ms)=result.latency_ms { monitor.record_metric(&format!("tcp:{}.latency",check.id),ms,result.timestamp); }
        evaluate_rules(output,monitor,rules,&format!("tcp:{}.reachable",check.id),Value::Bool(result.reachable),&check.alert_rule_ids);
        monitor.tcp_results.insert(check.id.clone(),result);
    }
    for check in target.http_checks.iter().filter(|c|c.enabled) {
        let result=sample_http(check);
        if let Some(ms)=result.response_time_ms { monitor.record_metric(&format!("http:{}.responseTime",check.id),ms,result.timestamp); }
        for (field,value) in &result.fields {
            let source=format!("http:{}.{}",check.id,field);
            if let Some(v)=numeric(value){monitor.record_metric(&source,v,result.timestamp);}
            if let Some(ids)=check.field_alerts.get(field){evaluate_rules(output,monitor,rules,&source,value.clone(),ids);}
        }
        if check.detect_changes { evaluate_rules(output,monitor,rules,&format!("http:{}.response",check.id),Value::String(result.body.clone()),check.field_alerts.get("response").map(Vec::as_slice).unwrap_or(&[])); }
        monitor.http_results.insert(check.id.clone(),result);
    }
}

fn source_value(monitor:&TargetMonitor,source:&str)->Value{
    if source=="none"||source.is_empty(){return Value::Null;}
    if source=="target.name"{return Value::String(monitor.target.name.clone());}
    if source=="ping.latency"{return monitor.samples.last().and_then(|s|s.latency_ms).map(Value::from).unwrap_or(Value::Null);}
    if let Some(rest)=source.strip_prefix("tcp:") {
        if let Some((id,field))=rest.split_once('.') {
            if let Some(r)=monitor.tcp_results.get(id) {
                return match field {"reachable"=>Value::Bool(r.reachable),"latency"=>r.latency_ms.map(Value::from).unwrap_or(Value::Null),_=>Value::Null};
            }
        }
    }
    if let Some(rest)=source.strip_prefix("http:") {
        if let Some((id,field))=rest.split_once('.') {
            if let Some(r)=monitor.http_results.get(id) {
                if field=="responseTime"{return r.response_time_ms.map(Value::from).unwrap_or(Value::Null);}
                if field=="status"{return r.status_code.map(|v|Value::from(v as u64)).unwrap_or(Value::Null);}
                return r.fields.get(field).cloned().unwrap_or(Value::Null);
            }
        }
    }
    Value::Null
}
fn display_value(v:&Value)->String{
    match v {Value::Null=>"—".into(),Value::Bool(b)=>if *b{"Yes".into()}else{"No".into()},Value::Number(n)=>n.as_f64().map(|f|if f.abs()>=100.0{format!("{f:.0}")}else{format!("{f:.1}")}).unwrap_or_else(||n.to_string()),Value::String(s)=>s.chars().take(22).collect(),_=>v.to_string()}
}
fn topbar_tone(state:&str)->&'static str{match state{"OFFLINE"=>"danger","DEGRADED"=>"warning","ONLINE"=>"accent",_=>"muted"}}
fn topbar_value(store:&MonitorStore)->Value{
    let items=store.targets.values().filter(|m|m.target.enabled&&m.target.topbar.enabled).map(|m|{
        let title=display_value(&source_value(m,&m.target.topbar.title_source));
        let value=display_value(&source_value(m,&m.target.topbar.value_source));
        let graph=m.metric_history.get(&m.target.topbar.graph_source).cloned().unwrap_or_default();
        let start=graph.len().saturating_sub(TOPBAR_HISTORY_LIMIT);
        let series:Vec<Value>=graph[start..].iter().map(|p|json!({"value":p.value,"tone":topbar_tone(m.overall_state()),"color":m.target.graph_color_for_state(m.overall_state())})).collect();
        let mut children=vec![];
        if m.target.topbar.title_source!="none"{children.push(json!({"type":"text","label":title,"tone":"muted","color":m.target.target_name_color,"fontSize":9}));}
        if m.target.topbar.value_source!="none"{children.push(json!({"type":"metric","value":value,"tone":topbar_tone(m.overall_state()),"color":m.target.latency_color,"fontSize":14}));}
        if m.target.topbar.graph_source!="none"{children.push(json!({"type":"sparkline","tone":topbar_tone(m.overall_state()),"series":series}));}
        let unread=m.unread_alerts();
        if unread>0{children.push(json!({"type":"text","label":format!("⚠ {unread}"),"tone":"warning","fontSize":9}));}
        json!({"id":format!("target-monitor:{}",m.target.id),"title":m.target.name,"tooltip":format!("{} · {}",m.target.name,m.overall_state()),"size":{"width":124,"height":44},"content":{"type":"stack","tone":topbar_tone(m.overall_state()),"backgroundColor":m.target.background_color,"borderColor":m.target.border_color,"children":children},"action":{"type":"open-panel","panel":"target-monitor","context":{"targetId":m.target.id}},"targetId":m.target.id,"state":m.overall_state(),"unreadAlertCount":unread})
    }).collect::<Vec<_>>();
    json!({"items":items})
}
fn tray_value(store:&MonitorStore)->Value{
    let items=store.targets.values().filter(|m|m.target.show_in_system_tray).map(|m|{
        let history=m.metric_history.get(&m.target.topbar.graph_source).or_else(||m.metric_history.get("ping.latency")).cloned().unwrap_or_default();
        let start=history.len().saturating_sub(TOPBAR_HISTORY_LIMIT);
        let series:Vec<Value>=history[start..].iter().map(|p|json!({"value":p.value,"tone":topbar_tone(m.overall_state()),"color":m.target.graph_color_for_state(m.overall_state())})).collect();
        json!({"id":m.target.id,"title":m.target.name,"tooltip":format!("{} · {}",m.target.name,m.overall_state()),"tone":topbar_tone(m.overall_state()),"graphColor":m.target.graph_color_for_state(m.overall_state()),"backgroundColor":m.target.background_color,"borderColor":m.target.border_color,"visible":m.target.enabled,"series":series,"action":{"type":"open-panel","panel":"target-monitor","context":{"targetId":m.target.id}}})
    }).collect::<Vec<_>>();
    json!({"items":items})
}
fn emit_surfaces(output:&mpsc::Sender<Value>,store:&MonitorStore)->Result<(),String>{
    send_outbound(output,PluginEvent{message_type:"plugin.event",event:"topbar.update",payload:topbar_value(store)})?;
    send_outbound(output,PluginEvent{message_type:"plugin.event",event:"tray.update",payload:tray_value(store)})
}
fn status_value(store:&MonitorStore)->Value{json!({"targets":store.summaries(),"config":&store.config,"alerts":&store.config.alerts,"alertRules":&store.config.alert_rules})}

fn main()->Result<(),String>{
    let plugin_id=std::env::var("REMOTEHAND_PLUGIN_ID").map_err(|_|"REMOTEHAND_PLUGIN_ID is missing".to_string())?;
    let api_version:u32=std::env::var("REMOTEHAND_PLUGIN_API_VERSION").map_err(|_|"REMOTEHAND_PLUGIN_API_VERSION is missing".to_string())?.parse().map_err(|e|format!("invalid plugin API version: {e}"))?;
    let stdin=io::stdin(); let mut input=stdin.lock(); let stdout=io::stdout(); let mut handshake_output=stdout.lock();
    write_frame_raw(&mut handshake_output,&PluginHello{message_type:"plugin.hello",plugin_id:&plugin_id,api_version,plugin_version:env!("CARGO_PKG_VERSION")})?;
    let ready:HostReady=serde_json::from_slice(&read_frame(&mut input)?).map_err(|e|format!("invalid host.ready: {e}"))?;
    if ready.message_type!="host.ready"||ready.plugin_id!=plugin_id||ready.api_version!=api_version{return Err("host.ready does not match plugin".into());}
    drop(handshake_output);
    let (output_tx,output_rx)=mpsc::channel::<Value>();
    let writer=thread::spawn(move||{let stdout=io::stdout();let mut output=stdout.lock();while let Ok(value)=output_rx.recv(){write_frame_raw(&mut output,&value)?;}Ok::<(),String>(())});
    let store=Arc::new(Mutex::new(MonitorStore::new()));
    let mut host_request_id=1u64;
    ensure_config_loaded(&store,&mut input,&output_tx,&mut host_request_id)?;
    {
        let guard=store.lock().map_err(|_|"monitor state lock poisoned".to_string())?;
        send_outbound(&output_tx,PluginEvent{message_type:"plugin.event",event:"monitor.ready",payload:status_value(&guard)})?;
        emit_surfaces(&output_tx,&guard)?;
    }
    {
        let sampler_store=Arc::clone(&store); let sampler_output=output_tx.clone();
        thread::spawn(move||loop{
            thread::sleep(SAMPLE_INTERVAL);
            let target_ids=match sampler_store.lock(){Ok(s)=>s.targets.values().filter(|m|m.target.enabled).map(|m|m.target.id.clone()).collect::<Vec<_>>(),Err(_)=>return};
            for id in target_ids {
                let mut guard=match sampler_store.lock(){Ok(g)=>g,Err(_)=>return};
                let rules=guard.config.alert_rules.clone(); let legacy=guard.config.alerts.clone();
                if let Some(monitor)=guard.targets.get_mut(&id){sample_target(&sampler_output,monitor,&rules,&legacy);let summary=monitor.summary();let _=send_outbound(&sampler_output,PluginEvent{message_type:"plugin.event",event:"monitor.sampled",payload:summary});}
                if emit_surfaces(&sampler_output,&guard).is_err(){return;}
            }
        });
    }
    loop {
        let payload=match read_frame(&mut input){Ok(p)=>p,Err(e) if e.contains("failed to read plugin frame length")=>break,Err(e)=>return Err(e)};
        let request:HostRequest=serde_json::from_slice(&payload).map_err(|e|format!("invalid host.request: {e}"))?;
        if request.message_type!="host.request"{return Err("unexpected host message type".into());}
        let response=match request.method.as_str(){
            "monitor.status"=>{let guard=store.lock().map_err(|_|"monitor state lock poisoned".to_string())?;PluginResponse{message_type:"plugin.response",request_id:request.request_id,ok:true,result:status_value(&guard),error:String::new()}}
            "monitor.config.set"=>{
                let config:Result<StoredConfig,_>=serde_json::from_value(request.params.clone());
                match config.map_err(|e|format!("invalid config: {e}")).and_then(normalize_config){
                    Err(error)=>PluginResponse{message_type:"plugin.response",request_id:request.request_id,ok:false,result:Value::Null,error},
                    Ok(config)=>{
                        let previous=store.lock().map_err(|_|"monitor state lock poisoned".to_string())?.config.clone();
                        store.lock().map_err(|_|"monitor state lock poisoned".to_string())?.replace_config(config.clone());
                        match save_config(&mut input,&output_tx,&mut host_request_id,&config){
                            Ok(())=>{let guard=store.lock().map_err(|_|"monitor state lock poisoned".to_string())?;let _=emit_surfaces(&output_tx,&guard);PluginResponse{message_type:"plugin.response",request_id:request.request_id,ok:true,result:status_value(&guard),error:String::new()}},
                            Err(error)=>{store.lock().map_err(|_|"monitor state lock poisoned".to_string())?.replace_config(previous);PluginResponse{message_type:"plugin.response",request_id:request.request_id,ok:false,result:Value::Null,error}}
                        }
                    }
                }
            }
            "monitor.http.test"=>{
                let check:Result<HttpCheck,_>=serde_json::from_value(request.params.get("check").cloned().unwrap_or(Value::Null));
                match check {Ok(check)=>PluginResponse{message_type:"plugin.response",request_id:request.request_id,ok:true,result:serde_json::to_value(sample_http(&check)).unwrap_or(Value::Null),error:String::new()},Err(e)=>PluginResponse{message_type:"plugin.response",request_id:request.request_id,ok:false,result:Value::Null,error:format!("invalid HTTP check: {e}")}}
            }
            "monitor.alerts.acknowledge"=>{
                let target_id=request.params.get("targetId").and_then(Value::as_str).unwrap_or("");
                let mut guard=store.lock().map_err(|_|"monitor state lock poisoned".to_string())?;
                if let Some(m)=guard.targets.get_mut(target_id){for alert in m.active_alerts.values_mut(){alert.acknowledged=true;}}
                let _=emit_surfaces(&output_tx,&guard);
                PluginResponse{message_type:"plugin.response",request_id:request.request_id,ok:true,result:json!({"acknowledged":true}),error:String::new()}
            }
            "monitor.shutdown"=>{let response=PluginResponse{message_type:"plugin.response",request_id:request.request_id,ok:true,result:json!({"stopped":true}),error:String::new()};send_outbound(&output_tx,response)?;break}
            _=>PluginResponse{message_type:"plugin.response",request_id:request.request_id,ok:false,result:Value::Null,error:format!("unsupported Target Monitor method: {}",request.method)}
        };
        send_outbound(&output_tx,response)?;
    }
    drop(output_tx); let _=writer.join(); Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn default_config_has_one_ping_target(){let c=default_config();assert_eq!(c.targets.len(),1);assert!(c.targets[0].ping_enabled);}
    #[test] fn flatten_json_discovers_nested_numeric_fields(){let mut out=BTreeMap::new();flatten_json("",&json!({"system":{"temp":58.4},"hashRate_10m":1210}),&mut out);assert_eq!(out["system.temp"],json!(58.4));assert_eq!(out["hashRate_10m"],json!(1210));}
    #[test] fn regex_named_capture_is_numeric_when_possible(){let out=extract_fields("regex",r"temperature=(?<temp>[0-9.]+)","temperature=58.4","text/plain").unwrap();assert_eq!(out["temp"],json!(58.4));}
    #[test] fn generic_alert_numeric_operator_works(){let rule=GenericAlertRule{id:"x".into(),name:"Hot".into(),enabled:true,operator:">=".into(),value:Some(json!(55)),sound:None};assert!(rule_matches(&rule,&json!(58.4),None));assert!(!rule_matches(&rule,&json!(54.9),None));}
    #[test] fn changed_alert_requires_previous_value(){let rule=GenericAlertRule{id:"x".into(),name:"Changed".into(),enabled:true,operator:"changed".into(),value:None,sound:None};assert!(!rule_matches(&rule,&json!("a"),None));assert!(rule_matches(&rule,&json!("b"),Some(&json!("a"))));}
    #[test] fn config_rejects_unknown_rule_assignment(){let mut c=default_config();c.targets[0].tcp_checks.push(TcpCheck{id:"ssh".into(),name:"SSH".into(),port:22,enabled:true,alert_rule_ids:vec!["missing".into()]});assert!(validate_config(&c).is_err());}
    #[test] fn topbar_is_one_item_per_target(){let s=MonitorStore::new();assert_eq!(topbar_value(&s)["items"].as_array().unwrap().len(),1);}
    #[test] fn disabled_topbar_hides_target(){let mut s=MonitorStore::new();s.targets.get_mut(DEFAULT_TARGET_ID).unwrap().target.topbar.enabled=false;assert!(topbar_value(&s)["items"].as_array().unwrap().is_empty());}
}
