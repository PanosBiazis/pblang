# One-line applications

The compact facade launches each application class without repeating the full
specification envelope:

```pblang
import("stdlib/app.sapp").cli("Hello from pblang");
import("stdlib/app.sapp").gui("Tool", [map{ kind: "label", text: "Ready" }]);
import("stdlib/app.sapp").web("API", fn(ctx) { return ctx.json(map{ ok: true }); });
import("stdlib/app.sapp").rest("API", fn(ctx) { return map{ ok: true }; });
import("stdlib/app.sapp").desktop("Tool", [map{ kind: "label", text: "Ready" }]);
import("stdlib/app.sapp").modern("Tool", import("stdlib/modern_ui.sapp").screen([import("stdlib/modern_ui.sapp").heading("Ready")]));
import("stdlib/app.sapp").surface("Tool", import("stdlib/modern_ui.sapp").screen([import("stdlib/modern_ui.sapp").heading("Ready")]), map{ mode: "web", operation: "preview" });
import("stdlib/app.sapp").game("Loop", fn(ctx) { return ctx.modules.game.vec2(ctx.step, 0); });
import("stdlib/app.sapp").multimedia("Image", fn(ctx) { return ctx.modules.image.make_image(64, 64, 0); });
import("stdlib/app.sapp").database("Query", fn(ctx) { return ctx.modules.db.sql_placeholders(3); }, map{ allow_io: true });
import("stdlib/app.sapp").networking("Request", fn(ctx) { return ctx.modules.networking.new_request("GET", "https://example.com"); }, map{ allow_io: true });
import("stdlib/app.sapp").automation("Job", fn(ctx) { return ctx.step; });
import("stdlib/app.sapp").ai("Similarity", fn(ctx) { return ctx.modules.ai.cosine_similarity([1, 0], [1, 0]); }, map{ allow_io: true });
import("stdlib/app.sapp").ml("Model", fn(ctx) { return ctx.modules.ml.linreg_fit([1, 2], [2, 4]); });
import("stdlib/app.sapp").universal("Workspace", fn(request) { return request.surface; });
```

Pass `map{ preview: true }` as the last argument to GUI/web helpers to exercise
the real adapter without opening a window or listening socket. Domain previews
are plan-only; add `preview_execute: true` to run the bounded action. Database,
networking, and AI execution requires explicit `allow_io: true`. The callback
receives the real lower-level libraries through `ctx.modules`, so advanced users
retain their complete APIs rather than being restricted by the facade.

`stdlib/app.sapp` provides one declarative entry point for CLI, GUI, web, REST,
desktop, game, multimedia, database, networking, automation, AI, and ML
applications. The implementation and application sources are PBLang.

- `main.sapp`: terminating CLI application.
- `gui.sapp`: headless GUI preview using the real form builder.
- `gui_live.sapp`: interactive native GUI window.
- `web.sapp`: in-process hardened web preview.
- `web_live.sapp`: blocking loopback web server on port 8080.
- `fullstack.sapp`: one-line CLI + GUI + hardened web application preview.
- `compact.sapp`: compact helper API combining CLI, GUI, and web previews.
- `universal.sapp`: one business action exposed through CLI, native GUI, and a
  hardened Web UI/API without application-authored HTML, CSS, or JavaScript.
- `modern.sapp`: terminating preview of the responsive, themeable declarative
  UI contract.
- `modern_live.sapp`: interactive modern native GUI whose rows and grids adapt
  when the window is resized.
- `modern_android.sapp`: builds a signed, installable Android APK directly from
  the same declarative PBLang UI tree and portable actions, without Java/Kotlin.
- `modern_web.sapp`: generates and exercises the hardened responsive browser
  adapter without opening a socket.
- `modern_web_export.sapp`: exports a complete responsive static Web
  application and verification manifest.
- `modern_surface.sapp`: selects `auto`, `desktop`, `web`, or `android` while
  retaining one shared component tree.
- `domains.sapp`: terminating previews of REST, desktop, game, image processing,
  database, networking, automation, AI, and ML facades.

`modern_ui` runs with the native desktop adapter, builds installable Android
APKs through its PBLang JVM-bytecode adapter, and renders hardened responsive
Web applications. iOS and embedded targets remain validated contracts until
their runtime adapters are linked.
The current built-in multimedia provider is image processing. Audio and video
providers are not claimed by this facade until real PBLang libraries and tests
exist for them.

Run the terminating examples with `pblang run <file> --allow-all`. The live GUI
and web files intentionally block until the window closes or server is stopped.
