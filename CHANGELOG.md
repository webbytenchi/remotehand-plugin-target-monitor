# Changelog

## 0.7.10 - 2026-10-08

### Added
- Generic HTTP/HTTPS authentication for Target Monitor requests.
- Bearer Token, API Key, and Basic Authentication modes.
- Per-request authentication controls with masked credential fields.
- HTTP authentication is applied to both Test / Preview requests and background monitoring.

### Compatibility
- Existing unauthenticated HTTP/HTTPS checks remain unchanged.
- Plugin API remains **v1**.

## 0.7.9 - 2026-10-03

### Added
- **Enable Alerts History** setting for persistent Target Monitor History.
- **Clear History** action with an in-app confirmation dialog.
- Friendly in-app From/To date-time pickers with calendar navigation, hour/minute selection, AM/PM, Now, Clear, Cancel, and Apply actions.
- Calendar icons inside the History From and To fields.

### Improved
- Disabling Alerts History now stops only new History persistence. Monitoring, active-alert state, Ping/reusable alert evaluation, sounds, native notifications, reminders, and recovery behavior remain independent.
- Existing saved History remains available while History is disabled until explicitly cleared.
- History date-range filtering is easier to use while preserving the existing sortable/filterable/paginated History model.

### Fixed
- Clear History now uses the plugin's in-app modal flow instead of the WebView-native confirmation path.
- Disabled History no longer emits false History-changed notifications for events that were not stored.
- Ping High Latency and Offline alerts continue to deliver correctly with History disabled.
- Recovery now clears stale Ping alert cooldown state so a fresh High Latency or Offline transition can alert again immediately after recovery.

### Compatibility
- Plugin API remains **v1**.
- Existing targets, Ping/TCP/HTTP monitoring, reusable alerts, Network Identity, reports, top-bar/tray configuration, and retained History remain compatible.
- History disabling does not delete existing saved History.

### Verification
- Exact-main Rust validation passed with **68 tests and 0 failures**.
- Focused regression coverage verifies disabled History does not suppress reusable-alert or Ping High Latency/Offline delivery and does not persist new History entries.
- Optimized Windows package builds passed during feature validation.
- Manual Windows acceptance covered History disable behavior, Ping High Latency/Offline/Recovery delivery, Clear History, and the From/To picker/calendar-icon UX.


## 0.7.8 - 2026-09-29

### Added

- **Live top-bar severity indicator.** The left-side alert icon reflects only the configured Top Bar Value and Graph sources: Warning uses the target's configured Warning color, Critical uses the configured Critical color, Critical takes precedence, and normal state hides the icon.
- Ping sources map High Latency to Warning and Offline to Critical. Reusable alerts can raise Warning only when they belong to the selected top-bar source.

### Improved

- **Offline target creation.** Targets can now be saved even when the host is currently unreachable or DNS resolution fails. Malformed host/IP input is still rejected; availability and name-resolution failures are treated as live monitoring state.
- **Independent alert indicators.** Unrelated alerts no longer drive the left live-severity icon; the existing numbered unread-alert badge on the right remains independent.
- **Long top-bar titles.** Full title text is now passed to RemoteHand so the host can apply proper ellipsis instead of Target Monitor pre-truncating the value.

### Compatibility

- Plugin API remains **v1**.
- Existing targets, alerts, top-bar settings, and history remain compatible.

### Verification

- Windows Rust validation passed with **64 tests and 0 failures**.
- Optimized Windows package build passed.
- Manual Windows acceptance confirmed unresolvable targets can be saved and monitored, selected Value/Graph Warning and Critical states drive the left icon correctly, unrelated alerts remain right-badge-only, corner indicators align, and long titles ellipsize correctly.

All notable changes to RemoteHand Target Monitor are documented in this file.

The format follows Keep a Changelog and Semantic Versioning.

## 0.7.7 - 2026-09-29

### Changed

- Custom alert-sound selection now uses RemoteHand's native `files.open` host API.
- Selected custom sounds are read by the plugin backend and delivered to the UI as encoded audio.
- Removed browser-side file-reading fallbacks from the alert-sound workflow.

