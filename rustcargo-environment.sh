#!/bin/bash
set -e

# --- Docker check ---
if ! command -v docker &>/dev/null; then
    echo "Error: Docker is not installed on the system!"
    echo "To continue, install Docker with the following command (Debian/Ubuntu):"
    echo "  sudo apt update && sudo apt install docker.io"
    exit 1
fi

CONTAINER_NAME="rust-dev"
IMAGE="rust:latest"
WORKDIR="/usr/src/myapp"

ACTION=""

while getopts "o:" opt; do
    case "$opt" in
        o)
            ACTION="$OPTARG"
            ;;
        *)
            echo "Usage: $0 [-o delete|new]"
            exit 1
            ;;
    esac
done

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

echo "Installing espflash (first-time setup)..."
docker exec "$CONTAINER_NAME" cargo install espflash --locked    

exec docker exec -it \
    -w "$WORKDIR" \
    "$CONTAINER_NAME" bash



