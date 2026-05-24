# Tutorial — Reading user input

How to read text + a number from stdin, with graceful handling of
non-interactive runners.

## Run

```bash
pblang run examples/tutorial_user_input/main.sapp
```

The program will prompt for a name and a number, then echo them back.

## What you learn

- `read_line()` — blocking read of one line from stdin, returns `nil` at
  EOF (which is what you'll see when the program is piped from a file or
  run in a non-interactive context like CI).
- `str_trim` — strip trailing newline / surrounding whitespace.
- `parse_number` — string → number; wrap in `try ... catch nil` so a
  malformed input becomes `nil` instead of crashing.
- `if EXPR == nil { … } else { … }` — the standard nil-safety pattern.

## Non-interactive use

The example is defensive on purpose: it falls back to a default name and
prints a friendly message when `read_line()` returns `nil`. That makes it
safe to include in CI smoke tests:

```bash
echo -e "Alice\n42" | pblang run examples/tutorial_user_input/main.sapp
```
