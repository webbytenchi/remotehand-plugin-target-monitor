# RemoteHand Target Monitor

Target Monitor is a RemoteHand plugin for monitoring hosts, services, and HTTP/HTTPS endpoints from one compact target model.

It evolves from the validated RemoteHand Host Monitor v0.6.1 foundation and expands monitoring beyond ICMP latency into TCP services, HTTP/HTTPS endpoints, reusable alerts, native notifications, richer history, and configurable RemoteHand surfaces.

## Proposed Target Monitor Dashboard API (feature branch; not yet released)

Target Monitor can declare public, authenticated, **read-only** monitoring routes using the optional `api.json` descriptor and the existing RemoteHand Core Plugin API Gateway v1. All API data is computed inside Target Monitor through the generic `api.read` plugin IPC handler, so adding these routes requires **no changes to RemoteHand Core**.

This feature is undergoing validation on `emma/target-monitor-api-v1`. Its manifest remains v0.7.11 for testing; do not publish it as an already-released version. Older RemoteHand Core versions ignore the optional `api.json` file and continue running the existing plugin UI.

| GET route | Contents |
| --- | --- |
| `/api/v1/target-monitor` | Timestamp and configured/enabled, online/degraded/offline target counts, and active-alert count |
| `/api/v1/target-monitor/targets` | ID, human-readable name, enabled state, status, ping latency, packet loss, timestamps and check/alert counts per target |
| `/api/v1/target-monitor/targets/{id}` | Current status and measurements for one target |
| `/api/v1/target-monitor/targets/{id}/checks` | Enabled TCP and HTTP check names, IDs, port where relevant, reachability, response time, HTTP status and time of latest results |
| `/api/v1/target-monitor/targets/{id}/history` | Latest 120 ping samples, with latency, packet loss, state, and timestamp |
| `/api/v1/target-monitor/alerts` | Current active rule alerts with target name, rule/source labels, acknowledgment and start time |
| `/api/v1/target-monitor/catalog` | Dashboard metric definitions: stable JSON field names, labels, raw units, data sources and visualization kinds |

### Data contract for DisplayHand and Grafana

- Times use **Unix milliseconds**; latency and response times use **milliseconds**; packet loss uses **percent** (0–100). Unknown measurements remain JSON `null` rather than zero. HTTP check health is `null` until a measurement exists.
- Dashboard clients should bind by target `id` rather than array order. Check `id` is scoped to its target; names may change as users rename devices. `/catalog` describes generic fields. A future Grafana adapter can transform these JSON values into its preferred series; this plugin does not expose Prometheus format.
- `/targets` includes disabled targets for configuration awareness; the summary counts only enabled targets in online/degraded/offline totals. `/checks` includes only enabled checks.
- `/history` is bounded to 120 ping samples. Historical events (including archived alert events) are separate from this endpoint; `/alerts` contains only live active rule alerts.
- The API never returns saved HTTP credentials (Bearer, API Key, Basic), target host addresses, endpoint URLs, HTTP response bodies or extracted fields, user-defined request configuration, network interface identities, internal error details, or raw alert values that could contain secrets. Even a read-only API requires trusted bearer-token handling and HTTPS where practical.
- Unknown target IDs are rejected by the plugin; invalid API views are not exposed. API route availability depends on the plugin being enabled.
- These are **proposed feature-branch endpoints** until Windows tests and live endpoint checks pass. Do not merge, bump the release version or publish until the operator accepts the tested behavior.

## Current release

**Target Monitor v0.7.11** is the current Target Monitor release.

It preserves the proven Host Monitor Ping foundation while adding:

