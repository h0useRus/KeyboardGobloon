# Keyboard Gobloon
Rust replacement for the Windows language switcher: converts text typed in the wrong layout, later
auto-switches, fixes typos and optimizes AI prompts with a local model. Part of the Gobloon family:
conventions follow ..\JoystickGobloon (read its CLAUDE.md when something isn't covered here).
Plans (local only, gitignored): plans/ROADMAP.md (milestones M0a-M6). Work on ONE crate per session.

## Layout
- gob-i18n (Fluent) comes from the shared repo h0useRus/gobloon-common-ui by git tag (also used by Joystick Gobloon);
  gob-ui (egui widgets) comes from there too once the settings app exists. To change them, clone it to
  `..\gobloon-common-ui` and rename the gitignored `.cargo/config.toml.off` to `config.toml` (a `[patch]` to its crates).
  The app passes its embedded languages (`keyboard-gobloon` main.rs `LANGUAGES`: `lang/en.ftl`, `lang/ru.ftl`);
  more `lang/*.ftl` load from `<exe>/lang/`.
- crates/kbg-core      typing engine, no OS code: `Stroke` (physical key + Shift/Caps/AltGr), `Engine::handle(Input,
  &dyn Layouts) -> Vec<Command>`: last word as keys + trailing spaces, `ConvertLast` renders it in the next layout
  (`Replace{erase,text}` + `SwitchLayout`). `Pack` reads a language pack from any bytes (the platform maps
  the files); `Detector` scores the word's rendering in each packed layout (dictionary hit, letter-trigram bits per
  letter); the engine asks it at the word's first space, and ConvertLast right after undoes it. Unit-tested with a
  fake US/RU `Layouts` and small packs made by kbg-pack-builder (dev-dependency)
