#pragma once
#include <cstddef>
#include <cstdint>
inline int i2c0 = 0;
inline int mock_oled_address = 0x3C;
inline void i2c_init(int, unsigned int) {}
inline int i2c_write_timeout_us(int, uint8_t address, const uint8_t *, size_t n, bool, unsigned int) { return address == mock_oled_address ? int(n) : -1; }
