# pblang Tutorials

This page is a hands-on tutorial track for beginners.

If you are new, start with `docs/USAGE_GUIDE.md`, then come back here.

## Tutorial 1: CLI app (notes list)

Goal: build a tiny command-line script that adds and lists notes from a text file.

Create `examples/tutorial_cli/main.sapp`:

```sapp
let os = import("stdlib:os.sapp");

let args = os.script_args(); // os.args() is the RAW argv ("run", path, flags...)
let cmd = if len(args) > 0 { args[0] } else { "help" };

let db_file = "notes.txt";

fn read_notes() {
    // catch binds no name in pblang — `catch e { ... }` would crash the
    // handler with "undefined global 'e'". Use the expression form:
    return try fs_read(db_file) catch "";
}

fn write_notes(s) {
    fs_write(db_file, s);
}

if cmd == "add" {
    if len(args) < 2 {
        print "usage: add <note>";
    } else {
        let old = read_notes();
        let next = old + args[1] + "\n";
        write_notes(next);
        print "added";
    }
} else if cmd == "list" {
    print read_notes();
} else {
    print "commands: add, list";
}
```

Run:

```bash
pblang run examples/tutorial_cli/main.sapp -- add "buy milk"
pblang run examples/tutorial_cli/main.sapp -- list
```

What you learned:

- `os.script_args()` for CLI args (`os.args()` is the raw argv)
- simple file I/O (`fs_read`, `fs_write`)
- basic control flow

## Tutorial 2: HTTP API (hello endpoint)

Goal: run a local HTTP endpoint that returns JSON.

Create `examples/tutorial_http/main.sapp`:

```sapp
let server = import("stdlib:http_server.sapp");

fn handler(req) {
    return map{
        status: 200,
        content_type: "application/json",
        body: "{\"message\":\"hello from pblang\"}"
    };
}

print "listening on 127.0.0.1:8081";
server.serve_forever("127.0.0.1:8081", handler);
```

Run:

```bash
pblang run examples/tutorial_http/main.sapp
```

Test from another terminal:

```bash
curl http://127.0.0.1:8081
```

What you learned:

- returning structured HTTP responses
- long-running services in pblang

## Tutorial 3: Concurrency pipeline (worker + channels)

Goal: spawn workers, send jobs through channels, collect results.

Create `examples/tutorial_concurrency/main.sapp`:

```sapp
let thread = import("stdlib:thread.sapp");

let jobs = thread.channel_new();
let results = thread.channel_new();

let worker = thread.spawn(fn() {
    let n = thread.recv_value(jobs.receiver);
    thread.send_value(results.sender, n * n);
});

thread.send_value(jobs.sender, 12);
let out = thread.recv_value(results.receiver);
thread.join(worker);

print "result=" + str(out);
```

Run:

```bash
pblang run examples/tutorial_concurrency/main.sapp
```

Expected output:

```text
result=144
```

What you learned:

- `spawn`/`join`
- channel-based message passing
- worker-style decomposition

## Suggested next steps

After these tutorials:

- add tests for each script under `tests/`
- convert each into a package with `pblang init`
- bundle with `pblang bundle`
- read `docs/Tooling.md` for advanced workflows
