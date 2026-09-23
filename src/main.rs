use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::{self, Read, Write};
use std::net::{IpAddr, ToSocketAddrs};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;
const PING_TIMEOUT: Duration = Duration::from_secs(2);
const SAMPLE_INTERVAL: Duration = Duration::from_secs(2);
const HISTORY_LIMIT: usize = 1800;
const TOPBAR_HISTORY_LIMIT: usize = 30;
const STATE_CONFIRMATION_SAMPLES: u32 = 2;
const STORAGE_KEY: &str = "config";
const DEFAULT_TARGET_ID: &str = "target-8-8-8-8";
const DEFAULT_TARGET_NAME: &str = "Google DNS";
const DEFAULT_TARGET: &str = "8.8.8.8";
const MIN_COOLDOWN_SECONDS: u64 = 5;
const MAX_COOLDOWN_SECONDS: u64 = 24 * 60 * 60;
const MAX_SOUND_FILE_BYTES: u64 = 2 * 1024 * 1024;
const MAX_TOTAL_SOUND_BYTES: u64 = 6 * 1024 * 1024;
const MAX_SOUND_DURATION_MS: u64 = 10_000;
const DEFAULT_GRAPH_NORMAL_COLOR: &str = "#70a7e8";
const DEFAULT_GRAPH_HIGH_COLOR: &str = "#f1cf87";
const DEFAULT_GRAPH_OFFLINE_COLOR: &str = "#efaaaa";
const DEFAULT_TARGET_NAME_COLOR: &str = "#a8b4c2";
const DEFAULT_LATENCY_COLOR: &str = "#dce4ee";
const DEFAULT_BACKGROUND_COLOR: &str = "#16263a";
const DEFAULT_BORDER_COLOR: &str = "#3b5f8d";

fn default_graph_normal_color() -> String { DEFAULT_GRAPH_NORMAL_COLOR.to_string() }
fn default_graph_high_color() -> String { DEFAULT_GRAPH_HIGH_COLOR.to_string() }
fn default_graph_offline_color() -> String { DEFAULT_GRAPH_OFFLINE_COLOR.to_string() }
fn default_target_name_color() -> String { DEFAULT_TARGET_NAME_COLOR.to_string() }
fn default_latency_color() -> String { DEFAULT_LATENCY_COLOR.to_string() }
fn default_background_color() -> String { DEFAULT_BACKGROUND_COLOR.to_string() }
fn default_border_color() -> String { DEFAULT_BORDER_COLOR.to_string() }

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HostReady {
    #[serde(rename = "type")]
    message_type: String,
    plugin_id: String,
    api_version: u32,
    #[serde(default)]
    _granted_permissions: Vec<String>,
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
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MonitorTarget {
    id: String,
    name: String,
    address: String,
    enabled: bool,
    #[serde(default)]
    show_in_system_tray: bool,
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
            "OFFLINE" => &self.graph_offline_color,
            "HIGH LATENCY" => &self.graph_high_color,
            _ => self.graph_color.as_deref().unwrap_or(&self.graph_normal_color),
        }
    }

    fn effective_graph_normal_color(&self) -> &str {
        self.graph_color.as_deref().unwrap_or(&self.graph_normal_color)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SoundConfig {
    filename: String,
    mime: String,
    data_url: String,
    size_bytes: u64,
    duration_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AlertRule {
    enabled: bool,
    cooldown_seconds: u64,
    #[serde(default)]
    sound: Option<SoundConfig>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AlertConfig {
    enabled: bool,
    high_latency: AlertRule,
    offline: AlertRule,
    recovery: AlertRule,
    reminders_enabled: bool,
    reminder_interval_seconds: u64,
}

impl Default for AlertConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            high_latency: AlertRule { enabled: true, cooldown_seconds: 60, sound: None },
            offline: AlertRule { enabled: true, cooldown_seconds: 300, sound: None },
            recovery: AlertRule { enabled: true, cooldown_seconds: 5, sound: None },
            reminders_enabled: false,
            reminder_interval_seconds: 300,
        }
    }
}

