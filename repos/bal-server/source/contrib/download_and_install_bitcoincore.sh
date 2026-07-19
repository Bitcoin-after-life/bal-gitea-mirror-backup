#!/usr/bin/env bash
set -euo pipefail
# Copyright (c) 2024-2025 Joel Torres
# Distributed under the MIT software license, see the accompanying
# file LICENSE or https://opensource.org/license/mit.

VERSION=0.1.1
usage(){
  echo "\$ bash download_and_install_bitcoincore.sh <version>|update [username] [testnet] [force]"
  echo "\$ bash download_and_install_bitcoincore.sh version"
  echo "\$ bash download_and_install_bitcoincore.sh -h"
}
if [[ $1 == "version" ]]; then
  echo "Bitcoin Core Installer v$VERSION"
  exit 0
fi
if [[ $1 == "-h" ]]; then
  usage
  exit 0
fi

username=$(id -un)
if [[ "$2" != "" ]]; then
  username="$2"
fi

if [[ $3 == "testnet" ]]; then
  testnet=1
fi
if [[ $4 == "force" ]]; then
  force=1
fi

SOURCE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SOURCE_DIR/lib.sh"
PLATFORM_ARCH=$(uname -m)

if [ $(uname) == "Linux" ]; then
    PLATFORM_NAME="linux-gnu"

    BITCOIN_DIR=".bitcoin"
    SYSTEMD_DIR="/etc/systemd/system"
    GLOBAL_ALIASES="/etc/profile.d/bitcoin_aliases.sh"
else
    echo_e "Running script on unsupported platform, exiting"
    exit 1
fi


CMD_DEPENDENCIES="git gpg curl openssl"
for cmd in $CMD_DEPENDENCIES
do
    if [ $(which $cmd >/dev/null 2>&1; echo $?) != 0 ]; then
        echo_e "Command not found on path: $cmd, please install or add to path"
		sudo apt install -y $CMD_DEPENDENCIES
    fi
done
SYSTEMD_SERVICE="bitcoind.service"
BITCOIN_CONFIG_FILE="bitcoin.conf"
BITCOIN_CORE_URL="https://bitcoincore.org"
BIN_URL="$BITCOIN_CORE_URL/bin"
DOWNLOAD_URL="$BITCOIN_CORE_URL/en/download/"
if [ -n "$1" ] && [ $1 != "update" ]; then
    VERSION_NUM=$1
    if [[ ! $VERSION_NUM =~ ^[0-9]{1,3}.[0-9]{1,3}$ ]]; then
        echo_e "Error: invalid version number"
        exit 1
    fi
else
    VERSION_NUM=$(curl -s $DOWNLOAD_URL | grep "Latest version" | sed 's/.*Latest version: \([0-9]*\.[0-9]*\).*/\1/')
fi
VERSION_NUM_FULL="bitcoin-core-$VERSION_NUM"

KEYS_REPO="guix.sigs"
KEYS_REPO_URL="https://github.com/bitcoin-core/$KEYS_REPO"
KEYS_DIR="$KEYS_REPO/builder-keys"

if command -v bitcoin-cli &> /dev/null; then
  CURRENT_VERSION=$(bitcoin-cli --version | head -n1 | awk '{print $NF}')
else
  CURRENT_VERSION="none"
fi

is_bitcoin_core_running() {
  echo $(pgrep bitcoind >/dev/null 2>&1; echo $?)
}

have_to_update(){
  if [[ "v$VERSION_NUM" != "$CURRENT_VERSION" ]]; then
    echo 0
  else
    echo 1
  fi
}

start_bitcoin_core_exec() {
    echo_i "Starting bitcoind $1"
    bitcoind="bitcoind"
    if   [[ $1 == "testnet" ]]; then
      bitcoind="t$SYSTEMD_SERVICE" 
    fi
    sudo systemctl restart $bitcoind
}

stop_bitcoin_core_exec(){
    bitcoind="$SYSTEMD_SERVICE"
    debug_file="$BITCOIN_DIR_REAL/debug.log"
    if [[ "$1" == "testnet" ]]; then
      bitcoind="t$SYSTEMD_SERVICE" 
      debug_file="$BITCOIN_DIR_REAL/testnet3/debug.log"
    fi
    sudo systemctl stop $bitcoind
    #while true; do
    # last_line=$(tail -n 1 file.txt)
    #  if [[ $last_line == "*Shutdown: done*" ]]; then
    #    echo_i "$1 bitcoind terminated."
    #    break
    #  fi
    #done

}
stop_bitcoin_core() {
  stop_bitcoin_core_exec
  if [[ $testnet == 1 ]]; then
    stop_bitcoin_core testnet
  fi
}
start_bitcoin_core() {
  start_bitcoin_core_exec
  if [[ $testnet == 1 ]]; then
    start_bitcoin_core_exec testnet
  fi
}
download_bitcoin_core () {
    file_download_url="$BIN_URL/$VERSION_NUM_FULL/bitcoin-$VERSION_NUM-$PLATFORM_ARCH-$PLATFORM_NAME.tar.gz"
    bin_hash_url="$BIN_URL/$VERSION_NUM_FULL/SHA256SUMS"
    hash_sign_url="$bin_hash_url.asc"
    
    if [ ! -d $VERSION_NUM_FULL ]; then
        mkdir $VERSION_NUM_FULL
    fi

    for url in $file_download_url $bin_hash_url $hash_sign_url
    do
        echo_i "Downloading $url"
        curl -O --output-dir $VERSION_NUM_FULL $url
    done

    echo_i "Verifying sha-256 hash"
    cd $VERSION_NUM_FULL
    shasum -a 256 --ignore-missing --check SHA256SUMS
    if [ $? != 0 ]; then
        echo_e "Installation aborted: failure on computing hashes"
        exit 1
    fi

    touch .hash_verified
    cd ..

}

