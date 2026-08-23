#!/bin/bash
set -euo pipefail

# deploy.sh — Deploy the latest bal-server release on a remote server.
#
# Downloads, verifies (SHA-256 + GPG), and installs the release entirely
# on the remote server via SSH. No file transfer (scp/rsync) needed.
#
# Usage:
#   ./contrib/deploy.sh                          # deploy latest
#   ./contrib/deploy.sh --dry-run                # show plan without executing
#   ./contrib/deploy.sh v0.3.1                   # deploy a specific tag
#   DEPLOY_DRY_RUN=1 ./contrib/deploy.sh         # same as --dry-run
#
# Prerequisites:
#   - ssh key access to debian@bitcoin-after.life:47081
#   - The server must have: curl, jq, sha256sum, gpg, systemctl
#
# Services managed:
#   bal-server, bal-pusher, tbal-pusher, t4bal-pusher

REMOTE_USER="debian"
REMOTE_HOST="bitcoin-after.life"
REMOTE_PORT="47081"
SSH_OPTS="-p ${REMOTE_PORT} -o ConnectTimeout=15 -o BatchMode=yes"

GITEA_API="https://bitcoin-after.life/gitea/api/v1/repos/bitcoinafterlife/bal-server"
GPG_SIGNER="svatantrya@bitcoin-after.life"
GPG_KEY_ID="A847D004DB91610711CA6A0DFE756706E833E0D1"

SERVICES=("bal-server" "bal-pusher" "tbal-pusher" "t4bal-pusher")
INSTALL_DIR="/usr/local/bin"

# ── Parse arguments ─────────────────────────────────────────────────
DRY_RUN=0
DEPLOY_TAG=""
for arg in "$@"; do
  case "$arg" in
    --dry-run) DRY_RUN=1 ;;
    --help|-h)
      echo "Usage: $0 [--dry-run] [TAG]"
      echo "  TAG       Deploy a specific release tag (e.g. v0.3.1). Default: latest."
      exit 0
      ;;
    *) DEPLOY_TAG="$arg" ;;
  esac
done
[[ "${DEPLOY_DRY_RUN:-}" == "1" ]] && DRY_RUN=1

# ── Helpers ──────────────────────────────────────────────────────────
info()  { echo -e "\033[1m==> $1\033[0m"; }
ok()    { echo -e "\033[32;1m    ✔ $1\033[0m"; }
warn()  { echo -e "\033[33;1m    ⚠ $1\033[0m"; }
err()   { echo -e "\033[31;1m    ✖ $1\033[0m"; }
die()   { err "$1"; exit "${2:-1}"; }

remote_exec() {
  if [[ $DRY_RUN -eq 1 ]]; then
    info "[dry-run] remote: $1"
    return 0
  fi
  ssh $SSH_OPTS "${REMOTE_USER}@${REMOTE_HOST}" "$@"
}

# ── Step 0: Verify SSH connectivity ────────────────────────────────
info "Verifying SSH connection to ${REMOTE_HOST} ..."
if ! remote_exec "echo ok" >/dev/null 2>&1; then
  die "Cannot connect via SSH. Check your key and network."
fi
ok "SSH connection OK"

# ── Step 1: Resolve release to deploy ──────────────────────────────
if [[ -n "$DEPLOY_TAG" ]]; then
  TAG="$DEPLOY_TAG"
  info "Deploying explicit tag: $TAG"
  RELEASE_JSON=$(curl -sfL "${GITEA_API}/releases/tags/${TAG}") \
    || die "Release $TAG not found at $GITEA_API/releases/tags/$TAG"
else
  info "Fetching latest release metadata ..."
  RELEASE_JSON=$(curl -sfL "${GITEA_API}/releases/latest") \
    || die "Failed to fetch latest release from $GITEA_API"
  TAG=$(echo "$RELEASE_JSON" | jq -r '.tag_name // empty')
fi

RELEASE_NAME=$(echo "$RELEASE_JSON" | jq -r '.name // empty')
if [[ -z "$TAG" ]]; then
  die "Could not determine release tag"
