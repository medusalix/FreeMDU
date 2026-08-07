#!/bin/bash

TARGET_CHIPS=("c3" "c6")
TARGET_ARCH_c3="riscv32imc-unknown-none-elf"
TARGET_ARCH_c6="riscv32imac-unknown-none-elf"
BINS=("automqtt" "bridge" "proximity" "standalone")

CLEAN_MODE=false
if [ "$1" == "--clean" ]; then
    CLEAN_MODE=true
fi

for CHIP in "${TARGET_CHIPS[@]}"; do

    ARCH_VAR="TARGET_ARCH_${CHIP}"
    ARCH="${!ARCH_VAR}"

    for ITEM in "${BINS[@]}"; do

        BIN_NAME="$ITEM"
        FILE_NAME="compile_${CHIP}_${ITEM}.sh"

        if [ "$CLEAN_MODE" = true ]; then
            if [ -f "$FILE_NAME" ]; then
                rm -f "$FILE_NAME"
                echo "Deleted: $FILE_NAME"
            fi
        else
            echo "cargo run --features esp32${CHIP} --target ${ARCH} --release --bin ${BIN_NAME}" > "$FILE_NAME"
            chmod +x "$FILE_NAME"
            echo "Created: $FILE_NAME"
        fi
    done
done

if [ "$CLEAN_MODE" = false ]; then
    echo "Remove them by --clean option: ./generate_compilescripts.sh --clean"
fi

