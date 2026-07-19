#!/bin/bash
set -euo pipefail
###############SETTINGS################
# These settings can be overridden by environment variables or arguments
# Usage: ./download_and_install_bal.sh <xpub> <fixed_fee> <willexecutor_url> <email> [info]
# Example: ./download_and_install_bal.sh bc1q... 50000 we.example.com info@example.com
# DO NOT commit this file with hardcoded secrets!

if [ -n "$1" ]; then xpub="$1"; else
  echo "Error: xpub address is required as first argument"
  echo "Usage: $0 <xpub> <fixed_fee> <willexecutor_url> <email> [info]"
  echo "Example: $0 bc1q... 50000 we.example.com info@example.com"
  exit 1
fi

if [ -n "$2" ]; then fixed_fee="$2"; else
  echo "Error: fixed_fee is required as second argument"
  echo "Usage: $0 <xpub> <fixed_fee> <willexecutor_url> <email> [info]"
  exit 1
fi

if [ -n "$3" ]; then willexecutor_url="$3"; else
  echo "Error: willexecutor_url is required as third argument"
  echo "Usage: $0 <xpub> <fixed_fee> <willexecutor_url> <email> [info]"
  exit 1
fi

if [ -n "$4" ]; then email="$4"; else
  echo "Error: email is required as fourth argument (for SSL certificate)"
  echo "Usage: $0 <xpub> <fixed_fee> <willexecutor_url> <email> [info]"
  exit 1
fi

if [ -n "$5" ]; then info="$5"; else info="commercial will executor server"; fi
#######################################



bal_server_conf=$(cat << EOF
BAL_SERVER_DB_FILE=/home/bal/bal.db
BAL_SERVER_BIND_ADDRESS=127.0.0.1
BAL_SERVER_BIND_PORT=9137
BAL_SERVER_BITCOIN_ADDRESS="$xpub"
BAL_SERVER_BITCOIN_FIXED_FEE=$fixed_fee
BAL_SERVER_INFO="$info"

EOF
)
bal_pusher_conf=$(cat << EOF
BAL_PUSHER_DB_FILE=/home/bal/bal.db
BAL_PUSHER_BITCOIN_COOKIE_FILE=/home/bitcoin/.bitcoin/.cookie

EOF
)
if ! command -v jq &> /dev/null; then
    echo "Installing jq... "
        sudo apt-get update
        sudo apt-get install -y jq
fi
if ! command -v curl &> /dev/null; then
    echo "Installing curl... "
        sudo apt-get update
        sudo apt-get install -y curl
fi

if ! command -v certbot &> /dev/null; then
    echo "Installing certbot... "
        sudo apt-get update
        sudo apt-get install -y certbot python3-certbot-nginx
fi

if ! command -v nginx &> /dev/null; then
    echo "Installing nginx... "
        sudo apt-get update
        sudo apt-get install -y nginx
fi


################## DOWNLOAD AND INSTALL BAL  #####################
url_releases="https://bitcoin-after.life/gitea/api/v1/repos/bitcoinafterlife/bal-server/releases/latest"

url_asset="$(curl -sfL $url_releases | jq -r .assets[0].browser_download_url)"
if [ -z "$url_asset" ] || [ "$url_asset" = "null" ]; then
    echo "Error: could not fetch download URL from Gitea releases"
    exit 1
fi
tempdir=$(mktemp -d)
cd $tempdir

curl -sfL -O "$url_asset"
echo "$url_asset"
filename=$(basename "$url_asset")
tar -xzf $filename

dirname=$(basename "$filename" .tar.gz)
echo "dirname $dirname"
cd $dirname
sudo install -m 0755 -o root -g root -t /usr/local/bin bal-server
sudo install -m 0755 -o root -g root -t /usr/local/bin bal-pusher

id bal >/dev/null 2>&1 || sudo adduser --gecos "" --disabled-password bal
printf "$bal_server_conf" | sudo -u bal tee "/home/bal/bal-server.env" > /dev/null
sudo chmod 600 /home/bal/bal-server.env
printf "$bal_pusher_conf" | sudo -u bal tee "/home/bal/bal-pusher.env" > /dev/null
sudo chmod 600 /home/bal/bal-pusher.env




