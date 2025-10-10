# AAEQ (Adaptive Audio Equalization) — Rust App (Local-First with Optional Cloud Sync)

## 0) One-liner

A cross-platform Rust application that automatically applies **per-song → album → genre → default** EQ on your local network playback devices (starting with WiiM / LinkPlay), with an optional cloud sync for sharing presets and mappings across computers.

---

## 1) Goals & Non-Goals

**Goals**

* Zero-cloud dependency for core operation (offline-first).
* Deterministic, smooth EQ switching at track start/change.
* Simple, reliable WiiM integration via local HTTP API.
* Clean UX to choose a preset and “Save for Song/Album/Genre/Default”.
* Extensible to other ecosystems (Sonos/Bluesound/HEOS/etc.) via plugins.
* Optional account-based sync (presets + mappings) across desktops.

**Non-Goals (v1)**

* No on-device room correction, mic calibration, or FIR convolution.
* No direct modification of streaming services; we read metadata only.
* No always-on remote cloud control of LAN devices (can come later).

---

## 2) Personas

* **Audiophile Power User:** Wants consistent tonal balance per album/master.
* **Streamer/DJ:** Curates playlists where EQ becomes part of the flow.
* **Prosumer Studio:** Shares per-album curves with collaborators.

---

## 3) High-Level Architecture

```
+------------------------------+         +---------------------------+
|          UI Layer            |         |  (Optional) Cloud Sync    |
|  (Iced / egui OR Tauri UI)   |  HTTPS  |  REST: login, presets,    |
+--------------+---------------+<------->|  mappings, device profiles|
               |                          +------------+--------------+
               v                                       ^
+--------------+---------------+                       |
|        Core Orchestrator     |                       |
|  (Rust, async, tokio)        |                       |
|  - State & rules engine      |                       |
|  - Mapping resolver          |                       |
|  - Debounce & conflict logic |                       |
+--------------+---------------+                       |
               |                                       |
               v                                       |
+--------------+---------------+         sync          |
|   Device Abstraction Layer   +-----------------------+
|  (Trait-based plugins)       |
|   - WiiM (LinkPlay HTTP)     |
|   - Future: Sonos/HEOS/...   |
+--------------+---------------+
               |
               v
+--------------+---------------+
|   Local Data & Config        |
|  - SQLite (rusqlite/SQLx)    |
|  - Config (TOML/YAML)        |
|  - Cache of device presets   |
+------------------------------+
```

---

## 4) Platform & Tech Choices

* **Language:** Rust stable
* **Runtime:** tokio (async), reqwest (HTTP), serde (serialization)
* **DB:** SQLite (SQLx or rusqlite), with embedded migrations
* **UI (choose one):**

  * **Iced** or **egui** for a pure-Rust native UI (smaller footprint)
  * **Tauri** for HTML/CSS/JS front-end with Rust backend (best theming, easiest web skills reuse)
* **Packaging:** Windows (MSI/EXE), macOS (.dmg + notarization), Linux (.deb/.rpm/AppImage)
* **Background:** Tray app + optional background daemon/service (systemd/launchd/Windows Service)
* **Cloud (optional add-on):** Axum + Postgres for REST sync (later milestone)

---

## 5) Device Integrations (v1: WiiM / LinkPlay)

**Core operations**

* `getPlayerStatus` → extract `artist`, `title`, `album`, `genre`
* `EQGetList` → list available EQ presets
* `EQLoad:<PresetName>` → apply preset
* (Optional) `EQOn`/`EQOff`, and band endpoints if firmware supports

**Discovery**

* mDNS/SSDP broadcast scan with fallback to last-known IP.
* Device fingerprint: name, IP, serial/MAC (if available).

**Debounce & timing**

* Detect “new track” (artist/title/album changes).
* Resolve mapping, compare with last applied EQ; only switch if different.
* Apply within ~50–300 ms of track change; no mid-track flapping.

---

## 6) Mapping Logic (Deterministic Hierarchy)

Order of precedence on track start:

1. **Song** (`artist - title`, normalized)
2. **Album** (`artist - album`, normalized)
3. **Genre** (verbatim or normalized per setting)
4. **Default** (e.g., `Flat`)

