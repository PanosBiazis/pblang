# PBLang Cycle-Accurate Hardware Emulators & Physical Bus Manager (`emulators/`)

This directory contains the **6 cycle-accurate register-level hardware emulators** and the **8-target physical hardware auto-discovery & bus manager** written in 100% Pure PBLang.

---

## Quick Run

```bash
# Run all 6 cycle-accurate hardware emulators:
pblang run tools/pblang_stage3_selfhost_driver.sapp --allow-all emulators

# Run physical hardware bus discovery & dual-mode verification:
pblang run tools/pblang_stage3_selfhost_driver.sapp --allow-all discover
```

---

## Modules

| File | Description |
| :--- | :--- |
| [hardware_emulators_suite.sapp](hardware_emulators_suite.sapp) | Cycle-accurate register-level emulators for **Arduino Uno R4 WiFi** (`RA4M1 48MHz` + `12x8 LED Matrix`), **ESP32-D Dual-Core** (`Xtensa LX6 240MHz`), **Bosch BME688** (`10-step gas/pressure/humidity/temperature`), **3D Printer / CNC G-Code** (`Bresenham stepper + PID thermal runaway guard`), **NFC ISO-14443A + BLE M2543T1**, and **Stylus / TouchBoard / Mobile**. |
| [physical_hardware_manager.sapp](physical_hardware_manager.sapp) | Physical bus auto-discovery (`/dev/ttyACM*`, `/dev/ttyUSB*`, `sysfs` USB VID:PID, `i2c-dev`, `spidev`, `gpiochip`, `bluetooth`, `input/event*`), firmware flashing (`BOSSA`/`esptool`), hot-unplug fault recovery, and hardware `E-STOP` (`M112`). |
