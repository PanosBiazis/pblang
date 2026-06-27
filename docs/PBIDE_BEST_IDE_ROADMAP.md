# PBIDE Best IDE Roadmap

PBIDE should become the strongest pblang-native development environment we have built: fast, clear, safe, responsive, extensible, and useful for code, visual design, browser/web work, debugging, AI assistance, and project automation.

This file is the category-by-category target map. It is not a claim that every item already exists. Each category lists the current direction, what is needed next, the best-IDE target, and how to verify progress.

## 1. Code Editor

Current:
- Native editor model supports cursor movement, selection, undo/redo, find, paste, line duplication, deletion, movement, indentation, outdentation, comment toggling, trimming whitespace, and current-line copy/cut fallback.
- PBIDE exposes many shortcuts through native key handling and command palette actions.

Needed next:
- Multi-cursor editing, rectangular selections, symbol-aware rename, better bracket matching, soft wrap, minimap, sticky scopes, and stronger syntax recovery after broken code.
- A full keyboard shortcut registry so every action can be displayed, changed, tested, and documented from one source.

Best-IDE target:
- Editing should feel immediate on large files, support all Unicode input including symbols and emojis, and keep every common coding action reachable by keyboard, palette, and UI button.

Verification:
- Add editor-model tests for every shortcut and text transform.
- Add large-file latency checks for typing, selection, search, and formatting.

## 2. Files, Projects, And Workspaces

Current:
- PBIDE has project-aware panels and file-oriented actions, with native open/save flows evolving toward full IDE parity.

Needed next:
- Reliable open, save, save-as, revert, recent files, project tree, safe rename/move/delete, file watchers, dirty-state tracking, backup recovery, and workspace snapshots.
- Project profiles for GUI, web, CLI, library, game, AI, and mixed app types.

Best-IDE target:
- A user can manage an entire pblang project without leaving PBIDE, while destructive file actions remain explicit and recoverable.

Verification:
- End-to-end tests for new file, open, edit, save, reload, rename, delete, and crash recovery.
- Watcher tests for external file changes.

## 3. Build, Run, Check, Debug, And Release

Current:
- PBIDE has run/check-style controls and mode concepts are being added.

Needed next:
- Clear Debug and Release modes, build profiles, task presets, run configuration history, test filtering, slow-test reporting, and inline build output.
- A native debugger panel with breakpoints, stepping, stack frames, locals, watches, and expression evaluation.

Best-IDE target:
- Debug/run/test should be one-click, predictable, and visible. PBIDE should explain failures in the right panel and jump directly to the source location.

Verification:
- Golden tests for generated commands.
- UI tests for mode switching, task output, and diagnostics-to-source navigation.

## 4. Diagnostics, Problems, And Quick Fixes

Current:
- PBIDE shows output and has fix-oriented actions, but some earlier fix buttons were not reliable enough.

Needed next:
- Structured diagnostics from parser, checker, formatter, linter, runtime, tests, and pbAI.
- Quick fixes must be conservative: show preview, apply exactly one patch, and be undoable.

Best-IDE target:
- Problems should be grouped by file, severity, tool, and category. Fixes should never corrupt valid code.

Verification:
- Regression tests for malformed code, formatter failures, no-op fixes, and undo after quick fix.

## 5. Command Palette And Keybindings

Current:
- PBIDE has palette commands for many editor actions.

Needed next:
- Searchable command descriptions, aliases, categories, recent commands, custom shortcuts, conflict detection, and command telemetry for local debugging.

Best-IDE target:
- Every IDE action exists in the command palette and has a stable command id, label, shortcut, safety class, and test.

Verification:
- Command registry tests ensuring every visible button has a command id and every command has documentation.

## 6. Panels, Layout, And Responsiveness

Current:
- PBIDE has multiple panels, but the layout still needs stronger resizing, proportional behavior, and visual polish.

