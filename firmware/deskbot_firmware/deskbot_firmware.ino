#include <Arduino.h>
#include <hardware/gpio.h>
#include <hardware/i2c.h>
#include <hardware/watchdog.h>
#include <pico/unique_id.h>
#include <tusb.h>
#include "fallback_face.h"
#include "standalone_storage.h"

// Shared Meep / Claw SSD1306 wiring: GP4=SDA, GP5=SCL.
static constexpr uint SDA_PIN = 4;
static constexpr uint SCL_PIN = 5;
static uint8_t oled_address = 0x3C;
static bool oled_fast = true;
static unsigned oledMaxFps() { return oled_fast ? 30 : 5; }
static void oledSlowBus() {
  oled_fast = false;
  i2c_set_baudrate(i2c0, 100000);
}
static constexpr size_t FRAME_SIZE = 1024;
static constexpr size_t MAX_PACKET = 1044;
static constexpr size_t RX_STORAGE = 1100;

static constexpr uint8_t HELLO = 0x01, HELLO_ACK = 0x02, FRAME = 0x10,
                         MSG_DISPLAY = 0x11, FALLBACK = 0x12, PING = 0x20,
                         ACK = 0x21, ERROR_PACKET = 0x22, SAVED_BEGIN = 0x30,
                         SAVED_CHUNK = 0x31, SAVED_COMMIT = 0x32;

static uint8_t rx[RX_STORAGE];
static uint8_t decoded[MAX_PACKET];
static uint8_t transmit[MAX_PACKET + 8];
static uint8_t screen[FRAME_SIZE];
static uint16_t rx_length = 0;
static bool rx_discard = false;
static uint32_t rx_started = 0;
static uint32_t session = 0, last_sequence = 0, last_request_crc = 0;
static uint8_t last_reply_kind = 0;
static char last_reply[320] = {};
static char board_id[2 * PICO_UNIQUE_BOARD_ID_SIZE_BYTES + 1] = {};
static uint32_t last_host_command = 0;
static bool manual_off = false, oled_ready = false, fallback_shown = false;
static uint8_t contrast = 216;
static bool usb_suspended = false;
static bool standalone_active = false;
static uint8_t standalone_index = 0;
static uint32_t standalone_started = 0;

static uint16_t le16(const uint8_t *p) { return (uint16_t)p[0] | ((uint16_t)p[1] << 8); }
static uint32_t le32(const uint8_t *p) {
  return (uint32_t)p[0] | ((uint32_t)p[1] << 8) | ((uint32_t)p[2] << 16) | ((uint32_t)p[3] << 24);
}
static void put16(uint8_t *p, uint16_t n) { p[0] = n & 255; p[1] = (n >> 8) & 255; }
static void put32(uint8_t *p, uint32_t n) {
  p[0] = n & 255; p[1] = (n >> 8) & 255; p[2] = (n >> 16) & 255; p[3] = (n >> 24) & 255;
}

static uint32_t crc32(const uint8_t *data, size_t size) {
  uint32_t crc = 0xffffffffu;
  for (size_t i = 0; i < size; ++i) {
    crc ^= data[i];
    for (int bit = 0; bit < 8; ++bit) crc = (crc >> 1) ^ ((crc & 1) ? 0xedb88320u : 0);
  }
  return ~crc;
}

static size_t cobsDecode(const uint8_t *src, size_t length, uint8_t *dst) {
  size_t read = 0, written = 0;
  while (read < length) {
    uint8_t code = src[read++];
    if (code == 0) return 0;
    const size_t copied = code - 1;
    if (copied > length - read) return 0;
    const bool separator = code != 255 && copied < length - read;
    // A non-final block also emits an implicit zero. Reserve room for that
    // byte before writing; otherwise an oversized block can overwrite board_id.
    if (copied + size_t(separator) > MAX_PACKET - written) return 0;
    for (uint8_t i = 1; i < code; ++i) dst[written++] = src[read++];
    if (separator) dst[written++] = 0;
  }
  return written;
}