- ICMP / Ping monitoring
- multiple TCP checks per target
- multiple HTTP / HTTPS checks per target
- JSON, plain-text, and named-capture regex extraction
- reusable alert rules
- alert acknowledgment and reminders
- per-target native Windows tray notifications
- notification click-through to the originating target report
- queued native notifications
- configurable source prefix/suffix formatting
- configurable graph thresholds and colors
- configurable top-bar Title, Value, and Graph sources
- separate normal/alert colors for the top-bar Value
- configurable monitoring intervals and history retention
- target reports with Ping, TCP, HTTP, configured field alerts, retained-History trigger statistics, active-alert, and recent-history detail
- persistent unified alert / notification / Network Identity History
- lazy-loaded, sortable, filterable, paginated History with CSV export
- restored Network Identity with configurable auto refresh and change alerts
- per-target system-tray graph contributions
- runtime RemoteHand sidebar visibility control
- independent Sound and Notification delivery controls for Ping and reusable alerts
- The global **Enable alert sounds** preference is respected by Ping Alerts: when disabled, Ping alert transitions do not emit the sound-triggering `monitor.alert` event
- Muting Ping alert sounds does not disable native notifications or persistent History
- full-width responsive target cards that make better use of the available RemoteHand panel width
- drag-to-reorder target cards with persisted plugin-owned card order and a Reset order action
- Enable Alerts History setting that stops new History persistence without disabling alert evaluation, sound, notifications, reminders, or recovery
- Clear History with an in-app confirmation flow
- easier From/To History filtering through an in-app calendar/time picker with calendar icons

See `V0.7.0-DESIGN.md` for the delivered product model.

## Core model

A Target Monitor target may contain:

- ICMP / Ping monitoring
- multiple TCP port reachability checks
- multiple HTTP / HTTPS checks
- reusable background alert rules
- one compact RemoteHand top-bar contribution
- an optional per-target system-tray graph

One target produces at most one Target Monitor top-bar item.

Monitoring, alert evaluation, presentation, and alert delivery are separate concerns.

## ICMP / Ping

Target Monitor preserves the proven Host Monitor Ping path:

- real ICMP latency sampling
- Normal / High Latency / Offline state classification
- two-sample state confirmation to reduce flapping
- time-based rolling history
- one-minute default top-bar history
- configurable sampling interval and history retention
- state-change alert sounds
- optional reminders
- per-target tray contribution support

Ping keeps its real state semantics rather than using generic numeric graph thresholds.

## TCP monitoring

Targets may contain multiple TCP checks.

Built-in presets include:

- HTTP — 80
- HTTPS — 443
- SSH — 22
- RDP — 3389
- SMB — 445
- DNS — 53
- SMTP — 25
- IMAP — 143
- POP3 — 110
- MySQL — 3306
- PostgreSQL — 5432

A Custom option allows an arbitrary service name and port.

TCP checks expose reachability and connection-time values for monitoring, presentation, and alerting.

## HTTP / HTTPS monitoring

Targets may contain one or more HTTP/HTTPS requests.

Each request can expose:

- HTTP status
- response time
- content type
- raw or whole-response content
- extracted fields

### JSON responses

JSON fields are discovered automatically and can be used for:

- display
- top-bar Value selection
- numeric graphing
- reusable alert rules
- source formatting

### Plain-text responses

Plain-text responses can be displayed directly, monitored as a whole value, and used for change detection.

### Regex extraction

Non-JSON responses may use user-configured regular expressions with named capture groups.

Captured values become monitored fields. Numeric captures may be graphed or evaluated numerically; text captures may participate in text alerts.

The configuration UI provides Test / Preview before saving extraction settings.

Discovered fields may also be marked **Expose**. Exposure is a presentation choice: exposed values appear in concise Target Report HTTP details and in the target's top-bar hover details, while non-exposed fields continue to remain available for monitoring, graphing, formatting, and alert rules.

HTML is treated as response content or readable text, not rendered as an arbitrary web page inside Target Monitor.

### HTTP authentication

HTTP / HTTPS checks may optionally use generic per-request authentication:

- Bearer Token
- API Key with a configurable header name
- Basic Authentication

Authentication is applied to both Test / Preview requests and background monitoring requests. Existing HTTP checks remain unauthenticated by default.

Credentials are configured per HTTP check and are never placed in the request URL.

## Top bar

Each enabled target may publish one top-bar contribution.

The user independently chooses:

- Title
- Value
- Graph

Any slot may be disabled.

Example:

    Title: Bitaxe
    Value: System Info · hashRate_10m
    Graph: System Info · temp

Fields not displayed in the top bar remain monitored and may still trigger alerts.

### Value colors

The top-bar Value has two user-configurable colors:

- Value normal
- Value alert

The Value switches to Alert color only when the exact selected Value source currently has an active alert.

For Ping latency, High Latency and Offline use the Alert color; Normal uses the Normal color.

An unrelated alert on another source does not recolor the displayed Value.

### Source formatting

