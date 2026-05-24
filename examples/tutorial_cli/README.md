# Tutorial — CLI tool

A tiny note-taking CLI that demonstrates command-line argument parsing,
filesystem read/write, and conditional dispatch in pblang.

## Run

```bash
# add a note
pblang run examples/tutorial_cli/main.sapp add "buy milk"

# list notes
pblang run examples/tutorial_cli/main.sapp list

# clear notes
pblang run examples/tutorial_cli/main.sapp clear
```

The notes are persisted to `notes.txt` in the working directory.

## What you learn

- `stdlib:os.sapp` — `os.args()` returns the CLI argument vector.
- `fs_read` / `fs_write` builtins for plain file I/O (no capability gating
  needed when the file lives in the current directory; see
  `stdlib:fs.sapp` for the safer, sandbox-aware wrappers).
- `try ... catch` for graceful "file doesn't exist yet" handling.
- Conditional `if`-as-expression: `let cmd = if len(args) > 0 { args[0] } else { "help" };`