Normalization options: lowercase, trim, stripping “(Remastered YYYY)” suffix heuristics.

Collision policy:

* Song beats album; album beats genre; genre beats default.
* User can see and edit all mappings from a single “Rules” view.

---

## 7) Data Model

### 7.1 Config (TOML)

```toml
[app]
log_level = "info"         # debug|info|warn|error
poll_interval_ms = 1000
auto_start = true
default_preset = "Flat"

[device.wiim]
# Expectations for LinkPlay; override per device if needed
debounce_ms = 300
```

### 7.2 SQLite schema (initial)

```sql
-- devices
CREATE TABLE device (
  id INTEGER PRIMARY KEY,
  kind TEXT NOT NULL,              -- "wiim" | future kinds
  label TEXT NOT NULL,
  host TEXT NOT NULL,              -- IP or hostname
  discovered_at INTEGER NOT NULL
);

-- presets known on device (cache; resynced periodically)
CREATE TABLE device_preset (
  id INTEGER PRIMARY KEY,
  device_id INTEGER NOT NULL REFERENCES device(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  UNIQUE(device_id, name)
);

-- mapping rules (scope: song | album | genre | default)
CREATE TABLE mapping (
  id INTEGER PRIMARY KEY,
  scope TEXT NOT NULL,             -- "song" | "album" | "genre" | "default"
  key_normalized TEXT,             -- null for "default"
  preset_name TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  UNIQUE(scope, key_normalized)
);

-- last applied state (for debounce/UX)
CREATE TABLE last_applied (
  id INTEGER PRIMARY KEY,
  device_id INTEGER NOT NULL REFERENCES device(id) ON DELETE CASCADE,
  last_track_key TEXT,             -- artist|title|album|genre
  last_preset TEXT,
  updated_at INTEGER NOT NULL
);
```

### 7.3 In-memory structs (Rust)

```rust
#[derive(Clone, Debug)]
pub struct TrackMeta {
    pub artist: String,
    pub title:  String,
    pub album:  String,
    pub genre:  String,
}

#[derive(Clone, Debug)]
pub enum Scope { Song, Album, Genre, Default }

#[derive(Clone, Debug)]
pub struct Mapping {
    pub scope: Scope,
    pub key_normalized: Option<String>,
    pub preset_name: String,
}
```

---

## 8) Core Orchestrator (Rust)

### Trait: `DeviceController`

```rust
#[async_trait::async_trait]
pub trait DeviceController: Send + Sync {
    fn id(&self) -> String;                   // stable id (label or serial)
    async fn discover() -> Vec<Self> where Self: Sized;
    async fn get_now_playing(&self) -> anyhow::Result<TrackMeta>;
    async fn list_presets(&self) -> anyhow::Result<Vec<String>>;
    async fn apply_preset(&self, preset: &str) -> anyhow::Result<()>;
}
```

### WiiM Implementation

* Uses `reqwest` to call `http://{host}/httpapi.asp?command=...`
* JSON or text parsing fallback
* Graceful error handling & retry with backoff

### Mapping Resolver

```rust
pub fn resolve_preset(meta: &TrackMeta, rules: &RulesIndex, default: &str) -> String {
    // song -> album -> genre -> default
}
```

### Debounce Loop

* Poll `get_now_playing` at interval
* If `track_key` changed → resolve → compare with last-applied → `apply_preset`

---

## 9) UI Spec

### Views

* **Now Playing**: Artist — Title (Album) [Device selector]

  * Preset dropdown (live apply)
  * Buttons: Save for **Song/Album/Genre/Default**
* **Rules**: table of mappings with search, edit, delete
* **Devices**: discovered devices, test command, preset refresh
* **Settings**: default preset, normalization, interval, start-on-login
* **(Optional) Cloud**: sign-in/out, last sync, conflict resolution

### UX details

* Live audition: changing dropdown fires `apply_preset` immediately (logs in history).
* Mapping creation: shows the computed keys it will save (e.g., `pink floyd - time`).
* Conflict banner if two rules would match (shows which one wins).

---

## 10) Optional Cloud Sync (Phase 2)