### Compatibility

- Existing targets, reusable alerts, History, reports, Network Identity, top-bar, and tray configuration remain compatible.
- Plugin API remains **v1**.

### Verification

- Windows Rust validation passed with **58 tests and 0 failures**.
- Optimized Windows release build passed.
- Manual Windows acceptance confirmed native sound picking, selected-sound playback, and sound persistence after restart.

## 0.7.6 - 2026-09-27

### Added

- Target Report HTTP/HTTPS field tables now show configured reusable alert rules alongside exposed field values.
- Added **Triggered** and **Last triggered** columns for configured field alerts.
- Trigger counts and timestamps are derived from retained persistent History.
- Multiple alert rules assigned to the same field are displayed as aligned per-rule rows across the alert-stat columns.

### Improved

- Currently active field alerts retain the highlighted alert-chip appearance while configured inactive rules remain visible with the normal configured-rule style.
- Reminder events do not increase the Triggered count; only actual alert activation events with status `Triggered` are counted.
- Alert statistics refresh while a Target Report is open when new History events arrive.
- The report explicitly identifies these statistics as retained-History values rather than lifetime counters.

### Compatibility

- No saved configuration migration is required.
- Existing History data is reused directly; no separate alert counter store is introduced.
- RemoteHand core and Plugin API v1 are unchanged.

### Verification

- Windows Rust validation passed with **58 tests and 0 failures**.
- Added regression coverage confirming alert trigger statistics count real triggers, exclude reminders, remain target-scoped, and report the latest trigger timestamp.
- Manual acceptance on the BitAxe report confirmed the Alerts, Triggered, and Last triggered columns display and align as intended.

## 0.7.5 - 2026-09-27

### Added

- Added a reusable alert operator shown as **didn't change** for monitored values that are expected to keep changing.
- The operator uses a configurable **For** duration in seconds and becomes active only after the source value remains unchanged continuously for that duration.
- Any value change immediately resets the unchanged-duration timer.

### Fixed

- The reusable alert editor now fully hides the Comparison value field for the existing `changed` operator.
- Selecting **didn't change** replaces Comparison value with a dedicated **For ... seconds** control rather than overloading the comparison value.
- Hidden alert-rule fields now reliably stay hidden even with the shared field layout styles.

### Improved

- Reusable rule validation treats `changed` and `unchanged` as comparison-value-free operators while preserving existing numeric and equality operators.
- The Windows packaging script now derives the release ZIP version from `plugin.json` and creates the final versioned filename directly instead of a `-test-` validation filename.

### Compatibility

- Existing saved alert rules remain compatible.
- The new unchanged-duration setting defaults to 30 seconds when absent.
- RemoteHand core and Plugin API v1 are unchanged.

### Verification

- Windows Rust validation passed with **57 tests and 0 failures**.
- New regression coverage verifies unchanged-duration activation, timer reset after a value change, and configuration without a comparison value.
- Manual acceptance confirmed `changed` shows no extra value field, **didn't change** shows a **For ... seconds** field, and the unchanged-duration behavior triggers and resets as intended.

## 0.7.4 - 2026-09-25

### Added

- Target cards can now be reordered by dragging the card-header handle.
- Target Monitor persists card order in plugin-owned configuration and restores it on later loads.
- Added a Reset order action to return cards to the underlying target-list order.
- Added backward-compatible card-order normalization so missing, duplicate, stale, and newly added target IDs are handled safely.

### Improved

- Removed the old centered 1180 px content-width cap so Target Monitor can use the full panel width provided by RemoteHand.
- Reduced target-card minimum width, card padding, summary label width, and grid gaps so more cards can fit per row without changing target semantics.
- Live sample events avoid rebuilding the target grid while a drag is in progress.

### Compatibility

- Existing saved configurations remain compatible. Card order defaults to the existing target order when the new field is absent.
- RemoteHand core is unchanged; target-card layout and ordering remain entirely owned by Target Monitor.
- Monitoring, alerts, reports, top-bar, tray, History, and Network Identity semantics are unchanged.

