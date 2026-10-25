#include <Arduino.h>
#include <Wire.h>
#include <hardware/clocks.h>
#include <hardware/gpio.h>
#include <hardware/pio.h>
#include <pico/unique_id.h>

// Standalone, fixed-wiring diagnostic for the Waveshare RP2040-Zero Meep.
// The Arduino-Pico USB implementation retains its 1200-baud reset behavior.
static_assert(PIN_WIRE0_SDA == 4 && PIN_WIRE0_SCL == 5,
              "Build for the documented RP2040-Zero display wiring");
static_assert(PIN_NEOPIXEL == 16, "Build for the RP2040-Zero onboard RGB LED");
static constexpr uint8_t SDA_PIN = 4;
static constexpr uint8_t SCL_PIN = 5;
static uint8_t displayAddress = 0;
static bool ack3c = false;
static bool ack3d = false;
static bool writeOk = false;
static int sdaIdle = 1;
static int sclIdle = 1;
static const char *phase = "starting";
static uint8_t nextPhase = 0;
static uint32_t phaseDue = 0;
static uint32_t lastReport = 0;
static char boardId[PICO_UNIQUE_BOARD_ID_SIZE_BYTES * 2 + 1];
static int rgbSm = -1;

// Four-instruction WS2812 timing, using the SDK's PIO instruction encoders.
// 10 PIO clocks per bit: LOW 4 clocks, HIGH 3 or 6 clocks, total 1.25 us.
static void rgbBegin() {
  static const uint16_t code[] = {
    static_cast<uint16_t>(pio_encode_out(pio_x, 1) | pio_encode_sideset(1, 0) | pio_encode_delay(3)),
    static_cast<uint16_t>(pio_encode_jmp_not_x(3) | pio_encode_sideset(1, 1) | pio_encode_delay(2)),
    static_cast<uint16_t>(pio_encode_jmp(0) | pio_encode_sideset(1, 1) | pio_encode_delay(2)),
    static_cast<uint16_t>(pio_encode_nop() | pio_encode_sideset(1, 0) | pio_encode_delay(2)),
  };
  static const pio_program program = {code, 4, -1, 0};
  if (!pio_can_add_program(pio0, &program)) return;
  rgbSm = pio_claim_unused_sm(pio0, false);
  if (rgbSm < 0) return;
  const uint offset = pio_add_program(pio0, &program);
  pio_gpio_init(pio0, PIN_NEOPIXEL);
  pio_sm_set_consecutive_pindirs(pio0, rgbSm, PIN_NEOPIXEL, 1, true);
  pio_sm_config config = pio_get_default_sm_config();
  sm_config_set_wrap(&config, offset, offset + 3);
  sm_config_set_sideset(&config, 1, false, false);
  sm_config_set_sideset_pins(&config, PIN_NEOPIXEL);
  sm_config_set_out_shift(&config, false, true, 24);
  sm_config_set_fifo_join(&config, PIO_FIFO_JOIN_TX);
  sm_config_set_clkdiv(&config, clock_get_hz(clk_sys) / 8000000.0f);
  pio_sm_init(pio0, rgbSm, offset, &config);
  pio_sm_set_enabled(pio0, rgbSm, true);
}

static void rgbSet(uint8_t red, uint8_t green, uint8_t blue) {
  if (rgbSm < 0 || pio_sm_is_tx_fifo_full(pio0, rgbSm)) return;
  const uint32_t grb = (uint32_t(green) << 24) | (uint32_t(red) << 16) | (uint32_t(blue) << 8);
  pio_sm_put(pio0, rgbSm, grb);
}

static bool probe(uint8_t address) {
  // Arduino-Pico's zero-length probe has bounded clock-stretch waits.
  Wire.beginTransmission(address);
  return Wire.endTransmission() == 0;
}

static bool commands(const uint8_t *bytes, size_t length) {
  Wire.beginTransmission(displayAddress);
  Wire.write(uint8_t(0x00));
  Wire.write(bytes, length);
  return Wire.endTransmission() == 0;
}

static bool allOn(bool sh1106) {
  const uint8_t common[] = {
    0xAE, 0xD5, 0x80, 0xA8, 0x3F, 0xD3, 0x00, 0x40,
    0xA1, 0xC8, 0xDA, 0x12, 0x81, 0x20, 0xD9, 0x22,
    0xDB, 0x20, 0xA6
  };
  const uint8_t ssd1306[] = {0x8D, 0x14, 0x20, 0x02};
  const uint8_t sh1106Commands[] = {0xAD, 0x8B};
  const uint8_t enable[] = {0xA5, 0xAF};
  if (!commands(common, sizeof(common))) return false;
  if (sh1106) {
    if (!commands(sh1106Commands, sizeof(sh1106Commands))) return false;
  } else if (!commands(ssd1306, sizeof(ssd1306))) {
    return false;
  }
  return commands(enable, sizeof(enable));
}