################## SERVICES  #####################
bal_server_service=$(cat << EOF
[Unit]
Description=bal-server daemon
After=network.target

[Service]

EnvironmentFile=/home/bal/bal-server.env

ExecStart=/usr/local/bin/bal-server 

SyslogIdentifier=bal-server

Type=simple
PIDFile=/run/bal-server/bal-server.pid
Restart=always
TimeoutSec=300
RestartSec=5

User=bal
UMask=0027

RuntimeDirectory=bal-server
RuntimeDirectoryMode=0710


ProtectSystem=full

NoNewPrivileges=true

PrivateDevices=true

[Install]
WantedBy=multi-user.target


EOF
)
bal_pusher_service=$(cat << EOF
[Unit]
Description=bal-pusher daemon
After=bitcoind.service

[Service]

EnvironmentFile=/home/bal/bal-pusher.env

ExecStart=/usr/local/bin/bal-pusher bitcoin

StandardOutput=syslog
StandardError=syslog
SyslogIdentifier=bal-pusher


Type=simple
PIDFile=/run/bal-pusher/bal-pusher.pid
Restart=always
TimeoutSec=120
RestartSec=300
KillMode=process

User=bal
Group=bitcoin
UMask=0027

RuntimeDirectory=bal-pusher
RuntimeDirectoryMode=0710

PrivateTmp=true

ProtectSystem=full

NoNewPrivileges=true

PrivateDevices=true

MemoryDenyWriteExecute=true

[Install]
WantedBy=multi-user.target

EOF
)


printf "$bal_server_service" | sudo tee "/etc/systemd/system/bal-server.service" > /dev/null
printf "$bal_pusher_service" | sudo tee "/etc/systemd/system/bal-pusher.service" > /dev/null

sudo systemctl daemon-reload
sudo systemctl enable bal-server.service
sudo systemctl restart bal-server.service

sudo systemctl enable bal-pusher.service
sudo systemctl restart bal-pusher.service

################## TODO SSL #####################
sudo systemctl restart nginx
echo "Asking certificate for domain  $willexecutor_url..."
sudo certbot --nginx --non-interactive --agree-tos --email $email -d $willexecutor_url

if [ -n "/etc/letsencrypt/live/$willexecutor_url/fullchain.pem" ]; then
    sudo openssl x509 -in "/etc/letsencrypt/live/$willexecutor_url/fullchain.pem" -noout -text | grep -E "Issuer:|Subject:|Not Before:|Not After :"
else
    echo "Error getting certificate"
    exit 1
fi

(crontab -l 2>/dev/null; echo "0 0,12 * * * /usr/bin/certbot renew --quiet") | crontab -
echo "ssl certificate installed"
sudo systemctl status nginx




################## NGNIX ########################
nginx_reverse_proxy=$(cat << EOF
server {
    listen 443 ssl;
    server_name $willexecutor_url;

    ssl_certificate /etc/letsencrypt/live/$willexecutor_url/fullchain.pem; # managed by Certbot
    ssl_certificate_key /etc/letsencrypt/live/$willexecutor_url/privkey.pem; # managed by Certbot

    location / {
        proxy_pass http://127.0.0.1:9137;
        # Include standard proxy headers from above
        proxy_set_header Host \$host;
        proxy_set_header X-Real-IP \$remote_addr;
        proxy_set_header X-Forwarded-For \$proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto \$scheme;
    }
}

server {
    listen 80;
    server_name $willexecutor_url;
    return 301 https://$willexecutor_url;
}

EOF
)

printf "$nginx_reverse_proxy" | sudo tee "/etc/nginx/sites-available/$willexecutor_url" > /dev/null
sudo ln -s "/etc/nginx/sites-available/$willexecutor_url"  "/etc/nginx/sites-enabled/" || true
sudo systemctl restart nginx

rm -r $tempdir
echo "done"
