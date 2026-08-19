# Complete Verified Circuit

This document records the complete wired circuit used by the Embedded Control
Center example with one Arduino UNO, one classic ESP32 DevKitC V4, one shared
10 kOhm potentiometer, two pushbuttons, and the four application-controlled
LEDs. A fifth observed LED on ESP32 `GPIO0` is documented separately because it
is not part of the application circuit.

## Safety first

1. Disconnect both USB cables before changing a wire.
2. Never connect the UNO `5V` rail to ESP32 `3V3` or to any ESP32 GPIO.
3. Join the boards only through a common `GND` and the bounded signal paths
   shown below.
4. Power the shared potentiometer from ESP32 `3V3`, never from UNO `5V`.
5. Keep ESP32 `GPIO6` through `GPIO11` unused; they are connected to flash.
6. Do not use ESP32 `GPIO0`, `GPIO2`, `GPIO5`, `GPIO12`, or `GPIO15` for this
   example. Their boot-strapping levels can prevent a normal startup.
7. Keep ESP32 `GPIO1` and `GPIO3` free for the controller UART.
8. Every external LED requires its own series resistor.

## Parts

| Quantity | Part | Recommended value |
|---:|---|---|
| 1 | Arduino UNO / compatible ATmega328P board | 5 V logic |
| 1 | ESP32 DevKitC V4 / ESP32-WROOM-32D | 3.3 V logic |
| 1 | Linear potentiometer | 10 kOhm |
| 2 | Momentary pushbuttons | Normally open |
| 4 | Application LEDs | Any ordinary low-current LED |
| 4 | LED series resistors | 330 Ohm each |
| 2 | Loopback series resistors | 1 kOhm each |
| 2 | ADC branch resistors | 1 kOhm each |
| 4 | Input pull-down resistors | 10 kOhm each |
| 1 | Breadboard and jumper wires | As required |

Values in the range `220..470 Ohm` are acceptable for the UNO LEDs. Use at
least `330 Ohm` for the ESP32 LEDs. The diagrams below use `330 Ohm` consistently.

## Complete topology

```text
Laptop USB                         Laptop USB
    |                                  |
    v                                  v
Arduino UNO                        ESP32 DevKitC V4
    |                                  |
    +-------------- GND ---------------+
                     |
             common breadboard GND

UNO D7     -- 330R --|>|-- GND       ESP GPIO23 -- 330R --|>|-- GND
main LED                              main LED

UNO D6 ----+-- 330R --|>|-- GND       ESP GPIO25 --+-- 330R --|>|-- GND
            | analog LED                              | analog LED
            +-- 1k -- UNO D2                          +-- 1k -- ESP GPIO33
                         |                                          |
                        10k                                        10k
                         |                                          |
                        GND                                        GND

UNO 5V -- button -- UNO D4          ESP 3V3 -- button -- ESP GPIO27
                       |                                      |
                      10k                                    10k
                       |                                      |
                      GND                                    GND

                         shared 10k potentiometer
                    +------------------------------+
ESP 3V3 ------------| outer                    outer|------------ GND
                    +-------------+----------------+
                                  |
                               wiper
                                  |
                        +---------+---------+
                        |                   |
                       1k                  1k
                        |                   |
                     UNO A0          ESP GPIO34
```

`|>|` denotes an LED. Connect the resistor to the LED anode, normally the
longer leg. Connect the cathode, normally the shorter leg and flat side, to
`GND`.

## Arduino UNO wiring

| Function | Wiring | Firmware behavior |
|---|---|---|
| Main LED | `D7 -> 330 Ohm -> LED -> GND` | On, off, toggle and blink |
| Analog LED | `D6 -> 330 Ohm -> LED -> GND` | 8-bit hardware PWM following `A0` |
| Loopback | `D6 -> 1 kOhm -> D2`, plus `D2 -> 10 kOhm -> GND` | Verifies LOW and HIGH read-back |
| Button | `5V -> button -> D4`, plus `D4 -> 10 kOhm -> GND` | Active-high, debounced toggle |
| ADC | Potentiometer wiper `-> 1 kOhm -> A0` | 10-bit sample, `0..1023` |