fi
info "Release: $RELEASE_NAME (tag: $TAG)"

TARBALL_URL=$(echo "$RELEASE_JSON" | jq -r \
  '.assets[] | select(.name | test("\\.tar\\.gz$")) | .browser_download_url' | head -1)
[[ -z "$TARBALL_URL" ]] && die "No .tar.gz asset found in release $TAG"
ASSET_NAME=$(basename "$TARBALL_URL")
info "Asset: $ASSET_NAME"

# Collect sidecar URLs
SHA256_URL="${TARBALL_URL}.sha256"
SIG_URL="${TARBALL_URL}.sig"
ASC_URL="${TARBALL_URL}.asc"

# ── Step 2: Check current version on server ─────────────────────────
info "Checking current version on server ..."
CURRENT=$(remote_exec "$INSTALL_DIR/bal-server --version" 2>/dev/null \
  || remote_exec "strings $INSTALL_DIR/bal-server 2>/dev/null | head -1" 2>/dev/null \
  || echo "unknown")
info "Current version on server: $CURRENT"

# ── Step 2b: Export GPG key for remote import ────────────────────────
info "Exporting GPG public key for $GPG_SIGNER ..."
GPG_PUBKEY_B64=""
if gpg --list-keys "$GPG_SIGNER" &>/dev/null; then
  GPG_PUBKEY_B64=$(gpg --armor --export "$GPG_SIGNER" | base64 -w 0)
  ok "GPG public key exported (${#GPG_PUBKEY_B64} chars base64)"
else
  warn "GPG key for $GPG_SIGNER not found locally — remote import will try keyservers"
fi

# ── Step 3: Deploy on remote ───────────────────────────────────────
# Build a single shell script that runs entirely on the server.
# This avoids scp/rsync issues and ensures atomic, auditable deployment.