static bool checker(bool sh1106) {
  // Page addressing works on both controllers; SH1106 typically has a 2-column offset.
  for (uint8_t page = 0; page < 8; ++page) {
    const uint8_t position[] = {uint8_t(0xB0 | page), uint8_t(sh1106 ? 0x02 : 0x00), 0x10};
    if (!commands(position, sizeof(position))) return false;
    for (uint8_t column = 0; column < 128; column += 16) {
      uint8_t pixels[16];
      for (uint8_t i = 0; i < sizeof(pixels); ++i) {
        pixels[i] = (((column + i) / 8 + page) & 1) ? 0xFF : 0x00;
      }
      Wire.beginTransmission(displayAddress);
      Wire.write(uint8_t(0x40));
      Wire.write(pixels, sizeof(pixels));
      if (Wire.endTransmission() != 0) return false;
      yield();
    }
  }
  const uint8_t ram[] = {0xA4};
  return commands(ram, sizeof(ram));
}

static void sampleBus() {
  sdaIdle = gpio_get(SDA_PIN) ? 1 : 0;
  sclIdle = gpio_get(SCL_PIN) ? 1 : 0;
  ack3c = probe(0x3C);
  ack3d = probe(0x3D);
  const uint8_t found = ack3c ? 0x3C : (ack3d ? 0x3D : 0);
  if (found != displayAddress) {
    displayAddress = found;
    nextPhase = 0;
    phaseDue = millis();
    writeOk = false;
  }
  if (!displayAddress) phase = "no_ack";
}

static void advanceDisplay() {
  if (!displayAddress || int32_t(millis() - phaseDue) < 0) return;
  const bool sh1106 = nextPhase >= 2;
  const bool fullOn = (nextPhase & 1) == 0;
  writeOk = fullOn ? allOn(sh1106) : checker(sh1106);
  if (!writeOk) {
    phase = "write_error";
    nextPhase = 0;
    phaseDue = millis() + 2000;
    return;
  }
  phase = sh1106 ? (fullOn ? "sh1106_all_on" : "sh1106_checker")
                 : (fullOn ? "ssd1306_all_on" : "ssd1306_checker");
  nextPhase = (nextPhase + 1) % 4;
  phaseDue = millis() + (fullOn ? 2000 : 6000);
}

static void report() {
  char address[8];
  if (displayAddress) snprintf(address, sizeof(address), "%u", unsigned(displayAddress));
  else snprintf(address, sizeof(address), "null");
  char line[384];
  const int length = snprintf(line, sizeof(line),
    "{\"kind\":\"droidfun_display_test\",\"version\":1,\"boardId\":\"%s\",\"uptime_ms\":%lu,"
    "\"sda\":%d,\"scl\":%d,\"ack_3c\":%s,\"ack_3d\":%s,\"address\":%s,"
    "\"phase\":\"%s\",\"write_ok\":%s,\"rgb_ready\":%s}\n",
    boardId, static_cast<unsigned long>(millis()), sdaIdle, sclIdle,
    ack3c ? "true" : "false", ack3d ? "true" : "false", address, phase,
    writeOk ? "true" : "false", rgbSm >= 0 ? "true" : "false");
  // Keep the display and USB-reset responsive if no host consumes diagnostics.
  if (Serial && length > 0 && length < int(sizeof(line)) && Serial.availableForWrite() >= length) {
    Serial.write(reinterpret_cast<const uint8_t *>(line), length);
  }
}

void setup() {
  Serial.begin(115200);
  pico_get_unique_board_id_string(boardId, sizeof(boardId));
  rgbBegin();
  rgbSet(0, 0, 12);
  Wire.setSDA(SDA_PIN);
  Wire.setSCL(SCL_PIN);
  Wire.setClock(100000);
  Wire.setTimeout(20, false);
  Wire.begin();
  delay(250); // Let a USB-powered panel settle, without waiting for a host.
  sampleBus();
  advanceDisplay();
  report();
  lastReport = millis();
}

void loop() {
  // Ignore the normal application's binary packets; this test is self-contained.
  for (int n = 0; n < 128 && Serial.available(); ++n) Serial.read();
  advanceDisplay();
  if (uint32_t(millis() - lastReport) >= 1000) {
    lastReport = millis();
    sampleBus();
    advanceDisplay();
    report();
    // Subtle 1 Hz color pulse: green after successful writes, red with no reply.
    const uint8_t level = ((lastReport / 1000) & 1) ? 12 : 3;
    rgbSet(writeOk ? 0 : level, writeOk ? level : 0, 0);
  }
  delay(5);
}
