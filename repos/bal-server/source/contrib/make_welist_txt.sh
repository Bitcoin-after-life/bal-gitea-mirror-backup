#!/usr/bin/env bash
# Print the WeTime v3 DNSSEC TXT record for an executor hostname.
set -euo pipefail

usage() {
    cat <<'EOF'
Usage: make_welist_txt.sh <domain> [key]

  <domain>  Executor hostname or URL, for example we.example.com
  [key]     Ed25519 private/public PEM file, or the base64
            SubjectPublicKeyInfo DER value to publish.

If [key] is omitted, the script uses ./private_key.pem when it exists,
otherwise it generates ./private_key.pem and ./public_key.pem.

The record is published at _welist-key.<domain> with value:
  v=we1; alg=ed25519; key=<base64 SPKI DER>
EOF
}

if [ "$#" -lt 1 ] || [ "$#" -gt 2 ]; then
    usage >&2
    exit 2
fi

domain=$1
key_input=${2:-}

# Normalize the hostname: strip scheme, path, trailing dot, and lowercase.
domain=${domain#http://}
domain=${domain#https://}
domain=${domain%%/*}
domain=${domain%.}
domain=$(printf '%s' "$domain" | tr '[:upper:]' '[:lower:]')

# Avoid doubling the prefix if the caller already supplied it.
domain=${domain#_welist-key.}

if [ -z "$domain" ] || ! printf '%s' "$domain" | grep -Eq '^[a-z0-9.-]+$'; then
    echo "error: invalid hostname: $domain" >&2
    echo "       use the host only, without scheme, port, or path" >&2
    exit 1
fi

if printf '%s' "$domain" | grep -Eq '^[0-9.]+$'; then
    echo "error: DNSSEC key discovery requires a hostname, not an IP address" >&2
    exit 1
fi

if [ -z "$key_input" ]; then
    private_key="./private_key.pem"
    public_key="./public_key.pem"

    if [ -f "$private_key" ]; then
        key_input=$private_key
        echo "Using existing $private_key" >&2
    else
        if [ -e "$private_key" ] || [ -e "$public_key" ]; then
            echo "error: $private_key or $public_key exists and will not be overwritten" >&2
            exit 1
        fi

        if ! (umask 077; openssl genpkey -algorithm ED25519 -out "$private_key"); then
            echo "error: cannot generate $private_key" >&2
            exit 1
        fi
        if ! openssl pkey -in "$private_key" -pubout -out "$public_key"; then
            echo "error: cannot derive $public_key" >&2
            exit 1
        fi
        chmod 600 "$private_key"
        chmod 644 "$public_key"

        key_input=$private_key
        cat >&2 <<EOF
Generated Ed25519 key pair:
  private key: $private_key (keep secret)
  public key:  $public_key

Set SSL_KEY_PATH and BAL_SERVER_PUB_KEY_PATH to these files.
EOF
    fi
fi

tmp_der=$(mktemp)
trap 'rm -f "$tmp_der"' EXIT

if [ -f "$key_input" ]; then
    if ! openssl pkey -in "$key_input" -pubout -outform DER -out "$tmp_der" 2>/dev/null; then
        if ! openssl pkey -pubin -in "$key_input" -outform DER -out "$tmp_der" 2>/dev/null; then
            echo "error: cannot read an Ed25519 key from $key_input" >&2
            exit 1
        fi
    fi
else
    printf '%s' "$key_input" | tr -d '[:space:]' \
        | openssl base64 -d -A -out "$tmp_der" 2>/dev/null || {
            echo "error: key is neither a PEM file nor a base64 SPKI DER value" >&2
            exit 1
        }
fi

der_len=$(wc -c < "$tmp_der" | tr -d ' ')
prefix_hex=$(od -An -tx1 -N12 "$tmp_der" | tr -d ' \n')
if [ "$der_len" -ne 44 ] || [ "$prefix_hex" != "302a300506032b6570032100" ]; then
    echo "error: key is not a 44-byte Ed25519 SubjectPublicKeyInfo DER" >&2
    exit 1
fi

key_b64=$(openssl base64 -A -in "$tmp_der")
value="v=we1; alg=ed25519; key=$key_b64"
name="_welist-key.$domain"

cat <<EOF
TXT name:  $name
TXT value: $value

BIND zone line:
$name. 3600 IN TXT "$value"
EOF
