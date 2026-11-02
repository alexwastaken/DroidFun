#include <cassert>
#include <iostream>
#include "../deskbot_firmware/deskbot_firmware.ino"
namespace DroidSaved { extern "C" { uint8_t _FS_start = 0, _FS_end = 0; } }
using Bytes = std::vector<uint8_t>;

static void packet(uint8_t kind, uint32_t sequence, const Bytes &payload = {}, uint32_t id = 7) {
  Bytes bytes(20 + payload.size());
  bytes[0] = 'D'; bytes[1] = 'B'; bytes[2] = 1; bytes[3] = kind;
  put32(bytes.data() + 4, id); put32(bytes.data() + 8, sequence);
  put16(bytes.data() + 12, payload.size());
  if (!payload.empty()) memcpy(bytes.data() + 16, payload.data(), payload.size());
  put32(bytes.data() + 16 + payload.size(), crc32(bytes.data(), 16 + payload.size()));
  processPacket(bytes.data(), bytes.size());
}
static Bytes request(uint8_t model) {
  Bytes bytes = {1, model, 1, 0};
  const char *id = "0123456789ABCDEF";
  bytes.insert(bytes.end(), id, id + 16); return bytes;
}
static void reload() { DroidIdentity::load(board_id); }

int main() {
  Serial.connected = true; setup();
  assert(DroidIdentity::current_slot == -1 && mock.format_calls == 0 && mock.write_calls == 0);
  packet(HELLO, 1);
  assert(strstr(last_reply, "\"identity\":null") && strstr(last_reply, "\"provisioningSupported\":true"));
  assert(strstr(last_reply, "\"standaloneContentCrc\":null"));
  const std::string original_board_id = board_id;
  const Bytes customer_file = {1, 2, 3, 4};
  mock.files[DroidSaved::paths[0]] = customer_file;

  uint32_t sequence = 2;
  auto good = request(3);
  for (auto [offset, value] : std::vector<std::pair<size_t, uint8_t>>{{0, 2}, {1, 0}, {1, 5}, {2, 2}, {3, 1}, {4, 'f'}}) {
    auto bad = good; bad[offset] = value;
    packet(PROVISION, sequence++, bad); assert(last_reply_kind == ERROR_PACKET);
  }
  packet(PROVISION, sequence++, Bytes(19)); assert(last_reply_kind == ERROR_PACKET);
  packet(PROVISION, sequence++, Bytes(21)); assert(last_reply_kind == ERROR_PACKET);
  packet(PROVISION, sequence++, good, 8); // Wrong session cannot write.
  assert(mock.write_calls == 0 && mock.format_calls == 0);
  manual_off = true; contrast = 0;
  packet(PROVISION, sequence++, good);
  assert(last_reply_kind == ACK && strstr(last_reply, "\"model\":\"meep\""));
  assert(!manual_off && contrast == 216 && oled_ready && mock_oled_on);
  assert(memcmp(screen, FALLBACK_FACE, FRAME_SIZE) == 0);
  assert(DroidIdentity::current_slot == 0 && DroidSaved::read32(DroidIdentity::current + 8) == 1);
  assert(mock.files[DroidSaved::paths[0]] == customer_file);
  assert(original_board_id == board_id);
  const auto first_files = mock.files;
  const auto writes = mock.write_calls;
  packet(PROVISION, sequence - 1, good); // Same packet replays the ACK only.
  packet(PROVISION, sequence++, good); // New request for same model is idempotent.
  assert(mock.write_calls == writes && mock.files == first_files);
  mock_oled_address = -1;
  packet(PROVISION, sequence++, good);
  assert(last_reply_kind == ACK && strstr(last_reply, "\"displayHealth\":\"i2c_error\""));
  assert(!oled_ready && DroidIdentity::current[5] == 3 && mock.write_calls == writes);
  mock_oled_address = 0x3C;
  packet(PROVISION, sequence++, good); assert(oled_ready && mock.write_calls == writes);
  reload(); assert(DroidIdentity::current[5] == 3);
  packet(HELLO, 1);
  assert(strstr(last_reply, "\"identity\":{\"model\":\"meep\"") && last_reply[strlen(last_reply) - 1] == '}');
  assert(strlen(last_reply) < REPLY_CAPACITY - 1);
  uint8_t decoded_reply[MAX_PACKET];
  const auto length = cobsDecode(Serial.output.data(), Serial.output.size() - 1, decoded_reply);
  assert(length == 20 + strlen(last_reply) && le16(decoded_reply + 12) == strlen(last_reply));

  auto next = request(1); sequence = 2;
  mock.write_limit = 10;
  packet(PROVISION, sequence++, next); assert(last_reply_kind == ERROR_PACKET);
  mock.write_limit = SIZE_MAX; reload(); assert(DroidIdentity::current[5] == 3);
  mock.corrupt_writes = true;
  packet(PROVISION, sequence++, next); assert(last_reply_kind == ERROR_PACKET);
  mock.corrupt_writes = false; reload(); assert(DroidIdentity::current[5] == 3);
  mock.fail_open_write = true;
  packet(PROVISION, sequence++, next); assert(last_reply_kind == ERROR_PACKET);
  mock.fail_open_write = false;
  packet(PROVISION, sequence++, next); assert(last_reply_kind == ACK);
  reload(); assert(DroidIdentity::current[5] == 1 && DroidSaved::read32(DroidIdentity::current + 8) == 2);
  assert(mock.files[DroidSaved::paths[0]] == customer_file && original_board_id == board_id);
  mock.files[DroidIdentity::paths[1]][5] ^= 1;
  reload(); assert(DroidIdentity::current[5] == 3); // Last intact record survives.

  Bytes playlist(1046, 0x55);
  memcpy(playlist.data(), "DFP1", 4); playlist[4] = 1; playlist[5] = 1; playlist[6] = 85; playlist[7] = 0;
  memset(playlist.data() + 8, 0, 8); put16(playlist.data() + 16, 3);
  DroidSaved::setGeneration(playlist.data(), playlist.size(), 0);
  assert(DroidSaved::upload.begin(playlist.size(), DroidSaved::checksum(playlist.data(), playlist.size())));
  packet(PROVISION, sequence++, next); assert(last_reply_kind == ERROR_PACKET); // No interleaved flash writers.
  for (size_t offset = 0; offset < playlist.size(); offset += 1022) {
    assert(DroidSaved::upload.append(offset, playlist.data() + offset, min(size_t(1022), playlist.size() - offset)));
  }
  packet(SAVED_COMMIT, sequence++);
  assert(last_reply_kind == ACK);
  char expected_content[48];
  snprintf(expected_content, sizeof(expected_content), "\"standaloneContentCrc\":%lu",
      static_cast<unsigned long>(DroidSaved::checksum(playlist.data(), playlist.size() - 4)));
  assert(strstr(last_reply, expected_content));
  reload(); assert(DroidIdentity::current[5] == 3);
  DroidSaved::load();
  packet(HELLO, 1);
  assert(strstr(last_reply, expected_content) && last_reply[strlen(last_reply) - 1] == '}');
  assert(strlen(last_reply) < REPLY_CAPACITY - 1);
  sequence = 2;

  auto overflow = mock.files[DroidIdentity::paths[0]];
  DroidSaved::write32(overflow.data() + 8, UINT32_MAX);
  DroidSaved::write32(overflow.data() + 28, DroidSaved::checksum(overflow.data(), 28));
  mock.files[DroidIdentity::paths[0]] = overflow; reload();
  packet(PROVISION, sequence++, next); assert(last_reply_kind == ERROR_PACKET);
  auto foreign = overflow; foreign[12] = 'f';
  DroidSaved::write32(foreign.data() + 28, DroidSaved::checksum(foreign.data(), 28));
  mock.files[DroidIdentity::paths[0]] = foreign; reload(); assert(DroidIdentity::current_slot == -1);
  mock.mounted = false; DroidSaved::storage_ready = false; reload();
  packet(PROVISION, sequence++, next); assert(last_reply_kind == ERROR_PACKET && mock.format_calls == 0);
  // Restore a writable identity slot after the storage-failure checks.
  mock.mounted = true; DroidSaved::storage_ready = true;
  mock.files[DroidIdentity::paths[0]] = first_files.at(DroidIdentity::paths[0]);
  mock.files.erase(DroidIdentity::paths[1]); reload();
  const auto saved_before_nimbus = mock.files[DroidSaved::paths[0]];
  // Nimbus uses a distinct durable model code, retaining board UID and saved screens.
  packet(HELLO, 1);
  packet(PROVISION, 2, request(4));
  assert(last_reply_kind == ACK && strstr(last_reply, "\"model\":\"nimbus\""));
  reload(); assert(DroidIdentity::current[5] == 4);
  packet(HELLO, 1);
  assert(strstr(last_reply, "\"model\":\"nimbus\""));
  assert(original_board_id == board_id);
  assert(mock.files[DroidSaved::paths[0]] == saved_before_nimbus);
  std::cout << "Factory identity bounds, UID binding, persisted readback, torn writes, duplicate requests, playlist isolation, and no boot formatting: PASS\n";
}