Monitored values may define a display prefix and suffix, for example:

- `$`
- `°C`
- `%`
- `TH/s`

Formatting affects presentation only. Graphs, thresholds, and alert evaluation continue to use raw values.

## Graphs and visual thresholds

Numeric graph sources can define optional Warning and Critical thresholds.

Supported operators:

- `>=`
- `<=`
- `>`
- `<`
- `=`

Critical is evaluated before Warning.

Graph colors are configurable for:

- Series / normal
- Warning
- Critical
- Unavailable / offline

Ping uses its real Normal / High Latency / Offline state colors instead of generic thresholds.

## Alerts

Target Monitor includes reusable named alert rules.

Examples:

- HighTemp
- CriticalTemp
- LowHashrate
- PoolChanged

Numeric conditions include:

- `>=`
- `<=`
- `=`

Text/value rules include:

- `equals`
- `changed`
- `didn't change` (stored as `unchanged`), with a configurable duration in seconds

`changed` triggers when the current value differs from the previous sample. It does not use a comparison value.

`didn't change` is intended for counters, timestamps, heartbeats, progress values, and similar sources that are expected to keep changing. It becomes active only after the value remains identical continuously for the configured duration, and any new value immediately resets that timer.

A monitored field may have zero, one, or multiple assigned alerts.

Alerts are transition-based. A rule becomes active once, remains active while its condition is true, and resolves when the condition is no longer true.

Optional reminders may repeat while an alert remains active.

### Sound and notification delivery

Sound playback and notification delivery are separate controls. The global **Enable alert sounds** preference suppresses Ping alert audio when turned off. Ping alert transitions still retain their native-notification and persistent-History behavior; muting audio does not disable monitoring, alert evaluation, or History recording.

## Alert acknowledgment

When a monitored field triggers an unacknowledged alert, the target's top-bar item shows an alert badge.

Opening the target report acknowledges unread alerts and clears the badge.

Acknowledgment does not resolve the condition. An acknowledged alert remains active and visible until its condition resolves.

If the condition triggers again after resolving, it becomes a new alert event.

## Native notifications

Target Monitor v0.7.9 can request native Windows notifications through RemoteHand's generic plugin notification capability.

Per target, the user may independently choose:

- Show this target in system tray
- Show alert notifications

Native notifications are silent so Target Monitor's existing MP3/WAV or fallback-beep audio remains in control.

Multiple notifications for the same target are queued by RemoteHand rather than blindly overwriting each other.

Clicking a Target Monitor notification:

- restores RemoteHand
- opens or focuses Target Monitor
- selects the originating target
- opens that target's Report

## Target reports

Each target has a report containing:

- overall state
- current Ping state and value
- retained Ping history
- TCP check results
- HTTP / HTTPS check results
- extracted HTTP fields
- configured and active alerts
- the ten most recent persisted History events for that target

For HTTP / HTTPS checks, Target Reports show only fields explicitly marked Expose. Missing exposed values are shown as an em dash while the exposure choice remains configured.

Exposed HTTP field tables also show any reusable alert rules assigned to each field, plus **Triggered** and **Last triggered** statistics for each configured rule. These statistics are derived from retained persistent History: reminders are excluded from the trigger count, and the values represent retained History rather than lifetime totals.

## History

Disabling **Alerts History** stops new persistent History entries only. It does not disable monitoring, alert evaluation, active-alert state, Ping/reusable alert delivery, sounds, notifications, reminders, or recovery. Existing saved History remains available until explicitly cleared.

The History view supports search, filters, sorting, pagination, CSV export, Clear History, and From/To filtering through an in-app calendar/time picker.



Target Monitor maintains a bounded persistent event history for operational events including:

- Ping High latency / Offline / Recovery events and reminders
- reusable alert triggers, reminders, and resolutions
- Network Identity changes

History is separate from delivery. An event may be recorded even when Sound and Notification delivery are disabled.

The History tab is lazy-loaded and supports:

- pagination
- search
- target, type, source, status, and date/time filters
- sortable columns
- CSV export of the complete current filtered/sorted result set rather than only the visible page

History currently retains the newest 10,000 events in plugin-scoped storage.

## Network Identity

Network Identity shows:

- Public IP
- local interfaces
- MAC addresses
- IPv4 and IPv6 addresses
- a selectable top-bar address
- copy actions
- manual refresh