### Verification

- Windows Rust validation passed with **54 tests and 0 failures** after the card-order persistence update.
- Optimized Windows validation package built successfully.
- Manual layout acceptance confirmed the wider, denser target-card grid makes better use of the available panel width.
- Manual acceptance confirmed drag/reorder persistence survives a full RemoteHand restart.

## 0.7.3 - 2026-09-25

### Fixed

- System-tray updates now cap tooltips to RemoteHand's 256-byte host limit, preserving UTF-8 boundaries instead of allowing an oversized tooltip to make later tray updates get rejected.
- System-tray graph payloads now keep only the newest 60 points, matching the RemoteHand tray API limit even when the configured graph-history window contains more samples.
- Added regression coverage for UTF-8-safe tooltip truncation and newest-point retention.

### Compatibility

- No saved configuration migration is required.
- Monitoring, alerts, reports, top-bar content, and exposed HTTP field semantics are unchanged.

### Verification

- Windows Rust validation passed with **53 tests and 0 failures**.
- Optimized Windows plugin package built successfully.
- Validation ZIP installed successfully through RemoteHand's native **Install ZIP** flow.
- Manual acceptance confirmed the system-tray graph continues updating past the previous approximately four-bar freeze point.

## 0.7.2 - 2026-09-24

### Added

- **Exposed HTTP fields.** HTTP Test / Preview now lets each discovered field be marked Expose independently of alert assignment.
- **Concise Target Report HTTP details.** Target Reports now show only fields explicitly exposed for each HTTP request, in configured order.
- **Top-bar hover details for exposed fields.** Exposed HTTP values are included in the target's top-bar tooltip, with unavailable values shown as an em dash.

### Improved

- Exposed-field selections persist with the HTTP request and remain selected across Test / Preview filtering and pagination.
- Exposed fields remain configured even if a later response temporarily omits the field, so transient schema/value changes do not silently discard presentation choices.
- HTTP report presentation is intentionally concise instead of repeating the full searchable/paginated discovered-field table.

### Fixed

- Reusable alert rules using the `changed` operator now hide the entire Comparison value control because change detection does not use a comparison value.
- Switching from `changed` back to another operator restores the Comparison value control without changing rule semantics.

### Compatibility

- Existing saved HTTP checks remain compatible; missing `exposed_fields` data defaults to an empty list.
- Exposure affects presentation only. Monitoring, extraction, graphing, and alert evaluation continue to use the underlying field data independently.

### Verification

- Windows Rust validation passed with **51 tests and 0 failures**.
- Optimized Windows plugin package built successfully.
- Manual packaged-plugin acceptance confirmed exposed fields appear in Target Reports and top-bar hover details.
- Manual acceptance confirmed the RemoteHand v1.2.10 host-rendered tooltip remains stable while the live top-bar graph updates.

## 0.7.1 - 2026-09-24

### Added

- Independent Sound and Notification delivery controls for Ping High latency, Offline, and Recovery events.
- Independent Sound and Notification delivery controls for reusable alert rules, with built-in beep fallback when Sound is enabled without a custom file.
- Reusable Alert Rules list now shows Sound/Notification delivery icons and expandable assignment details for “Used by N sources”.
- Reusable alert rules now have a master enabled/disabled switch that preserves assignments and configuration while suppressing evaluation.
- Reusable alert names are now case-insensitively unique; legacy duplicate names are migrated safely on load.
- HTTP field alert pickers and configured alert chips now keep disabled assigned rules visible, clearly marked Disabled, and show compact delivery indicators.
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
- Packaged-plugin smoke test passed against released RemoteHand v1.2.9.
- Manual acceptance confirmed the Target Monitor sidebar icon, live hide/show toggle, and preserved plugin operation while hidden.
- Final RemoteHand v1.2.9 integration smoke passed for plugin load/enable, native CSV Save As cancel/save, live sidebar visibility, Network Identity top-bar behavior, reusable-alert controls, and floating notices.

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

