#pragma once
#include <cstdint>
#include <cstddef>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <vector>
using uint = unsigned int;
inline uint32_t mock_millis = 0;
inline uint32_t millis() { return mock_millis; }
inline void delay(uint32_t ms) { mock_millis += ms; }
struct MockSerial {
  bool connected = false;
  std::vector<uint8_t> output;
  void begin(unsigned int) {}
  int available() { return 0; }
  int read() { return -1; }
  bool dtr() { return connected; }
  size_t write(const uint8_t *p, size_t n) { output.assign(p, p + n); return n; }
};
inline MockSerial Serial;
