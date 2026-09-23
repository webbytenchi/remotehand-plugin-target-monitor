# Changelog

All notable changes to RemoteHand Target Monitor will be documented in this file.

The format follows Keep a Changelog and Semantic Versioning.

## 1.0.0 - Development candidate

### Added

- Multi-check target model with ICMP/Ping, TCP services, and HTTP/HTTPS requests.
- TCP presets for HTTP, HTTPS, SSH, RDP, SMB, DNS, SMTP, IMAP, POP3, MySQL, and PostgreSQL, plus custom ports.
- HTTP status, response-time, content-type, raw-body, JSON field discovery, plain-text monitoring, and named-capture regex extraction.
- Reusable named alert rules with numeric thresholds, equality, and change detection.
- Active/unacknowledged, active/acknowledged, and resolved alert lifecycle.
- One configurable top-bar contribution per target with independent Title, Value, and Graph sources.
- Target report view covering Ping history, TCP results, HTTP fields, and active alerts.
- Redesigned Targets, Alerts, and Settings navigation.

### Preserved

- Real ICMP/Ping sampling and two-sample state confirmation.
- Existing Ping transition alerts, sounds, cooldowns, reminders, and per-target tray contributions.
- Plugin-scoped configuration persistence and existing target color settings.

### Development status

- Implemented on `feature/v1.0.0-target-monitor`.
- Not merged, tagged, or released yet; local RemoteHand acceptance is required first.

## 0.6.1 - Initial Target Monitor baseline

Target Monitor begins as a clean successor repository to RemoteHand Host Monitor.

The codebase starts from the validated Host Monitor v0.6.1 implementation, with the product identity changed to Target Monitor while preserving the proven ICMP/Ping monitoring behavior.

### Inherited foundation

- Multi-target ICMP/Ping monitoring.
- Normal / High Latency / Offline state classification.
- Two-sample confirmation to reduce flapping.
- Rolling Ping history.
- One-minute top-bar sparkline history.
- Per-target appearance controls.
- Existing Ping alert sounds and state transitions.
- Per-target system-tray contributions.
- Network Identity support.
- Plugin-scoped persistence.
- RemoteHand-owned interface contribution ordering.

### Product identity

- Plugin renamed to RemoteHand Target Monitor.
- Plugin ID changed to `remotehand.target-monitor`.
- Runtime binary changed to `remotehand-target-monitor`.
- Panel ID changed to `target-monitor`.
- Source binary entry changed to `src/target_monitor.rs`.

### Roadmap

The next development work targets the design documented in:

    V1.0.0-DESIGN.md

That roadmap expands Target Monitor with TCP monitoring, HTTP/HTTPS monitoring, response extraction, reusable alerts, richer target summaries, and one configurable top-bar contribution per target.

### Lineage

RemoteHand Host Monitor v0.6.1 is the historical predecessor.

RemoteHand Target Monitor continues from that validated foundation in a new repository and new product line.
