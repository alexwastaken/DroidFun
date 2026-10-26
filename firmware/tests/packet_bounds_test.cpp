#include <array>
#include <cassert>
#include <iostream>
#include "../deskbot_firmware/deskbot_firmware.ino"
namespace DroidSaved { extern "C" { uint8_t _FS_start = 0, _FS_end = 0; } }
using Bytes = std::vector<uint8_t>;

// Construct COBS boundary cases independently of the production encoder.
static Bytes boundaryBlock(uint8_t finalCode) {
  Bytes encoded;
  for (int block = 0; block < 4; ++block) {
    encoded.push_back(255);
    encoded.insert(encoded.end(), 254, 0x55);
  }
  encoded.push_back(finalCode);
  encoded.insert(encoded.end(), finalCode - 1, 0x55);
  return encoded;
}

static void expectRejectedWithoutOverwrite(const Bytes &encoded) {
  struct GuardedOutput {
    std::array<uint8_t, MAX_PACKET> bytes;
    std::array<uint8_t, 16> guard;
  } output;
  output.bytes.fill(0xCC);
  output.guard.fill(0xA5);
  assert(cobsDecode(encoded.data(), encoded.size(), output.bytes.data()) == 0);
  for (uint8_t value : output.guard) assert(value == 0xA5);
}

static Bytes packet(uint8_t kind, uint32_t sequence, const Bytes &payload = {}) {
  Bytes bytes(20 + payload.size());
  bytes[0] = 'D'; bytes[1] = 'B'; bytes[2] = 1; bytes[3] = kind;
  put32(bytes.data() + 4, 7); put32(bytes.data() + 8, sequence);
  put16(bytes.data() + 12, payload.size());
  if (!payload.empty()) memcpy(bytes.data() + 16, payload.data(), payload.size());
  put32(bytes.data() + 16 + payload.size(), crc32(bytes.data(), 16 + payload.size()));
  Bytes encoded(bytes.size() + bytes.size() / 254 + 2);
  encoded.resize(cobsEncode(bytes.data(), bytes.size(), encoded.data()));
  encoded.push_back(0);
  return encoded;
}

static void receive(const Bytes &bytes) {
  Serial.input = bytes;
  Serial.input_offset = 0;
  loop();
  assert(Serial.available() == 0);
  assert(rx_length == 0 && !rx_discard);
}

int main() {
  // Exactly 1044 nonzero decoded bytes fit, as does an all-zero maximum packet.
  std::array<uint8_t, MAX_PACKET> output;
  const Bytes maximum = boundaryBlock(29);
  assert(cobsDecode(maximum.data(), maximum.size(), output.data()) == MAX_PACKET);
  for (uint8_t value : output) assert(value == 0x55);
  const Bytes zeroes(MAX_PACKET + 1, 1);
  assert(cobsDecode(zeroes.data(), zeroes.size(), output.data()) == MAX_PACKET);
  for (uint8_t value : output) assert(value == 0);

  // This separator used to overwrite the byte immediately following decoded[].
  Bytes badSeparator = maximum;
  badSeparator.push_back(1);
  expectRejectedWithoutOverwrite(badSeparator);
  expectRejectedWithoutOverwrite(boundaryBlock(30)); // One excess copied byte.
  expectRejectedWithoutOverwrite(Bytes(MAX_PACKET + 2, 1)); // Excess implicit zero.
  expectRejectedWithoutOverwrite({0});
  expectRejectedWithoutOverwrite({3, 0x55}); // Truncated source block.

  // A fresh board with no saved playlist still displays the fallback face.
  Serial.connected = true;
  setup();
  assert(oled_ready && fallback_shown && !manual_off);
  assert(memcmp(screen, FALLBACK_FACE, FRAME_SIZE) == 0);
  receive(packet(HELLO, 1));
  assert(session == 7 && strstr(last_reply, "\"firmware\":\"1.2.4\""));
  const std::string originalId = board_id;

  // Reject malformed input, retain identity, and consume the next valid frame
  // in the same serial batch instead of poisoning the established connection.
  Bytes frame(FRAME_SIZE);
  for (size_t i = 0; i < frame.size(); ++i) frame[i] = uint8_t(i);
  Bytes batch = badSeparator;
  batch.push_back(0);
  const Bytes next = packet(FRAME, 2, frame);
  batch.insert(batch.end(), next.begin(), next.end());
  receive(batch);
  assert(originalId == board_id);
  assert(oled_ready && last_reply_kind == ACK && last_sequence == 2);
  assert(memcmp(screen, frame.data(), FRAME_SIZE) == 0);

  // Both extreme full-size framebuffer values remain valid through framing.
  for (uint8_t value : {uint8_t(0), uint8_t(0xFF)}) {
    frame.assign(FRAME_SIZE, value);
    receive(packet(FRAME, last_sequence + 1, frame));
    assert(last_reply_kind == ACK && memcmp(screen, frame.data(), FRAME_SIZE) == 0);
  }
  receive(packet(FALLBACK, last_sequence + 1));
  assert(oled_ready && memcmp(screen, FALLBACK_FACE, FRAME_SIZE) == 0);
  std::cout << "COBS capacity boundaries, malformed-packet recovery, full-size frames, and fresh-board fallback: PASS\n";
}
