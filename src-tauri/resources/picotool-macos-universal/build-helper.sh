#!/bin/bash
# Offline rebuild of the accompanying universal helper and replaceable LGPL library.
# Prerequisites: macOS, Apple Command Line Tools, and CMake (tested with 3.31.6).
# CMAKE=/absolute/path/to/cmake ./build-helper.sh [output-directory]
set -euo pipefail
source_dir=$(cd "$(dirname "$0")" && pwd)
output_dir=${1:-$source_dir}
cmake_bin=${CMAKE:-cmake}
command -v "$cmake_bin" >/dev/null || { echo "Set CMAKE to an installed CMake executable." >&2; exit 1; }
[[ $(uname -s) == Darwin ]] || { echo "This recipe requires macOS." >&2; exit 1; }
(cd "$source_dir" && /usr/bin/shasum -a 256 -c SOURCE_SHA256SUMS)
task_tmp=${TMPDIR:-/tmp}
build_dir=$(mktemp -d "${task_tmp%/}/droidfun-helper-build.XXXXXXXX")
trap 'rm -rf "$build_dir"' EXIT
mkdir -p "$build_dir/src" "$build_dir/libusb-build" "$build_dir/install" "$build_dir/result"
tar -xjf "$source_dir/libusb-1.0.27-source.tar.bz2" -C "$build_dir/src"
tar -xzf "$source_dir/picotool-2.0.0-source.tar.gz" -C "$build_dir/src"
tar -xzf "$source_dir/pico-sdk-2.0.0-source.tar.gz" -C "$build_dir/src"
export MACOSX_DEPLOYMENT_TARGET=13.0
export SDKROOT
SDKROOT=$(xcrun --show-sdk-path)
export CC=/usr/bin/clang CXX=/usr/bin/clang++
# A conservative fixed command limit also works when sandboxed sysctl is denied.
export lt_cv_sys_max_cmd_len=32768
(
  cd "$build_dir/libusb-build"
  CFLAGS='-O2 -arch arm64 -arch x86_64 -mmacosx-version-min=13.0' \
  LDFLAGS='-arch arm64 -arch x86_64 -mmacosx-version-min=13.0' \
    "$build_dir/src/libusb-1.0.27/configure" --prefix="$build_dir/install" \
      --disable-static --disable-examples-build --disable-tests-build
  /usr/bin/make -j4
  /usr/bin/make install
)
"$cmake_bin" -S "$build_dir/src/picotool-2.0.0" -B "$build_dir/picotool-build" \
  '-DCMAKE_OSX_ARCHITECTURES=arm64;x86_64' -DCMAKE_OSX_DEPLOYMENT_TARGET=13.0 \
  -DCMAKE_BUILD_TYPE=Release -DCMAKE_EXE_LINKER_FLAGS=-Wl,-headerpad_max_install_names \
  -DPICO_SDK_PATH="$build_dir/src/pico-sdk-2.0.0" \
  -DLIBUSB_INCLUDE_DIR="$build_dir/install/include/libusb-1.0" \
  -DLIBUSB_LIBRARIES="$build_dir/install/lib/libusb-1.0.0.dylib"
"$cmake_bin" --build "$build_dir/picotool-build" --parallel 4
cp "$build_dir/picotool-build/picotool" "$build_dir/result/picotool"
cp "$build_dir/install/lib/libusb-1.0.0.dylib" "$build_dir/result/libusb-1.0.0.dylib"
/usr/bin/install_name_tool -change "$build_dir/install/lib/libusb-1.0.0.dylib" \
  '@executable_path/libusb-1.0.0.dylib' "$build_dir/result/picotool"
/usr/bin/install_name_tool -id '@executable_path/libusb-1.0.0.dylib' "$build_dir/result/libusb-1.0.0.dylib"
for binary in picotool libusb-1.0.0.dylib; do
  file_path="$build_dir/result/$binary"
  /usr/bin/lipo "$file_path" -verify_arch arm64 x86_64
  for arch_name in arm64 x86_64; do
    xcrun vtool -arch "$arch_name" -show-build "$file_path" | /usr/bin/grep -q 'minos 13.0$'
  done
  while IFS= read -r dependency; do
    case "$dependency" in
      @executable_path/libusb-1.0.0.dylib|/usr/lib/*|/System/Library/*) ;;
      *) echo "Unexpected dependency in $binary: $dependency" >&2; exit 1 ;;
    esac
  done < <(/usr/bin/otool -L "$file_path" | /usr/bin/awk '/^\t/ {print $1}')
  /usr/bin/codesign --force --sign - "$file_path"
  /usr/bin/codesign --verify --strict "$file_path"
done
# Version/help do not enumerate or open USB devices.
"$build_dir/result/picotool" version
"$build_dir/result/picotool" help load >/dev/null
mkdir -p "$output_dir"
install -m 755 "$build_dir/result/picotool" "$output_dir/picotool"
install -m 755 "$build_dir/result/libusb-1.0.0.dylib" "$output_dir/libusb-1.0.0.dylib"
(cd "$output_dir" && /usr/bin/shasum -a 256 picotool libusb-1.0.0.dylib > BINARY_SHA256SUMS)
echo "Built universal arm64/x86_64 helper with minimum macOS 13.0: $output_dir"
