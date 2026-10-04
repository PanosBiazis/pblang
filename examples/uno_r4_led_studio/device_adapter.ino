// Thin temporary Arduino SDK boundary. Protocol and playback live in device_firmware.sapp.
#include <Arduino.h>
#if !defined(ARDUINO_UNOR4_WIFI)
#error "Requires Arduino UNO R4 WiFi (arduino:renesas_uno:unor4wifi)."
#endif
#include <Arduino_LED_Matrix.h>
static ArduinoLEDMatrix matrix;
static double cells[640];
static double pb_get(double index) { return (index >= 0 && index < 640 && index == (int)index) ? cells[(int)index] : 0; }
static double pb_set(double index, double value) { if (index >= 0 && index < 640 && index == (int)index) cells[(int)index] = value; return 0; }
static double pb_millis() { return (double)(uint32_t)millis(); }
static double pb_serial_begin() { Serial.begin(115200); return 0; }
static double pb_available() { return Serial.available(); }
static double pb_read() { return Serial.read(); }
static double pb_writable() {
#if defined(NO_USB)
  // UNO R4 WiFi uses UART1 through its ESP USB bridge. UART inherits Print's
  // always-zero availableForWrite(); its single-byte write is interrupt paced.
  return 1;
#else
  return Serial.availableForWrite();
#endif
}
static double pb_put(double value) { return Serial.write((uint8_t)value); }
static double pb_begin() { return matrix.begin() ? 1 : 0; }
static double pb_clear() { matrix.clear(); return 0; }
static double pb_render(double a, double b, double c) { const uint32_t words[3] = {(uint32_t)a, (uint32_t)b, (uint32_t)c}; matrix.loadFrame(words); return 0; }
#include "pblang_firmware.h"
void setup() { pb_setup(); }
void loop() { pb_loop(); }
