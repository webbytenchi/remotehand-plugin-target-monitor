# RemoteHand Target Monitor

Target Monitor is a RemoteHand plugin for monitoring hosts, services, and HTTP/HTTPS endpoints from one compact target model.

It evolves from the proven RemoteHand Host Monitor v0.6.1 foundation while expanding beyond ICMP latency monitoring.

## Status

Target Monitor 0.6.1 remains the stable repository baseline. Version 1.0.0 is implemented on the `feature/v1.0.0-target-monitor` development branch and is awaiting local/runtime acceptance before merge or tagging.

The v1 implementation preserves the validated ICMP/Ping foundation and adds TCP monitoring, HTTP/HTTPS monitoring, extraction, reusable alert rules, target reports, and configurable one-item-per-target top-bar presentation.

See `V1.0.0-DESIGN.md` for the agreed product design.

## Core model

A Target Monitor target may contain:

- ICMP / Ping monitoring
- Multiple TCP port reachability checks
- Multiple HTTP / HTTPS checks
- Background alert rules
- One compact RemoteHand top-bar contribution

One target always produces at most one Target Monitor top-bar item.

Monitoring, alert evaluation, and top-bar presentation are separate concerns.

## ICMP / Ping

The existing Host Monitor ICMP implementation is the stable foundation for Target Monitor.

It currently provides:

- real ICMP latency sampling
- Normal / High Latency / Offline state classification
- two-sample state confirmation to reduce flapping
- rolling history
- one-minute top-bar sparkline history
- configurable appearance
- state-change alert sounds
- per-target tray contribution support

The v1.0.0 work should preserve this behavior rather than rewrite it.

## TCP monitoring

Targets may contain multiple TCP checks.

Target Monitor will provide common service presets such as:

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

A Custom option will allow an arbitrary service name and port.

TCP monitoring reports whether the configured service can be reached.

## HTTP / HTTPS monitoring

Targets may also contain one or more HTTP/HTTPS requests.

Each request may capture:

- HTTP status
- response time
- content type
- response body

HTTP monitoring is intentionally content-agnostic.

### JSON responses

JSON replies can expose fields for:

- friendly display
- top-bar Value selection
- numeric graphing
- alert evaluation

### Plain-text responses

Plain-text replies can be displayed directly and monitored for value or content changes.

### Regex extraction

Non-JSON responses may use user-configured regular expressions with named capture groups.

Captured values become monitored fields. Numeric captures may participate in numeric threshold alerts and graphing; text captures may participate in text alerts.

The configuration UI should provide a Test / Preview step before saving regex extraction.

## Top bar

Each target has one top-bar contribution.

The user chooses what appears in its three presentation slots:

- Title
- Value
- Graph

Any slot may be disabled.

For example:

    Title: Bitaxe
    Value: hashRate_10m
    Graph: temp

Fields that are not displayed in the top bar remain monitored and may still trigger alerts.

## Alerts

Target Monitor will provide a dedicated Alerts tab.

Existing Ping alert behavior remains unchanged.

HTTP/HTTPS monitored fields may be assigned reusable named alert rules.

Examples:

- HighTemp
- CriticalTemp
- LowHashrate
- PoolChanged

Numeric conditions include:

- >=
- <=
- =

Text/value rules include:

- equals
- changed

A single field may have multiple alerts assigned.

Alerts run independently from top-bar presentation.

## Alert acknowledgment

When any monitored field on a target triggers an unacknowledged alert, that target's single top-bar item shows an alert badge.

Clicking the target top-bar item:

- opens or focuses the target report
- acknowledges unread alerts
- clears the top-bar badge

Acknowledgment does not resolve an active condition.

An active acknowledged alert remains visible in the report until its underlying condition resolves.

## Targets view

The current ping-centric Targets view will evolve to summarize:

- target identity
- overall state
- Ping status
- TCP status
- HTTP/HTTPS status
- target actions

The current Ping-only lower dashboard will be removed from the main Targets screen during the v1.0.0 redesign.

Its underlying Ping history functionality is retained for possible use in a richer target detail/report view.

## Network Identity

The inherited Network Identity functionality remains part of the current foundation:

- public IP display
- local interface discovery
- MAC / IPv4 / IPv6 information
- copy actions
- optional top-bar Network Identity contribution

Its long-term placement may be revisited as Target Monitor evolves.

## Architecture

RemoteHand owns generic infrastructure.

Target Monitor owns monitoring semantics.

RemoteHand core provides generic facilities such as:

- executable plugin runtime
- network access
- network inspection
- plugin-scoped storage
- clipboard integration
- tray contributions
- top-bar contribution primitives
- panel hosting

Target Monitor owns:

- Ping behavior
- TCP service monitoring
- HTTP/HTTPS request monitoring
- response extraction
- alert rules
- target state
- monitoring presentation

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

Target Monitor should not hard-code a Bitaxe-specific dashboard. These values are simply an example of generic JSON fields that the user may display, graph, or attach alerts to.

## Build

Prerequisites:

- Rust stable toolchain
- On Linux, unprivileged ICMP availability depends on host ICMP socket policy

Build:

    cargo build --release

## Plugin identity

Plugin ID:

    remotehand.target-monitor

Runtime binary:

    remotehand-target-monitor

Panel ID:

    target-monitor

Current development version:

    1.0.0

## Lineage

Target Monitor starts from the validated RemoteHand Host Monitor v0.6.1 codebase.

Host Monitor remains frozen in its original repository as the historical pre-1.0 product line.

Target Monitor is the successor product beginning with v1.0.0.

## Product boundary

Target Monitor should remain compact, generic, and user-controlled.

The goal is not to become a full enterprise monitoring platform.

The v1.0.0 focus is:

- proven Ping monitoring
- TCP service reachability
- HTTP/HTTPS monitoring
- JSON, plain-text, and regex extraction
- one top-bar item per target
- reusable background alerts
- clear acknowledgment and resolution behavior

## License

See the project license when added to this repository. Third-party components retain their own licenses.