static size_t cobsEncode(const uint8_t *src, size_t length, uint8_t *dst) {
  size_t read = 0, written = 1, code_index = 0;
  uint8_t code = 1;
  while (read < length) {
    uint8_t value = src[read++];
    if (value == 0) {
      dst[code_index] = code;
      code_index = written++;
      code = 1;
    } else {
      dst[written++] = value;
      if (++code == 255) {
        dst[code_index] = code;
        code_index = written++;
        code = 1;
      }
    }
  }
  dst[code_index] = code;
  return written;
}

static void sendPacket(uint8_t type, uint32_t id, uint32_t sequence, const char *message) {
  size_t length = strnlen(message, 319);
  uint8_t body[16 + 320 + 4];
  body[0] = 'D'; body[1] = 'B'; body[2] = 1; body[3] = type;
  put32(body + 4, id); put32(body + 8, sequence);
  put16(body + 12, length); put16(body + 14, 0);
  memcpy(body + 16, message, length);
  put32(body + 16 + length, crc32(body, 16 + length));
  size_t bytes = cobsEncode(body, 20 + length, transmit);
  transmit[bytes++] = 0;
  if (Serial.dtr()) Serial.write(transmit, bytes);
}

static void reply(uint8_t type, uint32_t sequence, uint32_t request_crc, const char *message) {
  last_sequence = sequence;
  last_request_crc = request_crc;
  last_reply_kind = type;
  strncpy(last_reply, message, sizeof(last_reply) - 1);
  last_reply[sizeof(last_reply) - 1] = 0;
  sendPacket(type, session, sequence, last_reply);
}

static bool writeI2c(const uint8_t *data, size_t size) {
  return i2c_write_timeout_us(i2c0, oled_address, data, size, false, 25000) == (int)size;
}

static bool oledCommands(const uint8_t *commands, size_t size) {
  if (size > 30) return false;
  uint8_t bytes[31];
  bytes[0] = 0x00;
  memcpy(bytes + 1, commands, size);
  return writeI2c(bytes, size + 1);
}

static bool oledPower(bool on) {
  const uint8_t commands[] = {static_cast<uint8_t>(on ? 0xaf : 0xae)};
  return oledCommands(commands, sizeof(commands));
}

static bool oledContrast(uint8_t value) {
  const uint8_t commands[] = {0x81, value};
  return oledCommands(commands, sizeof(commands));
}

static bool oledFrameOnce(const uint8_t *bitmap) {
  const uint8_t mode[] = {0x20, 0x02};
  if (!oledCommands(mode, sizeof(mode))) return false;
  uint8_t chunk[17];
  chunk[0] = 0x40;
  for (uint8_t page = 0; page < 8; ++page) {
    const uint8_t position[] = {static_cast<uint8_t>(0xb0 | page), 0x00, 0x10};
    if (!oledCommands(position, sizeof(position))) return false;
    for (size_t column = 0; column < 128; column += 16) {
      memcpy(chunk + 1, bitmap + page * 128 + column, 16);
      if (!writeI2c(chunk, sizeof(chunk))) return false;
      watchdog_update();
    }
  }
  return true;
}

static bool oledFrame(const uint8_t *bitmap) {
  if (oledFrameOnce(bitmap)) return true;
  if (!oled_fast) return false;
  // A partially written frame must restart at page zero on the slower bus.
  oledSlowBus();
  return oledFrameOnce(bitmap);
}

static bool oledInitOnce() {
  const uint8_t commands[] = {
    0xae, 0xd5, 0x80, 0xa8, 0x3f, 0xd3, 0x00, 0x40,
    0x8d, 0x14, 0x20, 0x02, 0xa1, 0xc8, 0xda, 0x12,
    0x81, 0xd8, 0xd9, 0xf1, 0xdb, 0x40, 0xa4, 0xa6, 0x2e, 0xaf
  };
  // Standard SSD1306 modules use either address. Keep the commissioned GPIO
  // wiring; do not probe unrelated pins or assume an ACK proves visible pixels.
  for (uint8_t address : {uint8_t(0x3C), uint8_t(0x3D)}) {
    oled_address = address;
    if (oledCommands(commands, sizeof(commands))) return true;
  }
  return false;
}