fn default_network_identity_topbar_source() -> String {
    "public".to_string()
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredConfig {
    #[serde(default)]
    targets: Vec<MonitorTarget>,
    #[serde(default)]
    alerts: AlertConfig,
    #[serde(default)]
    show_network_identity_topbar: bool,
    #[serde(default = "default_network_identity_topbar_source")]
    network_identity_topbar_source: String,
}

#[derive(Debug, Clone, Default)]
struct NetworkIdentity {
    public_ip: Option<String>,
    public_ip_error: Option<String>,
    interfaces: Vec<Value>,
}

#[derive(Debug, Clone)]
struct SampleRecord {
    timestamp: u64,
    latency_ms: Option<f64>,
    packet_loss_percent: f64,
    state: String,
}

#[derive(Debug, Clone)]
struct TargetMonitor {
    target: MonitorTarget,
    samples: Vec<SampleRecord>,
    last_state: String,
    pending_state: String,
    pending_count: u32,
}

impl TargetMonitor {
    fn new(target: MonitorTarget) -> Self {
        Self {
            target,
            samples: Vec::new(),
            last_state: "READY".to_string(),
            pending_state: "READY".to_string(),
            pending_count: 0,
        }
    }

    fn record(&mut self, timestamp: u64, sample: &Value) -> Option<(String, String)> {
        let state = sample
            .get("state")
            .and_then(Value::as_str)
            .unwrap_or("OFFLINE")
            .to_string();
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

        self.samples.push(SampleRecord {
            timestamp,
            latency_ms: sample.get("latencyMs").and_then(Value::as_f64),
            packet_loss_percent: sample
                .get("packetLossPercent")
                .and_then(Value::as_f64)
                .unwrap_or(100.0),
            state,
        });

        if self.samples.len() > HISTORY_LIMIT {
            let remove = self.samples.len() - HISTORY_LIMIT;
            self.samples.drain(..remove);
        }

        if previous_state != self.last_state {
            Some((previous_state, self.last_state.clone()))
        } else {
            None
        }
    }

    fn summary(&self) -> Value {
        let latest = self.samples.last();
        let latencies: Vec<f64> = self.samples.iter().filter_map(|sample| sample.latency_ms).collect();
        let avg = if latencies.is_empty() {
            None
        } else {
            Some(latencies.iter().sum::<f64>() / latencies.len() as f64)
        };
        let min = latencies.iter().copied().reduce(f64::min);
        let max = latencies.iter().copied().reduce(f64::max);
        let history: Vec<Value> = self.samples.iter().map(|sample| {
            json!({
                "timestamp": sample.timestamp,
                "latencyMs": sample.latency_ms,
                "packetLossPercent": sample.packet_loss_percent,
                "state": sample.state,
            })
        }).collect();

        json!({
            "id": self.target.id,
            "name": self.target.name,
            "target": self.target.address,
            "enabled": self.target.enabled,
            "showInSystemTray": self.target.show_in_system_tray,
            "graphNormalColor": self.target.effective_graph_normal_color(),
            "graphHighColor": self.target.graph_high_color,
            "graphOfflineColor": self.target.graph_offline_color,
            "targetNameColor": self.target.target_name_color,
            "latencyColor": self.target.latency_color,
            "backgroundColor": self.target.background_color,
            "borderColor": self.target.border_color,
            "state": self.last_state,
            "sampleCount": self.samples.len(),
            "currentLatencyMs": latest.and_then(|sample| sample.latency_ms),
            "minLatencyMs": min,
            "avgLatencyMs": avg,
            "maxLatencyMs": max,
            "packetLossPercent": latest.map(|sample| sample.packet_loss_percent).unwrap_or(0.0),
            "history": history,
        })
    }
}

#[derive(Debug)]
struct MonitorStore {
    targets: BTreeMap<String, TargetMonitor>,
    alerts: AlertConfig,
    config_loaded: bool,
    show_network_identity_topbar: bool,
    network_identity_topbar_source: String,
    network_identity: NetworkIdentity,
    last_alert_at: BTreeMap<String, u64>,
}

impl MonitorStore {
    fn new() -> Self {
        let target = MonitorTarget {
            id: DEFAULT_TARGET_ID.to_string(),
            name: DEFAULT_TARGET_NAME.to_string(),
            address: DEFAULT_TARGET.to_string(),
            enabled: true,
            show_in_system_tray: false,
            graph_color: None,
            graph_normal_color: default_graph_normal_color(),
            graph_high_color: default_graph_high_color(),
            graph_offline_color: default_graph_offline_color(),
            target_name_color: default_target_name_color(),
            latency_color: default_latency_color(),
            background_color: default_background_color(),
            border_color: default_border_color(),
        };
        let mut targets = BTreeMap::new();
        targets.insert(target.id.clone(), TargetMonitor::new(target));
        Self {
            targets,
            alerts: AlertConfig::default(),
            config_loaded: false,
            show_network_identity_topbar: false,
            network_identity_topbar_source: default_network_identity_topbar_source(),
            network_identity: NetworkIdentity::default(),
            last_alert_at: BTreeMap::new(),
        }
    }

    fn config(&self) -> StoredConfig {
        StoredConfig {
            targets: self.targets.values().map(|monitor| monitor.target.clone()).collect(),
            alerts: self.alerts.clone(),
            show_network_identity_topbar: self.show_network_identity_topbar,
            network_identity_topbar_source: self.network_identity_topbar_source.clone(),
        }
    }

    fn replace_config(&mut self, config: StoredConfig) {
        let old = std::mem::take(&mut self.targets);
        let mut next = BTreeMap::new();
        for target in config.targets {
            if let Some(existing) = old.get(&target.id) {
                if existing.target.address == target.address && existing.target.name == target.name {
                    let mut monitor = existing.clone();
                    monitor.target.enabled = target.enabled;
                    next.insert(target.id.clone(), monitor);
                    continue;
                }
            }
            next.insert(target.id.clone(), TargetMonitor::new(target));
        }
        self.targets = next;
        self.alerts = config.alerts;
        self.show_network_identity_topbar = config.show_network_identity_topbar;
        self.network_identity_topbar_source = config.network_identity_topbar_source;
        self.last_alert_at.clear();
        self.config_loaded = true;
    }

    fn enabled_targets(&self) -> Vec<MonitorTarget> {
        self.targets
            .values()
            .filter(|monitor| monitor.target.enabled)
            .map(|monitor| monitor.target.clone())
            .collect()
    }
}

fn default_config() -> StoredConfig {
    StoredConfig {
        targets: vec![MonitorTarget {
            id: DEFAULT_TARGET_ID.to_string(),
            name: DEFAULT_TARGET_NAME.to_string(),
            address: DEFAULT_TARGET.to_string(),
            enabled: true,
            show_in_system_tray: false,
            graph_color: None,
            graph_normal_color: default_graph_normal_color(),
            graph_high_color: default_graph_high_color(),
            graph_offline_color: default_graph_offline_color(),
            target_name_color: default_target_name_color(),
            latency_color: default_latency_color(),
            background_color: default_background_color(),
            border_color: default_border_color(),
        }],
        alerts: AlertConfig::default(),
        show_network_identity_topbar: true,
        network_identity_topbar_source: default_network_identity_topbar_source(),
    }
}

fn normalize_config(mut config: StoredConfig) -> Result<StoredConfig, String> {
    validate_targets(&config.targets)?;
    validate_alerts(&config.alerts)?;
    if config.targets.is_empty() {
        return Ok(default_config());
    }
    if !config.targets.iter().any(|target| target.enabled) {
        config.targets[0].enabled = true;
    }
    if config.network_identity_topbar_source.trim().is_empty()
        || config.network_identity_topbar_source.len() > 512
        || config.network_identity_topbar_source.chars().any(char::is_control)
    {
        config.network_identity_topbar_source = default_network_identity_topbar_source();
    }
    Ok(config)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn read_frame<R: Read>(reader: &mut R) -> Result<Vec<u8>, String> {
    let mut length = [0u8; 4];
    reader
        .read_exact(&mut length)
        .map_err(|error| format!("failed to read plugin frame length: {error}"))?;
    let length = u32::from_le_bytes(length) as usize;
    if length == 0 || length > MAX_FRAME_BYTES {
        return Err(format!("invalid frame length: {length}"));
    }
    let mut payload = vec![0u8; length];
    reader
        .read_exact(&mut payload)
        .map_err(|error| format!("failed to read plugin frame payload: {error}"))?;
    Ok(payload)
}

fn write_frame_raw<W: Write, T: Serialize>(writer: &mut W, value: &T) -> Result<(), String> {
    let payload = serde_json::to_vec(value)
        .map_err(|error| format!("failed to encode frame: {error}"))?;
    if payload.is_empty() || payload.len() > MAX_FRAME_BYTES {
        return Err("plugin frame exceeds runtime limit".to_string());
    }
    writer
        .write_all(&(payload.len() as u32).to_le_bytes())
        .and_then(|_| writer.write_all(&payload))
        .and_then(|_| writer.flush())
        .map_err(|error| format!("failed to write plugin frame: {error}"))
}

fn send_outbound(output: &mpsc::Sender<Value>, value: impl Serialize) -> Result<(), String> {
    output
        .send(serde_json::to_value(value).map_err(|error| format!("failed to encode outbound message: {error}"))?)
        .map_err(|_| "plugin output writer stopped".to_string())
}

fn parse_public_ip(value: &str) -> Option<String> {
    let trimmed = value.trim();
    trimmed.parse::<IpAddr>().ok().map(|address| address.to_string())
}

fn lookup_public_ip() -> Result<String, String> {
    const SERVICES: [&str; 2] = [
        "https://api.ipify.org",
        "https://ifconfig.me/ip",
    ];
    let mut last_error = "public IP lookup failed".to_string();
    for url in SERVICES {
        match ureq::get(url).timeout(Duration::from_secs(4)).call() {
            Ok(response) => match response.into_string() {
                Ok(body) => {
                    if let Some(address) = parse_public_ip(&body) {
                        return Ok(address);
                    }
                    last_error = format!("{url} returned an invalid IP address");
                }
                Err(error) => last_error = format!("{url} response failed: {error}"),
            },
            Err(error) => last_error = format!("{url} lookup failed: {error}"),
        }
    }
    Err(last_error)
}

fn topbar_selectable_ip(value: &str) -> bool {
    match value.parse::<IpAddr>() {
        Ok(IpAddr::V4(address)) => !address.is_loopback() && !address.is_unspecified(),
        Ok(IpAddr::V6(address)) => !address.is_loopback() && !address.is_unspecified(),
        Err(_) => false,
    }
}

fn interface_topbar_icon(name: &str, family: &str) -> &'static str {
    if family == "ipv6" {
        return "ipv6";
    }
    let lower = name.to_ascii_lowercase();
    if lower.contains("wi-fi") || lower.contains("wifi") || lower.contains("wireless") || lower.contains("wlan") {
        "wifi"
    } else if lower.contains("tailscale")
        || lower.contains("vpn")
        || lower.contains("wireguard")
        || lower.starts_with("wg")
        || lower.contains("zerotier")
        || lower.contains("tun")
        || lower.contains("tap")
    {
        "shield"
    } else {
        "ethernet"
    }
}

fn interface_topbar_label(name: &str, family: &str) -> &'static str {
    if family == "ipv6" {
        return "IPv6";
    }
    match interface_topbar_icon(name, family) {
        "wifi" => "Wi-Fi",
        "shield" => "VPN",
        _ => "Ethernet / LAN",
    }
}

fn network_identity_topbar_options(identity: &NetworkIdentity) -> Vec<Value> {
    let mut options = Vec::new();
    if let Some(public_ip) = identity.public_ip.as_deref() {
        options.push(json!({
            "key": "public",
            "label": "Public IP",
            "address": public_ip,
            "detail": "Internet-facing address",
            "icon": "globe",
        }));
    }

    for interface in &identity.interfaces {
        let name = interface.get("name").and_then(Value::as_str).unwrap_or("Interface");
        let addresses = interface.get("addresses").and_then(Value::as_array).cloned().unwrap_or_default();
        for family in ["ipv4", "ipv6"] {
            let address = addresses.iter().find_map(|entry| {
                let entry_family = entry.get("family").and_then(Value::as_str).unwrap_or("");
                let address = entry.get("address").and_then(Value::as_str)?;
                (entry_family == family && topbar_selectable_ip(address)).then_some(address)
            });
            let Some(address) = address else { continue; };
            let key = format!("interface:{name}:{family}");
            options.push(json!({
                "key": key,
                "label": interface_topbar_label(name, family),
                "address": address,
                "detail": name,
                "icon": interface_topbar_icon(name, family),
            }));
        }
    }
    options
}

fn network_identity_value(identity: &NetworkIdentity) -> Value {
    json!({
        "publicIp": identity.public_ip,
        "publicIpError": identity.public_ip_error,
        "interfaces": identity.interfaces,
        "topbarOptions": network_identity_topbar_options(identity),
    })
}

