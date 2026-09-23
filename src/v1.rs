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
const DEFAULT_PING_INTERVAL_SECONDS: u64 = 2;
const DEFAULT_TCP_INTERVAL_SECONDS: u64 = 5;
const DEFAULT_HTTP_INTERVAL_SECONDS: u64 = 10;
const DEFAULT_HISTORY_RETENTION_SECONDS: u64 = 60 * 60;
const DEFAULT_TOPBAR_HISTORY_SECONDS: u64 = 60;
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
const DEFAULT_GRAPH_CRITICAL_COLOR: &str = "#efaaaa";
const DEFAULT_GRAPH_OFFLINE_COLOR: &str = "#8f9baa";
const DEFAULT_TARGET_NAME_COLOR: &str = "#a8b4c2";
const DEFAULT_LATENCY_COLOR: &str = "#dce4ee";
const DEFAULT_VALUE_ALERT_COLOR: &str = "#efaaaa";
const DEFAULT_BACKGROUND_COLOR: &str = "#16263a";
const DEFAULT_BORDER_COLOR: &str = "#3b5f8d";

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}
fn default_true() -> bool { true }
fn default_graph_normal_color() -> String { DEFAULT_GRAPH_NORMAL_COLOR.to_string() }
fn default_graph_high_color() -> String { DEFAULT_GRAPH_HIGH_COLOR.to_string() }
fn default_graph_critical_color() -> String { DEFAULT_GRAPH_CRITICAL_COLOR.to_string() }
fn default_graph_offline_color() -> String { DEFAULT_GRAPH_OFFLINE_COLOR.to_string() }
fn default_target_name_color() -> String { DEFAULT_TARGET_NAME_COLOR.to_string() }
fn default_latency_color() -> String { DEFAULT_LATENCY_COLOR.to_string() }
fn default_value_alert_color() -> String { DEFAULT_VALUE_ALERT_COLOR.to_string() }
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sound_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    notification_enabled: Option<bool>,
    #[serde(default = "default_legacy_cooldown")]
    cooldown_seconds: u64,
    #[serde(default)]
    sound: Option<SoundConfig>,
}
fn default_legacy_cooldown() -> u64 { 60 }
fn legacy_rule(cooldown_seconds: u64) -> LegacyAlertRule {
    LegacyAlertRule {
        enabled: true,
        sound_enabled: Some(true),
        notification_enabled: Some(true),
        cooldown_seconds,
        sound: None,
    }
}
impl LegacyAlertRule {
    fn sound_enabled(&self) -> bool { self.sound_enabled.unwrap_or(self.enabled) }
    fn notification_enabled(&self) -> bool { self.notification_enabled.unwrap_or(self.enabled) }
}
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
    #[serde(default)]
    reminders_enabled: bool,
    #[serde(default = "default_reminder_interval")]
    reminder_interval_seconds: u64,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct SourceFormat {
    #[serde(default)]
    prefix: String,
    #[serde(default)]
    suffix: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphThreshold {
    operator: String,
    value: f64,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct GraphThresholdConfig {
    #[serde(default)]
    warning: Option<GraphThreshold>,
    #[serde(default)]
    critical: Option<GraphThreshold>,
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
    show_alert_notifications: bool,
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
    #[serde(default = "default_graph_critical_color")]
    graph_critical_color: String,
    #[serde(default = "default_graph_offline_color")]
    graph_offline_color: String,
    #[serde(default = "default_target_name_color")]
    target_name_color: String,
    #[serde(default = "default_latency_color")]
    latency_color: String,
    #[serde(default = "default_value_alert_color")]
    value_alert_color: String,
    #[serde(default = "default_background_color")]
    background_color: String,
    #[serde(default = "default_border_color")]
    border_color: String,
    #[serde(default)]
    graph_thresholds: BTreeMap<String, GraphThresholdConfig>,
    #[serde(default)]
    source_formats: BTreeMap<String, SourceFormat>,
}
impl MonitorTarget {
    fn graph_color_for_state(&self, state: &str) -> &str {
        match state {
            "OFFLINE" | "ERROR" => &self.graph_offline_color,
            "HIGH LATENCY" | "DEGRADED" => &self.graph_high_color,
            _ => self.graph_color.as_deref().unwrap_or(&self.graph_normal_color),
        }
    }
    fn graph_color_for_value(&self, source: &str, value: f64) -> &str {
        let Some(config) = self.graph_thresholds.get(source) else {
            return self.graph_color.as_deref().unwrap_or(&self.graph_normal_color);
        };
        if config.critical.as_ref().map(|t| threshold_matches(value, t)).unwrap_or(false) {
            return &self.graph_critical_color;
        }
        if config.warning.as_ref().map(|t| threshold_matches(value, t)).unwrap_or(false) {
            return &self.graph_high_color;
        }
        self.graph_color.as_deref().unwrap_or(&self.graph_normal_color)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MonitoringSettings {
    #[serde(default = "default_ping_interval_seconds")]
    ping_interval_seconds: u64,
    #[serde(default = "default_tcp_interval_seconds")]
    tcp_interval_seconds: u64,
    #[serde(default = "default_http_interval_seconds")]
    http_interval_seconds: u64,
    #[serde(default = "default_history_retention_seconds")]
    history_retention_seconds: u64,
    #[serde(default = "default_topbar_history_seconds")]
    topbar_history_seconds: u64,
}
fn default_ping_interval_seconds() -> u64 { DEFAULT_PING_INTERVAL_SECONDS }
fn default_tcp_interval_seconds() -> u64 { DEFAULT_TCP_INTERVAL_SECONDS }
fn default_http_interval_seconds() -> u64 { DEFAULT_HTTP_INTERVAL_SECONDS }
fn default_history_retention_seconds() -> u64 { DEFAULT_HISTORY_RETENTION_SECONDS }
fn default_topbar_history_seconds() -> u64 { DEFAULT_TOPBAR_HISTORY_SECONDS }
impl MonitoringSettings {
    fn new_defaults() -> Self {
        Self {
            ping_interval_seconds: DEFAULT_PING_INTERVAL_SECONDS,
            tcp_interval_seconds: DEFAULT_TCP_INTERVAL_SECONDS,
            http_interval_seconds: DEFAULT_HTTP_INTERVAL_SECONDS,
            history_retention_seconds: DEFAULT_HISTORY_RETENTION_SECONDS,
            topbar_history_seconds: DEFAULT_TOPBAR_HISTORY_SECONDS,
        }
    }
}
impl Default for MonitoringSettings {
    fn default() -> Self { Self::new_defaults() }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredConfig {
    #[serde(default)]
    targets: Vec<MonitorTarget>,
    #[serde(default)]
    monitoring: MonitoringSettings,
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

#[derive(Debug, Clone, Default)]
struct NetworkIdentity {
    public_ip: Option<String>,
    public_ip_error: Option<String>,
    interfaces: Vec<Value>,
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
    last_reachable_at: Option<u64>,
    offline_since: Option<u64>,
    tcp_results: BTreeMap<String, TcpResult>,
    http_results: BTreeMap<String, HttpResult>,
    metric_history: BTreeMap<String, Vec<MetricRecord>>,
    previous_values: BTreeMap<String, Value>,
    active_alerts: BTreeMap<String, ActiveAlert>,
    last_generic_alert_at: BTreeMap<String, u64>,
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
            last_reachable_at: None,
            offline_since: None,
            tcp_results: BTreeMap::new(),
            http_results: BTreeMap::new(),
            metric_history: BTreeMap::new(),
            previous_values: BTreeMap::new(),
            active_alerts: BTreeMap::new(),
            last_generic_alert_at: BTreeMap::new(),
            last_legacy_alert_at: BTreeMap::new(),
        }
    }
    fn record_ping(&mut self, sample: &Value, retention_seconds: u64) -> Option<(String, String)> {
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
        if latency.is_some() { self.last_reachable_at = Some(timestamp); }
        if previous_state != self.last_state {
            if self.last_state == "OFFLINE" { self.offline_since = Some(timestamp); }
            else if previous_state == "OFFLINE" { self.offline_since = None; }
        }
        self.samples.push(SampleRecord {
            timestamp,
            latency_ms: latency,
            packet_loss_percent: sample.get("packetLossPercent").and_then(Value::as_f64).unwrap_or(100.0),
            state,
        });
        let cutoff = timestamp.saturating_sub(retention_seconds.saturating_mul(1000));
        self.samples.retain(|p| p.timestamp >= cutoff);
        if let Some(v) = latency { self.record_metric("ping.latency", v, timestamp, retention_seconds); }
        if previous_state != self.last_state { Some((previous_state, self.last_state.clone())) } else { None }
    }
    fn record_metric(&mut self, source: &str, value: f64, timestamp: u64, retention_seconds: u64) {
        let history = self.metric_history.entry(source.to_string()).or_default();
        history.push(MetricRecord { timestamp, value });
        let cutoff = timestamp.saturating_sub(retention_seconds.saturating_mul(1000));
        history.retain(|p| p.timestamp >= cutoff);
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
            "lastReachableAt": self.last_reachable_at,
            "offlineSince": self.offline_since,
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
#[derive(Debug)]
struct MonitorStore {
    targets: BTreeMap<String, TargetMonitor>,
    config: StoredConfig,
    network_identity: NetworkIdentity,
}
impl MonitorStore {
    fn new() -> Self {
        let config = default_config();
        let targets = config.targets.iter().cloned().map(|t| (t.id.clone(), TargetMonitor::new(t))).collect();
        Self { targets, config, network_identity: NetworkIdentity::default() }
    }
    fn replace_config(&mut self, config: StoredConfig) {
        let old = std::mem::take(&mut self.targets);
        let mut next = BTreeMap::new();
        for target in &config.targets {
            if let Some(existing) = old.get(&target.id) {
                if existing.target.address == target.address {
                    let mut monitor = existing.clone();
                    monitor.target = target.clone();
                    let tcp_ids = target.tcp_checks.iter().map(|c| c.id.as_str()).collect::<Vec<_>>();
                    let http_ids = target.http_checks.iter().map(|c| c.id.as_str()).collect::<Vec<_>>();
                    monitor.tcp_results.retain(|id, _| tcp_ids.iter().any(|current| *current == id.as_str()));
                    monitor.http_results.retain(|id, _| http_ids.iter().any(|current| *current == id.as_str()));
                    let source_is_current = |source: &str| {
                        if source == "ping.latency" { return true; }
                        if let Some(rest) = source.strip_prefix("tcp:") {
                            return rest.split('.').next().map(|id| tcp_ids.iter().any(|current| *current == id)).unwrap_or(false);
                        }
                        if let Some(rest) = source.strip_prefix("http:") {
                            return rest.split('.').next().map(|id| http_ids.iter().any(|current| *current == id)).unwrap_or(false);
                        }
                        true
                    };
                    monitor.metric_history.retain(|source, _| source_is_current(source));
                    monitor.previous_values.retain(|source, _| source_is_current(source));

                    let rule_ids = config.alert_rules.iter().map(|r| r.id.as_str()).collect::<BTreeSet<_>>();
                    let assignment_is_current = |source: &str, rule_id: &str| {
                        if !rule_ids.contains(rule_id) { return false; }
                        if let Some(rest) = source.strip_prefix("tcp:") {
                            let Some((check_id, field)) = rest.split_once('.') else { return false; };
                            if field != "reachable" { return false; }
                            return target.tcp_checks.iter()
                                .find(|check| check.id == check_id)
                                .map(|check| check.alert_rule_ids.iter().any(|id| id == rule_id))
                                .unwrap_or(false);
                        }
                        if let Some(rest) = source.strip_prefix("http:") {
                            let Some((check_id, field)) = rest.split_once('.') else { return false; };
                            return target.http_checks.iter()
                                .find(|check| check.id == check_id)
                                .and_then(|check| check.field_alerts.get(field))
                                .map(|ids| ids.iter().any(|id| id == rule_id))
                                .unwrap_or(false);
                        }
                        false
                    };
                    monitor.active_alerts.retain(|_, alert| assignment_is_current(&alert.source, &alert.rule_id));
                    monitor.last_generic_alert_at.retain(|key, _| {
                        key.rsplit_once(':')
                            .map(|(source, rule_id)| assignment_is_current(source, rule_id))
                            .unwrap_or(false)
                    });
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
        enabled: true, ping_enabled: true, show_in_system_tray: false, show_alert_notifications: false, tcp_checks: vec![], http_checks: vec![],
        topbar: TopbarConfig::default(), graph_color: None, graph_normal_color: default_graph_normal_color(),
        graph_high_color: default_graph_high_color(), graph_critical_color: default_graph_critical_color(),
        graph_offline_color: default_graph_offline_color(), target_name_color: default_target_name_color(),
        latency_color: default_latency_color(), value_alert_color: default_value_alert_color(), background_color: default_background_color(),
        border_color: default_border_color(), graph_thresholds: BTreeMap::new(), source_formats: BTreeMap::new(),
    }
}
fn default_config() -> StoredConfig {
    StoredConfig {
        targets: vec![default_target()], monitoring: MonitoringSettings::new_defaults(), alerts: LegacyAlertConfig::default(), alert_rules: vec![],
        show_network_identity_topbar: false, network_identity_topbar_source: default_network_identity_topbar_source(),
    }
}

fn threshold_matches(value: f64, threshold: &GraphThreshold) -> bool {
    match threshold.operator.as_str() {
        ">=" => value >= threshold.value,
        "<=" => value <= threshold.value,
        ">" => value > threshold.value,
        "<" => value < threshold.value,
        "=" => (value - threshold.value).abs() < f64::EPSILON,
        _ => false,
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
    for (name, seconds) in [
        ("Ping", config.monitoring.ping_interval_seconds),
        ("TCP", config.monitoring.tcp_interval_seconds),
        ("HTTP", config.monitoring.http_interval_seconds),
    ] {
        if seconds < 1 || seconds > 3600 { return Err(format!("{name} sampling interval must be 1-3600 seconds")); }
    }
    if config.monitoring.history_retention_seconds < 60 || config.monitoring.history_retention_seconds > 7 * 24 * 60 * 60 {
        return Err("history retention must be 1 minute to 7 days".into());
    }
    if config.monitoring.topbar_history_seconds < 10 || config.monitoring.topbar_history_seconds > 60 * 60 {
        return Err("top-bar history must be 10 seconds to 1 hour".into());
    }
    let mut target_ids = BTreeSet::new();
    let mut rule_ids = BTreeSet::new();
    let mut sound_total = 0u64;
    for rule in &config.alert_rules {
        if rule.id.trim().is_empty() || !rule_ids.insert(rule.id.clone()) { return Err("alert rule ids must be unique and non-empty".into()); }
        if rule.name.trim().is_empty() || rule.name.len() > 128 { return Err("alert rule name is missing or too long".into()); }
        if !matches!(rule.operator.as_str(), ">="|"<="|"="|"equals"|"changed") { return Err(format!("unsupported alert operator: {}", rule.operator)); }
        if rule.operator != "changed" && rule.value.is_none() { return Err(format!("alert rule {} requires a comparison value", rule.name)); }
        if rule.reminder_interval_seconds < 5 || rule.reminder_interval_seconds > 86400 { return Err(format!("alert rule {} reminder interval must be 5-86400 seconds", rule.name)); }
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
        for color in [&target.graph_normal_color,&target.graph_high_color,&target.graph_critical_color,&target.graph_offline_color,&target.target_name_color,&target.latency_color,&target.value_alert_color,&target.background_color,&target.border_color] {
            if !valid_hex_color(color) { return Err("target colors must use #RRGGBB".into()); }
        }
        for (source, thresholds) in &target.graph_thresholds {
            if source.trim().is_empty() { return Err("graph threshold source cannot be empty".into()); }
            for threshold in [&thresholds.warning, &thresholds.critical].into_iter().flatten() {
                if !matches!(threshold.operator.as_str(), ">="|"<="|">"|"<"|"=") { return Err(format!("unsupported graph threshold operator: {}", threshold.operator)); }
                if !threshold.value.is_finite() { return Err("graph threshold values must be finite numbers".into()); }
            }
        }
        for (source, format) in &target.source_formats {
            if source.trim().is_empty() { return Err("source format source cannot be empty".into()); }
            if format.prefix.chars().count() > 24 || format.suffix.chars().count() > 24 {
                return Err("source format prefix/suffix must be 24 characters or less".into());
            }
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

fn parse_public_ip(value:&str)->Option<String>{value.trim().parse::<IpAddr>().ok().map(|address|address.to_string())}
fn lookup_public_ip()->Result<String,String>{
    const SERVICES:[&str;2]=["https://api.ipify.org","https://ifconfig.me/ip"];
    let mut last_error="public IP lookup failed".to_string();
    for url in SERVICES {
        match ureq::get(url).timeout(Duration::from_secs(4)).call() {
            Ok(response)=>match response.into_string() {
                Ok(body)=>if let Some(address)=parse_public_ip(&body){return Ok(address)}else{last_error=format!("{url} returned an invalid IP address")},
                Err(error)=>last_error=format!("{url} response failed: {error}"),
            },
            Err(error)=>last_error=format!("{url} lookup failed: {error}"),
        }
    }
    Err(last_error)
}
fn topbar_selectable_ip(value:&str)->bool{
    match value.parse::<IpAddr>() {
        Ok(IpAddr::V4(address))=>!address.is_loopback()&&!address.is_unspecified(),
        Ok(IpAddr::V6(address))=>!address.is_loopback()&&!address.is_unspecified(),
        Err(_)=>false,
    }
}
fn interface_topbar_icon(name:&str,family:&str)->&'static str{
    if family=="ipv6"{return "ipv6";}
    let lower=name.to_ascii_lowercase();
    if lower.contains("wi-fi")||lower.contains("wifi")||lower.contains("wireless")||lower.contains("wlan"){"wifi"}
    else if lower.contains("tailscale")||lower.contains("vpn")||lower.contains("wireguard")||lower.starts_with("wg")||lower.contains("zerotier")||lower.contains("tun")||lower.contains("tap"){"shield"}
    else{"ethernet"}
}
fn interface_topbar_label(name:&str,family:&str)->&'static str{
    if family=="ipv6"{return "IPv6";}
    match interface_topbar_icon(name,family){"wifi"=>"Wi-Fi","shield"=>"VPN",_=>"Ethernet / LAN"}
}
fn network_identity_topbar_options(identity:&NetworkIdentity)->Vec<Value>{
    let mut options=Vec::new();
    if let Some(public_ip)=identity.public_ip.as_deref(){options.push(json!({"key":"public","label":"Public IP","address":public_ip,"detail":"Internet-facing address","icon":"globe"}));}
    for interface in &identity.interfaces {
        let name=interface.get("name").and_then(Value::as_str).unwrap_or("Interface");
        let addresses=interface.get("addresses").and_then(Value::as_array).cloned().unwrap_or_default();
        for family in ["ipv4","ipv6"] {
            let address=addresses.iter().find_map(|entry|{
                let entry_family=entry.get("family").and_then(Value::as_str).unwrap_or("");
                let address=entry.get("address").and_then(Value::as_str)?;
                (entry_family==family&&topbar_selectable_ip(address)).then_some(address)
            });
            let Some(address)=address else{continue};
            options.push(json!({"key":format!("interface:{name}:{family}"),"label":interface_topbar_label(name,family),"address":address,"detail":name,"icon":interface_topbar_icon(name,family)}));
        }
    }
    options
}
fn network_identity_value(identity:&NetworkIdentity)->Value{
    json!({"publicIp":identity.public_ip,"publicIpError":identity.public_ip_error,"interfaces":identity.interfaces,"topbarOptions":network_identity_topbar_options(identity)})
}

fn network_identity_topbar_item(identity:&NetworkIdentity,selected_source:&str)->Value{
    let options=network_identity_topbar_options(identity);
    let selected=options.iter()
        .find(|option|option.get("key").and_then(Value::as_str)==Some(selected_source))
        .or_else(||options.iter().find(|option|option.get("key").and_then(Value::as_str)==Some("public")))
        .or_else(||options.first());
    let selected_address=selected.and_then(|option|option.get("address")).and_then(Value::as_str).unwrap_or("Unavailable");
    let selected_label=selected.and_then(|option|option.get("label")).and_then(Value::as_str).unwrap_or("Network");
    let selected_icon=selected.and_then(|option|option.get("icon")).and_then(Value::as_str).unwrap_or("globe");
    let menu_items=options.iter().enumerate().take(24).map(|(index,option)|{
        let address=option.get("address").and_then(Value::as_str).unwrap_or("");
        json!({
            "id":format!("network-ip-{index}"),
            "label":option.get("label").and_then(Value::as_str).unwrap_or("IP address"),
            "value":address,
            "detail":option.get("detail").and_then(Value::as_str).unwrap_or("Click to copy"),
            "icon":option.get("icon").and_then(Value::as_str).unwrap_or("ethernet"),
            "action":{"type":"plugin-call","method":"monitor.clipboard.write","params":{"text":address}}
        })
    }).collect::<Vec<_>>();
    json!({
        "id":"target-monitor:network-identity",
        "title":"Network Identity",
        "tooltip":format!("{selected_label} · {selected_address}"),
        "size":{"width":124,"height":44},
        "icon":selected_icon,
        "content":{"type":"text","label":selected_address,"tone":"accent","fontSize":10},
        "menu":{"items":menu_items}
    })
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
        let now=now_ms();
        if matches && !monitor.active_alerts.contains_key(&key) {
            let alert=ActiveAlert{key:key.clone(),target_id:monitor.target.id.clone(),rule_id:rule.id.clone(),rule_name:rule.name.clone(),source:source.into(),current_value:current.clone(),since:now,acknowledged:false};
            monitor.active_alerts.insert(key.clone(),alert.clone());
            monitor.last_generic_alert_at.insert(key.clone(),now);
            let _=send_outbound(output,PluginEvent{message_type:"plugin.event",event:"monitor.alert",payload:json!({"targetId":monitor.target.id,"targetName":monitor.target.name,"alertType":"rule","ruleId":rule.id,"ruleName":rule.name,"source":source,"currentValue":current,"reminder":false,"sound":sound_payload(&rule.sound),"fallbackBeep":rule.sound.is_none()})});
            emit_rule_notification(output,monitor,rule,source,&current,false);
        } else if matches {
            let should_remind=rule.reminders_enabled && monitor.last_generic_alert_at.get(&key).map(|last|now.saturating_sub(*last)>=rule.reminder_interval_seconds.saturating_mul(1000)).unwrap_or(true);
            if let Some(alert)=monitor.active_alerts.get_mut(&key){alert.current_value=current.clone();if should_remind{alert.acknowledged=false;}}
            if should_remind {
                monitor.last_generic_alert_at.insert(key.clone(),now);
                let _=send_outbound(output,PluginEvent{message_type:"plugin.event",event:"monitor.alert",payload:json!({"targetId":monitor.target.id,"targetName":monitor.target.name,"alertType":"rule","ruleId":rule.id,"ruleName":rule.name,"source":source,"currentValue":current,"reminder":true,"sound":sound_payload(&rule.sound),"fallbackBeep":rule.sound.is_none()})});
                emit_rule_notification(output,monitor,rule,source,&current,true);
            }
        } else {
            monitor.active_alerts.remove(&key);
            monitor.last_generic_alert_at.remove(&key);
        }
    }
    monitor.previous_values.insert(source.into(),current);
}
fn legacy_alert(output:&mpsc::Sender<Value>,monitor:&mut TargetMonitor,alerts:&LegacyAlertConfig,transition:Option<(String,String)>){
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
    let play_sound=alerts.enabled && rule.sound_enabled();
    let show_notification=rule.notification_enabled();
    if !play_sound && !show_notification{return;}
    let now=now_ms();
    let interval=if reminder{alerts.reminder_interval_seconds}else{rule.cooldown_seconds}.saturating_mul(1000);
    if monitor.last_legacy_alert_at.get(kind).map(|last|now.saturating_sub(*last)<interval).unwrap_or(false){return;}
    monitor.last_legacy_alert_at.insert(kind.to_string(),now);
    let target=&monitor.target;
    let _=send_outbound(output,PluginEvent{message_type:"plugin.event",event:"monitor.alert",payload:json!({
        "targetId":target.id,
        "targetName":target.name,
        "alertType":kind,
        "reminder":reminder,
        "playSound":play_sound,
        "sound":sound_payload(&rule.sound),
        "fallbackBeep":play_sound && rule.sound.is_none()
    })});
    if show_notification{emit_legacy_notification(output,monitor,kind,reminder);}
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
fn builtin_source_format(source:&str)->SourceFormat{
    if source=="ping.latency" || (source.starts_with("tcp:") && source.ends_with(".latency")) || (source.starts_with("http:") && source.ends_with(".responseTime")) {
        return SourceFormat{prefix:String::new(),suffix:" ms".into()};
    }
    SourceFormat::default()
}
fn source_format(monitor:&TargetMonitor,source:&str)->SourceFormat{
    monitor.target.source_formats.get(source).cloned().unwrap_or_else(||builtin_source_format(source))
}
fn formatted_source_value(monitor:&TargetMonitor,source:&str,unavailable_word:bool)->String{
    let raw=source_value(monitor,source);
    if raw.is_null(){return if unavailable_word{"Unavailable".into()}else{"—".into()};}
    let base=display_value(&raw);
    let format=source_format(monitor,source);
    format!("{}{}{}",format.prefix,base,format.suffix)
}
fn source_label(monitor:&TargetMonitor,source:&str)->String{
    if source=="none"||source.is_empty(){return "None".into();}
    if source=="target.name"{return "Target name".into();}
    if source=="ping.latency"{return "Ping latency".into();}
    if let Some(rest)=source.strip_prefix("tcp:"){
        if let Some((id,field))=rest.split_once('.'){
            if let Some(check)=monitor.target.tcp_checks.iter().find(|c|c.id==id){
                return match field{
                    "reachable"=>format!("{} · Reachable",check.name),
                    "latency"=>format!("{} · Connection time",check.name),
                    _=>format!("{} · {}",check.name,field),
                };
            }
        }
    }
    if let Some(rest)=source.strip_prefix("http:"){
        if let Some((id,field))=rest.split_once('.'){
            if let Some(check)=monitor.target.http_checks.iter().find(|c|c.id==id){
                return match field{
                    "responseTime"=>format!("{} · Response time",check.name),
                    "status"=>format!("{} · Status code",check.name),
                    "response"=>format!("{} · Whole response",check.name),
                    _=>format!("{} · {}",check.name,field),
                };
            }
        }
    }
    source.to_string()
}
fn formatted_value_for_source(monitor:&TargetMonitor,source:&str,value:&Value,unavailable_word:bool)->String{
    if value.is_null(){return if unavailable_word{"Unavailable".into()}else{"—".into()};}
    let base=display_value(value);
    let format=source_format(monitor,source);
    format!("{}{}{}",format.prefix,base,format.suffix)
}
fn bounded_notification_text(value:&str,limit:usize)->String{
    value.chars().take(limit).collect()
}
fn emit_native_notification(output:&mpsc::Sender<Value>,monitor:&TargetMonitor,title:String,body:String,tone:&str){
    if !monitor.target.show_alert_notifications{return;}
    let _=send_outbound(output,PluginEvent{
        message_type:"plugin.event",
        event:"notification.show",
        payload:json!({
            "itemId":monitor.target.id,
            "title":bounded_notification_text(&title,63),
            "body":bounded_notification_text(&body,255),
            "tone":tone,
            "silent":true,
            "action":{
                "type":"open-panel",
                "panel":"target-monitor",
                "context":{
                    "targetId":monitor.target.id,
                    "view":"report"
                }
            }
        })
    });
}
fn emit_rule_notification(output:&mpsc::Sender<Value>,monitor:&TargetMonitor,rule:&GenericAlertRule,source:&str,current:&Value,reminder:bool){
    let title=if reminder{
        format!("{} · {} · Reminder",monitor.target.name,rule.name)
    }else{
        format!("{} · {}",monitor.target.name,rule.name)
    };
    let body=format!("{}\nCurrent: {}",source_label(monitor,source),formatted_value_for_source(monitor,source,current,true));
    emit_native_notification(output,monitor,title,body,"warning");
}
fn emit_legacy_notification(output:&mpsc::Sender<Value>,monitor:&TargetMonitor,kind:&str,reminder:bool){
    let (label,tone,body)=match kind{
        "highLatency"=>(
            if reminder{"High latency · Reminder"}else{"High latency"},
            "warning",
            format!("Ping latency: {}",formatted_source_value(monitor,"ping.latency",true))
        ),
        "offline"=>(
            if reminder{"Offline · Reminder"}else{"Offline"},
            "danger",
            "Ping is unavailable".to_string()
        ),
        "recovery"=>(
            "Recovered",
            "success",
            format!("Ping latency: {}",formatted_source_value(monitor,"ping.latency",true))
        ),
        _=>return
    };
    emit_native_notification(output,monitor,format!("{} · {}",monitor.target.name,label),body,tone);
}
fn emit_legacy_test_notification(output:&mpsc::Sender<Value>,monitor:&TargetMonitor,kind:&str){
    let (label,tone)=match kind{
        "highLatency"=>("High latency","warning"),
        "offline"=>("Offline","danger"),
        "recovery"=>("Recovered","success"),
        _=>return
    };
    emit_native_notification(
        output,
        monitor,
        format!("{} · {} · Test",monitor.target.name,label),
        "Target Monitor test notification".to_string(),
        tone,
    );
}
fn surface_tooltip(monitor:&TargetMonitor,value_source:&str,graph_source:&str)->String{
    let mut parts=vec![format!("{} · {}",monitor.target.name,monitor.overall_state())];
    if value_source!="none"{
        parts.push(format!("Value: {} = {}",source_label(monitor,value_source),formatted_source_value(monitor,value_source,true)));
    }
    if graph_source!="none"{
        parts.push(format!("Graph: {} · last {}",source_label(monitor,graph_source),formatted_source_value(monitor,graph_source,true)));
    }
    parts.join("\n").chars().take(240).collect()
}
fn topbar_tone(state:&str)->&'static str{match state{"OFFLINE"=>"danger","DEGRADED"=>"warning","ONLINE"=>"accent",_=>"muted"}}
fn graph_series_for(m:&TargetMonitor,source:&str,history_seconds:u64)->Vec<Value>{
    let cutoff=now_ms().saturating_sub(history_seconds.saturating_mul(1000));
    if source=="ping.latency" {
        return m.samples.iter().filter(|p|p.timestamp>=cutoff).map(|p|json!({
            "value":p.latency_ms,
            "tone":match p.state.as_str(){"OFFLINE"=>"danger","HIGH LATENCY"=>"warning",_=>"accent"},
            "color":m.target.graph_color_for_state(&p.state)
        })).collect();
    }
    let mut series=m.metric_history.get(source)
        .map(|graph|graph.iter().filter(|p|p.timestamp>=cutoff).map(|p|json!({"value":p.value,"tone":"accent","color":m.target.graph_color_for_value(source,p.value)})).collect::<Vec<_>>())
        .unwrap_or_default();
    if source!="none" && source_value(m,source).is_null() {
        series.push(json!({"value":Value::Null,"tone":"danger","color":m.target.graph_offline_color}));
    }
    series
}
fn topbar_value_source_is_alerting(m:&TargetMonitor,source:&str)->bool{
    if source=="none" || source.is_empty(){return false;}
    if source=="ping.latency"{
        return matches!(m.last_state.as_str(),"HIGH LATENCY"|"OFFLINE");
    }
    m.active_alerts.values().any(|alert|alert.source==source)
}
fn topbar_value_color<'a>(m:&'a TargetMonitor,source:&str)->&'a str{
    if topbar_value_source_is_alerting(m,source){&m.target.value_alert_color}else{&m.target.latency_color}
}
fn topbar_value(store:&MonitorStore)->Value{
    let items=store.targets.values().filter(|m|m.target.enabled&&m.target.topbar.enabled).map(|m|{
        let title=display_value(&source_value(m,&m.target.topbar.title_source));
        let value=formatted_source_value(m,&m.target.topbar.value_source,false);
        let series=graph_series_for(m,&m.target.topbar.graph_source,store.config.monitoring.topbar_history_seconds);
        let mut children=vec![];
        if m.target.topbar.title_source!="none"{children.push(json!({"type":"text","label":title,"tone":"muted","color":m.target.target_name_color,"fontSize":9}));}
        if m.target.topbar.value_source!="none"{children.push(json!({"type":"metric","value":value,"tone":topbar_tone(m.overall_state()),"color":topbar_value_color(m,&m.target.topbar.value_source),"fontSize":14}));}
        if m.target.topbar.graph_source!="none"{children.push(json!({"type":"sparkline","tone":topbar_tone(m.overall_state()),"series":series}));}
        let unread=m.unread_alerts();
        if unread>0{children.push(json!({"type":"badge","label":unread.to_string(),"tone":"warning","fontSize":9}));}
        json!({"id":format!("target-monitor:{}",m.target.id),"title":m.target.name,"tooltip":surface_tooltip(m,&m.target.topbar.value_source,&m.target.topbar.graph_source),"size":{"width":124,"height":44},"content":{"type":"stack","tone":topbar_tone(m.overall_state()),"backgroundColor":m.target.background_color,"borderColor":m.target.border_color,"children":children},"action":{"type":"open-panel","panel":"target-monitor","context":{"targetId":m.target.id}},"targetId":m.target.id,"state":m.overall_state(),"unreadAlertCount":unread})
    }).collect::<Vec<_>>();
    let mut items=items;
    if store.config.show_network_identity_topbar {
        items.push(network_identity_topbar_item(&store.network_identity,&store.config.network_identity_topbar_source));
    }
    json!({"items":items})
}
fn tray_value(store:&MonitorStore)->Value{
    let items=store.targets.values().filter(|m|m.target.show_in_system_tray||m.target.show_alert_notifications).map(|m|{
        let source=if m.target.topbar.graph_source=="none"{"ping.latency"}else{m.target.topbar.graph_source.as_str()};
        let series=graph_series_for(m,source,store.config.monitoring.topbar_history_seconds);
        json!({"id":m.target.id,"title":m.target.name,"tooltip":surface_tooltip(m,&m.target.topbar.value_source,source),"tone":topbar_tone(m.overall_state()),"graphColor":m.target.graph_color_for_state(m.overall_state()),"backgroundColor":m.target.background_color,"borderColor":m.target.border_color,"visible":m.target.enabled&&m.target.show_in_system_tray,"series":series,"action":{"type":"open-panel","panel":"target-monitor","context":{"targetId":m.target.id}}})
    }).collect::<Vec<_>>();
    json!({"items":items})
}
fn emit_surfaces(output:&mpsc::Sender<Value>,store:&MonitorStore)->Result<(),String>{
    send_outbound(output,PluginEvent{message_type:"plugin.event",event:"topbar.update",payload:topbar_value(store)})?;
    send_outbound(output,PluginEvent{message_type:"plugin.event",event:"tray.update",payload:tray_value(store)})
}
fn status_value(store:&MonitorStore)->Value{json!({"targets":store.summaries(),"config":&store.config,"alerts":&store.config.alerts,"alertRules":&store.config.alert_rules,"networkIdentity":network_identity_value(&store.network_identity)})}

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
    if let Ok(result)=host_request(&mut input,&output_tx,&mut host_request_id,"network.interfaces",json!({})) {
        if let Ok(mut guard)=store.lock() {
            guard.network_identity.interfaces=result.get("interfaces").and_then(Value::as_array).cloned().unwrap_or_default();
        }
    }
    {
        let public_store=Arc::clone(&store);
        let public_output=output_tx.clone();
        thread::spawn(move||{
            let result=lookup_public_ip();
            if let Ok(mut guard)=public_store.lock() {
                match result {
                    Ok(address)=>{guard.network_identity.public_ip=Some(address);guard.network_identity.public_ip_error=None;}
                    Err(error)=>{guard.network_identity.public_ip=None;guard.network_identity.public_ip_error=Some(error);}
                }
                let _=emit_surfaces(&public_output,&guard);
            }
        });
    }
    {
        let guard=store.lock().map_err(|_|"monitor state lock poisoned".to_string())?;
        send_outbound(&output_tx,PluginEvent{message_type:"plugin.event",event:"monitor.ready",payload:status_value(&guard)})?;
        emit_surfaces(&output_tx,&guard)?;
    }
    {
        let ping_store=Arc::clone(&store); let ping_output=output_tx.clone();
        thread::spawn(move||loop{
            let interval=match ping_store.lock(){Ok(s)=>s.config.monitoring.ping_interval_seconds,Err(_)=>return};
            thread::sleep(Duration::from_secs(interval));
            let target_ids=match ping_store.lock(){Ok(s)=>s.targets.values().filter(|m|m.target.enabled).map(|m|m.target.id.clone()).collect::<Vec<_>>(),Err(_)=>return};
            for id in target_ids {
                let snapshot=match ping_store.lock(){Ok(s)=>s.targets.get(&id).map(|m|(m.target.clone(),s.config.alerts.clone(),s.config.monitoring.history_retention_seconds)),Err(_)=>return};
                let Some((target,legacy,retention))=snapshot else{continue};
                let ping=sample_ping(&target);
                let payload={
                    let mut guard=match ping_store.lock(){Ok(g)=>g,Err(_)=>return};
                    let Some(current)=guard.targets.get_mut(&id) else{continue};
                    if current.target!=target{continue}
                    let transition=current.record_ping(&ping,retention);
                    legacy_alert(&ping_output,current,&legacy,transition);
                    Some(current.summary())
                };
                if let Some(payload)=payload{let _=send_outbound(&ping_output,PluginEvent{message_type:"plugin.event",event:"monitor.sampled",payload});}
            }
            let guard=match ping_store.lock(){Ok(g)=>g,Err(_)=>return};
            if emit_surfaces(&ping_output,&guard).is_err(){return;}
        });
    }
    {
        let tcp_store=Arc::clone(&store); let tcp_output=output_tx.clone();
        thread::spawn(move||loop{
            let interval=match tcp_store.lock(){Ok(s)=>s.config.monitoring.tcp_interval_seconds,Err(_)=>return};
            thread::sleep(Duration::from_secs(interval));
            let target_ids=match tcp_store.lock(){Ok(s)=>s.targets.values().filter(|m|m.target.enabled).map(|m|m.target.id.clone()).collect::<Vec<_>>(),Err(_)=>return};
            for id in target_ids {
                let snapshot=match tcp_store.lock(){Ok(s)=>s.targets.get(&id).map(|m|(m.target.clone(),s.config.alert_rules.clone(),s.config.monitoring.history_retention_seconds)),Err(_)=>return};
                let Some((target,rules,retention))=snapshot else{continue};
                let results=target.tcp_checks.iter().filter(|x|x.enabled).map(|check|(check.clone(),sample_tcp(&target,check))).collect::<Vec<_>>();
                let payload={
                    let mut guard=match tcp_store.lock(){Ok(g)=>g,Err(_)=>return};
                    let Some(current)=guard.targets.get_mut(&id) else{continue};
                    if current.target!=target{continue}
                    for (check,result) in results {
                        if let Some(ms)=result.latency_ms{current.record_metric(&format!("tcp:{}.latency",check.id),ms,result.timestamp,retention);}
                        evaluate_rules(&tcp_output,current,&rules,&format!("tcp:{}.reachable",check.id),Value::Bool(result.reachable),&check.alert_rule_ids);
                        current.tcp_results.insert(check.id.clone(),result);
                    }
                    Some(current.summary())
                };
                if let Some(payload)=payload{let _=send_outbound(&tcp_output,PluginEvent{message_type:"plugin.event",event:"monitor.sampled",payload});}
            }
            let guard=match tcp_store.lock(){Ok(g)=>g,Err(_)=>return};
            if emit_surfaces(&tcp_output,&guard).is_err(){return;}
        });
    }
    {
        let http_store=Arc::clone(&store); let http_output=output_tx.clone();
        thread::spawn(move||loop{
            let interval=match http_store.lock(){Ok(s)=>s.config.monitoring.http_interval_seconds,Err(_)=>return};
            thread::sleep(Duration::from_secs(interval));
            let target_ids=match http_store.lock(){Ok(s)=>s.targets.values().filter(|m|m.target.enabled).map(|m|m.target.id.clone()).collect::<Vec<_>>(),Err(_)=>return};
            for id in target_ids {
                let snapshot=match http_store.lock(){Ok(s)=>s.targets.get(&id).map(|m|(m.target.clone(),s.config.alert_rules.clone(),s.config.monitoring.history_retention_seconds)),Err(_)=>return};
                let Some((target,rules,retention))=snapshot else{continue};
                let results=target.http_checks.iter().filter(|x|x.enabled).map(|check|(check.clone(),sample_http(check))).collect::<Vec<_>>();
                let payload={
                    let mut guard=match http_store.lock(){Ok(g)=>g,Err(_)=>return};
                    let Some(current)=guard.targets.get_mut(&id) else{continue};
                    if current.target!=target{continue}
                    for (check,result) in results {
                        if let Some(ms)=result.response_time_ms{current.record_metric(&format!("http:{}.responseTime",check.id),ms,result.timestamp,retention);}
                        for (field,value) in &result.fields {
                            let source=format!("http:{}.{}",check.id,field);
                            if let Some(v)=numeric(value){current.record_metric(&source,v,result.timestamp,retention);}
                            if let Some(ids)=check.field_alerts.get(field){evaluate_rules(&http_output,current,&rules,&source,value.clone(),ids);}
                        }
                        if check.detect_changes{evaluate_rules(&http_output,current,&rules,&format!("http:{}.response",check.id),Value::String(result.body.clone()),check.field_alerts.get("response").map(Vec::as_slice).unwrap_or(&[]));}
                        current.http_results.insert(check.id.clone(),result);
                    }
                    Some(current.summary())
                };
                if let Some(payload)=payload{let _=send_outbound(&http_output,PluginEvent{message_type:"plugin.event",event:"monitor.sampled",payload});}
            }
            let guard=match http_store.lock(){Ok(g)=>g,Err(_)=>return};
            if emit_surfaces(&http_output,&guard).is_err(){return;}
        });
    }
    loop {
        let payload=match read_frame(&mut input){Ok(p)=>p,Err(e) if e.contains("failed to read plugin frame length")=>break,Err(e)=>return Err(e)};
        let request:HostRequest=serde_json::from_slice(&payload).map_err(|e|format!("invalid host.request: {e}"))?;
        if request.message_type!="host.request"{return Err("unexpected host message type".into());}
        let response=match request.method.as_str(){
            "monitor.network.identity"=>{
                let interfaces=host_request(&mut input,&output_tx,&mut host_request_id,"network.interfaces",json!({}));
                match interfaces {
                    Ok(interfaces)=>{
                        let (public_ip,public_ip_error)=match lookup_public_ip(){Ok(address)=>(Some(address),None),Err(error)=>(None,Some(error))};
                        let identity=NetworkIdentity{
                            public_ip,
                            public_ip_error,
                            interfaces:interfaces.get("interfaces").and_then(Value::as_array).cloned().unwrap_or_default(),
                        };
                        if let Ok(mut guard)=store.lock(){guard.network_identity=identity.clone();let _=emit_surfaces(&output_tx,&guard);}
                        PluginResponse{message_type:"plugin.response",request_id:request.request_id,ok:true,result:network_identity_value(&identity),error:String::new()}
                    }
                    Err(error)=>PluginResponse{message_type:"plugin.response",request_id:request.request_id,ok:false,result:Value::Null,error}
                }
            }
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
            "monitor.alert.test"=>{
                let target_id=request.params.get("targetId").and_then(Value::as_str).unwrap_or("");
                let kind=request.params.get("kind").and_then(Value::as_str).unwrap_or("");
                let guard=store.lock().map_err(|_|"monitor state lock poisoned".to_string())?;
                match guard.targets.get(target_id){
                    Some(monitor) if monitor.target.show_alert_notifications=>{
                        emit_legacy_test_notification(&output_tx,monitor,kind);
                        PluginResponse{message_type:"plugin.response",request_id:request.request_id,ok:true,result:json!({"notified":true}),error:String::new()}
                    }
                    Some(_)=>PluginResponse{message_type:"plugin.response",request_id:request.request_id,ok:true,result:json!({"notified":false}),error:String::new()},
                    None=>PluginResponse{message_type:"plugin.response",request_id:request.request_id,ok:false,result:Value::Null,error:"target not found".into()}
                }
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
    #[test] fn time_window_excludes_old_topbar_points(){let mut s=MonitorStore::new();let m=s.targets.get_mut(DEFAULT_TARGET_ID).unwrap();let now=now_ms();m.metric_history.insert("x".into(),vec![MetricRecord{timestamp:now-120000,value:1.0},MetricRecord{timestamp:now,value:2.0}]);let series=graph_series_for(m,"x",60);let numeric=series.iter().filter(|p|p["value"].as_f64().is_some()).collect::<Vec<_>>();assert_eq!(numeric.len(),1);assert_eq!(numeric[0]["value"],json!(2.0));assert!(series.iter().any(|p|p["value"].is_null()));}
    #[test] fn config_rejects_invalid_monitoring_windows(){let mut c=default_config();c.monitoring.http_interval_seconds=0;assert!(validate_config(&c).is_err());c=default_config();c.monitoring.topbar_history_seconds=5;assert!(validate_config(&c).is_err());}
    #[test] fn new_config_uses_split_sampling_defaults(){let c=default_config();assert_eq!(c.monitoring.ping_interval_seconds,2);assert_eq!(c.monitoring.tcp_interval_seconds,5);assert_eq!(c.monitoring.http_interval_seconds,10);assert_eq!(c.monitoring.history_retention_seconds,3600);assert_eq!(c.monitoring.topbar_history_seconds,60);}
    #[test] fn legacy_config_without_monitoring_uses_current_defaults(){let raw=json!({"targets":[default_target()],"alerts":LegacyAlertConfig::default(),"alertRules":[],"showNetworkIdentityTopbar":false,"networkIdentityTopbarSource":"public"});let c:StoredConfig=serde_json::from_value(raw).unwrap();assert_eq!(c.monitoring.ping_interval_seconds,2);assert_eq!(c.monitoring.tcp_interval_seconds,5);assert_eq!(c.monitoring.http_interval_seconds,10);assert_eq!(c.monitoring.history_retention_seconds,3600);assert_eq!(c.monitoring.topbar_history_seconds,60);}
    #[test] fn default_config_has_one_ping_target(){let c=default_config();assert_eq!(c.targets.len(),1);assert!(c.targets[0].ping_enabled);}
    #[test] fn flatten_json_discovers_nested_numeric_fields(){let mut out=BTreeMap::new();flatten_json("",&json!({"system":{"temp":58.4},"hashRate_10m":1210}),&mut out);assert_eq!(out["system.temp"],json!(58.4));assert_eq!(out["hashRate_10m"],json!(1210));}
    #[test] fn regex_named_capture_is_numeric_when_possible(){let out=extract_fields("regex",r"temperature=(?<temp>[0-9.]+)","temperature=58.4","text/plain").unwrap();assert_eq!(out["temp"],json!(58.4));}
    #[test] fn generic_alert_numeric_operator_works(){let rule=GenericAlertRule{id:"x".into(),name:"Hot".into(),enabled:true,operator:">=".into(),value:Some(json!(55)),sound:None,reminders_enabled:false,reminder_interval_seconds:300};assert!(rule_matches(&rule,&json!(58.4),None));assert!(!rule_matches(&rule,&json!(54.9),None));}
    #[test] fn reusable_alert_reminders_default_off(){let raw=json!({"id":"x","name":"Hot","enabled":true,"operator":">=","value":55,"sound":null});let rule:GenericAlertRule=serde_json::from_value(raw).unwrap();assert!(!rule.reminders_enabled);assert_eq!(rule.reminder_interval_seconds,300);}
    #[test] fn legacy_alert_channel_flags_fall_back_to_old_enabled_field(){let enabled:LegacyAlertRule=serde_json::from_value(json!({"enabled":true,"cooldownSeconds":60,"sound":null})).unwrap();assert!(enabled.sound_enabled());assert!(enabled.notification_enabled());let disabled:LegacyAlertRule=serde_json::from_value(json!({"enabled":false,"cooldownSeconds":60,"sound":null})).unwrap();assert!(!disabled.sound_enabled());assert!(!disabled.notification_enabled());}
    #[test] fn legacy_alert_channel_flags_can_be_independent(){let rule:LegacyAlertRule=serde_json::from_value(json!({"enabled":false,"soundEnabled":true,"notificationEnabled":false,"cooldownSeconds":60,"sound":null})).unwrap();assert!(rule.sound_enabled());assert!(!rule.notification_enabled());}
    #[test] fn changed_alert_requires_previous_value(){let rule=GenericAlertRule{id:"x".into(),name:"Changed".into(),enabled:true,operator:"changed".into(),value:None,sound:None,reminders_enabled:false,reminder_interval_seconds:300};assert!(!rule_matches(&rule,&json!("a"),None));assert!(rule_matches(&rule,&json!("b"),Some(&json!("a"))));}
    #[test] fn config_rejects_unknown_rule_assignment(){let mut c=default_config();c.targets[0].tcp_checks.push(TcpCheck{id:"ssh".into(),name:"SSH".into(),port:22,enabled:true,alert_rule_ids:vec!["missing".into()]});assert!(validate_config(&c).is_err());}
    #[test] fn source_formatting_applies_to_display_without_changing_raw_value(){
        let mut s=MonitorStore::new();
        let m=s.targets.get_mut(DEFAULT_TARGET_ID).unwrap();
        m.target.source_formats.insert("metric.temp".into(),SourceFormat{prefix:"".into(),suffix:" °C".into()});
        m.metric_history.insert("metric.temp".into(),vec![MetricRecord{timestamp:now_ms(),value:65.0}]);
        assert_eq!(m.metric_history["metric.temp"][0].value,65.0);
        assert_eq!(source_format(m,"metric.temp").suffix," °C");
    }
    #[test] fn topbar_value_uses_alert_color_only_for_its_selected_source(){
        let mut s=MonitorStore::new();
        let m=s.targets.get_mut(DEFAULT_TARGET_ID).unwrap();
        m.target.topbar.value_source="metric.temp".into();
        m.target.latency_color="#112233".into();
        m.target.value_alert_color="#ff0000".into();
        m.active_alerts.insert("other:rule".into(),ActiveAlert{
            key:"other:rule".into(),target_id:m.target.id.clone(),rule_id:"rule".into(),rule_name:"Other".into(),
            source:"metric.fan".into(),current_value:json!(1),since:now_ms(),acknowledged:false,
        });
        assert_eq!(topbar_value_color(m,"metric.temp"),"#112233");
        m.active_alerts.insert("temp:rule".into(),ActiveAlert{
            key:"temp:rule".into(),target_id:m.target.id.clone(),rule_id:"rule".into(),rule_name:"Temp".into(),
            source:"metric.temp".into(),current_value:json!(70),since:now_ms(),acknowledged:true,
        });
        assert_eq!(topbar_value_color(m,"metric.temp"),"#ff0000");
    }
    #[test] fn ping_value_alert_color_follows_ping_state(){
        let mut s=MonitorStore::new();
        let m=s.targets.get_mut(DEFAULT_TARGET_ID).unwrap();
        m.target.latency_color="#112233".into();
        m.target.value_alert_color="#ff0000".into();
        m.last_state="NORMAL".into();
        assert_eq!(topbar_value_color(m,"ping.latency"),"#112233");
        m.last_state="HIGH LATENCY".into();
        assert_eq!(topbar_value_color(m,"ping.latency"),"#ff0000");
        m.last_state="OFFLINE".into();
        assert_eq!(topbar_value_color(m,"ping.latency"),"#ff0000");
    }
    #[test] fn topbar_and_tooltip_use_formatted_source_values(){
        let mut s=MonitorStore::new();
        {
            let m=s.targets.get_mut(DEFAULT_TARGET_ID).unwrap();
            m.record_ping(&json!({"state":"NORMAL","latencyMs":12.0,"packetLossPercent":0.0}),3600);
        }
        let topbar=topbar_value(&s);
        assert_eq!(topbar["items"][0]["content"]["children"][1]["value"],json!("12.0 ms"));
        let tooltip=topbar["items"][0]["tooltip"].as_str().unwrap();
        assert!(tooltip.contains("Value: Ping latency = 12.0 ms"));
        assert!(tooltip.contains("Graph: Ping latency · last 12.0 ms"));
    }
    #[test] fn notification_enabled_target_keeps_hidden_tray_handle(){
        let mut s=MonitorStore::new();
        {
            let m=s.targets.get_mut(DEFAULT_TARGET_ID).unwrap();
            m.target.show_in_system_tray=false;
            m.target.show_alert_notifications=true;
        }
        let tray=tray_value(&s);
        assert_eq!(tray["items"].as_array().unwrap().len(),1);
        assert_eq!(tray["items"][0]["visible"],json!(false));
    }
    #[test] fn legacy_test_notification_uses_target_notification_gate(){
        let mut s=MonitorStore::new();
        let m=s.targets.get_mut(DEFAULT_TARGET_ID).unwrap();
        m.target.show_alert_notifications=false;
        assert!(!m.target.show_alert_notifications);
        m.target.show_alert_notifications=true;
        assert!(m.target.show_alert_notifications);
    }
    #[test] fn notification_payload_targets_report_action(){
        let mut s=MonitorStore::new();
        let m=s.targets.get_mut(DEFAULT_TARGET_ID).unwrap();
        m.target.show_alert_notifications=true;
        let (tx,rx)=mpsc::channel::<Value>();
        emit_native_notification(&tx,m,"Target · Alert".into(),"Body".into(),"warning");
        let event=rx.recv().unwrap();
        assert_eq!(event["event"],json!("notification.show"));
        assert_eq!(event["payload"]["action"]["type"],json!("open-panel"));
        assert_eq!(event["payload"]["action"]["panel"],json!("target-monitor"));
        assert_eq!(event["payload"]["action"]["context"]["targetId"],json!(DEFAULT_TARGET_ID));
        assert_eq!(event["payload"]["action"]["context"]["view"],json!("report"));
    }
    #[test] fn notification_text_uses_source_formatting(){
        let mut s=MonitorStore::new();
        let m=s.targets.get_mut(DEFAULT_TARGET_ID).unwrap();
        m.target.source_formats.insert("metric.temp".into(),SourceFormat{prefix:"".into(),suffix:" °C".into()});
        assert_eq!(formatted_value_for_source(m,"metric.temp",&json!(65.0),true),"65.0 °C");
    }
    #[test] fn builtin_timing_sources_default_to_ms(){
        let s=MonitorStore::new();
        let m=s.targets.get(DEFAULT_TARGET_ID).unwrap();
        assert_eq!(source_format(m,"ping.latency").suffix," ms");
        assert_eq!(builtin_source_format("tcp:ssh.latency").suffix," ms");
        assert_eq!(builtin_source_format("http:api.responseTime").suffix," ms");
    }
    #[test] fn topbar_is_one_item_per_target(){let s=MonitorStore::new();assert_eq!(topbar_value(&s)["items"].as_array().unwrap().len(),1);}
    #[test] fn disabled_topbar_hides_target(){let mut s=MonitorStore::new();s.targets.get_mut(DEFAULT_TARGET_ID).unwrap().target.topbar.enabled=false;assert!(topbar_value(&s)["items"].as_array().unwrap().is_empty());}
    #[test] fn generic_graph_thresholds_color_numeric_series(){
        let mut s=MonitorStore::new();
        let m=s.targets.get_mut(DEFAULT_TARGET_ID).unwrap();
        m.target.graph_thresholds.insert("metric.temp".into(),GraphThresholdConfig{
            warning:Some(GraphThreshold{operator:">=".into(),value:55.0}),
            critical:Some(GraphThreshold{operator:">=".into(),value:70.0}),
        });
        m.record_metric("metric.temp",50.0,now_ms()-2000,3600);
        m.record_metric("metric.temp",60.0,now_ms()-1000,3600);
        m.record_metric("metric.temp",75.0,now_ms(),3600);
        let series=graph_series_for(m,"metric.temp",60);
        assert_eq!(series[0]["color"],json!(m.target.graph_normal_color));
        assert_eq!(series[1]["color"],json!(m.target.graph_high_color));
        assert_eq!(series[2]["color"],json!(m.target.graph_critical_color));
    }
    #[test] fn generic_graph_thresholds_support_low_is_bad(){
        let mut s=MonitorStore::new();
        let m=s.targets.get_mut(DEFAULT_TARGET_ID).unwrap();
        m.target.graph_thresholds.insert("metric.hash".into(),GraphThresholdConfig{
            warning:Some(GraphThreshold{operator:"<=".into(),value:500.0}),
            critical:Some(GraphThreshold{operator:"<=".into(),value:300.0}),
        });
        assert_eq!(m.target.graph_color_for_value("metric.hash",450.0),m.target.graph_high_color);
        assert_eq!(m.target.graph_color_for_value("metric.hash",250.0),m.target.graph_critical_color);
    }
    #[test] fn target_appearance_flows_to_topbar_and_tray(){
        let mut s=MonitorStore::new();
        {
            let m=s.targets.get_mut(DEFAULT_TARGET_ID).unwrap();
            m.target.show_in_system_tray=true;
            m.target.graph_normal_color="#aa33cc".into();
            m.target.graph_high_color="#cc9933".into();
            m.target.graph_offline_color="#dd3344".into();
            m.target.target_name_color="#abcdef".into();
            m.target.latency_color="#fedcba".into();
            m.target.background_color="#112233".into();
            m.target.border_color="#445566".into();
            m.last_state="NORMAL".into();
            m.pending_state="NORMAL".into();
            m.record_ping(&json!({"state":"NORMAL","latencyMs":10.0,"packetLossPercent":0.0}),3600);
        }
        let topbar=topbar_value(&s);
        let item=&topbar["items"][0];
        assert_eq!(item["content"]["backgroundColor"],"#112233");
        assert_eq!(item["content"]["borderColor"],"#445566");
        assert_eq!(item["content"]["children"][0]["color"],"#abcdef");
        assert_eq!(item["content"]["children"][1]["color"],"#fedcba");
        assert_eq!(item["content"]["children"][2]["series"][0]["color"],"#aa33cc");
        let tray=tray_value(&s);
        let tray_item=&tray["items"][0];
        assert_eq!(tray_item["backgroundColor"],"#112233");
        assert_eq!(tray_item["borderColor"],"#445566");
        assert_eq!(tray_item["series"][0]["color"],"#aa33cc");
    }
    #[test] fn ping_tracks_last_reachable_and_offline_since(){let mut m=TargetMonitor::new(default_target());m.last_state="NORMAL".into();m.pending_state="NORMAL".into();m.record_ping(&json!({"state":"NORMAL","latencyMs":12.0,"packetLossPercent":0.0}),3600);let reachable=m.last_reachable_at;assert!(reachable.is_some());m.record_ping(&json!({"state":"OFFLINE","latencyMs":Value::Null,"packetLossPercent":100.0}),3600);assert!(m.offline_since.is_none());m.record_ping(&json!({"state":"OFFLINE","latencyMs":Value::Null,"packetLossPercent":100.0}),3600);assert_eq!(m.last_state,"OFFLINE");assert!(m.offline_since.is_some());assert_eq!(m.last_reachable_at,reachable);m.record_ping(&json!({"state":"NORMAL","latencyMs":10.0,"packetLossPercent":0.0}),3600);m.record_ping(&json!({"state":"NORMAL","latencyMs":11.0,"packetLossPercent":0.0}),3600);assert_eq!(m.last_state,"NORMAL");assert!(m.offline_since.is_none());assert!(m.last_reachable_at.unwrap()>=reachable.unwrap());}
    #[test] fn replace_config_prunes_removed_tcp_results(){let mut s=MonitorStore::new();let mut c=s.config.clone();c.targets[0].tcp_checks=vec![TcpCheck{id:"keep".into(),name:"Keep".into(),port:80,enabled:true,alert_rule_ids:vec![]},TcpCheck{id:"drop".into(),name:"Drop".into(),port:22,enabled:true,alert_rule_ids:vec![]}];s.replace_config(c.clone());let m=s.targets.get_mut(DEFAULT_TARGET_ID).unwrap();m.tcp_results.insert("keep".into(),TcpResult{id:"keep".into(),name:"Keep".into(),port:80,reachable:true,latency_ms:Some(1.0),error:None,timestamp:1});m.tcp_results.insert("drop".into(),TcpResult{id:"drop".into(),name:"Drop".into(),port:22,reachable:false,latency_ms:None,error:None,timestamp:1});c.targets[0].tcp_checks.retain(|x|x.id=="keep");s.replace_config(c);let m=s.targets.get(DEFAULT_TARGET_ID).unwrap();assert_eq!(m.tcp_results.len(),1);assert!(m.tcp_results.contains_key("keep"));assert!(!m.tcp_results.contains_key("drop"));}
    #[test] fn replace_config_prunes_active_alert_when_rule_is_deleted(){
        let mut s=MonitorStore::new();
        let mut c=s.config.clone();
        let rule=GenericAlertRule{id:"hot".into(),name:"Hot".into(),enabled:true,operator:">=".into(),value:Some(json!(55)),sound:None,reminders_enabled:false,reminder_interval_seconds:300};
        c.alert_rules.push(rule);
        c.targets[0].http_checks.push(HttpCheck{id:"sys".into(),name:"System Info".into(),url:"http://example.invalid".into(),enabled:true,response_mode:"json".into(),regex:"".into(),monitor_whole_response:false,detect_changes:false,discovered_fields:BTreeMap::new(),field_alerts:BTreeMap::from([("temp".into(),vec!["hot".into()])])});
        s.replace_config(c.clone());
        {
            let m=s.targets.get_mut(DEFAULT_TARGET_ID).unwrap();
            m.active_alerts.insert("http:sys.temp:hot".into(),ActiveAlert{key:"http:sys.temp:hot".into(),target_id:DEFAULT_TARGET_ID.into(),rule_id:"hot".into(),rule_name:"Hot".into(),source:"http:sys.temp".into(),current_value:json!(60),since:1,acknowledged:true});
            m.last_generic_alert_at.insert("http:sys.temp:hot".into(),1);
        }
        c.alert_rules.clear();
        c.targets[0].http_checks[0].field_alerts.clear();
        s.replace_config(c);
        let m=s.targets.get(DEFAULT_TARGET_ID).unwrap();
        assert!(m.active_alerts.is_empty());
        assert!(m.last_generic_alert_at.is_empty());
    }

    #[test] fn replace_config_prunes_active_alert_when_source_is_unassigned(){
        let mut s=MonitorStore::new();
        let mut c=s.config.clone();
        let rule=GenericAlertRule{id:"hot".into(),name:"Hot".into(),enabled:true,operator:">=".into(),value:Some(json!(55)),sound:None,reminders_enabled:false,reminder_interval_seconds:300};
        c.alert_rules.push(rule);
        c.targets[0].http_checks.push(HttpCheck{id:"sys".into(),name:"System Info".into(),url:"http://example.invalid".into(),enabled:true,response_mode:"json".into(),regex:"".into(),monitor_whole_response:false,detect_changes:false,discovered_fields:BTreeMap::new(),field_alerts:BTreeMap::from([("temp".into(),vec!["hot".into()])])});
        s.replace_config(c.clone());
        {
            let m=s.targets.get_mut(DEFAULT_TARGET_ID).unwrap();
            m.active_alerts.insert("http:sys.temp:hot".into(),ActiveAlert{key:"http:sys.temp:hot".into(),target_id:DEFAULT_TARGET_ID.into(),rule_id:"hot".into(),rule_name:"Hot".into(),source:"http:sys.temp".into(),current_value:json!(60),since:1,acknowledged:false});
            m.last_generic_alert_at.insert("http:sys.temp:hot".into(),1);
        }
        c.targets[0].http_checks[0].field_alerts.clear();
        s.replace_config(c);
        let m=s.targets.get(DEFAULT_TARGET_ID).unwrap();
        assert!(m.active_alerts.is_empty());
        assert!(m.last_generic_alert_at.is_empty());
    }

    #[test] fn ping_graph_series_keeps_offline_samples_and_sample_colors(){let mut s=MonitorStore::new();let m=s.targets.get_mut(DEFAULT_TARGET_ID).unwrap();let now=now_ms();m.samples=vec![SampleRecord{timestamp:now-2000,latency_ms:Some(12.0),packet_loss_percent:0.0,state:"NORMAL".into()},SampleRecord{timestamp:now-1000,latency_ms:Some(250.0),packet_loss_percent:0.0,state:"HIGH LATENCY".into()},SampleRecord{timestamp:now,latency_ms:None,packet_loss_percent:100.0,state:"OFFLINE".into()}];let series=graph_series_for(m,"ping.latency",60);assert_eq!(series.len(),3);assert_eq!(series[0]["color"],json!(m.target.graph_normal_color));assert_eq!(series[1]["color"],json!(m.target.graph_high_color));assert_eq!(series[2]["color"],json!(m.target.graph_offline_color));assert!(series[2]["value"].is_null());}
}