static bool oledInit() {
  if (oledInitOnce()) return true;
  if (!oled_fast) return false;
  oledSlowBus();
  return oledInitOnce();
}

static void pixel(int x, int y) {
  if (x >= 0 && x < 128 && y >= 0 && y < 64) screen[(y / 8) * 128 + x] |= 1u << (y % 8);
}

static void makeFace() { memcpy(screen, FALLBACK_FACE, FRAME_SIZE); }

static void showFallback() {
  if (manual_off || !oled_ready) return;
  makeFace();
  oled_ready = oledFrame(screen);
  fallback_shown = true;
}

// A saved playlist owns the display only while there is no desktop session.
// USB wall chargers need neither USB enumeration nor an open serial port.
static void playStandalone() {
  using namespace DroidSaved;
  if (!saved_length) return;
  uint32_t now = millis();
  bool changed = false;
  if (!standalone_active) {
    standalone_active = true; standalone_index = 0; standalone_started = now;
    manual_off = false;
    contrast = (uint16_t(saved[6]) * 255 + 50) / 100;
    uint16_t orientation = read16(saved + 8);
    const uint8_t direction[] = {static_cast<uint8_t>(orientation ? 0xa0 : 0xa1),
                                static_cast<uint8_t>(orientation ? 0xc0 : 0xc8)};
    if (!oled_ready) oled_ready = oledInit();
    oled_ready = oledCommands(direction, sizeof(direction)) && oledContrast(contrast) && oledPower(true) && oled_ready;
    changed = true;
  } else if (now - standalone_started >= durationMs(saved, standalone_index)) {
    standalone_index = (standalone_index + 1) % saved[5];
    standalone_started = now; changed = true;
  }
  if (changed) {
    memcpy(screen, frame(saved, standalone_index), FRAME_SIZE);
    oled_ready = oledFrame(screen);
  }
}

static bool jsonBool(const char *json, const char *name, bool *result) {
  const char *value = strstr(json, name);
  if (!value) return false;
  value = strchr(value, ':');
  if (!value) return false;
  ++value;
  while (*value == ' ') ++value;
  if (strncmp(value, "true", 4) == 0) { *result = true; return true; }
  if (strncmp(value, "false", 5) == 0) { *result = false; return true; }
  return false;
}

static bool jsonNumber(const char *json, const char *name, int *result) {
  const char *value = strstr(json, name);
  if (!value) return false;
  value = strchr(value, ':');
  if (!value) return false;
  char *end = nullptr;
  long parsed = strtol(value + 1, &end, 10);
  if (end == value + 1 || parsed < 0 || parsed > 255) return false;
  *result = parsed;
  return true;
}

