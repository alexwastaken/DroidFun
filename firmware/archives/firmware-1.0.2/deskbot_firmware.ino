#include <Arduino.h>
#include <hardware/gpio.h>
#include <hardware/i2c.h>
#include <hardware/watchdog.h>
#include <pico/unique_id.h>

// User-supplied wiring: GP4=SDA, GP5=SCL. Bus ACKs at 0x3C;
// visible output and the controller model are still being commissioned.
static constexpr uint SDA_PIN = 4;
static constexpr uint SCL_PIN = 5;
static constexpr uint8_t OLED_ADDRESS = 0x3C;
static constexpr size_t FRAME_SIZE = 1024;
static constexpr size_t MAX_PACKET = 1044;
static constexpr size_t RX_STORAGE = 1100;

static constexpr uint8_t HELLO = 0x01, HELLO_ACK = 0x02, FRAME = 0x10,
                         MSG_DISPLAY = 0x11, FALLBACK = 0x12, PING = 0x20,
                         ACK = 0x21, ERROR_PACKET = 0x22;

static uint8_t rx[RX_STORAGE];
static uint8_t decoded[MAX_PACKET];
static uint8_t transmit[MAX_PACKET + 8];
static uint8_t screen[FRAME_SIZE];
static uint16_t rx_length = 0;
static bool rx_discard = false;
static uint32_t rx_started = 0;
static uint32_t session = 0, last_sequence = 0, last_request_crc = 0;
static uint8_t last_reply_kind = 0;
static char last_reply[160] = {};
static char board_id[2 * PICO_UNIQUE_BOARD_ID_SIZE_BYTES + 1] = {};
static uint32_t last_host_command = 0;
static bool manual_off = false, oled_ready = false, fallback_shown = false;
static uint8_t idle_stage = 0;
static uint8_t contrast = 216;

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
    if (code == 0 || read + code - 1 > length || written + code > MAX_PACKET + 1) return 0;
    for (uint8_t i = 1; i < code; ++i) dst[written++] = src[read++];
    if (code != 255 && read < length) dst[written++] = 0;
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
  size_t length = strnlen(message, 159);
  uint8_t body[16 + 160 + 4];
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
  return i2c_write_timeout_us(i2c0, OLED_ADDRESS, data, size, false, 25000) == (int)size;
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

static bool oledFrame(const uint8_t *bitmap) {
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

static bool oledInit() {
  const uint8_t commands[] = {
    0xae, 0xd5, 0x80, 0xa8, 0x3f, 0xd3, 0x00, 0x40,
    0x8d, 0x14, 0x20, 0x02, 0xa1, 0xc8, 0xda, 0x12,
    0x81, 0xd8, 0xd9, 0xf1, 0xdb, 0x40, 0xa4, 0xa6, 0x2e, 0xaf
  };
  return oledCommands(commands, sizeof(commands));
}

static void pixel(int x, int y) {
  if (x >= 0 && x < 128 && y >= 0 && y < 64) screen[(y / 8) * 128 + x] |= 1u << (y % 8);
}

static void makeFace() {
  memset(screen, 0, sizeof(screen));
  const int centers[] = {38, 90};
  for (int center : centers) {
    for (int y = 12; y <= 37; ++y) for (int x = center - 9; x <= center + 9; ++x) {
      int dx = x - center, dy = y - 24;
      int distance = dx * dx * 144 + dy * dy * 64;
      if (distance >= 6000 && distance <= 9216) pixel(x, y);
    }
  }
  for (int x = 55; x <= 72; ++x) {
    int dx = x - 64;
    int y = 48 - (dx * dx * 5) / 81;
    pixel(x, y); pixel(x, y + 1);
  }
}

static void showFallback() {
  if (manual_off || !oled_ready) return;
  makeFace();
  oled_ready = oledFrame(screen);
  fallback_shown = true;
}

static void wakeDisplay() {
  if (!manual_off && idle_stage >= 2) {
    bool restored = oledContrast(contrast);
    if (idle_stage >= 3) restored = oledPower(true) && restored;
    oled_ready = restored && oled_ready;
  }
  idle_stage = 0;
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
    wakeDisplay();
    last_host_command = millis(); fallback_shown = false;
    char response[160];
    snprintf(response, sizeof(response),
      "{\"firmware\":\"1.0.2\",\"boardId\":\"%s\",\"width\":128,\"height\":64,\"profile\":\"GP4-GP5-0x3C\",\"maxFps\":8,\"displayHealth\":\"%s\"}",
      board_id, oled_ready ? "ready" : "i2c_error");
    reply(HELLO_ACK, sequence, supplied_crc, response);
    return;
  }
  if (id != session || session == 0 || sequence == 0) return;
  if (sequence == last_sequence) {
    if (supplied_crc == last_request_crc) sendPacket(last_reply_kind, session, sequence, last_reply);
    return;
  }
  if (sequence <= last_sequence) return;
  wakeDisplay();
  last_host_command = millis();
  const uint8_t *payload = packet + 16;
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
    snprintf(response, sizeof(response), "{\"displayHealth\":\"ready\",\"frameCrc\":%lu,\"nonzeroBytes\":%u}",
             static_cast<unsigned long>(frame_crc), nonzero);
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
    reply(ACK, sequence, supplied_crc, oled_ready ? "{\"displayHealth\":\"ready\"}" : "{\"displayHealth\":\"i2c_error\"}");
    return;
  }
  reply(ERROR_PACKET, sequence, supplied_crc, "{\"code\":\"unknown_command\"}");
}

void setup() {
  Serial.begin(115200);
  delay(250);
  pico_get_unique_board_id_string(board_id, sizeof(board_id));
  i2c_init(i2c0, 100000);
  gpio_set_function(SDA_PIN, GPIO_FUNC_I2C);
  gpio_set_function(SCL_PIN, GPIO_FUNC_I2C);
  gpio_pull_up(SDA_PIN); gpio_pull_up(SCL_PIN);
  watchdog_enable(8000, true);
  oled_ready = oledInit();
  showFallback();
  last_host_command = millis();
}

void loop() {
  watchdog_update();
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
  // Packet handling advances millis() and updates last_host_command. Using
  // the earlier loop timestamp can underflow and falsely trigger all idle
  // stages, including display-off, immediately after a valid command.
  uint32_t idle = millis() - last_host_command;
  if (idle >= 10000 && idle_stage < 1 && !manual_off) { showFallback(); idle_stage = 1; }
  if (idle >= 60000 && idle_stage < 2 && oled_ready && !manual_off) { oledContrast(32); idle_stage = 2; }
  if (idle >= 600000 && idle_stage < 3 && oled_ready && !manual_off) { oledPower(false); idle_stage = 3; }
  delay(1);
}
