#include <cassert>
#include <iostream>
#include "../deskbot_firmware/deskbot_firmware.ino"
namespace DroidSaved { extern "C" { uint8_t _FS_start = 0, _FS_end = 0; } }
int main() {
  mock_oled_address = 0x3C;
  assert(oledInit() && oled_address == 0x3C);
  assert(oledFrame(FALLBACK_FACE));
  mock_oled_address = 0x3D;
  assert(oledInit() && oled_address == 0x3D);
  assert(oledFrame(FALLBACK_FACE));
  mock_oled_address = -1;
  assert(!oledInit());
  assert(!oledFrame(FALLBACK_FACE));
  mock_oled_address = 0x3C;
  assert(oledInit() && oled_address == 0x3C);
  std::cout << "OLED address discovery, absent display and recovery passed\n";
}