fn network_identity_topbar_item(identity: &NetworkIdentity, selected_source: &str) -> Value {
    let options = network_identity_topbar_options(identity);
    let selected = options
        .iter()
        .find(|option| option.get("key").and_then(Value::as_str) == Some(selected_source))
        .or_else(|| options.iter().find(|option| option.get("key").and_then(Value::as_str) == Some("public")))
        .or_else(|| options.first());

    let selected_address = selected
        .and_then(|option| option.get("address"))
        .and_then(Value::as_str)
        .unwrap_or("Unavailable");
    let selected_label = selected
        .and_then(|option| option.get("label"))
        .and_then(Value::as_str)
        .unwrap_or("Network");
    let selected_icon = selected
        .and_then(|option| option.get("icon"))
        .and_then(Value::as_str)
        .unwrap_or("globe");

    let menu_items: Vec<Value> = options
        .iter()
        .enumerate()
        .take(24)
        .map(|(index, option)| {
            let address = option.get("address").and_then(Value::as_str).unwrap_or("");
            json!({
                "id": format!("network-ip-{index}"),
                "label": option.get("label").and_then(Value::as_str).unwrap_or("IP address"),
                "value": address,
                "detail": option.get("detail").and_then(Value::as_str).unwrap_or("Click to copy"),
                "icon": option.get("icon").and_then(Value::as_str).unwrap_or("ethernet"),
                "action": {
                    "type": "plugin-call",
                    "method": "monitor.clipboard.write",
                    "params": { "text": address },
                }
            })
        })
        .collect();

    json!({
        "id": "target-monitor:network-identity",
        "title": "Network Identity",
        "tooltip": format!("{selected_label} · {selected_address}"),
        "size": { "width": 124, "height": 44 },
        "icon": selected_icon,
        "content": {
            "type": "text",
            "label": selected_address,
            "tone": "accent",
            "fontSize": 10
        },
        "menu": { "items": menu_items }
    })
}

fn host_request(
    input: &mut impl Read,
    output: &mpsc::Sender<Value>,
    request_id: &mut u64,
    method: &'static str,
    params: Value,
) -> Result<Value, String> {
    let id = *request_id;
    *request_id = request_id.wrapping_add(1).max(1);
    send_outbound(output, PluginHostRequest {
        message_type: "plugin.request",
        request_id: id,
        method,
        params,
    })?;

    loop {
        let payload = read_frame(input)?;
        let value: Value = serde_json::from_slice(&payload)
            .map_err(|error| format!("invalid host message: {error}"))?;
        match value.get("type").and_then(Value::as_str) {
            Some("host.response") => {
                let response: HostResponse = serde_json::from_value(value)
                    .map_err(|error| format!("invalid host response: {error}"))?;
                if response.request_id != id {
                    continue;
                }
                if response.ok {
                    return Ok(response.result);
                }
                return Err(if response.error.is_empty() {
                    format!("host request {method} failed")
                } else {
                    response.error
                });
            }
            Some("host.request") | Some("host.event") | _ => continue,
        }
    }
}

fn load_config(
    input: &mut impl Read,
    output: &mpsc::Sender<Value>,
    request_id: &mut u64,
) -> Result<Option<StoredConfig>, String> {
    let result = host_request(input, output, request_id, "storage.get", json!({ "key": STORAGE_KEY }))?;
    match result.get("value") {
        None | Some(Value::Null) => Ok(None),
        Some(value) => serde_json::from_value(value.clone())
            .map(Some)
            .map_err(|error| format!("invalid stored Target Monitor config: {error}")),
    }
}

fn save_config(
    input: &mut impl Read,
    output: &mpsc::Sender<Value>,
    request_id: &mut u64,
    config: &StoredConfig,
) -> Result<(), String> {
    host_request(
        input,
        output,
        request_id,
        "storage.set",
        json!({ "key": STORAGE_KEY, "value": config }),
    )?;
    Ok(())
}

fn resolve_target(target: &str) -> Result<std::net::IpAddr, String> {
    let target = target.trim();
    if target.is_empty() || target.len() > 255 {
        return Err("target must be 1-255 characters".to_string());
    }
    if let Ok(ip) = target.parse() {
        return Ok(ip);
    }
    (target, 0)
        .to_socket_addrs()
        .map_err(|error| format!("failed to resolve target: {error}"))?
        .next()
        .map(|address| address.ip())
        .ok_or_else(|| "target did not resolve to an address".to_string())
}

fn classify_latency(latency_ms: f64) -> &'static str {
    if latency_ms > 200.0 { "HIGH LATENCY" } else { "NORMAL" }
}

fn sample_target(target: &MonitorTarget) -> Value {
    match resolve_target(&target.address) {
        Ok(ip) => match ping::new(ip).timeout(PING_TIMEOUT).send() {
            Ok(reply) => {
                let latency_ms = reply.rtt.as_secs_f64() * 1000.0;
                json!({
                    "id": target.id,
                    "name": target.name,
                    "target": target.address,
                    "resolvedAddress": ip.to_string(),
                    "state": classify_latency(latency_ms),
                    "latencyMs": (latency_ms * 1000.0).round() / 1000.0,
                    "packetLossPercent": 0.0,
                })
            }
            Err(error) => json!({
                "id": target.id,
                "name": target.name,
                "target": target.address,
                "resolvedAddress": ip.to_string(),
                "state": "OFFLINE",
                "latencyMs": Value::Null,
                "packetLossPercent": 100.0,
                "error": error.to_string(),
            }),
        },
        Err(error) => json!({
            "id": target.id,
            "name": target.name,
            "target": target.address,
            "state": "OFFLINE",
            "latencyMs": Value::Null,
            "packetLossPercent": 100.0,
            "error": error,
        }),
    }
}

fn topbar_tone(state: &str) -> &'static str {
    match state {
        "OFFLINE" => "danger",
        "HIGH LATENCY" => "warning",
        "NORMAL" => "accent",
        _ => "muted",
    }
}

fn tray_tone(state: &str) -> &'static str {
    match state {
        "OFFLINE" => "danger",
        "HIGH LATENCY" => "warning",
        "NORMAL" => "success",
        _ => "muted",
    }
}

fn topbar_value(store: &MonitorStore) -> Value {
    let mut items: Vec<Value> = store
        .targets
        .values()
        .filter(|monitor| monitor.target.enabled)
        .map(|monitor| {
            let summary = monitor.summary();
            let history = summary
                .get("history")
                .and_then(Value::as_array)
                .map(|history| {
                    let start = history.len().saturating_sub(TOPBAR_HISTORY_LIMIT);
                    history[start..].to_vec()
                })
                .unwrap_or_default();
            let current_latency = summary.get("currentLatencyMs").cloned().unwrap_or(Value::Null);
            let current_state = summary
                .get("state")
                .and_then(Value::as_str)
                .unwrap_or("READY");
            let series: Vec<Value> = history.iter().map(|entry| {
                let state = entry.get("state").and_then(Value::as_str).unwrap_or("OFFLINE");
                json!({
                    "value": entry.get("latencyMs").cloned().unwrap_or(Value::Null),
                    "tone": topbar_tone(state),
                    "color": monitor.target.graph_color_for_state(state),
                })
            }).collect();
            let value_text = match current_latency.as_f64() {
                Some(latency) => format!("{} ms", if latency < 10.0 { format!("{latency:.1}") } else { format!("{latency:.0}") }),
                None => "—".to_string(),
            };
            json!({
                "id": format!("target-monitor:{}", monitor.target.id),
                "title": monitor.target.name,
                "tooltip": format!("{} · {}", monitor.target.name, monitor.target.address),
                "size": { "width": 112, "height": 44 },
                "content": {
                    "type": "stack",
                    "tone": topbar_tone(current_state),
                    "backgroundColor": monitor.target.background_color,
                    "borderColor": monitor.target.border_color,
                    "children": [
                        {
                            "type": "text",
                            "label": monitor.target.name,
                            "tone": "muted",
                            "color": monitor.target.target_name_color,
                            "fontSize": 9,
                        },
                        {
                            "type": "metric",
                            "value": value_text,
                            "tone": topbar_tone(current_state),
                            "color": monitor.target.latency_color,
                            "fontSize": 14,
                        },
                        {
                            "type": "sparkline",
                            "tone": topbar_tone(current_state),
                            "series": series,
                        }
                    ],
                },
                "action": {
                    "type": "open-panel",
                    "panel": "target-monitor",
                    "context": {
                        "targetId": monitor.target.id,
                        "target": monitor.target.address,
                    }
                },
                "targetId": monitor.target.id,
                "target": monitor.target.address,
                "currentLatencyMs": current_latency,
                "history": history,
                "state": current_state,
                "panel": "target-monitor",
            })
        })
        .collect();
    if store.show_network_identity_topbar {
        items.push(network_identity_topbar_item(
            &store.network_identity,
            &store.network_identity_topbar_source,
        ));
    }
    json!({ "items": items })
}

