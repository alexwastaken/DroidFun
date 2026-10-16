#pragma once
#include <cstddef>
#include <cstdint>
inline int i2c0 = 0;
inline void i2c_init(int, unsigned int) {}
inline int i2c_write_timeout_us(int, uint8_t, const uint8_t *, size_t n, bool, unsigned int) { return int(n); }