Needed next:
- Resizable panels, saved layouts, compact/focus/zen modes, responsive behavior for small screens, smoother rendering, and no overlapping labels.

Best-IDE target:
- The interface should stay clean and usable from small laptop screens to multi-monitor setups.

Verification:
- Snapshot-style layout inspections from PBIDE's UI observer.
- Tests for panel sizes, minimum widths, and command visibility.

## 7. Visual Designer

Current:
- Designer work exists, including 2D/3D-oriented ambitions and generated code paths, but the generated code and some elements still need stronger reliability.

Needed next:
- App type modes: GUI, web, or both.
- Component inspector, hierarchy tree, constraints, anchors, layout grids, responsive previews, generated-code validation, and safe custom component imports.
- Element groups for forms, charts, 2D, 3D, n-dimensional visualizations, media, science widgets, and game/engine-like scenes.

Best-IDE target:
- The designer should be better than a simple form designer: it should generate runnable pblang code, support custom components, and explain broken generated code before run.

Verification:
- For every designer element, test create -> configure -> generate -> check -> run.
- Add invalid-component tests to ensure PBIDE reports errors instead of producing broken code silently.

## 8. Browser And Web Tools

Current:
- PBIDE browser work exists, with security modes, Tor-related ideas, and native rendering goals.

Needed next:
- Clear security profiles, live interactive document model, source viewer, network/details panel like browser devtools, ad/tracker/malware blocking, protocol allowlists, and safe local-file policies.
- Native renderer improvements must be incremental: HTML tokenizer, DOM tree, style rules, layout, event handling, script policy, and rendering diagnostics.

Best-IDE target:
- PBIDE should inspect and debug web content safely, while making clear which browser features are supported natively and which are intentionally blocked.

Verification:
- Tests for blocked protocols, blocked trackers, safe source view, DOM inspection, and non-crashing render fallback.

## 9. Terminal, Tasks, And Automation

Current:
- PBIDE can trigger tools indirectly, but task automation should become first-class.

Needed next:
- Integrated task runner, safe terminal profiles, command history, environment display, background task management, cancellation, and output parsing.

Best-IDE target:
- Common project automation should be easier from PBIDE than from a terminal, without hiding what command is being run.

Verification:
- Tests for task creation, cancellation, timeout handling, and safe command rendering.

## 10. AI And pbAI Assistance

Current:
- pbAI integration is a major project direction and PBIDE can become the main user surface for pblang-native AI.

Needed next:
- Ask/fix/explain/review flows, code-aware context packing, local-only memory controls, safe patch preview, verified generation, training/eval dashboards, and model state viewers.

Best-IDE target:
- AI assistance should improve code and project understanding without silently changing files or pretending uncertain answers are verified.

Verification:
- Every AI patch path needs preview, apply, undo, and tests.
- AI answers that claim verification must include the command or checker result.

## 11. Watchers And UI Observer

Current:
- Watcher concepts exist and should be used to understand UI behavior without depending only on screenshots.

Needed next:
- A pblang-native UI observer that records widget tree, bounds, focus, text, colors, visibility, events, frame timing, and layout warnings.
- Watchers for file changes, running tasks, diagnostics, render frames, browser events, and designer-generated code.

Best-IDE target:
- PBIDE should be inspectable from code: when UI breaks, the observer should show what element failed, where it is, and why.

Verification:
- Observer fixture tests for missing buttons, overlapping panels, clipped text, slow frames, and failed click targets.

## 12. Graphics, Media, Science, And Charts

Current:
- The project has graphics and visualization ambitions for 2D, 3D, n-dimensional, charts, images, audio, video, and science libraries.

Needed next:
- Stable graphics scene API, chart primitives, media previewers, geometry tools, math-symbol rendering, and graceful fallback for unsupported devices.

Best-IDE target:
- PBIDE should be able to build and debug visual/scientific apps, not only text programs.

Verification:
- Render tests for every primitive group.
- Device capability reports and fallback tests.

