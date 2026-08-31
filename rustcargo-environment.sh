#!/bin/bash
set -e

# --- Docker check ---
if ! command -v docker &>/dev/null; then
    echo "Warning: The Docker command was not found. Docker may not be installed, so please check your Docker installation."
    echo "To continue, install Docker with the following command (Debian/Ubuntu):"
    echo
    echo "  sudo apt update && sudo apt install docker.io"
    echo "  sudo apt install docker-cli"
    echo
    echo "Check it and/or start it:" 
    echo
    echo "    sudo systemctl status docker"
    echo "    sudo systemctl start docker"
    echo
    echo "# 1. Create the 'docker' group (if the installer has not already created it)"
    echo "sudo groupadd docker"
    echo
    echo "# 2. Add the current user ($USER) to the docker group"
    echo "sudo usermod -aG docker $USER"
    echo
    echo "# 3. Refresh the group memberships in the current session (or log out and log back in)"
    echo "newgrp docker"
    echo
    echo "Make sure that your ESP32-C3 or ESP32-C6 device is connected via USB before running the script again."
    echo
    
    exit 1
fi

CONTAINER_NAME="rust-dev"
IMAGE="rust:latest"
WORKDIR="/usr/src/myapp"

ACTION=""

while getopts ":o:" opt; do
    case "$opt" in
        o)
            ACTION="$OPTARG"
            ;;
        \?)
            echo "Error: Invalid option: -$OPTARG" >&2
            echo "Usage: $0 [-o delete|new]" >&2
            exit 1
            ;;
        :)
            echo "Error: Option -$OPTARG requires an argument." >&2
            echo "Usage: $0 [-o delete|new]" >&2
            exit 1
            ;;
    esac
done

shift $((OPTIND -1))

if [ $# -gt 0 ]; then
    echo "Error: Unexpected argument(s): $*" >&2
    echo "Usage: $0 [-o delete|new]" >&2
    exit 1
fi

container_exists() {
    docker ps -a --format '{{.Names}}' | grep -qx "$CONTAINER_NAME"
}

container_running() {
    docker ps --format '{{.Names}}' | grep -qx "$CONTAINER_NAME"
}

delete_container() {
    if container_exists; then
        echo "Removing container '$CONTAINER_NAME'..."
        docker rm -f "$CONTAINER_NAME" >/dev/null
    else
        echo "Container '$CONTAINER_NAME' does not exist."
    fi
}

case "$ACTION" in
    delete)
        delete_container
        exit 0
        ;;
    new)
        delete_container
        ;;
    "")
        ;;
    *)
        echo "Unknown option: $ACTION"
        echo "Usage: $0 [-o delete|new]"
        exit 1
        ;;
esac

echo "Project: $PWD"

if container_running; then
    echo "Container is already running."
    docker exec "$CONTAINER_NAME" bash /usr/src/myapp/home/generate_compilescripts.sh
    exec docker exec -it \
        -w "$WORKDIR" \
        "$CONTAINER_NAME" bash
fi

if container_exists; then
    echo "Starting existing container..."
    docker start "$CONTAINER_NAME" >/dev/null
    exec docker exec -it \
        -w "$WORKDIR" \
        "$CONTAINER_NAME" bash
fi

echo "Creating new container..."

docker run -dit \
    --name "$CONTAINER_NAME" \
    --user "$(id -u):$(id -g)" \
    --group-add $(getent group dialout | cut -d: -f3) \
    -v "$PWD":"$WORKDIR" \
    -w "$WORKDIR" \
    --device=/dev/ttyACM0 \
    "$IMAGE" \
    bash >/dev/null

docker exec "$CONTAINER_NAME" bash /usr/src/myapp/home/generate_compilescripts.sh

echo "Installing espflash (first-time setup)..."
docker exec "$CONTAINER_NAME" cargo install espflash --locked    

echo "Setting up Bash history..."
docker exec "$CONTAINER_NAME" bash -c 'cat << "EOF" >> ~/.bash_history
cd /usr/src/myapp/home
cd /usr/src/myapp/protocol
cd /usr/src/myapp/tui
compile_c3_automqtt.sh
compile_c3_bridge.sh
compile_c3_proximity.sh
compile_c3_standalone.sh
compile_c6_automqtt.sh
compile_c6_bridge.sh
compile_c6_proximity.sh
compile_c6_standalone.sh
cargo run --features esp32c3 --target riscv32imc-unknown-none-elf --release --bin asciisending
cargo run --features esp32c3 --target riscv32imc-unknown-none-elf --release --bin checkforerror
cargo run --features esp32c3 --target riscv32imc-unknown-none-elf --release --bin proximity
cargo run --features esp32c3 --target riscv32imc-unknown-none-elf --release --bin receiver
EOF'

exec docker exec -it \
    -w "$WORKDIR" \
    "$CONTAINER_NAME" bash