The UNO analog LED uses Timer0 `OC0A` on `D6` at approximately 976 Hz. Because
the shared potentiometer has a 3.3 V endpoint while the UNO ADC reference is
5 V `AVcc`, the physical maximum is expected near `651/1023`. The application
calibrates this endpoint to 100% brightness.

## ESP32 wiring

| Function | Wiring | Firmware behavior |
|---|---|---|
| Main LED | `GPIO23 -> 330 Ohm -> LED -> GND` | On, off, toggle and blink |
| Analog LED | `GPIO25 -> 330 Ohm -> LED -> GND` | 8-bit DAC following `GPIO34` |
| Loopback | `GPIO25 -> 1 kOhm -> GPIO33`, plus `GPIO33 -> 10 kOhm -> GND` | Verifies LOW and HIGH read-back |
| Button | `3V3 -> button -> GPIO27`, plus `GPIO27 -> 10 kOhm -> GND` | Active-high, debounced toggle |
| ADC | Potentiometer wiper `-> 1 kOhm -> GPIO34` | ADC1 channel 6, 12-bit sample |

`GPIO34` is input-only and has no internal pull-up or pull-down. It is suitable
for this ADC1 input and does not have the classic ESP32 ADC2/Wi-Fi conflict.
`GPIO25` alternates safely between DAC ownership and temporary digital
loopback ownership; the firmware restores analog-follow mode after the test.

## Shared potentiometer

Use one potentiometer for both boards:

```text
ESP32 3V3 -> first outer leg
GND       -> second outer leg
wiper     -> separate 1 kOhm resistor -> UNO A0
wiper     -> separate 1 kOhm resistor -> ESP32 GPIO34
```

If clockwise rotation decreases instead of increases the value, swap only the
two outer potentiometer legs. Do not change the wiper branches.

The UNO and ESP32 grounds must be connected whenever they share the
potentiometer signal. Their `5V` and `3V3` rails must remain separate.

## Pushbutton orientation

Many four-leg tactile buttons internally connect the two legs on each side.
Place the button across the breadboard center gap and use opposite sides:

```text
supply ----[ button ]---- input pin
                              |
                             10k
                              |
                             GND
```

Released must read LOW. Pressed must read HIGH. If it always reads HIGH or LOW,
rotate the button by 90 degrees or verify its paired legs with continuity mode
while USB power is disconnected.

## The fifth GPIO0 LED

The physical diagnostic run observed five illuminated LEDs: the four declared
application outputs above and an already-lit indicator connected to ESP32
`GPIO0`. PBLang deliberately did not drive `GPIO0`.

`GPIO0` controls the ESP32 boot mode. It is not part of this example and should
preferably be removed from the breadboard. If an existing board indicator or
external LED remains connected, it must use a series resistor and must not pull
`GPIO0` LOW during reset. Its state is not evidence that an application command
succeeded.

## Expected behavior

| Action | UNO | ESP32 |
|---|---|---|
| Turn main LED on/off | `D7` changes | `GPIO23` changes |
| Rotate potentiometer | `D6` LED brightness changes | `GPIO25` LED brightness changes |
| Press button once | Stops blink and toggles `D7` | Stops blink and toggles `GPIO23` |
| Run loopback | Tests `D6 -> D2` | Tests `GPIO25 -> GPIO33` |
| Read ADC | Reports normalized `A0` value | Reports normalized `GPIO34` value |

The analog LEDs follow the potentiometer autonomously after firmware boot. The
desktop, Web, or Android UI is needed only for monitoring and explicit commands.

## Power-on checklist

1. Disconnect both USB cables.
2. Confirm that no power rail is shorted to `GND`.
3. Confirm `UNO GND`, `ESP32 GND`, LED cathodes and all pull-downs share ground.
4. Confirm that UNO `5V` and ESP32 `3V3` are not connected together.
5. Set the potentiometer near its middle position.
6. Connect the UNO and ESP32 USB cables to the Linux host.
7. Discover the devices instead of assuming stable port numbers.
8. Select the exact board and detected port in Embedded Control Center.
9. Test `Read A/D`, each button, main LED on/off, and finally loopback.

The currently verified mapping is UNO on `/dev/ttyUSB0` and ESP32 on
`/dev/ttyUSB1`, but Linux can rename ports after reconnecting devices. These
names are test evidence, not hardcoded board identities.