fn emit_topbar(output: &mpsc::Sender<Value>, store: &MonitorStore) -> Result<(), String> {
    send_outbound(output, PluginEvent {
        message_type: "plugin.event",
        event: "topbar.update",
        payload: topbar_value(store),
    })
}

fn tray_value(store: &MonitorStore) -> Value {
    let items: Vec<Value> = store
        .targets
        .values()
        .filter(|monitor| monitor.target.show_in_system_tray)
        .map(|monitor| {
            let summary = monitor.summary();
            let history = summary
                .get("history")
                .and_then(Value::as_array)
                .map(|history| {
                    let start = history.len().saturating_sub(TOPBAR_HISTORY_LIMIT);
                    history[start..].to_vec()
                })
                .unwrap_or_default();
            let current_state = summary
                .get("state")
                .and_then(Value::as_str)
                .unwrap_or("READY");
            let series: Vec<Value> = history.iter().map(|entry| {
                let state = entry.get("state").and_then(Value::as_str).unwrap_or("OFFLINE");
                json!({
                    "value": entry.get("latencyMs").cloned().unwrap_or(Value::Null),
                    "tone": tray_tone(state),
                    "color": monitor.target.graph_color_for_state(state),
                })
            }).collect();
            let latency = summary.get("currentLatencyMs").and_then(Value::as_f64);
            let tooltip = match latency {
                Some(value) => format!("{} · {} · {:.1} ms · {}", monitor.target.name, monitor.target.address, value, current_state),
                None => format!("{} · {} · {}", monitor.target.name, monitor.target.address, current_state),
            };
            json!({
                "id": monitor.target.id,
                "title": monitor.target.name,
                "tooltip": tooltip,
                "tone": tray_tone(current_state),
                "graphColor": monitor.target.graph_color_for_state(current_state),
                "backgroundColor": monitor.target.background_color,
                "borderColor": monitor.target.border_color,
                "visible": monitor.target.enabled,
                "series": series,
                "action": {
                    "type": "open-panel",
                    "panel": "target-monitor",
                    "context": {
                        "targetId": monitor.target.id,
                        "target": monitor.target.address,
                    }
                }
            })
        })
        .collect();
    json!({ "items": items })
}

fn emit_tray(output: &mpsc::Sender<Value>, store: &MonitorStore) -> Result<(), String> {
    send_outbound(output, PluginEvent {
        message_type: "plugin.event",
        event: "tray.update",
        payload: tray_value(store),
    })
}


fn sound_data_bytes(sound: &SoundConfig) -> Option<u64> {
    let (_, encoded) = sound.data_url.split_once(",")?;
    let len = encoded.len() as u64;
    if len % 4 == 1 {
        return None;
    }
    let mut bytes = (len / 4) * 3;
    match len % 4 {
        2 => bytes += 1,
        3 => bytes += 2,
        _ => {}
    }
    let padding = encoded.as_bytes().iter().rev().take(2).filter(|byte| **byte == b'=').count() as u64;
    Some(bytes.saturating_sub(padding))
}

fn validate_sound(sound: &SoundConfig) -> Result<(), String> {
    if sound.filename.trim().is_empty() || sound.filename.len() > 128 {
        return Err("sound filename is missing or too long".to_string());
    }
    let mime_ok = matches!(sound.mime.as_str(), "audio/mpeg" | "audio/mp3" | "audio/wav" | "audio/x-wav");
    if !mime_ok {
        return Err("sound must be MP3 or WAV".to_string());
    }
    if sound.size_bytes == 0 || sound.size_bytes > MAX_SOUND_FILE_BYTES {
        return Err("sound file must be 1 byte to 2 MiB".to_string());
    }
    if sound.duration_ms == 0 || sound.duration_ms > MAX_SOUND_DURATION_MS {
        return Err("sound duration must be 1-10 seconds".to_string());
    }
    let prefix_ok = sound.data_url.starts_with("data:audio/mpeg;base64,")
        || sound.data_url.starts_with("data:audio/mp3;base64,")
        || sound.data_url.starts_with("data:audio/wav;base64,")
        || sound.data_url.starts_with("data:audio/x-wav;base64,");
    if !prefix_ok {
        return Err("sound data must be a base64 MP3/WAV data URL".to_string());
    }
    let encoded_bytes = sound_data_bytes(sound).ok_or_else(|| "invalid sound data URL".to_string())?;
    if encoded_bytes == 0 || encoded_bytes > MAX_SOUND_FILE_BYTES {
        return Err("sound data exceeds the 2 MiB limit".to_string());
    }
    if encoded_bytes.abs_diff(sound.size_bytes) > 3 {
        return Err("sound size metadata does not match sound data".to_string());
    }
    Ok(())
}

fn validate_alerts(alerts: &AlertConfig) -> Result<(), String> {
    if alerts.high_latency.cooldown_seconds < MIN_COOLDOWN_SECONDS || alerts.high_latency.cooldown_seconds > MAX_COOLDOWN_SECONDS {
        return Err("high-latency cooldown must be 5 seconds to 24 hours".to_string());
    }
    if alerts.offline.cooldown_seconds < MIN_COOLDOWN_SECONDS || alerts.offline.cooldown_seconds > MAX_COOLDOWN_SECONDS {
        return Err("offline cooldown must be 5 seconds to 24 hours".to_string());
    }
    if alerts.recovery.cooldown_seconds < MIN_COOLDOWN_SECONDS || alerts.recovery.cooldown_seconds > MAX_COOLDOWN_SECONDS {
        return Err("recovery cooldown must be 5 seconds to 24 hours".to_string());
    }
    if alerts.reminder_interval_seconds < MIN_COOLDOWN_SECONDS || alerts.reminder_interval_seconds > MAX_COOLDOWN_SECONDS {
        return Err("reminder interval must be 5 seconds to 24 hours".to_string());
    }
    let sounds = [&alerts.high_latency.sound, &alerts.offline.sound, &alerts.recovery.sound];
    let mut total = 0u64;
    for sound in sounds.into_iter().flatten() {
        validate_sound(sound)?;
        total = total.saturating_add(sound.size_bytes);
    }
    if total > MAX_TOTAL_SOUND_BYTES {
        return Err("total custom alert sounds exceed the 6 MiB limit".to_string());
    }
    Ok(())
}

fn alert_rule<'a>(alerts: &'a AlertConfig, kind: &str) -> Option<&'a AlertRule> {
    match kind {
        "highLatency" => Some(&alerts.high_latency),
        "offline" => Some(&alerts.offline),
        "recovery" => Some(&alerts.recovery),
        _ => None,
    }
}

fn alert_kind(previous: &str, current: &str) -> Option<&'static str> {
    match (previous, current) {
        ("NORMAL", "HIGH LATENCY") | ("READY", "HIGH LATENCY") => Some("highLatency"),
        ("NORMAL", "OFFLINE") | ("HIGH LATENCY", "OFFLINE") | ("READY", "OFFLINE") => Some("offline"),
        ("OFFLINE", "NORMAL") | ("OFFLINE", "HIGH LATENCY") | ("HIGH LATENCY", "NORMAL") => Some("recovery"),
        _ => None,
    }
}

fn maybe_emit_alert(
    output: &mpsc::Sender<Value>,
    store: &mut MonitorStore,
    target_id: &str,
    kind: &str,
    reminder: bool,
) -> Result<(), String> {
    if !store.alerts.enabled {
        return Ok(());
    }
    let Some(rule) = alert_rule(&store.alerts, kind) else { return Ok(()); };
    if !rule.enabled {
        return Ok(());
    }
    let now = now_ms();
    let key = format!("{target_id}:{kind}");
    let interval = if reminder {
        store.alerts.reminder_interval_seconds
    } else {
        rule.cooldown_seconds
    } * 1000;
    if let Some(last) = store.last_alert_at.get(&key) {
        if now.saturating_sub(*last) < interval {
            return Ok(());
        }
    }
    store.last_alert_at.insert(key, now);
    let target = store.targets.get(target_id).map(|monitor| monitor.target.clone());
    let Some(target) = target else { return Ok(()); };
    let payload = json!({
        "targetId": target.id,
        "target": target.address,
        "targetName": target.name,
        "alertType": kind,
        "reminder": reminder,
        "fallbackBeep": rule.sound.is_none(),
        "sound": rule.sound.as_ref().map(|sound| json!({
            "filename": sound.filename,
            "mime": sound.mime,
            "dataUrl": sound.data_url,
            "durationMs": sound.duration_ms,
        })),
    });
    send_outbound(output, PluginEvent { message_type: "plugin.event", event: "monitor.alert", payload })
}