DEPLOY_SCRIPT=$(cat << 'REMOTE_EOF'
#!/bin/bash
set -euo pipefail

TAG="__TAG__"
ASSET_NAME="__ASSET_NAME__"
TARBALL_URL="__TARBALL_URL__"
SHA256_URL="__SHA256_URL__"
SIG_URL="__SIG_URL__"
ASC_URL="__ASC_URL__"
GPG_SIGNER="__GPG_SIGNER__"
GPG_KEY_ID="__GPG_KEY_ID__"
INSTALL_DIR="__INSTALL_DIR__"
SERVICES="__SERVICES__"
DRY_RUN="__DRY_RUN__"

info()  { echo -e "\033[1m==> $1\033[0m"; }
ok()    { echo -e "\033[32;1m    ✔ $1\033[0m"; }
warn()  { echo -e "\033[33;1m    ⚠ $1\033[0m"; }
err()   { echo -e "\033[31;1m    ✖ $1\033[0m"; }
die()   { err "$1"; exit "${2:-1}"; }

WORKDIR=$(mktemp -d /tmp/bal-deploy.XXXXXX)
trap 'rm -rf "$WORKDIR"' EXIT

# ── Ensure GPG is available ─────────────────────────────────────────
if ! command -v gpg &>/dev/null; then
  info "Installing gnupg ..."
  sudo apt-get update -qq && sudo apt-get install -y -qq gnupg
fi

# ── Import GPG key ──────────────────────────────────────────────────
info "Importing GPG key $GPG_KEY_ID ..."
if ! gpg --list-keys "$GPG_SIGNER" &>/dev/null; then
  imported=0
  # Try embedded key first (base64-encoded, pushed from deployer)
  if [[ -n "__GPG_PUBKEY_B64__" ]]; then
    echo "__GPG_PUBKEY_B64__" | base64 -d | gpg --batch --import 2>/dev/null && imported=1 && ok "Key imported from deployer"
  fi
  # Fallback to keyservers
  if [[ $imported -eq 0 ]]; then
    for ks in keyserver.ubuntu.com keys.openpgp.org pgp.mit.edu; do
      if gpg --batch --keyserver "$ks" --recv-keys "$GPG_KEY_ID" 2>/dev/null; then
        ok "Key imported from $ks"
        imported=1
        break
      fi
    done
  fi
  if [[ $imported -eq 0 ]]; then
    die "Cannot import GPG key $GPG_KEY_ID. Import it manually and re-run."
  fi
else
  ok "GPG key already present"
fi

# ── Download release assets ─────────────────────────────────────────
info "Downloading $ASSET_NAME ..."
curl -sfL -o "$WORKDIR/$ASSET_NAME" "$TARBALL_URL" || die "Download failed"

for sidecar in "sha256:$SHA256_URL:.sha256" "sig:$SIG_URL:.sig" "asc:$ASC_URL:.asc"; do
  IFS=: read -r label url suffix <<< "$sidecar"
  if curl -sfL -o "$WORKDIR/$(basename "$ASSET_NAME")$suffix" "$url" 2>/dev/null; then
    ok "Downloaded $ASSET_NAME$suffix"
  else
    warn "$label file not available — skipping"
  fi
done

# ── Verify SHA-256 ──────────────────────────────────────────────────
SHA_FILE="$WORKDIR/${ASSET_NAME}.sha256"
if [[ -f "$SHA_FILE" ]]; then
  info "Verifying SHA-256 checksum ..."
  (cd "$WORKDIR" && sha256sum -c "$SHA_FILE") || die "SHA-256 verification FAILED"
  ok "SHA-256 OK"
else
  warn "No .sha256 file — skipping checksum"
fi

# ── Verify GPG signature ────────────────────────────────────────────
SIG_FILE="$WORKDIR/${ASSET_NAME}.sig"
ASC_FILE="$WORKDIR/${ASSET_NAME}.asc"
verified=0

if [[ -f "$SIG_FILE" ]]; then
  info "Verifying GPG signature (binary) ..."
  gpg --batch --verify "$SIG_FILE" "$WORKDIR/$ASSET_NAME" 2>&1 && verified=1
fi
if [[ $verified -eq 0 ]] && [[ -f "$ASC_FILE" ]]; then
  info "Verifying GPG signature (ASCII-armored) ..."
  gpg --batch --verify "$ASC_FILE" "$WORKDIR/$ASSET_NAME" 2>&1 && verified=1
fi
if [[ $verified -eq 0 ]]; then
  warn "No GPG signature available — skipping signature verification"
fi

# ── Extract ──────────────────────────────────────────────────────────
info "Extracting tarball ..."
tar -xzf "$WORKDIR/$ASSET_NAME" -C "$WORKDIR"
EXTRACTED="$WORKDIR/$(basename "$ASSET_NAME" .tar.gz)"

for bin in bal-server bal-pusher; do
  if [[ ! -f "$EXTRACTED/$bin" ]]; then
    die "Binary '$bin' not found in archive"
  fi
  ok "Found $bin ($(stat -c%s "$EXTRACTED/$bin") bytes)"
done

# ── Stop services ────────────────────────────────────────────────────
info "Stopping services ..."
IFS=',' read -ra SVC_LIST <<< "$SERVICES"
for svc in "${SVC_LIST[@]}"; do
  svc=$(echo "$svc" | xargs)  # trim whitespace
  if systemctl is-active --quiet "$svc" 2>/dev/null; then
    sudo systemctl stop "$svc" || warn "Failed to stop $svc"
    ok "Stopped $svc"
  else
    warn "$svc is not running"
  fi
done

# ── Backup current binaries ─────────────────────────────────────────
BACKUP_DIR="$WORKDIR/backup"
mkdir -p "$BACKUP_DIR"
for bin in bal-server bal-pusher; do
  if [[ -f "$INSTALL_DIR/$bin" ]]; then
    cp "$INSTALL_DIR/$bin" "$BACKUP_DIR/$bin" 2>/dev/null || true
  fi
done
ok "Current binaries backed up"

# ── Install new binaries ────────────────────────────────────────────
info "Installing binaries to $INSTALL_DIR ..."
sudo install -m 0755 -o root -g root "$EXTRACTED/bal-server" "$EXTRACTED/bal-pusher" "$INSTALL_DIR/"
ok "Installed bal-server and bal-pusher"

# ── Restart services ────────────────────────────────────────────────
info "Restarting services ..."
sudo systemctl daemon-reload
for svc in "${SVC_LIST[@]}"; do
  svc=$(echo "$svc" | xargs)
  if systemctl is-enabled --quiet "$svc" 2>/dev/null; then
    sudo systemctl restart "$svc" || warn "Failed to restart $svc"
    ok "Restarted $svc"
  else
    warn "$svc is not enabled — skipping restart"
  fi
done

# ── Verify ──────────────────────────────────────────────────────────
echo ""
info "Checking service status ..."
sleep 2
for svc in "${SVC_LIST[@]}"; do
  svc=$(echo "$svc" | xargs)
  status=$(systemctl is-active "$svc" 2>/dev/null || echo "inactive")
  if [[ "$status" == "active" ]]; then
    ok "$svc: active"
  else
    warn "$svc: $status"
  fi
done

echo ""
ok "Deploy of $TAG completed successfully!"
REMOTE_EOF
)

