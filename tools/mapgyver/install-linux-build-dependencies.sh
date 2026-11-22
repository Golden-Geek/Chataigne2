#!/usr/bin/env bash
set -euo pipefail

export DEBIAN_FRONTEND=noninteractive
apt-get update
apt-get install -y --no-install-recommends \
    build-essential ca-certificates git clang libclang-dev libatomic1 pkg-config dpkg-dev \
    libwebkit2gtk-4.1-dev libxdo-dev libayatana-appindicator3-dev librsvg2-dev \
    libasound2-dev libjack-jackd2-dev libpipewire-0.3-dev libdbus-1-dev libudev-dev \
    libavcodec-dev libavformat-dev libavutil-dev libswresample-dev libswscale-dev
