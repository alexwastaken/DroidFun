#pragma once
#include <stdint.h>
#include <stddef.h>
#include <string.h>

// DFP1, shared with protocol::StandalonePlaylist. No Arduino dependencies so the
// bounds, partial-upload and corruption rules can be tested on a host computer.
namespace DroidSaved {
constexpr size_t FRAME_BYTES = 1024, RECORD_BYTES = 1026, MAX_FRAMES = 30;
constexpr size_t HEADER_BYTES = 16, MAX_BYTES = 20 + MAX_FRAMES * RECORD_BYTES;
inline uint16_t read16(const uint8_t *p) { return p[0] | (uint16_t(p[1]) << 8); }
inline uint32_t read32(const uint8_t *p) { return p[0] | (uint32_t(p[1]) << 8) | (uint32_t(p[2]) << 16) | (uint32_t(p[3]) << 24); }
inline void write32(uint8_t *p, uint32_t n) { for (int i = 0; i < 4; ++i) p[i] = n >> (i * 8); }
inline uint32_t checksum(const uint8_t *p, size_t n) {
  uint32_t crc = ~uint32_t(0);
  while (n--) { crc ^= *p++; for (int b = 0; b < 8; ++b) crc = (crc >> 1) ^ ((crc & 1) ? 0xedb88320u : 0); }
  return ~crc;
}
inline bool valid(const uint8_t *p, size_t n) {
  if (n < 20 + RECORD_BYTES || n > MAX_BYTES || memcmp(p, "DFP1", 4) || p[4] != 1
      || p[5] < 1 || p[5] > MAX_FRAMES || p[6] > 100 || p[7] || read16(p + 10)
      || (read16(p + 8) != 0 && read16(p + 8) != 180)
      || n != 20 + size_t(p[5]) * RECORD_BYTES) return false;
  for (size_t i = 0; i < p[5]; ++i) {
    uint16_t seconds = read16(p + HEADER_BYTES + i * RECORD_BYTES);
    if (seconds < 3 || seconds > 120) return false;
  }
  return checksum(p, n - 4) == read32(p + n - 4);
}
inline void setGeneration(uint8_t *p, size_t n, uint32_t generation) {
  write32(p + 12, generation);
  write32(p + n - 4, checksum(p, n - 4));
}
inline const uint8_t *frame(const uint8_t *p, uint8_t index) { return p + HEADER_BYTES + size_t(index) * RECORD_BYTES + 2; }
inline uint32_t durationMs(const uint8_t *p, uint8_t index) { return uint32_t(read16(frame(p, index) - 2)) * 1000; }

struct Upload {
  uint8_t bytes[MAX_BYTES] = {};
  size_t expected = 0, received = 0;
  uint32_t crc = 0;
  void cancel() { expected = received = 0; }
  bool begin(size_t length, uint32_t checksum) {
    cancel();
    if (length < 20 + RECORD_BYTES || length > MAX_BYTES || (length - 20) % RECORD_BYTES) return false;
    expected = length; crc = checksum; return true;
  }
  bool append(size_t offset, const uint8_t *data, size_t length) {
    if (!expected || offset != received || !length || length > 1022 || length > expected - received) return false;
    memcpy(bytes + received, data, length); received += length; return true;
  }
  bool complete() const {
    return expected && received == expected && checksum(bytes, received) == crc
        && valid(bytes, received) && read32(bytes + 12) == 0;
  }
};
}
