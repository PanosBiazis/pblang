# Embedded / Robotics — GPIO, LCD, PWM, servo, debounce

Pure-pblang hardware drivers harvested from the LightLedRasPi project and the
Arduino LiquidCrystal backlog. The design rule throughout: **protocol logic is
pure and unit-testable; only the last layer touches hardware.**

| Module | What it drives |
|--------|----------------|
| `stdlib/gpio.sapp` | Linux sysfs GPIO: export/direction/read/write, plus pure plans (blink, software PWM, servo) and a pure debounce state machine |
| `stdlib/lcd.sapp` | HD44780 character LCDs (the Arduino "LiquidCrystal" part) over gpio, 4-bit mode |

## The injectable backend

`gpio.new(backend)` takes `{read: fn(path), write: fn(path, text)}`. Tests
inject an in-memory fake; hardware uses `gpio.sysfs_backend()` (two lambdas
over `fs_read`/`fs_write`). Same seam as `ollama.sapp`'s transport — it is why
every driver here has real tests on a machine with no GPIO.

## LCD: plans, then lowering, then execution (the codegen way)

```pblang
let GP = import("stdlib:gpio.sapp");
let LCD = import("stdlib:lcd.sapp");

let pins = map{ rs: 25, e: 24, d4: 23, d5: 17, d6: 27, d7: 22 };  // BCM numbers
let g = GP.new(GP.sysfs_backend());
LCD.setup_pins(g, pins);                    // export + direction out
LCD.run(g, LCD.print_plan("Hello", pins));  // init sequence + text
LCD.run(g, LCD.to_gpio_steps(LCD.cursor_ops(1, 0), pins));  // line 2
```

1. `*_ops` build `[{rs, byte}]` command/data plans (pure).
2. `to_gpio_steps` lowers to `[{pin, value}]` — 4-bit protocol, 13 writes per
   byte: rs, high nibble on d4–d7, E pulse, low nibble, E pulse (pure).
3. `run` replays steps over a gpio handle (the only hardware touch).

## Pure schedules

- `GP.blink_plan(pin, times, ms)` — on/off pairs.
- `GP.pwm_plan(pin, duty_pct, period_ms, cycles)` — software PWM.
- `GP.servo_plan(pin, angle_deg, cycles)` — 50Hz frames, 1–2ms pulse for 0–180°.
- `GP.debounce_new(initial, stable_ms)` + `debounce_step(state, raw, now_ms)` —
  contact-bounce filter as a pure state machine.

Any executor can replay a schedule: a loop with sleeps, a watcher, a simulator.

## Honest boundaries

- sysfs GPIO is the legacy-but-ubiquitous interface and needs permissions
  (gpio group or root). The newer gpiod character-device API is ioctl-based —
  reachable via the codegen syscall path, not via file read/write.
- Software timing from userspace is jittery: fine for LEDs, LCDs, and rough
  servo positioning; not for precision PWM (use the pwmchip sysfs interface or
  hardware PWM for that).

Tests: `tests/semantics/gpio_test.sapp`, `tests/semantics/lcd_test.sapp`
(hand-computed 4-bit sequences, fake backend).
