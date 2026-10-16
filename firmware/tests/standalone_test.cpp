#include <cassert>
#include <iostream>
#include "../deskbot_firmware/standalone_storage.h"

// Link placeholders only: tests use a mounted mock filesystem. No host flash
// memory is formatted or dereferenced by initializeBlankStorage.
namespace DroidSaved { extern "C" { uint8_t _FS_start = 0, _FS_end = 0; } }

using Bytes = std::vector<uint8_t>;
static Bytes playlist(uint8_t count = 1, uint8_t pattern = 0x55) {
  Bytes bytes(20 + count * DroidSaved::RECORD_BYTES, pattern);
  memcpy(bytes.data(), "DFP1", 4);
  bytes[4] = 1; bytes[5] = count; bytes[6] = 85; bytes[7] = 0;
  memset(bytes.data() + 8, 0, 8);
  for (size_t i = 0; i < count; ++i) {
    bytes[16 + i * DroidSaved::RECORD_BYTES] = 3 + i;
    bytes[17 + i * DroidSaved::RECORD_BYTES] = 0;
  }
  DroidSaved::setGeneration(bytes.data(), bytes.size(), 0);
  return bytes;
}
static void uploadAll(const Bytes &bytes) {
  auto &upload = DroidSaved::upload;
  assert(upload.begin(bytes.size(), DroidSaved::checksum(bytes.data(), bytes.size())));
  for (size_t offset = 0; offset < bytes.size(); offset += 1022) {
    assert(upload.append(offset, bytes.data() + offset, min(size_t(1022), bytes.size() - offset)));
  }
  assert(upload.complete());
}
static void reboot() {
  DroidSaved::saved_length = 0; DroidSaved::saved_slot = -1;
  DroidSaved::storage_ready = false; DroidSaved::upload.cancel();
  DroidSaved::load();
}
static Bytes savedBytes() {
  return Bytes(DroidSaved::saved, DroidSaved::saved + DroidSaved::saved_length);
}

int main() {
  using namespace DroidSaved;
  assert(checksum(reinterpret_cast<const uint8_t *>("123456789"), 9) == 0xcbf43926);
  auto one = playlist();
  assert(valid(one.data(), one.size()));
  assert(one.size() == 1046 && durationMs(one.data(), 0) == 3000);
  assert(checksum(one.data(), one.size() - 4) == 0x3e20575b); // shared Rust fixture
  assert(frame(one.data(), 0)[0] == 0x55);
  auto full = playlist(30);
  assert(valid(full.data(), full.size()) && full.size() == MAX_BYTES);
  assert(!valid(one.data(), one.size() - 1));
  assert(!valid(nullptr, 0));
  for (auto [offset, value] : std::vector<std::pair<size_t, uint8_t>> {
      {4, 2}, {5, 0}, {5, 31}, {6, 101}, {7, 1}, {8, 90}, {10, 1}, {16, 2}, {16, 121}}) {
    auto bad = one; bad[offset] = value; setGeneration(bad.data(), bad.size(), 0);
    assert(!valid(bad.data(), bad.size()));
  }
  auto corrupt = one; corrupt[40] ^= 1;
  assert(!valid(corrupt.data(), corrupt.size()));

  Upload partial;
  assert(!partial.begin(0, 0) && !partial.begin(MAX_BYTES + 1, 0));
  assert(!partial.begin(one.size() + 1, 0));
  assert(partial.begin(one.size(), checksum(one.data(), one.size())));
  assert(!partial.append(1, one.data(), 10));
  assert(!partial.append(0, one.data(), 1023));
  assert(partial.append(0, one.data(), 100));
  assert(!partial.complete());
  assert(!partial.append(0, one.data(), 100)); // duplicate data must not advance
  assert(partial.received == 100);
  assert(partial.append(100, one.data() + 100, one.size() - 100));
  assert(partial.complete());
  partial.bytes[40] ^= 1; assert(!partial.complete());
  partial.cancel(); assert(!partial.complete() && !partial.append(0, one.data(), 10));

  reboot(); assert(!saved_length && !mock.auto_format && mock.format_calls == 0);
  uploadAll(one); assert(commit());
  auto original = savedBytes();
  assert(saved_slot == 0 && read32(saved + 12) == 1 && valid(saved, saved_length));
  assert(!upload.expected);
  reboot(); assert(savedBytes() == original);

  auto next = playlist(2, 0xaa);
  assert(upload.begin(next.size(), checksum(next.data(), next.size())));
  assert(upload.append(0, next.data(), 100));
  assert(!commit()); assert(savedBytes() == original);
  reboot(); assert(savedBytes() == original);

  uploadAll(next); mock.write_limit = 200;
  assert(!commit()); assert(savedBytes() == original);
  mock.write_limit = SIZE_MAX;
  reboot(); assert(savedBytes() == original); // truncated inactive slot rejected

  uploadAll(next); mock.corrupt_writes = true;
  assert(!commit()); assert(savedBytes() == original);
  mock.corrupt_writes = false;
  reboot(); assert(savedBytes() == original); // corrupt inactive slot rejected

  uploadAll(next); mock.fail_open_write = true;
  assert(!commit()); assert(savedBytes() == original);
  mock.fail_open_write = false;
  uploadAll(next); assert(commit());
  assert(saved_slot == 1 && saved[5] == 2 && read32(saved + 12) == 2);
  auto replacement = savedBytes();
  reboot(); assert(savedBytes() == replacement);
  mock.files[paths[1]][20] ^= 1;
  reboot(); assert(savedBytes() == original); // newest slot corrupt, old survives

  mock.mounted = false;
  reboot(); assert(!saved_length && mock.format_calls == 0);
  std::cout << "Standalone format, upload bounds, partial writes, corruption, slot recovery, and no boot formatting: PASS\n";
}
