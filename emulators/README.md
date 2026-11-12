# PBLang Deterministic Hardware Emulators & Bus Manager (`emulators/`)

This directory contains **6 deterministic register-level hardware simulation models** and the **8-target hardware bus manager** written in 100% Pure PBLang (`2.0.0`).

---

## Quick Run

```bash
# Run all 6 deterministic hardware emulator models:
pblang run tools/pblang_stage3_selfhost_driver.sapp --allow-all -- emulators

# Run physical hardware bus discovery & simulation verification:
pblang run tools/pblang_stage3_selfhost_driver.sapp --allow-all -- discover
```

---

## Modules

| File | Description |
| :--- | :--- |
| [hardware_emulators_suite.sapp](hardware_emulators_suite.sapp) | Deterministic register-level simulation models (`mode: "in_vm_simulation"`) for **Arduino Uno R4 WiFi** (`RA4M1` + `12x8 LED Matrix`), **ESP32-D Dual-Core**, **Bosch BME688** (`gas/pressure/humidity/temperature`), **3D Printer / CNC G-Code** (`Bresenham stepper + PID thermal runaway guard`), **NFC ISO-14443A + BLE**, and **Stylus / TouchBoard / Mobile**. |
| [physical_hardware_manager.sapp](physical_hardware_manager.sapp) | Linux bus inspection (`/dev/ttyACM*`, `/dev/ttyUSB*`, `sysfs` USB VID:PID, `i2c-dev`, `spidev`, `gpiochip`, `bluetooth`, `input/event*`), flashing workflow descriptors (`BOSSA`/`esptool`), fault recovery, and emergency stop (`M112`). |
