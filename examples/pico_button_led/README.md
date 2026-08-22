# Pico Button and LED

A PBLang-only migration of the earlier Raspberry Pi Pico MicroPython/Wokwi
exercise. An active-low pushbutton on `GPIO14` controls an active-high LED on
`GPIO15` through a real debounce state machine.

## Wiring

```text
Pico GPIO15 -> 220..1000 ohm resistor -> LED anode
LED cathode -> Pico GND

Pico GPIO14 -> normally-open pushbutton -> Pico GND
```

`GPIO14` uses the internal pull-up. Released is HIGH and pressed is LOW. Use
3.3 V GPIO logic and disconnect power before changing the circuit.

## Improvements

- native PBLang model, validation, simulation, CLI, and tests;
- bounded and monotonic debounce processing instead of a fixed sleep comment;
- explicit resistor requirement for a physical LED;
- portable board aliases and RP2040 technical-manual integration;
- deterministic contact-bounce simulation with exact transition evidence;
- source hashes without carrying author or simulator metadata; and
- fail-closed reporting for missing RP2040 codegen and conditional lowering.

The PBLang RP2040 descriptor and pin validation are implemented. Native
RP2040 firmware generation, conditional digital-read lowering, UF2 packaging,
flashing, and physical Pico verification remain incomplete. The simulator does
not claim to have executed on hardware.

## Run

```sh
pblang run examples/pico_button_led/main.sapp -- demo
pblang run examples/pico_button_led/main.sapp -- firmware
pblang run examples/pico_button_led/main.sapp -- manual
```

## Verify

```sh
pblang run examples/pico_button_led/test.sapp --allow-all
pblang lint examples/pico_button_led/model.sapp
```
