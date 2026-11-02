#pragma once
#include <cstdio>
constexpr int PICO_UNIQUE_BOARD_ID_SIZE_BYTES = 8;
inline void pico_get_unique_board_id_string(char *p, size_t n) { snprintf(p, n, "0123456789abcdef"); }