fn process_sample_alert(
    output: &mpsc::Sender<Value>,
    store: &mut MonitorStore,
    target_id: &str,
    transition: Option<(String, String)>,
) {
    if let Some((previous, current)) = transition {
        store.last_alert_at.remove(&format!("{target_id}:highLatency"));
        store.last_alert_at.remove(&format!("{target_id}:offline"));
        store.last_alert_at.remove(&format!("{target_id}:recovery"));
        if let Some(kind) = alert_kind(&previous, &current) {
            let _ = maybe_emit_alert(output, store, target_id, kind, false);
        }
        return;
    }
    if !store.alerts.reminders_enabled {
        return;
    }
    let current = store.targets.get(target_id).map(|monitor| monitor.last_state.clone());
    let Some(current) = current else { return; };
    let kind = match current.as_str() {
        "OFFLINE" => "offline",
        "HIGH LATENCY" => "highLatency",
        _ => return,
    };
    let _ = maybe_emit_alert(output, store, target_id, kind, true);
}

fn ensure_config_loaded(
    store: &Arc<Mutex<MonitorStore>>,
    input: &mut impl Read,
    output: &mpsc::Sender<Value>,
    request_id: &mut u64,
) -> Result<(), String> {
    let loaded = store
        .lock()
        .map_err(|_| "monitor state lock poisoned".to_string())?
        .config_loaded;
    if loaded {
        return Ok(());
    }

    let loaded_config = load_config(input, output, request_id)?;
    match loaded_config {
        Some(config) => {
            let normalized = normalize_config(config.clone())?;
            let changed = normalized != config;
            store
                .lock()
                .map_err(|_| "monitor state lock poisoned".to_string())?
                .replace_config(normalized.clone());
            if changed {
                save_config(input, output, request_id, &normalized)?;
            }
        }
        None => {
            let config = default_config();
            save_config(input, output, request_id, &config)?;
            store
                .lock()
                .map_err(|_| "monitor state lock poisoned".to_string())?
                .replace_config(config);
        }
    }
    Ok(())
}

fn alert_summary(alerts: &AlertConfig) -> Value {
    json!({
        "enabled": alerts.enabled,
        "highLatency": {
            "enabled": alerts.high_latency.enabled,
            "cooldownSeconds": alerts.high_latency.cooldown_seconds,
            "sound": alerts.high_latency.sound.as_ref().map(|sound| json!({
                "filename": sound.filename,
                "mime": sound.mime,
                "sizeBytes": sound.size_bytes,
                "durationMs": sound.duration_ms,
            })),
        },
        "offline": {
            "enabled": alerts.offline.enabled,
            "cooldownSeconds": alerts.offline.cooldown_seconds,
            "sound": alerts.offline.sound.as_ref().map(|sound| json!({
                "filename": sound.filename,
                "mime": sound.mime,
                "sizeBytes": sound.size_bytes,
                "durationMs": sound.duration_ms,
            })),
        },
        "recovery": {
            "enabled": alerts.recovery.enabled,
            "cooldownSeconds": alerts.recovery.cooldown_seconds,
            "sound": alerts.recovery.sound.as_ref().map(|sound| json!({
                "filename": sound.filename,
                "mime": sound.mime,
                "sizeBytes": sound.size_bytes,
                "durationMs": sound.duration_ms,
            })),
        },
        "remindersEnabled": alerts.reminders_enabled,
        "reminderIntervalSeconds": alerts.reminder_interval_seconds,
    })
}

