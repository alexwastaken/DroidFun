#include <cassert>
#include <iostream>
#include "../deskbot_firmware/deskbot_firmware.ino"
namespace DroidSaved { extern "C" { uint8_t _FS_start = 0, _FS_end = 0; } }
int main() {
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
    // A screen may ACK initialization yet fail a later fast transfer.
    mock_fast_writes_until_failure = 15;
    mock_display_bytes = 0;
    mock_slow_display_bytes = 0;
    assert(oledFrame(FALLBACK_FACE));
    assert(mock_display_bytes > FRAME_SIZE);
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
