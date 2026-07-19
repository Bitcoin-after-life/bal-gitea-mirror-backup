#!/bin/bash
set -euo pipefail

sudo apt-get update
sudo apt-get install -y \
  gpg \
  wget \
  lsb-release

ARCH=$(dpkg --print-architecture)
DISTRO=$(lsb_release -cs)

sudo tee /etc/apt/sources.list.d/tor.list > /dev/null <<EOF
deb     [arch=$ARCH signed-by=/usr/share/keyrings/tor-archive-keyring.gpg] https://deb.torproject.org/torproject.org $DISTRO main
deb-src [arch=$ARCH signed-by=/usr/share/keyrings/tor-archive-keyring.gpg] https://deb.torproject.org/torproject.org $DISTRO main
EOF

wget -qO- https://deb.torproject.org/torproject.org/A3C4F0F979CAA22CDBA8F512EE8CBC9E886DDD89.asc | gpg --dearmor | sudo tee /usr/share/keyrings/tor-archive-keyring.gpg >/dev/null
sudo apt-get update
sudo apt install -y tor deb.torproject.org-keyring

# Backup existing torrc before modifying
if [ -f /etc/tor/torrc ]; then
    sudo cp /etc/tor/torrc /etc/tor/torrc.bak.$(date +%Y%m%d%H%M%S)
fi
sudo sed -i '/^ControlPort/d; /^CookieAuthentication/d; /^CookieAuthFileGroupReadable/d' /etc/tor/torrc

sudo tee -a /etc/tor/torrc > /dev/null << EOF

# Added by script ($(date))
 ControlPort 127.0.0.1:9051
 CookieAuthentication 1
 CookieAuthFileGroupReadable 1
 DisableDebuggerAttachment 1
EOF
sudo systemctl restart tor
