#!/usr/bin/env python3
"""Issue a WeTime ping key revocation certificate.

A revocation is permanent and key-scoped: every ping signed with the revoked
key is rejected for that host until the key is replaced. Two modes exist:

* Onion delegation (signed by the onion identity key):

    contrib/make_ping_key_revocation.py \\
        --onion <address>.onion \\
        --identity-key /var/lib/tor/hidden_service/hs_ed25519_secret_key \\
        --certificate onion-delegation.json \\
        --out onion-revocation.json

* DNSSEC TXT key (self-signed by the key being revoked):

    contrib/make_ping_key_revocation.py \\
        --host we.example.com \\
        --key private_key.pem \\
        --out dns-revocation.json

Submit the result to the WeTime service (idempotent, safe to retry):

    curl -sS -X POST -H 'Content-Type: application/json' \\
        --data @onion-revocation.json https://<service>/ping-keys/revoke

The signed bytes are:

    WELIST-PING-REVOCATION-V1\\n

    {"host":"<host>","key":"<base64 SPKI DER>","revoked_at":<unix>}

Key-loading and signing helpers are shared with `make_onion_delegation.py`.
Requirements: Python 3.10+, `cryptography`, and `pynacl` for Tor-format keys.
"""

import argparse
import base64
import json
import sys
import time
from pathlib import Path

try:
    from cryptography.hazmat.primitives import serialization
    from cryptography.hazmat.primitives.asymmetric.ed25519 import (
        Ed25519PrivateKey,
        Ed25519PublicKey,
    )
except ImportError:  # pragma: no cover - dependency hint
    print("error: the 'cryptography' package is required", file=sys.stderr)
    sys.exit(2)

from make_onion_delegation import decode_onion, load_identity_signer, load_ping_key

REVOCATION_PREFIX = b"WELIST-PING-REVOCATION-V1\n"


def validate_key_b64(key_b64: str) -> str:
    """Return `key_b64` after checking it is a base64 Ed25519 SPKI key."""
    try:
        der = base64.b64decode(key_b64, validate=True)
        public_key = serialization.load_der_public_key(der)
    except Exception as exc:
        raise ValueError(f"invalid delegated key: {exc}") from exc
    if not isinstance(public_key, Ed25519PublicKey):
        raise ValueError("delegated key is not Ed25519")
    return key_b64


def load_certificate(path: Path) -> tuple[str, str]:
    """Return `(onion, key_b64)` from a delegation certificate."""
    try:
        certificate = json.loads(path.read_text())
        onion = str(certificate["onion"]).strip().lower().rstrip(".")
        key = str(certificate["key"]).strip()
    except (OSError, ValueError, KeyError) as exc:
        raise ValueError(f"cannot read delegation certificate {path}: {exc}") from exc
    return onion, validate_key_b64(key)


def load_ping_private_key(path: Path) -> Ed25519PrivateKey:
    """Load a PKCS#8 Ed25519 private key."""
    try:
        key = serialization.load_pem_private_key(path.read_bytes(), password=None)
    except Exception as exc:
        raise ValueError(f"cannot read private key {path}: {exc}") from exc
    if not isinstance(key, Ed25519PrivateKey):
        raise ValueError(f"{path} is not an Ed25519 private key")
    return key


def canonical_message(host: str, key_b64: str, revoked_at: int) -> bytes:
    """Build the exact bytes covered by the revocation signature."""
    body = {"host": host, "key": key_b64, "revoked_at": revoked_at}
    encoded = json.dumps(
        body, separators=(",", ":"), sort_keys=True, ensure_ascii=False
    ).encode("utf-8")
    return REVOCATION_PREFIX + encoded


def main() -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    target = parser.add_mutually_exclusive_group()
    target.add_argument("--onion", help="onion host (address.onion)")
    target.add_argument("--host", help="DNS hostname of the executor")
    parser.add_argument("--identity-key",
                        help="Tor hs_ed25519_secret_key or PKCS#8 PEM identity key (onion mode)")
    parser.add_argument("--certificate",
                        help="existing onion delegation certificate (reads host and key)")
    parser.add_argument("--key", "--ping-key", dest="key",
                        help="PKCS#8 PEM of the key to revoke; also the signer for DNS mode")
    parser.add_argument("--revoked-at", type=int,
                        help="revocation Unix timestamp (default: now)")
    parser.add_argument("--out", default="ping-key-revocation.json",
                        help="output JSON path (default: ping-key-revocation.json)")
    parser.add_argument("--service-url",
                        help="WeTime service base URL, used to print the curl command")
    args = parser.parse_args()

    if args.revoked_at is not None and args.revoked_at <= 0:
        print("error: --revoked-at must be positive", file=sys.stderr)
        return 2
    revoked_at = args.revoked_at if args.revoked_at is not None else int(time.time())

    mode: str
    host: str
    key_b64: str

    if args.certificate:
        mode = "onion"
        certificate_onion, key_b64 = load_certificate(Path(args.certificate))
        host = args.onion.strip().lower().rstrip(".") if args.onion else certificate_onion
        if host != certificate_onion:
            print(
                f"error: certificate is bound to {certificate_onion}, not {host}",
                file=sys.stderr,
            )
            return 2
        if args.key:
            certificate_key = load_ping_key(Path(args.key))
            if certificate_key != key_b64:
                print("error: --key does not match the certificate key", file=sys.stderr)
                return 2
    elif args.onion:
        mode = "onion"
        host = args.onion.strip().lower().rstrip(".")
        if not args.key:
            print("error: onion revocations require --key or --certificate", file=sys.stderr)
            return 2
        key_b64 = validate_key_b64(load_ping_key(Path(args.key)))
    elif args.host:
        mode = "dns"
        host = args.host.strip().lower().rstrip(".")
        if not args.key:
            print("error: DNS revocations require --key", file=sys.stderr)
            return 2
        if args.identity_key:
            print("error: --identity-key is only valid for onion hosts", file=sys.stderr)
            return 2
        key_b64 = validate_key_b64(load_ping_key(Path(args.key)))
    else:
        print("error: provide --onion or --host", file=sys.stderr)
        return 2

    if not host or host.endswith("."):
        print("error: invalid host", file=sys.stderr)
        return 2

    message = canonical_message(host, key_b64, revoked_at)

    if mode == "onion":
        if not args.identity_key:
            print("error: onion revocations require --identity-key", file=sys.stderr)
            return 2
        public_key = decode_onion(host)
        signer = load_identity_signer(Path(args.identity_key), public_key)
        signature = signer(message)
    else:
        signing_key = load_ping_private_key(Path(args.key))
        signature = signing_key.sign(message)
    certificate = {
        "host": host,
        "key": key_b64,
        "revoked_at": revoked_at,
        "signature": base64.b64encode(signature).decode("ascii"),
    }
    Path(args.out).write_text(json.dumps(certificate, indent=2) + "\n")

    service = (args.service_url or "https://<service>").rstrip("/")
    print(f"wrote {args.out} (host={host}, revoked_at={revoked_at})")
    print("submit with:")
    print(
        "  curl -sS -X POST -H 'Content-Type: application/json' "
        f"--data @{args.out} {service}/ping-keys/revoke"
    )
    if mode == "dns":
        print("also remove the revoked key from the _welist-key TXT record.")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except ValueError as err:
        print(f"error: {err}", file=sys.stderr)
        sys.exit(1)
