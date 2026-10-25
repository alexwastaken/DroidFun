#include <cassert>
#include <iostream>
#include "../deskbot_firmware/deskbot_firmware.ino"
namespace DroidSaved { extern "C" { uint8_t _FS_start = 0, _FS_end = 0; } }
using Bytes = std::vector<uint8_t>;

static void packet(uint8_t kind, uint32_t id, uint32_t sequence, const Bytes &payload = {}) {
  Bytes bytes(20 + payload.size());
  bytes[0] = 'D'; bytes[1] = 'B'; bytes[2] = 1; bytes[3] = kind;
  put32(bytes.data() + 4, id); put32(bytes.data() + 8, sequence);
  put16(bytes.data() + 12, payload.size());
  if (!payload.empty()) memcpy(bytes.data() + 16, payload.data(), payload.size());
  put32(bytes.data() + 16 + payload.size(), crc32(bytes.data(), 16 + payload.size()));
  processPacket(bytes.data(), bytes.size());
}
static Bytes text(const char *s) { return Bytes(s, s + strlen(s)); }
static void assertFace() {
  assert(oled_ready && mock_oled_on && fallback_shown);
  assert(memcmp(screen, FALLBACK_FACE, FRAME_SIZE) == 0);
}
int main() {
  // Fresh Meep/Boo/Claw use this same firmware, including on a wall charger.
  Serial.connected = false; setup(); assertFace();
  for (uint32_t elapsed : {60000u, 600000u, 86400000u, 0xffff0000u}) {
    mock_millis += elapsed; loop(); assertFace();
  }
  Serial.connected = true;
  packet(HELLO, 7, 1);
  const Bytes desktop(1024, 0x33);
  packet(FRAME, 7, 2, desktop); assert(screen[0] == 0x33);
  mock_millis += 4999; loop(); assert(screen[0] == 0x33);
  mock_millis += 2; loop(); assertFace();
  // A cached static host frame must be requested again after standby.
  packet(PING, 7, 3); assert(strstr(last_reply, "hostFrameNeeded"));
  packet(FRAME, 7, 4, desktop); assert(!fallback_shown && screen[0] == 0x33);

  // Mac USB suspension must show the face immediately and keep the panel on.
  // Partial USB data/uploads cannot leak across suspend/resume.
  rx_length = 4; rx_discard = true;
  assert(DroidSaved::upload.begin(20 + DroidSaved::RECORD_BYTES, 123));
  mock_usb_suspended = true; loop(); assertFace();
  assert(session == 7 && rx_length == 0 && !rx_discard && !DroidSaved::upload.expected);
  mock_millis += 86400000; loop(); assertFace();
  mock_usb_suspended = false; loop(); assertFace();
  packet(PING, 7, 5); assert(strstr(last_reply, "hostFrameNeeded"));
  packet(FRAME, 7, 6, desktop); assert(screen[0] == 0x33 && mock_oled_on);

  // Closing/reopening serial and a changed host session also recover normally.
  Serial.connected = false; loop(); assertFace();
  Serial.connected = true; packet(HELLO, 9, 1);
  packet(FRAME, 9, 2, desktop); assert(screen[0] == 0x33 && mock_oled_on);
  packet(FRAME, 7, 99, Bytes(1024, 0xaa)); assert(screen[0] == 0x33);

  // Intentional screen-off and quiet-hours commands remain authoritative.
  packet(MSG_DISPLAY, 9, 3, text("{\"on\":false,\"contrast\":216,\"orientation\":0}"));
  assert(manual_off && !mock_oled_on);
  mock_usb_suspended = true; mock_millis += 700000; loop();
  assert(manual_off && !mock_oled_on);
  mock_usb_suspended = false; loop();
  packet(MSG_DISPLAY, 9, 4, text("{\"on\":true,\"contrast\":216,\"orientation\":0}"));
  packet(FRAME, 9, 5, desktop); assert(!manual_off && mock_oled_on && screen[0] == 0x33);
  assert(mock.format_calls == 0);
  std::cout << "Powered standby: wall power, long idle, timer wrap, USB sleep, cached-frame handoff, reopen, stale sessions and manual off: PASS\n";
}
