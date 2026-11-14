# Realm Companion

A read-only desktop avatar for realm-agent. It reads a complete HTTP status URL, discovers SSE automatically, and renders your character from your own WoW 1.12.1 install. It does not log in to the game or control the character.

```sh
cargo run --release -p realm-companion -- --data /path/to/WoW/Data \
  --url 'http://192.168.3.16:18112/status?view=companion'
```

On macOS use the menu-bar icon: the class icon → **设置…** or **选择角色…**. Right-click/F2 also opens settings. Left-drag moves the avatar. Settings is an independent window centered on the active desktop; there is no settings button drawn over the avatar. Multiple instances are supported, and each exits through its own menu or selection-page Exit action.

## Connections and startup

Adding a connection requires only a URL. Character names are read from status in the background and saved automatically; until known, the host/port is shown. Settings allows adding, editing URLs, removing, selecting, and saving connections. With one connection, startup connects directly. With multiple connections, startup opens character selection, remembers the last highlighted character, and waits for a double-click or **进入角色**. Existing windows keep their own connection when another window changes the shared last selection. Deleting an entry does not terminate a different window already using it.

Enter a status JSON URL, e.g. `/status?view=companion`. Pasting the known `/companion/events` URL is corrected to its sibling status endpoint while preserving reverse-proxy prefixes and other query parameters. The SSE address is discovered through `companion_events_url`.

Before receiving status, a previously seen character uses its cached appearance: local time 22:00–06:00 sleeps; otherwise it idles. Real status then takes over. Subsequent confirmed offline/disconnected state plays sleep, then wakes when reconnected. Only character name and appearance are cached, never position, movement, casting or other live actions. Caches are separated by both connection ID and source URL, so editing a URL or opening multiple windows cannot mix character appearances.

On macOS settings live at `~/Library/Application Support/AzerothFoundry/companion/settings.json`; `REALM_COMPANION_CONFIG` selects another configuration. Saving uses short file transactions, a three-way merge and atomic renames, not a single-instance restriction. Data path changes require saving and restarting; equipment visibility and scale preview while editing; save persists, closing without saving restores the saved presentation. Missing resources open settings.

## Presentation

Frames, buttons, character selection highlight, checkboxes and Chinese/Latin fonts are read from the install's original MPQ artwork. The application icon uses the game's WoW logo. The menu bar has no text title: while connected it shows the stock class icon from the character-creation atlas, and while disconnected/choosing a role it shows the stock WoW logo. Hovering identifies the instance's character. No game art is committed to this source repository. The avatar window starts hidden with transparent clear color and is revealed only when its requested appearance is assembled.

100% uses the stock `PaperDollFrame.xml` CharacterModelFrame size, 233×224 UI units, scaled by monitor logical height/768. The preview uses the existing character-pane camera and display-aspect compensation, with a fixed front-facing model. The game's configurable UI scale is not transmitted by the headless agent; this baseline uses the default UI scale. User zoom is 25–400%. All desktops (default) or startup desktop can be selected.

The renderer reuses Benilla's MPQ/DBC/BLP/M2, character skin, equipment and skeleton pipelines. Appearance changes rebuild this single preview through existing asset caches; unchanged appearance does not rebuild. Animations update independently. Sleeping uses a broadside view fitted to the current posed skeleton, then smoothly returns to the front view after waking.

Double-click starts local race-specific dancing. A further single click or a new SSE character-state event cancels dancing and resumes the underlying state; heartbeat comments and identical polling snapshots do not. Sleeping/offline/dead avatars cannot start dancing. At least six logical pixels of pointer movement starts a drag, which does not count as a click. No game command is sent.

Authored equipment M2 particles, billboards, ribbons and registered material-animation lanes use a shared preview path. Spell precast/cast/state visual kits are resolved from the original tables; self spell and explicit visual-kit edges arrive in `visual_events`, keyed by `visual_session` and `visual_sequence`. Reconnect and polling use a bounded recent-event replay with cursor/freshness checks. Aura state kits also initialize from the current snapshot. There are no per-item or per-drink ID branches.

