#pragma once
#include <algorithm>
#include <cstdint>
#include <cstring>
#include <map>
#include <string>
#include <vector>
using std::min;
inline void watchdog_update() {}

struct LittleFSConfig {
  bool auto_format = true;
  void setAutoFormat(bool value) { auto_format = value; }
};
struct MockFilesystem {
  std::map<std::string, std::vector<uint8_t>> files;
  bool mounted = true, corrupt_writes = false, fail_open_write = false;
  bool auto_format = true;
  size_t write_limit = SIZE_MAX, format_calls = 0;
};
inline MockFilesystem mock;
class File {
  std::string path;
  bool opened = false;
  size_t cursor = 0;
public:
  File() = default;
  File(std::string name, bool valid) : path(name), opened(valid) {}
  explicit operator bool() const { return opened; }
  size_t size() const { return opened ? mock.files.at(path).size() : 0; }
  size_t read(uint8_t *out, size_t count) {
    if (!opened) return 0;
    auto &bytes = mock.files[path];
    count = min(count, bytes.size() - cursor);
    memcpy(out, bytes.data() + cursor, count); cursor += count; return count;
  }
  size_t write(const uint8_t *data, size_t count) {
    if (!opened) return 0;
    count = min(count, mock.write_limit);
    auto &bytes = mock.files[path];
    bytes.assign(data, data + count);
    if (mock.corrupt_writes && count) bytes.back() ^= 1;
    return count;
  }
  void flush() {}
  void close() { opened = false; }
};
struct MockLittleFS {
  void setConfig(const LittleFSConfig &config) { mock.auto_format = config.auto_format; }
  bool begin() { return mock.mounted; }
  bool format() { ++mock.format_calls; mock.files.clear(); return true; }
  File open(const char *name, const char *mode) {
    if (*mode == 'w') {
      if (mock.fail_open_write) return {};
      mock.files[name].clear(); return File(name, true);
    }
    return File(name, mock.files.count(name) != 0);
  }
};
inline MockLittleFS LittleFS;
