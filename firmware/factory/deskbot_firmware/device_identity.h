#pragma once
#include "standalone_storage.h"

// Factory identity is separate from customer playlists. Keep the immutable
// flash UID as the transport identity; never derive it from the selected model.
namespace DroidIdentity {
constexpr size_t RECORD_BYTES = 32, REQUEST_BYTES = 20;
constexpr uint8_t SCHEMA = 1, HARDWARE_PROFILE = 1;
static constexpr const char *PROFILE = "rp2040-zero-gp4-gp5-ssd1306";
static const char *const paths[] = {"/identity-a.dfi", "/identity-b.dfi"};
static uint8_t current[RECORD_BYTES] = {};
static int current_slot = -1;

inline const char *modelName(uint8_t code) {
  switch (code) { case 1: return "boo"; case 2: return "claw"; case 3: return "meep"; case 4: return "nimbus"; default: return nullptr; }
}
inline uint8_t lowerHex(uint8_t c) { return c >= 'A' && c <= 'F' ? c + ('a' - 'A') : c; }
inline bool sameBoard(const uint8_t *encoded, const char *board) {
  if (!board || strlen(board) != 16) return false;
  for (size_t i = 0; i < 16; ++i) {
    uint8_t c = lowerHex(encoded[i]);
    if (!((c >= '0' && c <= '9') || (c >= 'a' && c <= 'f')) || c != lowerHex(board[i])) return false;
  }
  return true;
}
inline bool valid(const uint8_t *record, size_t length, const char *board) {
  return length == RECORD_BYTES && !memcmp(record, "DFI1", 4)
      && record[4] == SCHEMA && modelName(record[5]) && record[6] == HARDWARE_PROFILE && !record[7]
      && DroidSaved::read32(record + 8) != 0 && sameBoard(record + 12, board)
      && DroidSaved::checksum(record, 28) == DroidSaved::read32(record + 28);
}
inline bool readSlot(int slot, uint8_t *record, const char *board) {
  File file = LittleFS.open(paths[slot], "r");
  if (!file || file.size() != RECORD_BYTES) { if (file) file.close(); return false; }
  size_t read = file.read(record, RECORD_BYTES); file.close();
  return valid(record, read, board);
}
inline void load(const char *board) {
  current_slot = -1; memset(current, 0, sizeof(current));
  if (!DroidSaved::storage_ready) return;
  uint8_t candidate[RECORD_BYTES];
  for (int slot = 0; slot < 2; ++slot) {
    if (readSlot(slot, candidate, board)
        && (current_slot < 0 || DroidSaved::read32(candidate + 8) > DroidSaved::read32(current + 8))) {
      memcpy(current, candidate, RECORD_BYTES); current_slot = slot;
    }
  }
}
inline bool provision(const uint8_t *request, size_t length, const char *board) {
  // Version, model, hardware profile, reserved zero, exact flash UID (16 hex).
  if (length != REQUEST_BYTES || request[0] != SCHEMA || !modelName(request[1])
      || request[2] != HARDWARE_PROFILE || request[3] || !sameBoard(request + 4, board)
      || DroidSaved::upload.expected) return false;
  if (current_slot >= 0 && current[5] == request[1] && current[6] == request[2]) return true;
  if (!DroidSaved::initializeBlankStorage()) return false;
  const uint32_t generation = current_slot >= 0 ? DroidSaved::read32(current + 8) : 0;
  if (generation == UINT32_MAX) return false;
  uint8_t next[RECORD_BYTES] = {};
  memcpy(next, "DFI1", 4); next[4] = SCHEMA; next[5] = request[1]; next[6] = request[2];
  DroidSaved::write32(next + 8, generation + 1);
  for (size_t i = 0; i < 16; ++i) next[12 + i] = lowerHex(board[i]);
  DroidSaved::write32(next + 28, DroidSaved::checksum(next, 28));
  const int slot = current_slot == 0 ? 1 : 0;
  File file = LittleFS.open(paths[slot], "w");
  if (!file) return false;
  size_t written = file.write(next, RECORD_BYTES); file.flush(); file.close(); watchdog_update();
  uint8_t verified[RECORD_BYTES];
  if (written != RECORD_BYTES || !readSlot(slot, verified, board) || memcmp(verified, next, RECORD_BYTES)) return false;
  memcpy(current, next, RECORD_BYTES); current_slot = slot;
  return true;
}
inline void json(char *out, size_t capacity) {
  if (current_slot < 0) { snprintf(out, capacity, "null"); return; }
  snprintf(out, capacity, "{\"model\":\"%s\",\"hardwareProfile\":\"%s\",\"generation\":%lu}",
      modelName(current[5]), PROFILE, static_cast<unsigned long>(DroidSaved::read32(current + 8)));
}
}
