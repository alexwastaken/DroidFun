#pragma once
#include <LittleFS.h>
#include "standalone_playlist.h"

namespace DroidSaved {
extern "C" { extern uint8_t _FS_start, _FS_end; }
static uint8_t saved[MAX_BYTES];
static Upload upload;
static size_t saved_length = 0;
static int saved_slot = -1;
static bool storage_ready = false;
static const char *const paths[] = {"/screens-a.dfp", "/screens-b.dfp"};

inline bool readSlot(int slot, uint8_t *buffer, size_t &length) {
  File file = LittleFS.open(paths[slot], "r");
  if (!file || file.size() > MAX_BYTES) { if (file) file.close(); return false; }
  length = file.size();
  size_t count = file.read(buffer, length);
  file.close();
  return count == length && valid(buffer, length) && read32(buffer + 12) != 0;
}

inline void load() {
  LittleFSConfig config;
  config.setAutoFormat(false);
  LittleFS.setConfig(config);
  storage_ready = LittleFS.begin();
  if (!storage_ready) return; // Never format or erase anything merely by booting.
  for (int slot = 0; slot < 2; ++slot) {
    size_t length = 0;
    if (readSlot(slot, upload.bytes, length)
        && (!saved_length || read32(upload.bytes + 12) > read32(saved + 12))) {
      memcpy(saved, upload.bytes, length); saved_length = length; saved_slot = slot;
    }
  }
  upload.cancel();
}

inline bool initializeBlankStorage() {
  if (storage_ready) return true;
  // Formatting is allowed only on an explicit save and an entirely erased
  // reserved partition. A corrupt/nonblank filesystem is never auto-erased.
  uintptr_t start = reinterpret_cast<uintptr_t>(&_FS_start), end = reinterpret_cast<uintptr_t>(&_FS_end);
  if (end <= start || end - start < 131072) return false;
  for (uintptr_t address = start; address < end; ++address) {
    if (*reinterpret_cast<const volatile uint8_t *>(address) != 0xff) return false;
    if ((address & 4095) == 0) watchdog_update();
  }
  if (!LittleFS.format()) return false;
  storage_ready = LittleFS.begin();
  return storage_ready;
}

inline bool commit() {
  if (!upload.complete() || !initializeBlankStorage()) return false;
  uint32_t generation = saved_length ? read32(saved + 12) : 0;
  if (generation == UINT32_MAX) return false;
  setGeneration(upload.bytes, upload.expected, generation + 1);
  const int slot = saved_slot == 0 ? 1 : 0;
  File file = LittleFS.open(paths[slot], "w");
  if (!file) return false;
  size_t count = file.write(upload.bytes, upload.expected);
  file.flush(); file.close(); watchdog_update();
  if (count != upload.expected) return false;
  // Verify the actual flash bytes before changing the active in-memory slot.
  file = LittleFS.open(paths[slot], "r");
  if (!file || file.size() != upload.expected) { if (file) file.close(); return false; }
  uint8_t verify[256];
  bool verified = true;
  for (size_t offset = 0; offset < upload.expected; offset += sizeof(verify)) {
    size_t size = min(sizeof(verify), upload.expected - offset);
    if (file.read(verify, size) != size || memcmp(verify, upload.bytes + offset, size)) { verified = false; break; }
    watchdog_update();
  }
  file.close();
  if (!verified) return false;
  memcpy(saved, upload.bytes, upload.expected); saved_length = upload.expected; saved_slot = slot;
  upload.cancel();
  return true;
}
}
