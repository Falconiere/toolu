#!/usr/bin/env bash
set -euo pipefail

target=${1:?musl target is required}
if [ "$target" = x86_64-unknown-linux-musl ] && [ -f /etc/apt/apt-mirrors.txt ]; then
  sudo sed -i 's|http://azure.archive.ubuntu.com/ubuntu|https://archive.ubuntu.com/ubuntu|g' /etc/apt/apt-mirrors.txt
fi