On macOS the Metal/CoreAnimation surface consumes premultiplied RGB. The bundled wgpu27 backend advertises this as PostMultiplied, but only configures a non-opaque CAMetalLayer; it does not convert the colors. The companion therefore preserves native premultiplied RGB on macOS and never divides it by alpha. An actual owned-window Metal probe showed that completely zero-alpha emission is discarded by this compositor; only nonzero-RGB pixels with alpha below 1/255 receive that minimum alpha. Other color and alpha values are preserved. This keeps emission visible without brightening it or treating it as straight-alpha output. Other desktop backends have not been runtime-verified. See the upstream [alpha-mode fix](https://github.com/gfx-rs/wgpu/pull/9922).

Animation synchronization is being extended through protocol 1.1; exact producer/rendering coverage is documented by that contract and verified release notes. Transformed creature models and complete target-dependent visuals remain incomplete. Weapon stowing still follows the character-selection preview rather than all world animation flags; enchantment-derived glows and all special CharProc body changes/sound/beams are not yet fully connected. Older endpoints without visual-event replay may miss instantaneous actions.

## Event fallback and exit

SSE 404/503, malformed messages, timeout or stream loss falls back immediately to HTTP status. Healthy HTTP keeps the character live, polling approximately once per second; SSE is retried every30 seconds. Only HTTP failure reports a connection error. Requests run outside the render thread, have bounded response size/deadlines, and late replies from a previous URL cannot overwrite the new connection. A healthy SSE connection with no real snapshot/state event for five minutes is cross-checked through HTTP without closing SSE; heartbeat comments and named heartbeat messages do not reset that interval. A failed cross-check alone never marks the character offline. Identical visual snapshots are not republished, so calibration does not cancel local dancing; a changed snapshot corrects the avatar. URL generations still discard late replies, and buffered older snapshots cannot replace a newer calibration within the same visual session.

The desktop entry uses synchronous rendering to avoid a measured shutdown wait in Bevy's pipelined render-channel cleanup. Offline and real-Agent packaged runs have returned exit code0 without leaving a process.

## macOS bundle

```sh
crates/realm-companion/scripts/package-macos.sh '/path/to/Realm Companion.app'
```

The bundle contains the program. Local packaging may include the operator's generated application icon; runtime models and UI assets still come from the local game install. The runtime log is `~/Library/Application Support/AzerothFoundry/companion/run/app.log`. Linux/Windows have not been runtime-verified.

## Visual-effect preferences

Settings has separate Spell, Enchantment and Equipment effect preferences, initially enabled and applied after saving. Spell effects do not control character actions, sitting, eating/drinking, or food/cup props; recovery kits are recognized from the original spell metadata. Equipment effects control authored model glow/particles/billboards/ribbons while retaining the worn geometry and the character's own appearance. Enchantment visuals are not wired to the renderer yet: this preference is currently saved only and will take effect once enchantment adaptation is connected; the UI labels this limitation.

The fixed character-pane camera is close to the model and its transparent canvas is sized for the character. Large native Ice Block/area effects can extend outside that canvas; this framing limitation is separate from alpha/color correctness and is not addressed by shrinking every spell model.

## Local equipment visibility

The **外观显示** panel independently shows/hides the14 model-bearing slots: head, shoulders, cloak, chest, shirt, tabard, waist, wrists, hands, legs, feet, main hand, off hand and ranged. Necklaces, rings and trinkets have no ineffective controls. **全部显示** explicitly overrides the game's head/cloak hiding; **遵循游戏** clears all local overrides. Changes use the existing hair/skin-texture/geoset appearance bake, without modifying the incoming Frame, appearance cache, game character, other preferences or actual equipment. Absent equipment never creates a model. Head visibility retains the compatible `show_helm` null/true/false preference; other local overrides are a slot-indexed `equipment_visibility` map with independent three-way merge. Preview rebakes may briefly lose a nonpersistent cup until its next native kit edge (typically within five seconds).

## Companion SSE wire contract

The additive `realm-companion` protocol 1.1 retains `schema_version: 1`, complete `snapshot`/`state` payloads and the existing bounded visual cursor. The Agent publishes relative `schema_urls` for `/companion/schema.json`, `/companion/asyncapi.json` and `/companion/protocol`; discover these from the configured status URL. The canonical Chinese handbook and machine artifacts live in the adjacent realm-agent repository under `protocol/companion/`.

New animation fields describe native movement flags, derived locomotion, mount state, persistent Emotes.dbc state and attacking. Self `emote`, `attack`, `wound`, `jump` and `land` edges extend the existing spell/kit ring. Emotes.dbc IDs, AnimationData IDs, spell IDs and visual-kit IDs are distinct namespaces. Generic `animation` and `fly` are reserved until a reliable producer exists; supporting a schema entry alone is not a rendering completion claim.

Consumers ignore unknown fields, kinds and enum values, falling back to legacy states. `visual_session` changes reset the per-URL sequence cursor; same-session reconnect consumes only unseen fresh ring entries. Sixty-four entries/fifteen seconds are best-effort replay, not durable logs or Last-Event-ID delivery. Heartbeats and five-minute HTTP calibration preserve the existing local-dance, URL-generation and offline fallback semantics. Never restart an old one-shot merely because a full snapshot includes it again.

## 0.4.0 delivery coverage

The 1.1 companion stream drives persistent locomotion/posture/stealth/loot/stun state, native emotes, swings, wounds and defensive reactions. One-shots use session plus sequence identity; repeated upper-body events restart only that layer and retain the run clock. SpellVisualKit animation data supplies cast holds/releases, work and fishing, including cast-to-channel transitions; disabling spell effects does not disable the body animation. Mount rendering composes the actual creature, attachment-0 seat and rider, with a mounted-only elevated front camera.

On macOS, the visible character/equipment/mount geometry supplies an asynchronous hit mask; transparent margins pass through, and dragging begins on the visible body. Settings and the selector retain normal input. See [hit testing](../../docs/COMPANION-HIT-TESTING.md) for actual manual validation and the automated-test boundaries.

This does not make the headless task engine spontaneously jump/swim, implement taxi routing, or provide every transformation/CharProc visual. `fly` remains a reserved producer state; unsupported model animations fall back. Weapon stowing remains incomplete. Model resources missing from the operator's patch chain cannot be repaired by the SSE contract.

### Local double-click gestures (0.4.1)

Double-click the visible character to choose a supported social animation at random, avoiding an immediate repeat. The model plays one native cycle, including dance, then restores the latest Agent presentation. A new relevant Agent state interrupts immediately; heartbeat/identical calibration does not. A click cancels the gesture. Sleeping, dead and offline characters do not start gestures. This is local presentation only: no game emote is sent. Selection uses exact available model clips from talk/exclamation/question, bow, wave, cheer, dance, laugh, rude, roar, kneel, kiss, cry, chicken, beg, applaud, shout, flex, shy, point and salute; unsupported clips are excluded.

Local gestures also show a short matching Chinese dialogue bubble above the character. Each action has two short lines; the bubble is cosmetic, does not issue chat, and disappears on completion, click cancellation, offline/sleep/death or a relevant Agent update. It renders on a separate transparent UI camera and is excluded from the body-only native hit mask, so it does not enlarge the pointer/drag target.

### Bubble and voice preferences (0.4.2)

`show_speech_bubble` (default true) and `play_emote_sounds` (default false) are independent saved settings. The character keeps its original preview size; the window reserves a transparent band above it for the bubble, with the silhouette mask using the same viewport. Dialogue remains locally authored Chinese, not stock game dialogue or a transcript of the voice. Voice playback resolves the social command through EmotesText/EmotesTextSound and SoundEntries for the current appearance race/sex. Silent or unavailable mappings stay silent. The macOS standalone uses afplay with bounded per-process cached original audio files; cancellation, Agent state changes and disabling sound stop the current child. No network download, TTS, game-chat message or game-install modification.

Coverage note: the first standalone voice lane uses EmotesTextSound only. M2 key-timed `$CSD` laugh/cry sounds are a distinct game-client lane and are not yet routed here. Silence therefore may mean an unmapped lane, not proof the original game has no sound for that action.

### Native surface / hit testing fix (0.4.3)

The app bundle/window icon is now the stock WoW logo; the tray continues to show the connected actor's class (WoW when offline). Packaging exports the logo from the configured local Data before generating ICNS, avoiding the old generated Frostbolt image.

The speech band uses a clip-space projection remap inside a full-size transparent render target; it no longer creates a partial camera viewport. The geometry hit mask clones the exact same projection. A full transparent clear keeps empty margins transparent. The bubble retains its original 8px inset within the speech band, shifted down with the extra 96-unit effect margin. Speech UI renders later and never becomes a body mouse target.

### Settings and selector (0.4.4)

The selector uses 896×504 logical pixels (16:9), a compact character list, double-click or Enter Character, Manage Connections, Exit. Removed duplicate return and persistent saved-success text. Settings remain a 760×680 scrollable form. Stock slider/scroll knob artwork is read from MPQ; percentage is an editable text field instead of DragValue. Checked-state artwork remains visible on hover. Show All, Hide All, Follow Game control every supported equipment slot. Equipment visibility, FX preferences and scale preview the draft while editing; save persists, closing settings returns to saved presentation. Runtime/agent state is never modified by preview.

### Selector, role management and settings (0.4.12)

These are three separate pages. The default selector is approximately 450×800 logical (9:16): centered WoW logo at the top, “管理角色”, eight 60px rows with 6px spacing in a 544px list area (10px top /12px bottom padding), then fixed equal-width “进入角色” / “退出”. Extra roles scroll. Rows do not expose endpoint URLs on hover. The list backdrop uses the stock CharacterSelect insets (left10/right5/top4/bottom9) rather than uniform padding, so its translucent fill meets the authored border. Hover never selects; single-click selects, double-click or Enter enters, and arrows change selection. No character preview or rotation is loaded.

Small displays reduce the window and egui controls by the same factor, using the primary monitor's logical dimensions with a conservative32px horizontal /120px vertical allowance. The setting does not resize the desktop pet itself. The background reuses original racial scenes on a1920×1080 empty stage, then center-crops through UV coordinates to the portrait region. This avoids both stretching and the uncovered sky geometry that direct portrait projection can expose. Original mappings remain Human, Orc/Troll, Dwarf/Gnome, NightElf, Scourge and Tauren; unknown race uses UI_MainMenu, missing stage assets retain the dark dialog background. Stage rendering stops after leaving selection.

Role management is a separate compact760×420 form, also accessible with `--manage-roles`. It uses consistent “添加角色” / “移除角色” labels, automatically resolves names from URLs, and returns to selection after Save or Cancel. “测试连接” beside the URL performs a fresh read-only request independently of the background queue/cache and shows success, offline-role state or a failure reason. A different URL does not reuse an old test result. Testing does not activate the role or save an edited address. No persistent URL-entry hint is shown. Saving only persists the role registry; it cannot command the poller or switch the currently running pet. Removal never deletes a server character.

Settings remains a760×680 grouped form: display/performance, appearance/effects, interaction/audio and resources/status. The window/menu title is “设置”. Its fixed header shows the current character with a “选择角色” navigation button alongside it. There is no editable role picker or URL editor here. The normal footer contains only “保存” and “取消”; a restart action appears when a resource-directory change requires it. The menu's “选择角色…” remains another entry. If unsaved preferences exist, either entry offers Save and continue / Discard / Keep editing; a failed save retains the draft and does not navigate. Settings save only applies preferences and never role-selection/URL edits. Live preview is exclusive to the settings page; management and selection use saved preferences. Save/Cancel remain outside scrolling content.

Each role row shows name, level/class and last-known region. The resolver reads existing status data at most once per source per30seconds while the interface is open, without game actions. Per-profile/per-URL caches retain only role metadata. Region uses an explicit area name when supplied, otherwise the smallest enclosing WorldMapArea rectangle localized through AreaTable; this is a region estimate, not exact WMO/subzone detection. Unknown fields are shown as unknown. Controls remain Rust/egui with original textures/fonts; the system pointer is unchanged.

Configuration format remains version1; this release does not need a new migration or re-entry of role addresses. See [COMPANION-UPGRADES.md](../../docs/COMPANION-UPGRADES.md).

### Adaptive refresh and power saving (0.4.8)

The macOS standalone enables the saved **省电待机** preference by default. Idle and active refresh targets are independently editable from 2–120 FPS (defaults 4 and 30). Changes preview while settings are open and persist only on Save. Stable idle defaults to 4Hz; a native 20Hz timer checks pointer proximity and pending Agent state, waking the application without continuously rendering. Hover, dragging, settings, movement, casting, attacking, local gestures, recovery animations and visible dynamic effects use the configured active target (default 30Hz). State/posture transitions temporarily retain this active cadence. Higher targets trade CPU/power for smoothness. The desktop uses the Bevy/wgpu GPU renderer (Metal on the verified Apple M2 Max), while CPU work remains for animation, ECS and submission. This trades smooth idle breathing for lower CPU; disable custom power-saving idle for 20Hz idle / 10Hz sleep; the active target remains configurable. Platforms without native wake support retain those fallback rates. Window events can produce additional updates; these are targets, not hard FPS caps.

A stable idle body silhouette is reused until pose/appearance/geometry changes; pointer entry restores frequent capture. Pending GPU readbacks are checked against the current pose stamp. Transparent areas remain click-through. Empty speech render passes are disabled; unchanged framing/window properties are cached, and tiny-scene ECS schedules use serial execution with smaller task pools. The complete Bevy time/message lifecycle and renderer still run on each update; only unused fixed physics systems are disabled. This does not change Agent behavior or the normal game application.

Opt-in `REALM_COMPANION_PERF=1` logs measured updates/captures/readbacks per15seconds, without snapshots/URLs. `REALM_COMPANION_REFRESH=legacy` restores the old refresh/capture policy for local A/B measurement. Neither flag is persisted to settings. `scripts/measure-cpu.py PID OUTPUT_JSON SECONDS_PER_ROUND ROUNDS` records per-process CPU-time deltas and sampled median/P95; do not compare RSS from an old compressed process with a newly launched process as proof of a memory regression.

### Color output and effect margins (0.4.13)

The transparent desktop surface decodes the model renderer’s gamma output once before presentation, matching the game preview’s color transfer. The original 233×224 character pane sits inside a 361×396 transparent canvas so attached effects can extend above and beside the body without shrinking it. Geometry alone remains the pointer target; light planes, particles and transparent margins pass through. Saved positions on the primary display are fitted within that display when the canvas size changes.
