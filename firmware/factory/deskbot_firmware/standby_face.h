#pragma once
#include <stdint.h>
#include <string.h>
#include "fallback_face.h"

// Animate only the factory smile. The open frame and mouth stay identical to
// the app; custom saved images are never interpreted as faces or overwritten.
namespace DroidStandby {
struct Blink {
  uint32_t started = 0;
  bool closed = false;
  void reset(uint32_t now) { started = now; closed = false; }
  bool tick(uint32_t now) {
    const bool next = uint32_t(now - started) % 4200 >= 4000;
    if (next == closed) return false;
    closed = next; return true;
  }
  void render(uint8_t *out) const {
    memcpy(out, FALLBACK_FACE, 1024);
    if (!closed) return;
    for (int cx = 38; cx <= 90; cx += 52) {
      for (int y = 12; y < 37; ++y) for (int x = cx - 9; x <= cx + 9; ++x)
        out[(y / 8) * 128 + x] &= uint8_t(~(1u << (y % 8)));
      for (int y = 25; y <= 26; ++y) for (int x = cx - 9; x <= cx + 9; ++x)
        out[(y / 8) * 128 + x] |= uint8_t(1u << (y % 8));
    }
  }
};
}
