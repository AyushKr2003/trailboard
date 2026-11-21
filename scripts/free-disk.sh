#!/usr/bin/env bash
# Make room on a GitHub-hosted Ubuntu runner: remove preinstalled toolchains
# we don't use, and add swap on the second disk. A CEF checkout plus its
# build output needs ~70 GB.
set -uo pipefail

df -h / /mnt
sudo rm -rf \
    /usr/share/dotnet /usr/local/lib/android /opt/ghc /usr/local/.ghcup \
    /opt/hostedtoolcache /usr/local/share/boost /usr/share/swift \
    /usr/local/share/powershell /usr/local/share/chromium /usr/local/lib/node_modules \
    /opt/microsoft /opt/google /opt/az /usr/lib/jvm
sudo docker system prune -af >/dev/null 2>&1 || true
sudo apt-get clean

# Swap: the link of libcef.so peaks above the runner's 16 GB.
if [[ ! -f /mnt/swapfile ]]; then
    sudo fallocate -l 12G /mnt/swapfile && sudo chmod 600 /mnt/swapfile &&
        sudo mkswap /mnt/swapfile >/dev/null && sudo swapon /mnt/swapfile
fi

sudo mkdir -p /mnt/cef && sudo chown "$(id -u):$(id -g)" /mnt/cef
df -h / /mnt
free -h
