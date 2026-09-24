# Changelog

All notable changes to RemoteHand Target Monitor are documented in this file.

The format follows Keep a Changelog and Semantic Versioning.

## 0.7.1 - 2026-09-24

### Added

- Independent Sound and Notification delivery controls for Ping High latency, Offline, and Recovery events.
- Independent Sound and Notification delivery controls for reusable alert rules, with built-in beep fallback when Sound is enabled without a custom file.
- Reusable Alert Rules list now shows Sound/Notification delivery icons and expandable assignment details for “Used by N sources”.
- Reusable alert rules now have a master enabled/disabled switch that preserves assignments and configuration while suppressing evaluation.
- Reusable alert names are now case-insensitively unique; legacy duplicate names are migrated safely on load.
- HTTP field alert pickers and configured alert chips now keep disabled assigned rules visible, clearly marked Disabled, and show compact delivery indicators.
- Independent Sound and Notification delivery controls for reusable alert rules, with built-in beep fallback when Sound is enabled without a custom file.
- Notification-aware Test actions for Ping alerts, with randomized selection among eligible notification-enabled targets.
- Restored Network Identity with Public IP, local interface, MAC, IPv4/IPv6, copy, top-bar source, and manual refresh controls.
- Configurable Network Identity auto refresh with 1, 5, 10, 30, and 60 minute intervals; default 5 minutes.
- Last-updated timestamp for Network Identity refreshes.
- Optional Network Identity change sound and native notification delivery; the first snapshot is baseline-only.
- Persistent unified History for Ping events, reusable alerts, resolutions/reminders, and Network Identity changes.
- Lazy-loaded, paginated, sortable, searchable, filterable History table.
- CSV export for the full current filtered/sorted History result set, not only the visible page.
- Recent per-target History inside Target Reports with a View all history action.
- First-class RemoteHand sidebar contribution with a monitor/pulse icon.
- A Settings switch to show or hide Target Monitor in the RemoteHand sidebar without disabling the plugin.

### Changed

- Network Identity meaningful changes are recorded in History independently of whether Sound or Notification delivery is enabled.
- Reusable alert triggers and reminders are recorded in History independently of Sound and Notification delivery settings.
- Reusable alert delivery controls now follow the same presentation and behavior as Network Identity change delivery.
- Reusable alert triggers and reminders are recorded in History independently of Sound and Notification delivery settings.
- Reusable alert delivery controls now follow the same presentation and behavior as Network Identity change delivery.
- Successful Network Identity refreshes also refresh its top-bar contribution when values change.
- Network Identity is visually separated from the core Targets / Alerts / Settings navigation group.
- Sidebar visibility updates are applied live and preserve RemoteHand's generic contribution ordering.

### Fixed

- Corrected the Ping Alert Sounds master checkbox layout so it no longer inherits the global full-width input style inside RemoteHand.
- Kept the checkbox left-aligned with its “Enable alert sounds” label across normal and narrow panel layouts.
- Added spacing between the Network Identity last-updated text and Refresh button.

### Verification

- Windows Rust validation passed with **50 tests and 0 failures** after the unique-name and enablement UI update.
- Native History CSV Save As was manually validated, including cancel and successful save.
- Packaged-plugin smoke test passed against the updated RemoteHand core branch.
- Manual acceptance confirmed the Target Monitor sidebar icon, live hide/show toggle, and preserved plugin operation while hidden.

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

- Final Windows Rust validation passed with 29 tests and 0 failures.
- Final Windows release package built successfully.
- Final packaged-plugin smoke test passed.
- Manual Windows acceptance confirmed native alert notifications.
- Manual Windows acceptance confirmed notification click routing and queue plumbing after the RemoteHand core update.
- Manual UI acceptance confirmed adaptive tray graphs, multiline tooltips, source formatting, graph thresholds, alert-aware Value coloring, and final Appearance layout.
- Embedded Target Monitor UI JavaScript validation passes.

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

- Packaged-plugin UI smoke passed for the latest reusable alert assignment polish, including unique names, master enable/disable, disabled assignment visibility, compact picker indicators, and stable expanded assignment details.

- Floating toast notices were smoke-tested successfully across normal and modal contexts, including auto-dismiss and manual close behavior.
