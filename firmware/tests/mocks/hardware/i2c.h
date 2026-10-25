#pragma once
#include <cstddef>
#include <cstdint>
inline int i2c0 = 0;
inline int mock_oled_address = 0x3C;
inline bool mock_oled_on = false;
inline void i2c_init(int, unsigned int) {}
inline int i2c_write_timeout_us(int, uint8_t address, const uint8_t *data, size_t n, bool, unsigned int) {
  if (address != mock_oled_address) return -1;
  if (n >= 2 && data[0] == 0x00) {
    if (n == 2 && data[1] == 0xae) mock_oled_on = false;
    if (data[n - 1] == 0xaf) mock_oled_on = true;
  }
  return int(n);
}
