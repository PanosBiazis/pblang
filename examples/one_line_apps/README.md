# One-line applications

The compact facade launches each application class without repeating the full
specification envelope:

```pblang
import("stdlib/app.sapp").cli("Hello from pblang");
import("stdlib/app.sapp").gui("Tool", [map{ kind: "label", text: "Ready" }]);
import("stdlib/app.sapp").web("API", fn(ctx) { return ctx.json(map{ ok: true }); });
import("stdlib/app.sapp").rest("API", fn(ctx) { return map{ ok: true }; });
import("stdlib/app.sapp").desktop("Tool", [map{ kind: "label", text: "Ready" }]);
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
- `domains.sapp`: terminating previews of REST, desktop, game, image processing,
  database, networking, automation, AI, and ML facades.

The current built-in multimedia provider is image processing. Audio and video
providers are not claimed by this facade until real PBLang libraries and tests
exist for them.

Run the terminating examples with `pblang run <file> --allow-all`. The live GUI
and web files intentionally block until the window closes or server is stopped.
