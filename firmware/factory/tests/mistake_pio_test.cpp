#include <cassert>
#include <cstdint>
#include <iostream>
#define PICO_NO_HARDWARE 1
#include "../deskbot_firmware/mistake_i2c.pio.h"

int main() {
  // Decode the assembled instructions independently of the driver. Optional
  // side-set needs its enable bit as well as the clock value: omitting that
  // bit was the cause of the first physical prototype's missing START/STOP.
  for (unsigned clock = 0; clock < 2; ++clock) {
    for (unsigned data = 0; data < 2; ++data) {
      const auto word = mistake_i2c_lines_program_instructions[clock * 2 + data];
      assert((word >> 13) == 7);          // SET
      assert(((word >> 5) & 7) == 4);     // PINDIRS, open-drain enables
      assert((word & 31) == data);
      assert(word & 0x1000);             // optional side-set enabled
      assert(((word >> 11) & 1) == clock);
      assert(((word >> 8) & 7) == 7);     // setup/hold delay
    }
  }
  unsigned waits = 0;
  for (const auto word : mistake_i2c_program_instructions) {
    if ((word >> 13) == 1) {             // WAIT
      ++waits;
      assert((word & 0x80) != 0);        // wait for high
      assert(((word >> 5) & 3) == 0);    // absolute GPIO, not SDA-relative PIN
      assert((word & 31) == 4);          // clock is GP4, data is GP5
    }
  }
  assert(waits == 2);                    // data clock and ACK clock stretching
  std::cout << "Mistake PIO: open-drain SDA/clock levels, optional side-set, and GP4 clock-stretch waits: PASS\n";
}