* **What syncs:** presets names, mapping rules, normalization options.
* **What doesn’t:** device IPs or local discovery state.
* **API (minimal):**

  * `GET /v1/mappings` → list
  * `POST /v1/mappings` → upsert
  * `GET /v1/presets`
  * `POST /v1/presets`
* **Auth:** OAuth (Google/Apple) or email-magic link. All data encrypted in transit.
* **Offline behavior:** local queue, 2-way merge at next online session.

---

## 11) Packaging & Services

### Autostart

* **Windows:** Task Scheduler or service (sc.exe) for background agent.
* **macOS:** LaunchAgent/LaunchDaemon plist.
* **Linux:** systemd user service:

```ini
[Unit]
Description=Adaptive EQ Mapper

[Service]
ExecStart=/usr/local/bin/adapt-eq --background
Restart=on-failure

[Install]
WantedBy=default.target
```

### Installers

* Use cargo-bundle + platform-specific packagers, or Tauri bundler if using Tauri.

---

## 12) Security & Privacy

* No inbound ports; LAN calls originate from the app to the device.
* Minimal data collected by default; telemetry strictly opt-in (count of applied presets, anonymized).
* Local secrets (cloud token) stored in OS keychain (Keychain/DPAPI/libsecret).
* Crash logs scrub PII; user can export/delete data easily.

---

## 13) Testing Plan

* **Unit:** mapping resolver, normalization, text/JSON parsers.
* **Integration:** WiiM mock server (wiremock) + real device smoke tests.
* **Soak tests:** long-running playlist with frequent track changes.
* **UI tests:** basic regression of flows (save mapping, apply preset, edit/delete).

---

## 14) Roadmap

**v0.1 (MVP)**

* WiiM support, Now Playing, preset list/apply, rules CRUD, tray mode

**v0.2**

* Album/genre normalization helpers, import/export JSON/YAML, log viewer

**v0.3**

* Optional cloud sync (login, push/pull), multi-desktop sharing

**v1.0**

* Additional device plugin (e.g., Sonos or HEOS), fuzzy matching, backups

**v1.1+**

* Community preset sharing, ML suggestions, hotkeys, mini-overlay UI

---

## 15) Example Crate Layout

```
adapt-eq/
├─ crates/
│  ├─ core/                 # mapping engine, state, models
│  ├─ device-wiim/          # WiiM plugin (LinkPlay)
│  ├─ ui-iced/              # (or ui-tauri/) UI front-end
│  ├─ persistence/          # sqlite, migrations
│  └─ sync-api/             # optional cloud client (feature-gated)
├─ apps/
│  ├─ desktop/              # binary combining core + ui + device plugins
│  └─ service/              # headless background service (optional)
└─ Cargo.toml
```

**Feature flags**

* `--features tauri` vs `--features iced`
* `--features cloud-sync`
* `--features device-wiim,device-sonos`

---

## 16) Sample Rust Snippets

**WiiM call**

```rust
pub async fn wiim_cmd(host: &str, command: &str) -> anyhow::Result<String> {
    let url = format!("http://{host}/httpapi.asp?command={command}");
    let text = reqwest::get(&url).await?.text().await?;
    Ok(text)
}
```

**Apply with debounce**

```rust
if track_changed(&prev_meta, &meta) {
    let desired = resolve_preset(&meta, &rules, default);
    if last_applied.as_deref() != Some(&desired) {
        device.apply_preset(&desired).await?;
        last_applied = Some(desired);
    }
}
```

---

## 17) License & Community

* **License:** AGPL-3.0 for app (if you want to keep server forks honest) or Apache-2.0/MIT for wider adoption.
* **Contrib:** Device plugins via traits; publish a simple “Device Plugin API” guide.
* **Branding:** Keep “Adaptive EQ Mapper” as working title; we can pick a marketable name later.

---

### Done? Next Steps

* Pick **UI path** (Iced/egui native vs Tauri web UI).
* I can scaffold the **Cargo workspace** with the crates above, a minimal WiiM plugin, SQLite migrations, and a tiny Iced window showing “Now Playing” + preset dropdown + “Save mapping” buttons.
