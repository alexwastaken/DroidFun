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
  memcpy(bytes.data() + 16, payload.data(), payload.size());
  put32(bytes.data() + 16 + payload.size(), crc32(bytes.data(), 16 + payload.size()));
  processPacket(bytes.data(), bytes.size());
}
int main() {
  Bytes saved(20 + 2 * DroidSaved::RECORD_BYTES);
  memcpy(saved.data(), "DFP1", 4);
  saved[4] = 1; saved[5] = 2; saved[6] = 50;
  put16(saved.data() + 8, 180);
  put16(saved.data() + 16, 3);
  memset(saved.data() + 18, 0x55, 1024);
  put16(saved.data() + 16 + DroidSaved::RECORD_BYTES, 3);
  memset(saved.data() + 18 + DroidSaved::RECORD_BYTES, 0xaa, 1024);
  DroidSaved::setGeneration(saved.data(), saved.size(), 1);
  mock.files[DroidSaved::paths[0]] = saved;

  // No USB enumeration/serial client is present; boot still displays saved data.
  Serial.connected = false; setup();
  assert(standalone_active && screen[0] == 0x55 && contrast == 128);
  mock_millis += 3000; loop(); assert(screen[0] == 0xaa);
  mock_usb_suspended = true;
  mock_millis += 3000; loop(); assert(screen[0] == 0x55 && !manual_off);
  mock_millis += 700000; loop(); assert(standalone_active && oled_ready && !manual_off);
  assert(mock.format_calls == 0);

  mock_usb_suspended = false; Serial.connected = true;
  packet(HELLO, 7, 1);
  assert(session == 7 && !standalone_active && strstr(last_reply, "standaloneSupported"));
  const Bytes desktop(1024, 0x33);
  packet(FRAME, 7, 2, desktop);
  assert(screen[0] == 0x33 && !standalone_active);
  mock_millis += 4999; loop(); assert(screen[0] == 0x33);
  mock_millis += 2; loop(); assert(standalone_active && screen[0] == 0x55);

  // A dead/stale session must not displace persistent playback.
  const uint32_t last = last_host_command;
  packet(FRAME, 8, 3, desktop);
  assert(last_host_command == last && standalone_active);
  packet(FRAME, 7, 2, desktop); // exact old packet is only acknowledged again
  assert(last_host_command == last && standalone_active);
  packet(PING, 7, 3);
  assert(!standalone_active && strstr(last_reply, "hostFrameNeeded"));
  packet(FRAME, 7, 4, desktop); assert(screen[0] == 0x33);

  // New handshakes cancel incomplete RAM uploads without changing stored data.
  auto old_saved = Bytes(DroidSaved::saved, DroidSaved::saved + DroidSaved::saved_length);
  assert(DroidSaved::upload.begin(saved.size(), 123));
  assert(DroidSaved::upload.append(0, saved.data(), 100));
  packet(HELLO, 9, 1);
  assert(!DroidSaved::upload.expected);
  assert(Bytes(DroidSaved::saved, DroidSaved::saved + DroidSaved::saved_length) == old_saved);
  Serial.connected = false; loop(); assert(standalone_active && screen[0] == 0x55);
  std::cout << "Unenumerated wall-power boot, rotation, suspension, stalled/stale sessions, and host handoff: PASS\n";
}