static void processPacket(const uint8_t *packet, size_t size) {
  if (size < 20 || packet[0] != 'D' || packet[1] != 'B' || packet[2] != 1) return;
  uint8_t type = packet[3];
  uint32_t id = le32(packet + 4), sequence = le32(packet + 8);
  uint16_t length = le16(packet + 12), flags = le16(packet + 14);
  if (length > 1024 || size != 20u + length || flags != 0) return;
  uint32_t supplied_crc = le32(packet + 16 + length);
  if (supplied_crc != crc32(packet, 16 + length)) return;
  if (type == HELLO && id != 0 && sequence != 0 && length <= 160) {
    session = id; last_sequence = 0; last_request_crc = 0;
    standalone_active = false; DroidSaved::upload.cancel();
    last_host_command = millis(); fallback_shown = false;
    char response[320];
    snprintf(response, sizeof(response),
      "{\"firmware\":\"1.2.4\",\"boardId\":\"%s\",\"width\":128,\"height\":64,\"profile\":\"GP4-GP5\",\"maxFps\":%u,\"displayHealth\":\"%s\",\"standaloneSupported\":true,\"standaloneSaved\":%s,\"standaloneCount\":%u}",
      board_id, oledMaxFps(), oled_ready ? "ready" : "i2c_error", DroidSaved::saved_length ? "true" : "false", DroidSaved::saved_length ? DroidSaved::saved[5] : 0);
    reply(HELLO_ACK, sequence, supplied_crc, response);
    return;
  }
  if (id != session || session == 0 || sequence == 0) return;
  if (sequence == last_sequence) {
    if (supplied_crc == last_request_crc) sendPacket(last_reply_kind, session, sequence, last_reply);
    return;
  }
  if (sequence <= last_sequence) return;
  bool was_standalone = standalone_active;
  standalone_active = false;
  last_host_command = millis();
  const uint8_t *payload = packet + 16;
  if (type == SAVED_BEGIN) {
    if (length != 6 || !DroidSaved::upload.begin(le16(payload), le32(payload + 2))) {
      reply(ERROR_PACKET, sequence, supplied_crc, "{\"code\":\"invalid_saved_size\"}"); return;
    }
    reply(ACK, sequence, supplied_crc, "{\"receiving\":true}"); return;
  }
  if (type == SAVED_CHUNK) {
    if (length < 3 || !DroidSaved::upload.append(le16(payload), payload + 2, length - 2)) {
      reply(ERROR_PACKET, sequence, supplied_crc, "{\"code\":\"invalid_saved_chunk\"}"); return;
    }
    reply(ACK, sequence, supplied_crc, "{\"receiving\":true}"); return;
  }
  if (type == SAVED_COMMIT) {
    uint32_t uploaded_crc = DroidSaved::upload.crc;
    if (length || !DroidSaved::commit()) {
      DroidSaved::upload.cancel();
      reply(ERROR_PACKET, sequence, supplied_crc, "{\"code\":\"saved_verification_or_storage_failed\"}"); return;
    }
    char response[120];
    snprintf(response, sizeof(response), "{\"saved\":true,\"count\":%u,\"playlistCrc\":%lu}",
      DroidSaved::saved[5], static_cast<unsigned long>(uploaded_crc));
    reply(ACK, sequence, supplied_crc, response); return;
  }
  if (type == FRAME) {
    if (length != FRAME_SIZE) { reply(ERROR_PACKET, sequence, supplied_crc, "{\"code\":\"frame_size\"}"); return; }
    if (manual_off) { reply(ACK, sequence, supplied_crc, "{\"displayHealth\":\"off\"}"); return; }
    oled_ready = oledFrame(payload);
    if (!oled_ready) { reply(ERROR_PACKET, sequence, supplied_crc, "{\"code\":\"i2c_error\"}"); return; }
    memcpy(screen, payload, FRAME_SIZE);
    fallback_shown = false;
    uint32_t frame_crc = crc32(payload, FRAME_SIZE);
    uint16_t nonzero = 0;
    for (size_t i = 0; i < FRAME_SIZE; ++i) if (payload[i]) ++nonzero;
    char response[100];
    snprintf(response, sizeof(response), "{\"displayHealth\":\"ready\",\"frameCrc\":%lu,\"nonzeroBytes\":%u,\"maxFps\":%u}",
             static_cast<unsigned long>(frame_crc), nonzero, oledMaxFps());
    reply(ACK, sequence, supplied_crc, response);
    return;
  }
  if (type == MSG_DISPLAY && length > 0 && length < 160) {
    char json[160]; memcpy(json, payload, length); json[length] = 0;
    bool on; int level, orientation;
    if (!jsonBool(json, "\"on\"", &on) || !jsonNumber(json, "\"contrast\"", &level)
        || !jsonNumber(json, "\"orientation\"", &orientation) || (orientation != 0 && orientation != 180)) {
      reply(ERROR_PACKET, sequence, supplied_crc, "{\"code\":\"invalid_display\"}"); return;
    }
    if (!oled_ready) oled_ready = oledInit();
    manual_off = !on; contrast = level;
    const uint8_t direction[] = {static_cast<uint8_t>(orientation ? 0xa0 : 0xa1),
                                 static_cast<uint8_t>(orientation ? 0xc0 : 0xc8)};
    oled_ready = oledCommands(direction, sizeof(direction)) && oledContrast(contrast) && oledPower(on);
    if (!oled_ready) { reply(ERROR_PACKET, sequence, supplied_crc, "{\"code\":\"i2c_error\"}"); return; }
    if (on) { oled_ready = oledFrame(screen); }
    reply(ACK, sequence, supplied_crc, oled_ready ? "{\"displayHealth\":\"ready\"}" : "{\"displayHealth\":\"i2c_error\"}");
    return;
  }
  if (type == FALLBACK) {
    showFallback();
    reply(ACK, sequence, supplied_crc, oled_ready ? "{\"displayHealth\":\"ready\"}" : "{\"displayHealth\":\"i2c_error\"}");
    return;
  }
  if (type == PING) {
    reply(ACK, sequence, supplied_crc, !oled_ready ? "{\"displayHealth\":\"i2c_error\"}" : (was_standalone || fallback_shown)
      ? "{\"displayHealth\":\"ready\",\"hostFrameNeeded\":true}" : "{\"displayHealth\":\"ready\"}");
    return;
  }
  reply(ERROR_PACKET, sequence, supplied_crc, "{\"code\":\"unknown_command\"}");
}

