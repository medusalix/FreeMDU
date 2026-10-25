#!/bin/sh
# Builds the FreeMDU C interface for every target the ESPHome component supports
# and puts the static libraries where components/freemdu/__init__.py looks for them.
set -eu

cd "$(dirname "$0")"

for target in riscv32imc-unknown-none-elf riscv32imac-unknown-none-elf; do
  cargo build --manifest-path ffi/Cargo.toml --release --target "$target"
  mkdir -p "components/freemdu/lib/$target"
  cp "ffi/target/$target/release/libfreemdu_ffi.a" "components/freemdu/lib/$target/"
done