Auto refresh is enabled by default every five minutes and may be disabled or set to 1, 5, 10, 30, or 60 minutes. When disabled, the startup snapshot still loads once and manual Refresh remains available.

The UI shows the local Last updated timestamp. Successful refreshes also republish the Network Identity top-bar contribution so changed values are reflected there.

Meaningful changes can optionally play a sound and/or show a native notification. The initial snapshot establishes the baseline and never generates a change alert. Later changes are recorded in History even if both delivery channels are disabled.

## Monitoring settings

Sampling intervals are independently configurable for:

- Ping
- TCP
- HTTP / HTTPS

History settings include:

- retained monitoring history
- top-bar history window

The current defaults are:

- Ping: 2 seconds
- TCP: 5 seconds
- HTTP: 10 seconds
- retained history: 1 hour
- top-bar history: 1 minute

## Architecture

RemoteHand owns generic infrastructure.

Target Monitor owns monitoring semantics.

RemoteHand provides generic facilities such as:

- executable plugin runtime
- network access
- network inspection
- plugin-scoped storage
- clipboard integration
- tray contributions
- native plugin notification delivery
- top-bar contribution primitives
- panel hosting and navigation
- native file-save dialogs
- runtime sidebar contribution visibility

Target Monitor owns:

- Ping behavior
- TCP service monitoring
- HTTP/HTTPS monitoring
- response extraction
- source formatting
- graph thresholds
- alert rules and state
- alert sounds/reminders
- target reports
- Network Identity behavior and change detection
- persistent operational History and CSV presentation
- sidebar visibility preference
- monitoring presentation
- target-card layout, ordering, and persistence

Device-specific monitoring logic should not be added to RemoteHand core.

## Example: AxeOS / Bitaxe

AxeOS is a useful example of the generic HTTP/JSON model.

An endpoint such as `/api/system/info` may expose fields including:

- `hashRate_10m`
- `temp`
- `vrTemp`
- `power`
- `fanrpm`
- `wifiRSSI`
- `stratumURL`
- `miningPaused`
- `overheat_mode`

Target Monitor does not hard-code a Bitaxe dashboard. These values are simply generic JSON fields that may be displayed, graphed, formatted, or assigned alerts.

## Build

Prerequisites:

- Rust stable toolchain
- On Linux, unprivileged ICMP availability depends on host ICMP socket policy

Validate:

    cargo test

Build release runtime:

    cargo build --release

Windows installable package:

    powershell -ExecutionPolicy Bypass -File .\scripts\build-windows.ps1

The clean Windows package directory is created at:

    dist\remotehand-target-monitor-windows-x86_64

The Windows build script also creates the versioned release ZIP and checksum file directly, for example:

    dist\remotehand-target-monitor-v0.7.11-windows-x86_64.zip
    dist\SHA256SUMS.txt

Install the package directory during development, or use the versioned ZIP through RemoteHand's ZIP install flow.

## Plugin identity

Plugin ID:

    remotehand.target-monitor

Runtime binary:

    remotehand-target-monitor

Panel ID:

    target-monitor

Current release version:

    0.7.11

## Lineage

Target Monitor starts from the validated RemoteHand Host Monitor v0.6.1 codebase.

Host Monitor remains frozen in its original repository as the historical predecessor.

Target Monitor continues that product line as **v0.7.11**, reserving `v1.0.0` for a later milestone after the broader alert-delivery architecture has matured.

## Future alert delivery

Target Monitor v0.7.4 currently supports alert sound/reminders and native Windows notifications.

Future versions may add additional alert-delivery sinks such as:

- email
- webhook
- file / alert log output
- other integrations where justified

The intended architecture is to keep alert detection separate from alert delivery so new sinks can be added without changing monitoring semantics.

## Product boundary

Target Monitor should remain compact, generic, and user-controlled.

The goal is not to become a full enterprise monitoring platform.

The v0.7.4 release focuses on:

- proven Ping monitoring
- TCP reachability
- HTTP / HTTPS monitoring
- JSON, text, and regex extraction
- configurable monitoring/history
- one top-bar item per target
- reusable alerts and reminders
- native notifications
- clear acknowledgment and resolution behavior
- target-level reports
- configurable visual presentation

## License

Third-party components retain their own licenses.