void setup() {
  Serial.begin(115200);
  delay(250);
  pico_get_unique_board_id_string(board_id, sizeof(board_id));
  i2c_init(i2c0, 400000);
  gpio_set_function(SDA_PIN, GPIO_FUNC_I2C);
  gpio_set_function(SCL_PIN, GPIO_FUNC_I2C);
  gpio_pull_up(SDA_PIN); gpio_pull_up(SCL_PIN);
  watchdog_enable(8000, true);
  oled_ready = oledInit();
  DroidSaved::load();
  if (DroidSaved::saved_length) playStandalone(); else showFallback();
  last_host_command = millis();
}

void loop() {
  watchdog_update();
  bool suspended = tud_suspended();
  if (suspended && !usb_suspended) {
    // Sleep ends live desktop ownership, not screen power. Keep the same
    // session so a resumed host can recover without a USB re-enumeration.
    usb_suspended = true;
    rx_length = 0; rx_discard = false;
    DroidSaved::upload.cancel();
    if (!DroidSaved::saved_length) showFallback();
  } else if (!suspended && usb_suspended) {
    usb_suspended = false;
  }
  if (suspended) {
    if (DroidSaved::saved_length) playStandalone();
    delay(1); return;
  }
  uint32_t now = millis();
  if (rx_length && now - rx_started > 500) { rx_length = 0; rx_discard = false; }
  while (Serial.available()) {
    int value = Serial.read();
    if (value < 0) break;
    if (value == 0) {
      if (!rx_discard && rx_length > 0) {
        size_t size = cobsDecode(rx, rx_length, decoded);
        if (size > 0) processPacket(decoded, size);
      }
      rx_length = 0; rx_discard = false;
    } else if (!rx_discard) {
      if (!rx_length) rx_started = now;
      if (rx_length < RX_STORAGE - 1) rx[rx_length++] = value;
      else { rx_length = 0; rx_discard = true; }
    }
  }
  // Read the clock after processing packets to avoid unsigned underflow.
  uint32_t idle = millis() - last_host_command;
  if (DroidSaved::saved_length && (!Serial.dtr() || idle >= 5000)) {
    playStandalone(); delay(1); return;
  }
  // A charger, closed app, sleeping host or stalled connection all get a
  // visible face. There is no idle dim/off timer. Explicit screen-off remains
  // authoritative until the host turns it back on (or the board reboots).
  if ((!Serial.dtr() || idle >= 5000) && !fallback_shown && !manual_off) showFallback();
  delay(1);
}
