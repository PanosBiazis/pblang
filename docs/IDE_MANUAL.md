# pblang IDE Manual

This manual covers the browser IDE in `editors/ide/index.html` and its backend API served by `pblang serve`.

## 1. Start the IDE

Start backend:

```bash
cargo run -- serve --port 8090
```

Open:

- `editors/ide/index.html` in your browser.

Health endpoint:

- `GET /health`

## 2. Main workflow

- Write/edit code in the main editor.
- `Run` executes code through backend `/run`.
- `Check` runs parse/type diagnostics through `/check`.
- `Format` uses `/format` (falls back to local formatting if backend unavailable).
- `Auto-fix` applies simple known fixes and re-checks.

Keyboard:

- `Ctrl+Enter`: Run
- `Ctrl+Shift+Enter`: Check
- `Tab`: insert spaces in editor
- `F8`: next diagnostic
- `Shift+F8`: previous diagnostic
- `Ctrl+/`: open in-IDE manual modal

Manual modal:

- Toolbar `❓ Manual` opens in-app guide panel.
- Tabs switch between IDE and Project manuals fetched from backend:
  - `GET /manual/ide`
  - `GET /manual/project`

## 3. Diagnostics panel

Diagnostics include stage, message, location, and optionally source line text.

Features:

- click a diagnostic to jump to line/column
- prev/next navigation buttons and keyboard shortcuts
- active diagnostic highlighting
- quick-fix buttons on matching errors

## 4. History panel

History records latest run/check events (up to 30 entries), including:

- action (`run` or `check`)
- success/failure status
- timestamp
- output summary
- diagnostics snapshot
- code snapshot

Actions:

- `Show`: replay that entry output + diagnostics
- `Load code`: restore snapshot into editor
- `Clear`: clear history

All history is stored in browser local storage.

## 5. Examples

Examples are fetched dynamically from backend:

- `GET /examples`
- `GET /examples/<id>`

IDE behavior:

- loads fallback examples if backend example catalog is unavailable
- supports filter/search input by id/title

## 6. Persistence

IDE persists:

- editor autosave content
- theme preference
- output pane width
- example filter text
- run/check history

Storage keys are local browser keys under `localStorage`.

## 7. Layout controls

- drag splitter between editor and output pane to resize output width
- width is clamped and persisted

## 8. Backend API contract

Used endpoints:

- `POST /run` body: `{ code }`
- `POST /check` body: `{ code }`
- `POST /format` body: `{ code, max_line_length? }`
- `POST /normalize` body: `{ code }`
- `GET /health`
- `GET /examples`
- `GET /examples/<id>`

Responses are JSON and include CORS support for browser access.

## 9. Troubleshooting

- **Cannot connect to server**
  - start backend with `pblang serve --port 8090`
  - verify port and browser access
- **Examples missing**
  - backend may be offline; fallback examples should still appear
- **Diagnostics stale**
  - run `Check` explicitly
- **Formatting differences**
  - backend formatter may differ from local fallback when offline

## 10. Recommended usage pattern

1. Write/modify code.
2. Run `Check`.
3. Navigate diagnostics with `F8`.
4. Apply quick fixes as needed.
5. Run full execution with `Run`.
6. Use History to compare outputs/regressions quickly.
