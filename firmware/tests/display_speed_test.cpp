#include <cassert>
#include <iostream>
#include "../deskbot_firmware/deskbot_firmware.ino"
namespace DroidSaved { extern "C" { uint8_t _FS_start = 0, _FS_end = 0; } }
int main() {
  uint8_t moving[FRAME_SIZE];
  for (size_t i = 0; i < FRAME_SIZE; ++i) moving[i] = FALLBACK_FACE[i] ^ 0xff;
  for (int address : {0x3C, 0x3D}) {
    mock_oled_address = address;
    oled_fast = true;
    mock_fail_fast = false;
    mock_fast_writes_until_failure = -1;
    i2c_init(i2c0, 400000);
    assert(oledInit() && oledMaxFps() == 30 && mock_i2c_hz == 400000);
    mock_display_bytes = 0;
    assert(oledFrame(FALLBACK_FACE));
    assert(mock_display_bytes == FRAME_SIZE);
    assert(memcmp(mock_panel, FALLBACK_FACE, FRAME_SIZE) == 0);
    mock_display_bytes = 0;
    assert(oledFrame(FALLBACK_FACE) && mock_display_bytes == 0);
    uint8_t small_change[FRAME_SIZE];
    memcpy(small_change, FALLBACK_FACE, FRAME_SIZE);
    small_change[8] ^= 0xff;
    small_change[7 * 128 + 115] ^= 0xff;
    assert(oledFrame(small_change) && mock_display_bytes == 32);
    assert(memcmp(mock_panel, small_change, FRAME_SIZE) == 0);
    // A screen may ACK initialization yet fail a later fast transfer.
    mock_fast_writes_until_failure = 15;
    mock_display_bytes = 0;
    mock_slow_display_bytes = 0;
    assert(oledFrame(moving));
    assert(mock_display_bytes > FRAME_SIZE);
    assert(memcmp(mock_panel, moving, FRAME_SIZE) == 0);
    assert(mock_slow_display_bytes == FRAME_SIZE);
    assert(oledMaxFps() == 5 && mock_i2c_hz == 100000);
    assert(oledFrame(FALLBACK_FACE));
    // Slow-only screens must also boot without an app or manual setting.
    oled_fast = true;
    mock_fail_fast = true;
    i2c_init(i2c0, 400000);
    assert(oledInit() && oled_address == address && oledMaxFps() == 5);
  }
  mock_oled_address = -1;
  oled_fast = true;
  i2c_init(i2c0, 400000);
  assert(!oledInit() && !oledFrame(FALLBACK_FACE));
  std::cout << "Fast OLED transfer, slow-only startup, runtime fallback and missing display passed\n";
}