verify_bitcoin_core () {

    if [ ! -d $KEYS_REPO ]; then
        echo_i "Downloading builder-keys ($KEYS_REPO_URL)"
        git clone $KEYS_REPO_URL
    else
        echo_i "Updating builder-keys"
        git -C $KEYS_REPO pull
    fi
    
    echo_i "Importing and refreshing keys"
    gpg --import $KEYS_DIR/*
    gpg --keyserver hkps://keys.openpgp.org --refresh-keys

    echo_i "Verifying gpg signatures"
    cd $VERSION_NUM_FULL
    good_sign_str="Good signature"
    good_sign_out=$(gpg --verify SHA256SUMS.asc 2> >(grep "$good_sign_str"))
    if [[ ! $good_sign_out == *"$good_sign_str"* ]]; then
        echo_e "Installation aborted: no good gpg signatures found"
        exit 1
    fi
    echo "$good_sign_out"
    echo
    while true; do
        read -p "The above good signatures were found. Do you trust some of these? [y/n]: " answer
        case $answer in
            Y|y)
                touch .sign_verified; break;;
            N|n)
                echo_e "Installation aborted: keys not trusted"; exit 1;;
        esac
    done

    cd ..
}

install_bitcoin_core () {
    echo_i "Installing $VERSION_NUM_FULL"
    cd $VERSION_NUM_FULL
    tar xzf *.tar.gz

    # Copy binaries to /usr/local/bin
    extract_dir=$(find . -maxdepth 1 -type d -name 'bitcoin-*' | head -n1)
    if [ -d "$extract_dir/bin" ]; then
        sudo install -m 755 "$extract_dir/bin/bitcoind" /usr/local/bin/
        sudo install -m 755 "$extract_dir/bin/bitcoin-cli" /usr/local/bin/
    else
        echo_e "Installation aborted: binary directory not found in extracted archive"
        exit 1
    fi

    if [ $(is_bitcoin_core_running) == 0 ]; then
        echo_i "Stopping bitcoind before installing"
        stop_bitcoin_core
        sleep 5
    fi

    echo_s "Bitcoin Core $VERSION_NUM successfully installed!"
    touch .installed

    cd ..
    echo $VERSION_NUM > .version
}
install_and_start_services () {
    sudo systemctl daemon-reload
    sudo systemctl restart $SYSTEMD_SERVICE
    sudo systemctl enable $SYSTEMD_SERVICE
    if [[ $testnet == 1 ]]; then
      sudo systemctl restart "t$SYSTEMD_SERVICE"
      sudo systemctl enable "t$SYSTEMD_SERVICE"
    fi

}

init_bitcoin_core_config () {
    echo_i "USERNAME: $username"
    warning_file="File already present not forcing update."
    id $username >/dev/null 2>&1 || sudo adduser --gecos "" --disabled-password $username
    sudo adduser $username debian-tor
    userhome=$(getent passwd $username | cut -d: -f6)
    usergroup=$(sudo -u $username id -gn)

    BITCOIN_DIR_REAL="$userhome/$BITCOIN_DIR"
    BITCOIN_CONFIG="$BITCOIN_DIR_REAL/$BITCOIN_CONFIG_FILE"
    echo_i "GROUPNAME: $usergroup"
    echo_i "BITCOIN_DIR_REAL: $BITCOIN_DIR_REAL"
    echo_i "BITCOIN_CONFIG: $BITCOIN_CONFIG"

     sudo -u $username mkdir -p $BITCOIN_DIR_REAL

    sudo adduser $USER $usergroup
    sudo ln -s $BITCOIN_DIR_REAL $HOME
    bitcoinconf=$(cat << EOF
# Bitcoin daemon
server=1


# Activate v2 P2P
v2transport=1

# Connections
zmqpubhashblock=tcp://127.0.0.1:28332
zmqpubrawtx=tcp://127.0.0.1:28333

maxuploadtarget=5000

dbcache=2000
blocksonly=1
acceptnonstdtxn=0
peerbloomfilters=0
prune=550
listen=0
dnsseed=0
disablewallet=1

EOF
)

    if [ ! -e  "$BITCOIN_CONFIG" ] || [[ $force == 1 ]]; then
      printf "$bitcoinconf" | sudo -u $username tee "$BITCOIN_CONFIG" > /dev/null
    else
      echo_i "$warning_file: $BITCOIN_CONFIG"
    fi
    sudo -u $username chmod 640 $BITCOIN_CONFIG

    serviceconf=$(cat << EOF
# /etc/systemd/system/bitcoind.service

[Unit]
Description=Bitcoin daemon
After=network.target

[Service]

# Service execution
###################

ExecStart=/usr/local/bin/bitcoind -daemon \\
                                  -pid=/run/bitcoind/bitcoind.pid \\
                                  -conf=$BITCOIN_CONFIG \\
                                  -datadir=$BITCOIN_DIR_REAL \\
                                  -startupnotify="chmod g+r $BITCOIN_DIR_REAL/.cookie"

# Process management
####################
Type=forking
PIDFile=/run/bitcoind/bitcoind.pid
Restart=on-failure
TimeoutSec=300
RestartSec=30

# Directory creation and permissions
####################################
User=$username
UMask=0027

# /run/bitcoind
RuntimeDirectory=bitcoind
RuntimeDirectoryMode=0710

# Hardening measures
####################
# Provide a private /tmp and /var/tmp.
PrivateTmp=true

# Mount /usr, /boot/ and /etc read-only for the process.
ProtectSystem=full

# Disallow the process and all of its children to gain
# new privileges through execve().
NoNewPrivileges=true

# Use a new /dev namespace only populated with API pseudo devices
# such as /dev/null, /dev/zero and /dev/random.
PrivateDevices=true

# Deny the creation of writable and executable memory mappings.
MemoryDenyWriteExecute=true

[Install]
WantedBy=multi-user.target

EOF
)
    if [[ ! -e  "$SYSTEMD_DIR/$SYSTEMD_SERVICE" ]] || [[ $force == 1 ]]; then
      printf "$serviceconf" | sudo tee "$SYSTEMD_DIR/$SYSTEMD_SERVICE" > /dev/null
    else
      echo_i "$warning_file: $SYSTEMD_DIR/$SYSTEMD_SERVICE"
    fi

    if [[ $testnet == 1 ]]; then
      tserviceconf=$(printf "$serviceconf" | sed "s/bitcoind/tbitcoind/g")
      tserviceconf=$(printf "$tserviceconf" | sed "s/Bitcoin daemon/Bitcoin Testnet daemon/")
      tserviceconf=$(printf "$tserviceconf" | sed "s|tbitcoind -daemon|bitcoind -daemon -testnet|")

      if [[ ! -e  "$SYSTEMD_DIR/t$SYSTEMD_SERVICE" ]] || [[ $force == 1 ]]; then
        printf "$tserviceconf" | sudo tee "$SYSTEMD_DIR/t$SYSTEMD_SERVICE" > /dev/null
      else
        echo_i "$warning_file: $SYSTEMD_DIR/t$SYSTEMD_SERVICE"
      fi

      #install aliases avoid duplicates      
      
      echo $GLOBAL_ALIASES
      if ! grep -q "alias tbitcoind=" $GLOBAL_ALIASES ; then  
        printf "alias tbitcoind='bitcoind -testnet'" | sudo tee -a $GLOBAL_ALIASES > /dev/null
        echo_i "adding alias"
      else 
        echo_i "alias tbitcoind present"
      fi
      if ! grep -q "alias tbitcoin-cli=" $GLOBAL_ALIASES ; then  
        printf "alias tbitcoin-cli='bitcoin-cli -testnet'"  | sudo tee -a $GLOBAL_ALIASES > /dev/null 
        echo_i "adding alias"
      else 
        echo_i "alias tbitcoin-cli present"
      fi

    fi

    touch .config_init


}

if [ -e .version ] && [ $(cat .version) != $VERSION_NUM ] && [ -d $VERSION_NUM_FULL ]; then
    rm $VERSION_NUM_FULL/.installed
fi

if [ -e $VERSION_NUM_FULL/.hash_verified ] &&
   [ -e $VERSION_NUM_FULL/.sign_verified ] &&
   [ -e $VERSION_NUM_FULL/.installed ]
then
        echo_e "Bitcoin Core $VERSION_NUM already installed"
        exit 0
fi


if [ $(have_to_update) == 0 ]; then
  echo_i "New version available: v$VERSION_NUM(current: $CURRENT_VERSION)"
  init_bitcoin_core_config; 

  if [ ! -e $VERSION_NUM_FULL/.hash_verified ]; then download_bitcoin_core; fi
  if [ ! -e $VERSION_NUM_FULL/.sign_verified ]; then verify_bitcoin_core; fi
  if [ ! -e $VERSION_NUM_FULL/.installed ]; then install_bitcoin_core; fi
  if [ ! -e .config_init ]; then 
  install_and_start_services;
  fi

fi
