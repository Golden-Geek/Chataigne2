#!/usr/bin/env bash
set -euo pipefail

export DEBIAN_FRONTEND=noninteractive

build_tools=(build-essential ca-certificates git clang libclang-dev libatomic1 pkg-config dpkg-dev)
audio=(libasound2-dev libjack-jackd2-dev libpipewire-0.3-dev libdbus-1-dev libudev-dev)
target_packages=(
    "${audio[@]}"
    libwebkit2gtk-4.1-dev libxdo-dev libayatana-appindicator3-dev librsvg2-dev
    libavcodec-dev libavformat-dev libavutil-dev libswresample-dev libswscale-dev
)
if [[ $# -eq 1 && $1 == --armhf ]]; then
    dpkg --add-architecture armhf
    # The frontend's Rust code generators still link against host audio libraries.
    build_tools+=(gcc-arm-linux-gnueabihf g++-arm-linux-gnueabihf "${audio[@]}")
    target_packages+=(libvulkan-dev)
    for index in "${!target_packages[@]}"; do
        target_packages[index]+=:armhf
    done
elif [[ $# -ne 0 ]]; then
    echo 'Usage: install-linux-build-dependencies.sh [--armhf]' >&2
    exit 2
fi

apt-get update
apt-get install -y --no-install-recommends "${build_tools[@]}" "${target_packages[@]}"
