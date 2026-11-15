#!/usr/bin/env python3
"""Issue an onion delegation certificate for v2 WeTime ping signing.

The onion v3 identity key signs a certificate that binds a dedicated Ed25519
ping key to the onion host. The online pusher then signs pings with the
dedicated key, so the onion identity key can stay offline.

The certificate covers these exact bytes:

    WELIST-PING-DELEGATION-V1\n
    {"key":"<base64 SPKI DER>","onion":"<host>","valid_until":<unix>}

Usage:
    contrib/make_onion_delegation.py \
        --onion <address>.onion \
        --identity-key /var/lib/tor/hidden_service/hs_ed25519_secret_key \
        --ping-key /etc/bal/ping_private_key.pem \
        --days 365 \
        --out onion-delegation.json

`--identity-key` accepts either:
  * a Tor `hs_ed25519_secret_key` file (32-byte header + secret key), or
  * a standard PKCS#8 PEM Ed25519 private key.

Requirements: Python 3.10+, `cryptography`, and `pynacl` for Tor-format keys.
"""

import argparse
import base64
import hashlib
import json
import sys
import time
from pathlib import Path

try:
    from cryptography.hazmat.primitives import serialization
    from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
except ImportError:  # pragma: no cover - dependency hint
    print("error: the 'cryptography' package is required", file=sys.stderr)
    sys.exit(2)

DELEGATION_PREFIX = b"WELIST-PING-DELEGATION-V1\n"
TOR_HEADER_LEN = 32


def b32_decode(value: str) -> bytes:
    """Decode lowercase base32 without padding."""
    padding = "=" * ((8 - len(value) % 8) % 8)
    return base64.b32decode(value.upper() + padding)


def decode_onion(host: str) -> bytes:
    """Return the 32-byte Ed25519 identity public key of a v3 onion host."""
    host = host.strip().lower().rstrip(".")
    if not host.endswith(".onion"):
        raise ValueError("not an onion address")
    address = host[: -len(".onion")]
    if len(address) != 56:
        raise ValueError("unsupported onion address length")
    if not all(c in "abcdefghijklmnopqrstuvwxyz234567" for c in address):
        raise ValueError("invalid onion base32")
    decoded = b32_decode(address)
    if len(decoded) != 35:
        raise ValueError("invalid onion payload length")
    public_key, checksum, version = decoded[:32], decoded[32:34], decoded[34]
    if version != 3:
        raise ValueError("unsupported onion version")
    expected = hashlib.sha3_256(
        b".onion checksum" + public_key + bytes([version])
    ).digest()[:2]
    if checksum != expected:
        raise ValueError("onion checksum mismatch")
    return public_key


def load_ping_key(ping_key_path: Path) -> bytes:
    """Return the delegated ping public key as base64 SPKI DER."""
    key = serialization.load_pem_private_key(ping_key_path.read_bytes(), password=None)
    if not isinstance(key, Ed25519PrivateKey):
        raise ValueError("--ping-key must be an Ed25519 private key")
    return base64.b64encode(
        key.public_key().public_bytes(
            serialization.Encoding.DER,
            serialization.PublicFormat.SubjectPublicKeyInfo,
        )
    ).decode("ascii")


def sign_standard(key: Ed25519PrivateKey, message: bytes) -> bytes:
    """Sign with a standard Ed25519 private key."""
    return key.sign(message)


