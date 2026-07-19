# Glossary and Bitcoin Domain Knowledge

## Quick Reference
- **What this file contains:** Bitcoin-specific concepts, protocols, and standards needed to understand this codebase.
- **See also:** [01_project_overview.md](01_project_overview.md), [04_modules_detail.md](04_modules_detail.md)

## BIP-84: Derivation Path for P2WPKH

BIP-84 defines the derivation path for native SegWit (Bech32) addresses (P2WPKH). The standard path is `m/84'/<coin_type>'/<account>'/0/<address_index>`. In this project, `xpub.rs` derives P2WPKH addresses from an xpub or zpub using the path `m/84'/coin_type'/account'/0/index`. The `bitcoin` crate's `Xpub::derive_pub` and `Secp256k1` are used in `src/xpub.rs`.

## XPub, ZPub, and Extended Public Keys

An **XPub** (Extended Public Key) is a master key that allows derivation of child public keys without revealing private keys. A **ZPub** is the Bech32-encoded equivalent for native SegWit. The codebase uses `xpub.rs` to derive addresses from these and verify their checksums. See `src/xpub.rs` for the Base58 decoding and checksum logic.

## Locktime and nLockTime

A Bitcoin transaction can include a `nLockTime` field. If it is non-zero and below `500_000_000`, it is interpreted as a **block height** before which the transaction cannot be mined. If above, it is a **Unix timestamp**. The system evaluates whether the locktime has been met by comparing it against the blockchain's median time. See `src/bin/bal-pusher.rs` for the evaluation logic.

## P2WPKH (Pay to Witness Public Key Hash)

P2WPKH is a native SegWit output format that reduces transaction size and lowers transaction fees. The addresses are Bech32 encoded (e.g., `bc1q...`). The project assumes fee collection outputs are P2WPKH and uses `bitcoin::Address::p2wpkh` for derivation in `src/xpub.rs`.

## Bitcoin Block Header (80 bytes)

A Bitcoin block header is a fixed 80-byte structure containing `version` (4 bytes), `previous block hash` (32 bytes), `merkle root` (32 bytes), `timestamp` (4 bytes), `bits` (4 bytes), and `nonce` (4 bytes). The `bal-pusher` binary uses the `hashblock` ZMQ topic and calls `getblockchaininfo` to get the `mediantime`.

## ZMQ Publisher

Bitcoin Core can publish notifications over ZeroMQ. The project listens to two topics:
- **`hashblock`**: Sends the 32-byte block hash when a new block is found. The `bal-pusher` uses this to trigger an update cycle.
- **`rawblock`**: Sends the full raw block (including the 80-byte header). This is not used by the current `bal-pusher` implementation.

The ZMQ endpoint is per-network:
- Bitcoin: `tcp://127.0.0.1:28332`
- Regtest: `tcp://127.0.0.1:21332`
- Testnet: `tcp://127.0.0.1:23332`
- Testnet4: `tcp://127.0.0.1:24332`
- Signet: `tcp://127.0.0.1:22332`

## Bitcoin Core RPC

The project communicates with a local Bitcoin Core node via the JSON-RPC interface. Key methods used are:
- `sendrawtransaction`: To broadcast a pending transaction.
- `getblockchaininfo`: To retrieve the current block height and `mediantime` (used to evaluate locktime).
- `getblock`: To retrieve block data in `bal-pusher` (for median time calculation).

Authentication is done via `bitcoincore-rpc` using either `UserPass` or `CookieFile` (the `cookie` file is stored in `~/.bitcoin/.cookie`). See `src/bin/bal-pusher.rs`.

## Mempool and P2P

Transactions are validated against mempool rules before submission. The server checks that the fee output is paid to a specific address owned by the operator. It also ensures the transaction can be deserialized using the `bitcoin::Transaction` parser from the `bitcoin` crate. See `src/bin/bal-server.rs` (`pushtxs` endpoint).
