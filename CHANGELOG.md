# Changelog

All notable changes to RemoteHand Target Monitor are documented in this file.

The format follows Keep a Changelog and Semantic Versioning.

## 0.7.0 - 2026-09-24

### Added

- Multi-check target model with ICMP/Ping, TCP services, and HTTP/HTTPS requests.
- TCP presets for HTTP, HTTPS, SSH, RDP, SMB, DNS, SMTP, IMAP, POP3, MySQL, and PostgreSQL, plus custom ports.
- HTTP status, response-time, content-type, whole-response monitoring, JSON field discovery, plain-text monitoring, and named-capture regex extraction.
- Reusable named alert rules with numeric thresholds, equality, and change detection.
- Active/unacknowledged, active/acknowledged, and resolved alert lifecycle.
- Optional repeated reminders while reusable rules remain active.
- One configurable top-bar contribution per target with independent Title, Value, and Graph sources.
- Generic numeric graph Warning/Critical thresholds with user-configurable Normal, Warning, Critical, and Unavailable colors.
- Per-source display Prefix/Suffix formatting.
- Separate user-configurable Top-bar Value Normal and Alert colors.
- Per-target native Windows alert notifications through RemoteHand's generic `notifications.show` capability.
- Independent per-target controls for tray visibility and alert notifications.
- Queued native notifications so multiple alerts for one target are presented sequentially.
- Notification click-through that restores RemoteHand, opens Target Monitor, selects the originating target, and opens its Report.
- Target report view covering Ping history, TCP results, HTTP fields, and active alerts.
- Search, filtering, and pagination for HTTP field tables.
- Configurable Ping, TCP, and HTTP sampling intervals.
- Configurable retained-history and top-bar-history windows.
- Redesigned Targets, Alerts, Settings, Appearance, and report workflows.

### Changed

- Target Monitor release numbering continues from Host Monitor v0.6.1 and releases this feature set as **v0.7.0** rather than v1.0.0.
- Ping graph presentation retains real Normal / High Latency / Offline state semantics while non-Ping numeric graphs use generic thresholds.
- Top-bar Value alert coloring is source-specific: unrelated active alerts no longer change the displayed Value color.
- Appearance layout groups Reset colors with the actual color controls.
- Tooltips now use friendly source names, formatted values, and multiline presentation.
- System-tray graph bars adapt to sparse histories through RemoteHand's generic tray renderer.

### Preserved

- Real ICMP/Ping sampling and two-sample state confirmation.
- Existing Ping transition alerts, sounds, cooldowns, reminders, and per-target tray contributions.
- Plugin-scoped configuration persistence.
- Existing saved normal value color through the legacy `latencyColor` field for backward compatibility.
- RemoteHand-owned interface contribution ordering.

### Fixed

- Stale active alerts are pruned when a rule is deleted or no longer assigned to a source.
- Target/report state and source labels remain synchronized after configuration changes.
- Notification-only targets retain a hidden tray identity without forcing the live tray graph to be visible.
- Report and modal navigation preserve correct target context.
- Reset colors placement now clearly scopes the action to Appearance colors.

### Verification

- Manual Windows acceptance confirmed native alert notifications.
- Manual Windows acceptance confirmed notification click routing and queue plumbing after the RemoteHand core update.
- Manual UI acceptance confirmed adaptive tray graphs, multiline tooltips, source formatting, graph thresholds, alert-aware Value coloring, and final Appearance layout.
- Embedded Target Monitor UI JavaScript validation passes.
- Release validation still requires the final local `cargo test` and Windows release-package build after the v0.7.0 metadata change.

### Future

Potential future alert-delivery sinks include:

- email
- webhook
- file / alert log output
- other integrations where useful

These are intentionally not part of v0.7.0.

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

### Lineage

RemoteHand Host Monitor v0.6.1 is the historical predecessor.

RemoteHand Target Monitor continues from that validated foundation in a new repository and product line.