def sign_expanded(secret: bytes, public_key: bytes, message: bytes) -> bytes:
    """Sign with a Tor expanded Ed25519 secret key (a || prefix)."""
    try:
        from nacl.bindings import (
            crypto_core_ed25519_scalar_add,
            crypto_core_ed25519_scalar_mul,
            crypto_core_ed25519_scalar_reduce,
            crypto_scalarmult_ed25519_base_noclamp,
        )
    except ImportError as exc:  # pragma: no cover - dependency hint
        raise ValueError(
            "pynacl is required to sign with a Tor-format onion key"
        ) from exc

    scalar, prefix = secret[:32], secret[32:64]
    try:
        if crypto_scalarmult_ed25519_base_noclamp(scalar) != public_key:
            raise ValueError("onion secret key does not match the onion address")

        nonce = crypto_core_ed25519_scalar_reduce(
            hashlib.sha512(prefix + message).digest()
        )
        point = crypto_scalarmult_ed25519_base_noclamp(nonce)
        challenge = crypto_core_ed25519_scalar_reduce(
            hashlib.sha512(point + public_key + message).digest()
        )
        response = crypto_core_ed25519_scalar_add(
            nonce, crypto_core_ed25519_scalar_mul(challenge, scalar)
        )
        return point + response
    except ValueError:
        raise
    except Exception as exc:  # PyNaCl raises RuntimeError for invalid scalars
        raise ValueError(f"cannot sign with onion identity key: {exc}") from exc


def load_identity_signer(identity_key_path: Path, public_key: bytes):
    """Return a callable that signs with the onion identity key."""
    raw = identity_key_path.read_bytes()
    if raw.lstrip().startswith(b"-----BEGIN"):
        key = serialization.load_pem_private_key(raw, password=None)
        if not isinstance(key, Ed25519PrivateKey):
            raise ValueError("identity PEM key is not Ed25519")
        derived = key.public_key().public_bytes(
            serialization.Encoding.Raw, serialization.PublicFormat.Raw
        )
        if derived != public_key:
            raise ValueError("identity key does not match the onion address")
        return lambda message: sign_standard(key, message)

    if len(raw) < TOR_HEADER_LEN + 64:
        raise ValueError("Tor secret key file is too short")
    secret = raw[TOR_HEADER_LEN : TOR_HEADER_LEN + 64]

    # Tor stores the expanded secret key (scalar || prefix). Some tools export
    # a standard 32-byte seed instead, so accept that form too.
    try:
        seed_key = Ed25519PrivateKey.from_private_bytes(secret[:32])
        derived = seed_key.public_key().public_bytes(
            serialization.Encoding.Raw, serialization.PublicFormat.Raw
        )
    except Exception:
        seed_key = None
        derived = b""
    if seed_key is not None and derived == public_key:
        return lambda message: sign_standard(seed_key, message)

    return lambda message: sign_expanded(secret, public_key, message)


def canonical_message(key_b64: str, onion: str, valid_until: int) -> bytes:
    """Build the exact certificate bytes signed by the identity key."""
    body = {"key": key_b64, "onion": onion, "valid_until": valid_until}
    encoded = json.dumps(
        body, separators=(",", ":"), sort_keys=True, ensure_ascii=False
    ).encode("utf-8")
    return DELEGATION_PREFIX + encoded


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--onion", required=True, help="onion host (address.onion)")
    parser.add_argument("--identity-key", required=True,
                        help="Tor hs_ed25519_secret_key or PKCS#8 PEM")
    parser.add_argument("--ping-key", required=True,
                        help="PKCS#8 PEM private key of the delegated ping signer")
    parser.add_argument("--days", type=int, default=365,
                        help="certificate lifetime in days (default: 365)")
    parser.add_argument("--out", default="onion-delegation.json",
                        help="output JSON path (default: onion-delegation.json)")
    args = parser.parse_args()

    if args.days <= 0 or args.days > 3650:
        print("error: --days must be between 1 and 3650", file=sys.stderr)
        return 2

    onion = args.onion.strip().lower().rstrip(".")
    public_key = decode_onion(onion)
    key_b64 = load_ping_key(Path(args.ping_key))
    valid_until = int(time.time()) + args.days * 86_400
    message = canonical_message(key_b64, onion, valid_until)
    signer = load_identity_signer(Path(args.identity_key), public_key)

    certificate = {
        "onion": onion,
        "key": key_b64,
        "valid_until": valid_until,
        "signature": base64.b64encode(signer(message)).decode("ascii"),
    }
    Path(args.out).write_text(json.dumps(certificate, indent=2) + "\n")
    print(f"wrote {args.out} (onion={onion}, valid_until={valid_until})")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, TypeError, ValueError) as err:
        print(f"error: {err}", file=sys.stderr)
        sys.exit(1)