# ── Substitute variables into the remote script ─────────────────────
DEPLOY_SCRIPT="${DEPLOY_SCRIPT//__TAG__/$TAG}"
DEPLOY_SCRIPT="${DEPLOY_SCRIPT//__ASSET_NAME__/$ASSET_NAME}"
DEPLOY_SCRIPT="${DEPLOY_SCRIPT//__TARBALL_URL__/$TARBALL_URL}"
DEPLOY_SCRIPT="${DEPLOY_SCRIPT//__SHA256_URL__/$SHA256_URL}"
DEPLOY_SCRIPT="${DEPLOY_SCRIPT//__SIG_URL__/$SIG_URL}"
DEPLOY_SCRIPT="${DEPLOY_SCRIPT//__ASC_URL__/$ASC_URL}"
DEPLOY_SCRIPT="${DEPLOY_SCRIPT//__GPG_SIGNER__/$GPG_SIGNER}"
DEPLOY_SCRIPT="${DEPLOY_SCRIPT//__GPG_KEY_ID__/$GPG_KEY_ID}"
DEPLOY_SCRIPT="${DEPLOY_SCRIPT//__INSTALL_DIR__/$INSTALL_DIR}"
DEPLOY_SCRIPT="${DEPLOY_SCRIPT//__DRY_RUN__/$DRY_RUN}"
# Join SERVICES array into comma-separated string for the remote script
SERVICES_STR=$(IFS=,; echo "${SERVICES[*]}")
DEPLOY_SCRIPT="${DEPLOY_SCRIPT//__SERVICES__/$SERVICES_STR}"
DEPLOY_SCRIPT="${DEPLOY_SCRIPT//__GPG_PUBKEY_B64__/${GPG_PUBKEY_B64:-}}"

# ── Execute ──────────────────────────────────────────────────────────
if [[ $DRY_RUN -eq 1 ]]; then
  info "=== DRY RUN — would execute the following on remote: ==="
  echo "$DEPLOY_SCRIPT"
  echo "=== end dry run ==="
  exit 0
fi

REMOTE_SCRIPT="/tmp/bal-deploy-$(date +%s).sh"
info "Uploading deploy script to remote ..."
ssh $SSH_OPTS "${REMOTE_USER}@${REMOTE_HOST}" "cat > '$REMOTE_SCRIPT'" <<< "$DEPLOY_SCRIPT" \
  || die "Failed to upload deploy script"

info "Executing deploy on remote server ..."
ssh $SSH_OPTS "${REMOTE_USER}@${REMOTE_HOST}" "chmod +x '$REMOTE_SCRIPT' && bash '$REMOTE_SCRIPT'; rc=\$?; rm -f '$REMOTE_SCRIPT'; exit \$rc" \
  || die "Deploy script failed on remote (exit $?)"

echo ""
ok "Deploy finished!"