fn main() -> Result<(), String> {
    let plugin_id = std::env::var("REMOTEHAND_PLUGIN_ID")
        .map_err(|_| "REMOTEHAND_PLUGIN_ID is missing".to_string())?;
    let api_version: u32 = std::env::var("REMOTEHAND_PLUGIN_API_VERSION")
        .map_err(|_| "REMOTEHAND_PLUGIN_API_VERSION is missing".to_string())?
        .parse()
        .map_err(|error| format!("invalid plugin API version: {error}"))?;
    let plugin_version = env!("CARGO_PKG_VERSION");

    let stdin = io::stdin();
    let mut input = stdin.lock();
    let stdout = io::stdout();
    let mut handshake_output = stdout.lock();
    write_frame_raw(
        &mut handshake_output,
        &PluginHello {
            message_type: "plugin.hello",
            plugin_id: &plugin_id,
            api_version,
            plugin_version,
        },
    )?;
 handshake_output.flush().map_err(|error| error.to_string())?;


    let hello = read_frame(&mut input)?;
    let ready: HostReady = serde_json::from_slice(&hello)
        .map_err(|error| format!("invalid host.ready message: {error}"))?;
    if ready.message_type != "host.ready" {
        return Err("expected host.ready".to_string());
    }
    if ready.plugin_id != plugin_id || ready.api_version != api_version {
        return Err("host.ready does not match plugin identity or API version".to_string());
    }
    drop(handshake_output);

    let (output_tx, output_rx) = mpsc::channel::<Value>();
    let writer = thread::spawn(move || {
        let stdout = io::stdout();
        let mut output = stdout.lock();
        while let Ok(value) = output_rx.recv() {
            if let Err(error) = write_frame_raw(&mut output, &value) {
                return Err(error);
            }
        }
        Ok::<(), String>(())
    });

    let store = Arc::new(Mutex::new(MonitorStore::new()));
    let sampler_store = Arc::clone(&store);
    let sampler_output = output_tx.clone();

    thread::spawn(move || loop {
        thread::sleep(SAMPLE_INTERVAL);
        let targets = match sampler_store.lock() {
            Ok(store) => store.enabled_targets(),
            Err(_) => return,
        };
        for target in targets {
            let sample = sample_target(&target);
            if let Ok(mut store) = sampler_store.lock() {
                let transition = store
                    .targets
                    .entry(target.id.clone())
                    .or_insert_with(|| TargetMonitor::new(target.clone()))
                    .record(now_ms(), &sample);
                process_sample_alert(&sampler_output, &mut store, &target.id, transition);
                if let Some(summary) = store.targets.get(&target.id).map(TargetMonitor::summary) {
                    if send_outbound(&sampler_output, PluginEvent {
                        message_type: "plugin.event",
                        event: "monitor.sampled",
                        payload: summary,
                    }).is_err() {
                        return;
                    }
                }
                if emit_topbar(&sampler_output, &store).is_err()
                    || emit_tray(&sampler_output, &store).is_err()
                {
                    return;
                }
            } else {
                return;
            }
        }
    });

    let mut host_request_id = 1u64;
    ensure_config_loaded(&store, &mut input, &output_tx, &mut host_request_id)?;
    if let Ok(result) = host_request(
        &mut input,
        &output_tx,
        &mut host_request_id,
        "network.interfaces",
        json!({}),
    ) {
        if let Ok(mut guard) = store.lock() {
            guard.network_identity.interfaces = result
                .get("interfaces")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
        }
    }

    {
        let public_store = Arc::clone(&store);
        let public_output = output_tx.clone();
        thread::spawn(move || {
            let result = lookup_public_ip();
            if let Ok(mut guard) = public_store.lock() {
                match result {
                    Ok(address) => {
                        guard.network_identity.public_ip = Some(address);
                        guard.network_identity.public_ip_error = None;
                    }
                    Err(error) => {
                        guard.network_identity.public_ip = None;
                        guard.network_identity.public_ip_error = Some(error);
                    }
                }
                let _ = emit_topbar(&public_output, &guard);
            }
        });
    }

    {
        let store_guard = store
            .lock()
            .map_err(|_| "monitor state lock poisoned".to_string())?;
        send_outbound(
            &output_tx,
            PluginEvent {
                message_type: "plugin.event",
                event: "monitor.ready",
                payload: json!({
                    "status": "ready",
                    "targets": store_guard.config().targets,
                    "alerts": alert_summary(&store_guard.alerts),
                    "showNetworkIdentityTopbar": store_guard.show_network_identity_topbar,
                    "networkIdentityTopbarSource": store_guard.network_identity_topbar_source,
                }),
            },
        )?;
        emit_topbar(&output_tx, &store_guard)?;
        emit_tray(&output_tx, &store_guard)?;
    }

    loop {
        let payload = match read_frame(&mut input) {
            Ok(payload) => payload,
            Err(error) if error.contains("failed to read plugin frame length") => break,
            Err(error) => return Err(error),
        };
        let request: HostRequest = serde_json::from_slice(&payload)
            .map_err(|error| format!("invalid host.request: {error}"))?;
        if request.message_type != "host.request" {
            return Err("unexpected host message type".to_string());
        }

        let response = match request.method.as_str() {
            "monitor.network.identity" => {
                let interfaces = host_request(
                    &mut input,
                    &output_tx,
                    &mut host_request_id,
                    "network.interfaces",
                    json!({}),
                );
                match interfaces {
                    Ok(interfaces) => {
                        let (public_ip, public_ip_error) = match lookup_public_ip() {
                            Ok(address) => (Some(address), None),
                            Err(error) => (None, Some(error)),
                        };
                        let identity = NetworkIdentity {
                            public_ip,
                            public_ip_error,
                            interfaces: interfaces
                                .get("interfaces")
                                .and_then(Value::as_array)
                                .cloned()
                                .unwrap_or_default(),
                        };
                        if let Ok(mut guard) = store.lock() {
                            guard.network_identity = identity.clone();
                            emit_topbar(&output_tx, &guard)?;
                        }
                        PluginResponse {
                            message_type: "plugin.response",
                            request_id: request.request_id,
                            ok: true,
                            result: network_identity_value(&identity),
                            error: String::new(),
                        }
                    }
                    Err(error) => PluginResponse {
                        message_type: "plugin.response",
                        request_id: request.request_id,
                        ok: false,
                        result: Value::Null,
                        error,
                    },
                }
            }
            "monitor.clipboard.write" => {
                let text = request.params.get("text").and_then(Value::as_str).unwrap_or("");
                if text.is_empty() || text.len() > 4096 {
                    PluginResponse {
                        message_type: "plugin.response",
                        request_id: request.request_id,
                        ok: false,
                        result: Value::Null,
                        error: "clipboard text must be 1-4096 bytes".to_string(),
                    }
                } else {
                    match host_request(
                        &mut input,
                        &output_tx,
                        &mut host_request_id,
                        "clipboard.write",
                        json!({ "text": text }),
                    ) {
                        Ok(result) => PluginResponse {
                            message_type: "plugin.response",
                            request_id: request.request_id,
                            ok: true,
                            result,
                            error: String::new(),
                        },
                        Err(error) => PluginResponse {
                            message_type: "plugin.response",
                            request_id: request.request_id,
                            ok: false,
                            result: Value::Null,
                            error,
                        },
                    }
                }
            }
            "monitor.status" => {
                ensure_config_loaded(&store, &mut input, &output_tx, &mut host_request_id)?;
                let guard = store
                    .lock()
                    .map_err(|_| "monitor state lock poisoned".to_string())?;
                PluginResponse {
                    message_type: "plugin.response",
                    request_id: request.request_id,
                    ok: true,
                    result: json!({
                        "targets": guard.targets.values().map(TargetMonitor::summary).collect::<Vec<_>>(),
                        "alerts": guard.config().alerts,
                        "showNetworkIdentityTopbar": guard.show_network_identity_topbar,
                        "networkIdentityTopbarSource": guard.network_identity_topbar_source,
                    }),
                    error: String::new(),
                }
            }
            "monitor.config.get" => {
                ensure_config_loaded(&store, &mut input, &output_tx, &mut host_request_id)?;
                let guard = store
                    .lock()
                    .map_err(|_| "monitor state lock poisoned".to_string())?;
                PluginResponse {
                    message_type: "plugin.response",
                    request_id: request.request_id,
                    ok: true,
                    result: serde_json::to_value(guard.config())
                        .map_err(|error| format!("failed to encode config: {error}"))?,
                    error: String::new(),
                }
            }
            "monitor.sample" => {
                ensure_config_loaded(&store, &mut input, &output_tx, &mut host_request_id)?;
                let id = request.params.get("targetId").and_then(Value::as_str);
                let target = {
                    let guard = store
                        .lock()
                        .map_err(|_| "monitor state lock poisoned".to_string())?;
                    id.and_then(|target_id| guard.targets.get(target_id).map(|monitor| monitor.target.clone()))
                };
                match target {
                    Some(target) => {
                        let sample = sample_target(&target);
                        let mut guard = store
                            .lock()
                            .map_err(|_| "monitor state lock poisoned".to_string())?;
                        let transition = guard
                            .targets
                            .entry(target.id.clone())
                            .or_insert_with(|| TargetMonitor::new(target.clone()))
                            .record(now_ms(), &sample);
                        process_sample_alert(&output_tx, &mut guard, &target.id, transition);
                        emit_topbar(&output_tx, &guard)?;
                        emit_tray(&output_tx, &guard)?;
                        PluginResponse {
                            message_type: "plugin.response",
                            request_id: request.request_id,
                            ok: true,
                            result: guard
                                .targets
                                .get(&target.id)
                                .map(TargetMonitor::summary)
                                .unwrap_or(sample),
                            error: String::new(),
                        }
                    }
                    None => PluginResponse {
                        message_type: "plugin.response",
                        request_id: request.request_id,
                        ok: false,
                        result: Value::Null,
                        error: "target is required".to_string(),
                    },
                }
            }
            "monitor.alert.test" => {
                ensure_config_loaded(&store, &mut input, &output_tx, &mut host_request_id)?;
                let kind = request.params.get("alertType").and_then(Value::as_str).unwrap_or("offline");
                if !matches!(kind, "highLatency" | "offline" | "recovery") {
                    PluginResponse { message_type: "plugin.response", request_id: request.request_id, ok: false, result: Value::Null, error: "invalid alert type".to_string() }
                } else {
                    let guard = store
                        .lock()
                        .map_err(|_| "monitor state lock poisoned".to_string())?;
                    let id = guard.targets.values().next().map(|monitor| monitor.target.id.clone());
                    match id {
                        Some(id) if guard.alerts.enabled && alert_rule(&guard.alerts, kind).map(|rule| rule.enabled).unwrap_or(false) => {
                            let target = guard.targets.get(&id).map(|monitor| monitor.target.clone()).unwrap();
                            let sound = alert_rule(&guard.alerts, kind).and_then(|rule| rule.sound.as_ref());
                            let payload = json!({
                                "targetId": target.id,
                                "target": target.address,
                                "targetName": target.name,
                                "alertType": kind,
                                "test": true,
                                "reminder": false,
                                "fallbackBeep": sound.is_none(),
                                "sound": sound.map(|sound| json!({
                                    "filename": sound.filename,
                                    "mime": sound.mime,
                                    "dataUrl": sound.data_url,
                                    "durationMs": sound.duration_ms,
                                })),
                            });
                            send_outbound(&output_tx, PluginEvent { message_type: "plugin.event", event: "monitor.alert", payload })?;
                            PluginResponse { message_type: "plugin.response", request_id: request.request_id, ok: true, result: json!({ "tested": true }), error: String::new() }
                        }
                        _ => PluginResponse { message_type: "plugin.response", request_id: request.request_id, ok: true, result: json!({ "tested": false }), error: String::new() },
                    }
                }
            }
            "monitor.config.set" => {
                ensure_config_loaded(&store, &mut input, &output_tx, &mut host_request_id)?;
                let current = store
                    .lock()
                    .map_err(|_| "monitor state lock poisoned".to_string())?
                    .config();
                let targets: Vec<MonitorTarget> = serde_json::from_value(
                    request.params
                        .get("targets")
                        .cloned()
                        .unwrap_or_else(|| json!([])),
                )
                .map_err(|error| format!("invalid targets: {error}"))?;
                let alerts = match request.params.get("alerts") {
                    Some(value) => serde_json::from_value(value.clone())
                        .map_err(|error| format!("invalid alerts: {error}"))?,
                    None => current.alerts,
                };
                let show_network_identity_topbar = request.params
                    .get("showNetworkIdentityTopbar")
                    .and_then(Value::as_bool)
                    .unwrap_or(current.show_network_identity_topbar);
                let network_identity_topbar_source = request.params
                    .get("networkIdentityTopbarSource")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or(current.network_identity_topbar_source);
                let new_config = StoredConfig {
                    targets: targets.clone(),
                    alerts,
                    show_network_identity_topbar,
                    network_identity_topbar_source,
                };
                if let Err(error) = normalize_config(new_config.clone()) {
                    PluginResponse { message_type: "plugin.response", request_id: request.request_id, ok: false, result: Value::Null, error }
                } else {
                    let previous = store
                        .lock()
                        .map_err(|_| "monitor state lock poisoned".to_string())?
                        .config();
                    store
                        .lock()
                        .map_err(|_| "monitor state lock poisoned".to_string())?
                        .replace_config(new_config.clone());
                    match save_config(&mut input, &output_tx, &mut host_request_id, &new_config) {
                        Ok(()) => {
                            let guard = store
                                .lock()
                                .map_err(|_| "monitor state lock poisoned".to_string())?;
                            emit_topbar(&output_tx, &guard)?;
                        emit_tray(&output_tx, &guard)?;
                            PluginResponse { message_type: "plugin.response", request_id: request.request_id, ok: true, result: serde_json::to_value(guard.config()).map_err(|error| format!("failed to encode config: {error}"))?, error: String::new() }
                        }
                        Err(error) => {
                            store
                                .lock()
                                .map_err(|_| "monitor state lock poisoned".to_string())?
                                .replace_config(previous);
                            PluginResponse { message_type: "plugin.response", request_id: request.request_id, ok: false, result: Value::Null, error }
                        }
                    }
                }
            }
            "monitor.shutdown" => {
                let response = PluginResponse {
                    message_type: "plugin.response",
                    request_id: request.request_id,
                    ok: true,
                    result: json!({ "stopped": true }),
                    error: String::new(),
                };
                send_outbound(&output_tx, response)?;
                break;
            }
            _ => PluginResponse {
                message_type: "plugin.response",
                request_id: request.request_id,
                ok: false,
                result: Value::Null,
                error: format!("unsupported Target Monitor method: {}", request.method),
            },
        };

        send_outbound(&output_tx, response)?;
    }

    drop(output_tx);
    let _ = writer.join();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(id: &str, name: &str, address: &str, enabled: bool) -> MonitorTarget {
        MonitorTarget {
            id: id.to_string(),
            name: name.to_string(),
            address: address.to_string(),
            enabled,
            show_in_system_tray: false,
            graph_color: None,
            graph_normal_color: default_graph_normal_color(),
            graph_high_color: default_graph_high_color(),
            graph_offline_color: default_graph_offline_color(),
            target_name_color: default_target_name_color(),
            latency_color: default_latency_color(),
            background_color: default_background_color(),
            border_color: default_border_color(),
        }
    }

    #[test]
    fn parses_public_ip_addresses() {
        assert_eq!(parse_public_ip("203.0.113.10\n").as_deref(), Some("203.0.113.10"));
        assert_eq!(parse_public_ip("2001:db8::1").as_deref(), Some("2001:db8::1"));
        assert!(parse_public_ip("not-an-ip").is_none());
    }

    #[test]
    fn topbar_selector_includes_link_local_but_not_loopback() {
        assert!(topbar_selectable_ip("169.254.148.137"));
        assert!(topbar_selectable_ip("fe80::1"));
        assert!(!topbar_selectable_ip("127.0.0.1"));
        assert!(!topbar_selectable_ip("::1"));
    }

    #[test]
    fn network_identity_topbar_options_include_non_loopback_ipv4() {
        let identity = NetworkIdentity {
            public_ip: None,
            public_ip_error: None,
            interfaces: vec![json!({
                "name": "Ethernet",
                "addresses": [
                    { "address": "169.254.148.137", "family": "ipv4" },
                    { "address": "127.0.0.1", "family": "ipv4" }
                ]
            })],
        };
        let options = network_identity_topbar_options(&identity);
        assert!(options.iter().any(|option| option["address"] == "169.254.148.137"));
        assert!(options.iter().all(|option| option["address"] != "127.0.0.1"));
    }

    #[test]
    fn classifies_latency_states() {
        assert_eq!(classify_latency(20.0), "NORMAL");
        assert_eq!(classify_latency(200.0), "NORMAL");
        assert_eq!(classify_latency(200.1), "HIGH LATENCY");
    }

    #[test]
    fn rejects_empty_target() { assert!(resolve_target("").is_err()); }

    #[test]
    fn resolves_loopback_ip() {
        assert_eq!(resolve_target("127.0.0.1").unwrap(), "127.0.0.1".parse::<std::net::IpAddr>().unwrap());
    }

    #[test]
    fn keeps_history_bounded() {
        let mut monitor = TargetMonitor::new(target("a", "A", "127.0.0.1", true));
        for i in 0..(HISTORY_LIMIT + 5) {
            monitor.record(i as u64, &json!({ "state": "NORMAL", "latencyMs": 10.0, "packetLossPercent": 0.0 }));
        }
        assert_eq!(monitor.samples.len(), HISTORY_LIMIT);
        assert_eq!(monitor.samples.first().unwrap().timestamp, 5);
    }

    #[test]
    fn confirms_state_after_two_samples() {
        let mut monitor = TargetMonitor::new(target("a", "A", "127.0.0.1", true));
        assert!(monitor.record(1, &json!({ "state": "NORMAL" })).is_none());
        assert_eq!(monitor.last_state, "READY");
        let transition = monitor.record(2, &json!({ "state": "NORMAL" }));
        assert_eq!(monitor.last_state, "NORMAL");
        assert_eq!(transition, Some(("READY".to_string(), "NORMAL".to_string())));
    }

    #[test]
    fn validates_multiple_targets() {
        assert!(validate_targets(&[target("one", "One", "8.8.8.8", true), target("two", "Two", "1.1.1.1", false)]).is_ok());
    }

    #[test]
    fn default_config_is_google_dns() {
        let config = default_config();
        assert_eq!(config.targets.len(), 1);
        assert_eq!(config.targets[0].name, "Google DNS");
        assert_eq!(config.targets[0].address, "8.8.8.8");
        assert!(config.targets[0].enabled);
        assert_eq!(config.network_identity_topbar_source, "public");
    }

    #[test]
    fn normalizes_empty_config_to_default() {
        let config = normalize_config(StoredConfig { targets: vec![], alerts: AlertConfig::default(), show_network_identity_topbar: false, network_identity_topbar_source: default_network_identity_topbar_source() }).unwrap();
        assert_eq!(config.targets.len(), 1);
        assert_eq!(config.targets[0].name, "Google DNS");
        assert_eq!(config.targets[0].address, "8.8.8.8");
        assert!(config.targets[0].enabled);
    }

    #[test]
    fn normalizes_all_disabled_config() {
        let config = normalize_config(StoredConfig {
            targets: vec![target("one", "One", "8.8.8.8", false), target("two", "Two", "1.1.1.1", false)],
            alerts: AlertConfig::default(),
            show_network_identity_topbar: false,
            network_identity_topbar_source: default_network_identity_topbar_source(),
        }).unwrap();
        assert!(config.targets.iter().any(|target| target.enabled));
    }

    #[test]
    fn default_config_shows_network_identity_topbar() {
        assert!(default_config().show_network_identity_topbar);
    }

    #[test]
    fn network_identity_topbar_can_be_hidden() {
        let mut store = MonitorStore::new();
        store.show_network_identity_topbar = false;
        let value = topbar_value(&store);
        let items = value["items"].as_array().unwrap();
        assert!(items.iter().all(|item| item["id"] != "target-monitor:network-identity"));
    }

    #[test]
    fn network_identity_topbar_contains_copy_menu() {
        let mut store = MonitorStore::new();
        store.show_network_identity_topbar = true;
        store.network_identity.public_ip = Some("203.0.113.10".to_string());
        store.network_identity.interfaces = vec![json!({
            "name": "Ethernet",
            "addresses": [
                { "address": "192.168.1.10", "family": "ipv4" },
                { "address": "fe80::1", "family": "ipv6" }
            ]
        })];
        let value = topbar_value(&store);
        let items = value["items"].as_array().unwrap();
        let item = items.iter().find(|item| item["id"] == "target-monitor:network-identity").unwrap();
        assert_eq!(item["menu"]["items"][0]["value"], "203.0.113.10");
        assert_eq!(item["menu"]["items"][0]["action"]["method"], "monitor.clipboard.write");
        assert_eq!(item["icon"], "globe");
        assert_eq!(item["content"]["label"], "203.0.113.10");
        assert_eq!(item["size"]["width"], 124);
    }

    #[test]
    fn network_identity_topbar_uses_selected_interface_source() {
        let mut store = MonitorStore::new();
        store.show_network_identity_topbar = true;
        store.network_identity_topbar_source = "interface:Wi-Fi:ipv4".to_string();
        store.network_identity.public_ip = Some("203.0.113.10".to_string());
        store.network_identity.interfaces = vec![json!({
            "name": "Wi-Fi",
            "addresses": [{ "address": "192.168.1.25", "family": "ipv4" }]
        })];
        let value = topbar_value(&store);
        let items = value["items"].as_array().unwrap();
        let item = items.iter().find(|item| item["id"] == "target-monitor:network-identity").unwrap();
        assert_eq!(item["icon"], "wifi");
        assert_eq!(item["content"]["label"], "192.168.1.25");
    }

    #[test]
    fn topbar_contains_only_enabled_targets() {
        let mut store = MonitorStore::new();
        store.targets.insert("two".to_string(), TargetMonitor::new(target("two", "One", "1.1.1.1", false)));
        let items = topbar_value(&store)["items"].as_array().unwrap().clone();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["target"], "8.8.8.8");
        assert_eq!(items[0]["size"]["width"], 112);
        assert_eq!(items[0]["content"]["type"], "stack");
        assert_eq!(items[0]["content"]["children"][0]["type"], "text");
        assert_eq!(items[0]["content"]["children"][0]["fontSize"], 9);
        assert_eq!(items[0]["content"]["children"][1]["type"], "metric");
        assert_eq!(items[0]["content"]["children"][2]["type"], "sparkline");
    }

    #[test]
    fn replace_config_removes_missing_targets() {
        let mut store = MonitorStore::new();
        store.targets.insert(
            "two".to_string(),
            TargetMonitor::new(target("two", "Two", "1.1.1.1", true)),
        );
        let config = StoredConfig {
            targets: vec![target(DEFAULT_TARGET_ID, DEFAULT_TARGET_NAME, DEFAULT_TARGET, true)],
            alerts: AlertConfig::default(),
            show_network_identity_topbar: false,
            network_identity_topbar_source: default_network_identity_topbar_source(),
        };
        store.replace_config(config);
        assert!(store.targets.contains_key(DEFAULT_TARGET_ID));
        assert!(!store.targets.contains_key("two"));
        assert_eq!(store.targets.len(), 1);
    }

    #[test]
    fn replace_config_updates_tray_preference_without_losing_monitor_state() {
        let mut store = MonitorStore::new();
        if let Some(default) = store.targets.get_mut(DEFAULT_TARGET_ID) {
            default.record(1, &json!({ "state": "NORMAL", "latencyMs": 12.0, "packetLossPercent": 0.0 }));
        }
        let mut configured = default_config();
        configured.targets[0].show_in_system_tray = true;
        store.replace_config(configured);
        let monitor = store.targets.get(DEFAULT_TARGET_ID).unwrap();
        assert!(monitor.target.show_in_system_tray);
        assert_eq!(monitor.samples.len(), 1);
    }

    #[test]
    fn tray_contains_only_enabled_opted_in_targets() {
        let mut store = MonitorStore::new();
        if let Some(default) = store.targets.get_mut(DEFAULT_TARGET_ID) {
            default.target.show_in_system_tray = true;
        }
        store.targets.insert("two".to_string(), TargetMonitor::new(MonitorTarget {
            id: "two".to_string(),
            name: "Two".to_string(),
            address: "1.1.1.1".to_string(),
            enabled: true,
            show_in_system_tray: false,
            graph_color: None,
            graph_normal_color: default_graph_normal_color(),
            graph_high_color: default_graph_high_color(),
            graph_offline_color: default_graph_offline_color(),
            target_name_color: default_target_name_color(),
            latency_color: default_latency_color(),
            background_color: default_background_color(),
            border_color: default_border_color(),
        }));
        store.targets.insert("three".to_string(), TargetMonitor::new(MonitorTarget {
            id: "three".to_string(),
            name: "Three".to_string(),
            address: "9.9.9.9".to_string(),
            enabled: false,
            show_in_system_tray: true,
            graph_color: None,
            graph_normal_color: default_graph_normal_color(),
            graph_high_color: default_graph_high_color(),
            graph_offline_color: default_graph_offline_color(),
            target_name_color: default_target_name_color(),
            latency_color: default_latency_color(),
            background_color: default_background_color(),
            border_color: default_border_color(),
        }));
        let items = tray_value(&store)["items"].as_array().unwrap().clone();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0]["id"], DEFAULT_TARGET_ID);
        assert_eq!(items[0]["visible"], true);
        assert_eq!(items[0]["action"]["type"], "open-panel");
        assert_eq!(items[1]["id"], "three");
        assert_eq!(items[1]["visible"], false);
    }

    #[test]
    fn target_colors_flow_to_topbar_and_tray() {
        let mut store = MonitorStore::new();
        if let Some(default) = store.targets.get_mut(DEFAULT_TARGET_ID) {
            default.target.show_in_system_tray = true;
            default.target.graph_normal_color = "#aa33cc".to_string();
            default.target.graph_high_color = "#cc9933".to_string();
            default.target.graph_offline_color = "#dd3344".to_string();
            default.target.target_name_color = "#abcdef".to_string();
            default.target.latency_color = "#fedcba".to_string();
            default.target.background_color = "#112233".to_string();
            default.target.border_color = "#445566".to_string();
            default.record(1, &json!({ "state": "NORMAL", "latencyMs": 10.0, "packetLossPercent": 0.0 }));
            default.record(2, &json!({ "state": "HIGH LATENCY", "latencyMs": 250.0, "packetLossPercent": 0.0 }));
            default.record(3, &json!({ "state": "OFFLINE", "latencyMs": Value::Null, "packetLossPercent": 100.0 }));
        }
        let topbar = topbar_value(&store);
        let topbar_item = &topbar["items"][0];
        assert_eq!(topbar_item["content"]["backgroundColor"], "#112233");
        assert_eq!(topbar_item["content"]["borderColor"], "#445566");
        assert_eq!(topbar_item["content"]["children"][0]["color"], "#abcdef");
        assert_eq!(topbar_item["content"]["children"][1]["color"], "#fedcba");
        assert_eq!(topbar_item["content"]["children"][2]["series"][0]["color"], "#aa33cc");
        assert_eq!(topbar_item["content"]["children"][2]["series"][1]["color"], "#cc9933");
        assert_eq!(topbar_item["content"]["children"][2]["series"][2]["color"], "#dd3344");

        let tray = tray_value(&store);
        let tray_item = &tray["items"][0];
        assert_eq!(tray_item["graphColor"], "#aa33cc");
        assert_eq!(tray_item["backgroundColor"], "#112233");
        assert_eq!(tray_item["borderColor"], "#445566");
    }

    #[test]
    fn legacy_graph_color_remains_effective_until_next_save() {
        let value = json!({
            "id": "legacy",
            "name": "Legacy",
            "address": "8.8.8.8",
            "enabled": true,
            "showInSystemTray": true,
            "graphColor": "#123456",
            "backgroundColor": "#16263a",
            "borderColor": "#3b5f8d"
        });
        let target: MonitorTarget = serde_json::from_value(value).unwrap();
        assert_eq!(target.effective_graph_normal_color(), "#123456");
        assert_eq!(target.graph_color_for_state("HIGH LATENCY"), DEFAULT_GRAPH_HIGH_COLOR);
        assert_eq!(target.graph_color_for_state("OFFLINE"), DEFAULT_GRAPH_OFFLINE_COLOR);
    }

    #[test]
    fn topbar_tones_follow_state() {
        assert_eq!(topbar_tone("NORMAL"), "accent");
        assert_eq!(topbar_tone("HIGH LATENCY"), "warning");
        assert_eq!(topbar_tone("OFFLINE"), "danger");
        assert_eq!(topbar_tone("READY"), "muted");
    }

    #[test]
    fn default_alerts_are_enabled_with_no_custom_sounds() {
        let alerts = AlertConfig::default();
        assert!(alerts.enabled);
        assert!(alerts.high_latency.enabled);
        assert!(alerts.offline.enabled);
        assert!(alerts.recovery.enabled);
        assert!(!alerts.reminders_enabled);
        assert!(alerts.high_latency.sound.is_none());
    }

    #[test]
    fn rejects_alert_cooldown_below_minimum() {
        let mut alerts = AlertConfig::default();
        alerts.offline.cooldown_seconds = 4;
        assert!(validate_alerts(&alerts).is_err());
    }

    #[test]
    fn maps_confirmed_transitions_to_alert_types() {
        assert_eq!(alert_kind("NORMAL", "HIGH LATENCY"), Some("highLatency"));
        assert_eq!(alert_kind("NORMAL", "OFFLINE"), Some("offline"));
        assert_eq!(alert_kind("HIGH LATENCY", "OFFLINE"), Some("offline"));
        assert_eq!(alert_kind("OFFLINE", "NORMAL"), Some("recovery"));
        assert_eq!(alert_kind("NORMAL", "NORMAL"), None);
    }
}
