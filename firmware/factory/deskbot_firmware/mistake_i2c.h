#pragma once
// Write-only, open-drain OLED bus for the one supported wiring exception:
// SDA=GP5, SCL=GP4. PIO program adapted from Raspberry Pi pico-examples,
// Copyright (c) 2020 Raspberry Pi (Trading) Ltd., BSD-3-Clause.
// Unlike the upstream demonstration, every byte must ACK, every wait has a
// deadline, and errors reset the state machine and attempt a bounded STOP.
#include <hardware/clocks.h>
#include <hardware/pio.h>
#include <pico/time.h>
#include "mistake_i2c.pio.h"

namespace MistakeI2c {
static PIO bus = pio0;
static int sm = -1;
static uint offset = 0;
static constexpr uint SDA = 5, SCL = 4;
static constexpr uint32_t PINS = (1u << SDA) | (1u << SCL);

inline bool expired(uint64_t deadline) { return time_us_64() >= deadline; }
inline bool error() { return pio_interrupt_get(bus, sm); }

inline void reset() {
  pio_sm_set_enabled(bus, sm, false);
  pio_sm_clear_fifos(bus, sm);
  pio_sm_restart(bus, sm);
  pio_interrupt_clear(bus, sm);
  // Output latch stays zero; inverted output enables make 1 release the pin.
  pio_sm_set_pindirs_with_mask(bus, sm, PINS, PINS);
  pio_sm_exec(bus, sm, pio_encode_jmp(offset + mistake_i2c_wrap_target));
  pio_sm_set_enabled(bus, sm, true);
}

inline void baud(uint32_t hz) {
  if (sm >= 0) pio_sm_set_clkdiv(bus, sm, float(clock_get_hz(clk_sys)) / (32 * hz));
}

inline bool begin() {
  if (sm >= 0) return true;
  if (!pio_can_add_program(bus, &mistake_i2c_program)) return false;
  sm = pio_claim_unused_sm(bus, false);
  if (sm < 0) return false;
  offset = pio_add_program(bus, &mistake_i2c_program);
  auto config = mistake_i2c_program_get_default_config(offset);
  sm_config_set_out_pins(&config, SDA, 1);
  sm_config_set_set_pins(&config, SDA, 1);
  sm_config_set_in_pins(&config, SDA);
  sm_config_set_sideset_pins(&config, SCL);
  sm_config_set_jmp_pin(&config, SDA);
  sm_config_set_out_shift(&config, false, true, 16);
  // This transport only writes. Disable RX autopush so its FIFO cannot stall.
  sm_config_set_in_shift(&config, false, false, 8);
  sm_config_set_clkdiv(&config, float(clock_get_hz(clk_sys)) / (32 * 400000));
  gpio_pull_up(SDA); gpio_pull_up(SCL);
  pio_sm_set_pins_with_mask(bus, sm, PINS, PINS);
  pio_sm_set_pindirs_with_mask(bus, sm, PINS, PINS);
  pio_gpio_init(bus, SDA); pio_gpio_init(bus, SCL);
  gpio_set_oeover(SDA, GPIO_OVERRIDE_INVERT);
  gpio_set_oeover(SCL, GPIO_OVERRIDE_INVERT);
  pio_sm_set_pins_with_mask(bus, sm, 0, PINS);
  pio_sm_init(bus, sm, offset, &config);
  reset();
  return true;
}

inline bool put(uint16_t word, uint64_t deadline) {
  while (pio_sm_is_tx_fifo_full(bus, sm)) {
    if (error() || expired(deadline)) return false;
    tight_loop_contents();
  }
  if (error() || expired(deadline)) return false;
  // Narrow writes replicate into both halves, matching PIO's 16-bit autopull.
  *reinterpret_cast<io_rw_16 *>(&bus->txf[sm]) = word;
  return true;
}

inline uint16_t lines(bool scl, bool sda) {
  return mistake_i2c_lines_program_instructions[(scl ? 2 : 0) | (sda ? 1 : 0)];
}

inline bool stop(uint64_t deadline) {
  return put(2u << 10, deadline) && put(lines(false, false), deadline)
      && put(lines(true, false), deadline) && put(lines(true, true), deadline);
}

inline bool flush(uint64_t deadline) {
  const uint32_t stalled = 1u << (PIO_FDEBUG_TXSTALL_LSB + sm);
  bus->fdebug = stalled;
  while (!(bus->fdebug & stalled)) {
    if (error() || expired(deadline)) return false;
    tight_loop_contents();
  }
  return !error() && !expired(deadline);
}

inline bool write(uint8_t address, const uint8_t *data, size_t length) {
  if (sm < 0 || !data || !length || address > 0x7f) return false;
  const uint64_t deadline = time_us_64() + 25000;
  // Do not start on a stuck bus. Clock-stretch waits in PIO are also bounded
  // by put/flush, so a disconnected or shorted screen cannot hang USB.
  while (!gpio_get(SDA) || !gpio_get(SCL)) {
    if (expired(deadline)) return false;
    tight_loop_contents();
  }
  bool ok = put(2u << 10, deadline) && put(lines(true, false), deadline)
      && put(lines(false, false), deadline) && put(pio_encode_mov(pio_isr, pio_null), deadline)
      && put(uint16_t(address) << 2 | 1u, deadline);
  for (size_t i = 0; ok && i < length; ++i) {
    // Keep FINAL=0 even on the last byte: a final-byte NAK is a real failure.
    ok = put(uint16_t(data[i]) << 1 | 1u, deadline);
  }
  if (ok && stop(deadline) && flush(deadline)) return true;
  reset();
  const uint64_t recovery = time_us_64() + 1000;
  (void)(stop(recovery) && flush(recovery));
  reset();
  return false;
}
}