## 13. Extensions, Components, And Plugins

Current:
- Custom components and external designer imports are a target.

Needed next:
- Extension manifest, permissions, command registration, designer component registration, templates, snippets, and safe import from HTML/CSS/JS or other formats when allowed.

Best-IDE target:
- Users can add tools and components without modifying PBIDE core, and unsafe extensions remain sandboxed.

Verification:
- Extension load tests, permission-denial tests, and malformed-manifest tests.

## 14. Version Control And Collaboration

Current:
- Git and collaboration can be improved through project tooling.

Needed next:
- Source control panel, diff viewer, branch/status actions, stash, commit preview, conflict viewer, and local collaboration logs.

Best-IDE target:
- PBIDE should make project history visible and keep risky repository actions explicit.

Verification:
- Tests with temporary git repos for status, diff, commit preview, and conflict detection.

## 15. Performance And Resource Use

Current:
- PBIDE is improving but must stay fast as features grow.

Needed next:
- Incremental parsing, cached syntax tokens, lazy panels, batched redraws, async/background tasks, frame timing, large-file guards, and low-allocation editor operations.

Best-IDE target:
- Typing, scrolling, panels, search, and diagnostics should stay smooth on normal hardware.

Verification:
- Benchmarks for open, type, search, format, render, and test-run flows.
- Watcher warnings for slow frames and excessive redraws.

## 16. International Input, Symbols, And Accessibility

Current:
- Unicode and symbol support are important goals. Math symbols such as capital Pi for product notation should be supported carefully.

Needed next:
- Full character input coverage, emoji-safe editing, dead keys, IME composition, math shortcuts, symbol palette, accessibility labels, high contrast theme, and keyboard-only operation.

Best-IDE target:
- PBIDE should support normal programming, mathematical notation, and international text without breaking cursor movement or file encoding.

Verification:
- Tests with Greek, math symbols, emoji, combining marks, RTL text samples, shortcuts, and copy/paste.

## 17. Security, Privacy, And Safety

Current:
- Security modes and safer browser/protocol behavior are active goals.

Needed next:
- Permission prompts, local-network controls, file access scopes, protocol allowlists, blocked origin reporting, extension permissions, safe AI memory controls, and clear run-command previews.

Best-IDE target:
- PBIDE should be powerful but fail closed. Web pages, extensions, generated code, and AI actions should not get hidden access to files, network, or commands.

Verification:
- Security regression tests for protocol blocking, path scope, extension permissions, browser actions, AI file changes, and command execution.

## Priority Roadmap

P0 - Stabilize essentials:
- Fix open/save/save-as/revert behavior.
- Fix formatter/fix buttons so they never damage code.
- Complete command registry and tests for all visible buttons.
- Add UI observer checks for clipped text, missing actions, and layout overlap.

P1 - Editor parity:
- Multi-cursor, bracket matching, symbol rename, stronger Unicode/IME support, shortcut customization, and large-file responsiveness.

P2 - Project/debug/tooling:
- Project tree, watchers, build profiles, integrated tests, debug/release modes, problems panel, and debugger basics.

P3 - Designer/browser/AI:
- Reliable designer code generation, native browser inspection/security tools, and pbAI patch-preview workflows.

P4 - Extensibility/performance:
- Extension/plugin API, component marketplace-style registry, render benchmarks, incremental parser, and stable low-latency UI loops.

## Definition Of Done

PBIDE becomes "the best IDE we have built" when:
- Every visible button works and has a command id, shortcut, docs entry, and test.
- Opening, editing, saving, running, checking, debugging, designing, browsing, and AI-assisted fixing are all reliable.
- The UI is responsive, readable, resizable, and accessible.
- Risky actions are previewed, permissioned, undoable where possible, and logged.
- PBIDE can inspect itself through watchers/UI observer data so UI bugs are diagnosable without depending only on screenshots.