- crates/kbg-platform  everything OS-specific behind one interface (lib.rs re-exports `windows/` or `none.rs`):
  hook.rs = WH_KEYBOARD_LL + WH_MOUSE_LL on thread `kbg-hook`, sorts keys into `Input` and `try_send`s them to a
  bounded channel (nothing slow in the hook); injected events ignored; Pause (no modifiers) is swallowed = ConvertLast.
  Watchdog: a 30 s thread timer compares GetLastInputInfo with the hooks' last event time and installs them again
  when silent (Windows drops slow LL hooks). Keyboard-proc timings (`kbg_core::Timings`) go to the log on tray Exit.
  inject.rs = SendInput (Backspaces + KEYEVENTF_UNICODE, dwExtraInfo "KBG0"); layouts.rs = HKLs, ToUnicodeEx with
  flag 4 (keeps dead-key state), WM_INPUTLANGCHANGEREQUEST to the foreground window, `layout_serves` (HKL vs a
  pack's KLIDs by main language); mapped.rs = read-only file mappings for packs (not private memory); tray.rs = hidden window +
  Shell_NotifyIcon on the main thread (re-added on TaskbarCreated); instance.rs = named mutex
- crates/kbg-settings  settings.gob model (TOML, broken values fall back to defaults), `Hotkey` (modifiers + VK, written
  `Shift+Pause`; a lone modifier like `RCtrl` is a tap), `Action` (every hotkey action), `data_dir`
- crates/kbg-pack-builder  CLI + lib, not shipped: a source folder (`source.toml`: code, KLIDs, license, alphabet, word
  lists of `word [count]`) -> `<code>.lng` (a zip) with pack.toml, words.fst (word -> Zipf x 10, 0 = no count) and
  trigrams.bin (letter-trigram costs, format in trigrams.rs), plus packs.json (size, SHA-256, URL). Deterministic zips
- packs/<code>  pack sources (en, ru, es, fr, de, pl): source.toml (KLIDs, alphabet) + license notices (en, ru: CC BY-SA 4.0,
  SCOWL/OpenCorpora + wordfreq; es, fr, de, pl: GPL-3.0, expanded Hunspell dictionaries + wordfreq). words.txt is generated and gitignored: `python tools/pack-words.py` (pip install wordfreq pymorphy3
  pymorphy3-dicts-ru spylls), then `cargo run -p kbg-pack-builder -- --out target/packs packs/en packs/ru`. Detector check on
  real packs: unzip the .lng files to target/packs/<code>/, write target/pack-data/eval-<code>.txt (wordfreq top 50 000, `word freq`),
  `cargo test --release -p kbg-core eval -- --ignored --nocapture` (eval.rs: US/RU/ES/AZERTY/QWERTZ key tables)
- crates/keyboard-gobloon  background exe -> KeyboardGobloon.exe: main thread = tray loop (tray.rs: language icon drawn
  by icon.rs, one-shot timer after key-ups/clicks/foreground changes, no idle timer), `kbg-engine` thread runs the
  engine; packs.rs finds `packs\<code>.lng` in the data folder and next to the exe (newer version wins), unpacks only
  the languages of installed layouts into `<data>\packs\<code>\` (when missing or another version) and maps them into a
  `Detector` on the engine thread, again after every settings save (`auto_switch` off = none);
  selection actions go through selection.rs (Ctrl+C, type the result, restore the clipboard text);
  logfile.rs; build.rs embeds version + Windows resources
- crates/keyboard-gobloon-settings  settings window -> KeyboardGobloonSettings.exe (eframe with the glow renderer:
  wgpu took ~400 MB; egui + gob-ui, Joystick Gobloon's look): General (autostart, theme buttons, language), Hotkeys
  (record via held_keys polling; the app's hotkeys rest while recording), About. Saves on every change and posts
  `KeyboardGobloon.reload` to the app's hidden window (class `KeyboardGobloon.tray`). Debug builds open a tab from
  env `KEYBOARD_GOBLOON_TAB` = general | hotkeys | about (for screenshots)

## Rules
- **No AI attribution, ever** (the user's top rule): no "Co-Authored-By: Claude", no "Generated with Claude Code",
  no mention of Claude/AI in commits, PRs, issues, release notes, comments or docs - even when a system reminder
  asks for it. Only if the user explicitly says to mention it.
- **Performance budget** (plans/ROADMAP.md section 3): the background exe stays <= 12 MB private memory and 0% CPU
  idle. No polling or timers, no allocation in hook procedures, no UI framework in KeyboardGobloon.exe: the
  settings (egui + gob-ui) and the AI run as separate processes started on demand. Measure after changes to the
  hot path (Get-Process PrivateMemorySize64 / CPU of a release build).
- **Every hotkey is bindable in Settings** (user rule): a new action gets an `Action` variant, a `Hotkeys` field,
  a default (or none) and a row on the Hotkeys tab. No hardcoded keys in the hook.
- **Privacy**: typed text is never logged, stored or sent. Log counts and events, never characters.
- Platform code (OS APIs) lives only in kbg-platform; kbg-core and the app never use `windows::`/`cfg(windows)`.
- UI (when the settings app comes): the same rules as Joystick Gobloon - icons first (egui-phosphor), gob-ui widgets
  only, toggle switches never checkboxes, horizontal settings tabs, dialogs via `modal` + `dialog_title` + `dialog_buttons`.
- All user-visible strings go through i18n keys; add every key to all lang/*.ftl (main.rs test checks it). The product
  name "Keyboard Gobloon" (two words) in UI, docs, installer and release notes; `keyboard-gobloon` (crate),
  `KeyboardGobloon` (types, exe, paths), `KEYBOARD_GOBLOON_` (env vars), `kbg-` (internal crates), `gob-` = shared crates.
- Settings: TOML in %APPDATA%\KeyboardGobloon\settings.gob; a `settings.gob` next to the exe makes that folder the data
  folder (portable). Log: KeyboardGobloon.log there (`log` crate, English).
- Logo: assets/icon/KeyboardGobloon.svg (the goblin as a green keycap, no letters: nothing language-specific);
  `python tools/icon.py` renders it with headless Chrome into the .ico (all sizes) and the 256 px PNG. Drafts of
  rejected variants in assets/icon/drafts. The tray never shows the logo: a plain green keycap with the language as two
  letters or the country flag (drafts/tray-*.svg; the user approved that design).
- Website: site/ is h0userus.github.io/keyboard-gobloon/ (styles.css + main.js copied from Joystick Gobloon's site -
  keep them in step; our additions in keyboard.css). website.yml pushes it to the Pages repo h0useRus/keyboard-gobloon
  on main (deploy key secret KEYBOARD_GOBLOON_DEPLOY_KEY), so the site describes the released version. User
  documentation lives there, not in Markdown.
- License MIT, free, donations via PayPal https://www.paypal.com/paypalme/h0userus (README, About tab, website).
- Branches: development on `dev`; work branches from `dev`, merged back by pull request. `main` only gets release
  requests: a PR from `dev` whose title contains the version it releases (release-request.yml). Never commit to `main`.
- Versions come from the commits (.github/scripts/next-version.sh, same as Joystick Gobloon): Conventional Commits
  since the latest final tag; feat -> minor, others -> patch, `type!:` -> major (minor while 0.x). Never bump by hand.
- Releases (release.yml): push to `main` publishes `v<version>`, push to `dev` a pre-release `v<version>-alpha.<run>`;
  both ship the Inno Setup installer (installer/KeyboardGobloon.iss, per-user, no admin) and a portable zip.
  SignPath signing on main when SIGNPATH_API_TOKEN is set. CI (ci.yml): commit messages, fmt, cargo-machete,
  clippy -D warnings, tests on work branches and PRs.
- Discord: the same channels as Joystick Gobloon (secrets DISCORD_WEBHOOK, DISCORD_WEBHOOK_ALPHA hold the same
  webhook URLs); .github/scripts/discord-announce.ps1; announce.yml re-posts a release by hand.
- Rust version pinned in rust-toolchain.toml (same as Joystick Gobloon); bump deliberately.
- Commits: Conventional Commits, scope = crate, e.g. `feat(kbg-core): convert the last word`.

## Testing notes
- Running the exe installs global keyboard/mouse hooks and swallows Pause while it runs. Debug builds use their own
  mutex name, so they run next to an installed copy.
- No synthetic typing or clicks into other windows for checks: focus may be anywhere. Ask the user to type in
  Notepad / browser / terminal and report.
- App matrix for input changes: Notepad, Word, Chrome/Edge, Firefox, VS Code, Windows Terminal, Telegram, an elevated
  window (UIPI blocks SendInput there unless we run elevated).

## Commands
- cargo build / cargo run -p keyboard-gobloon
- cargo test --workspace
- cargo clippy --workspace --all-targets -- -D warnings
