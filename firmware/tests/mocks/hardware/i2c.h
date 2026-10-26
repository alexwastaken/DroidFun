#pragma once
#include <cstddef>
#include <cstdint>
inline int i2c0 = 0;
inline int mock_oled_address = 0x3C;
inline bool mock_oled_on = false;
inline unsigned int mock_i2c_hz = 100000;
inline bool mock_fail_fast = false;
inline size_t mock_display_bytes = 0;
inline size_t mock_slow_display_bytes = 0;
inline int mock_fast_writes_until_failure = -1;
inline void i2c_init(int, unsigned int hz) { mock_i2c_hz = hz; }
inline unsigned int i2c_set_baudrate(int, unsigned int hz) { mock_i2c_hz = hz; return hz; }
inline int i2c_write_timeout_us(int, uint8_t address, const uint8_t *data, size_t n, bool, unsigned int) {
  if (mock_i2c_hz > 100000) {
    if (mock_fail_fast || mock_fast_writes_until_failure == 0) return -1;
    if (mock_fast_writes_until_failure > 0) --mock_fast_writes_until_failure;
  }
  if (address != mock_oled_address) return -1;
  if (n > 1 && data[0] == 0x40) {
    mock_display_bytes += n - 1;
    if (mock_i2c_hz == 100000) mock_slow_display_bytes += n - 1;
  }
  if (n >= 2 && data[0] == 0x00) {
    if (n == 2 && data[1] == 0xae) mock_oled_on = false;
    if (data[n - 1] == 0xaf) mock_oled_on = true;
  }
  return int(n);
}
