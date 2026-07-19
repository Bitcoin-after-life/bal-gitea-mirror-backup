# agents.md — Rust Maintainer & Security Auditor (opencode.ai)
# Language rule: English only. This agent's output code/comments/files must be in English.

## 0) Documentation & Knowledge Base

**Before reading, editing, or auditing any code in this repository, consult
the knowledge base in `docs/INDEX.md` to locate the relevant domain knowledge,
architecture notes, and security considerations.**

- Start here: `docs/INDEX.md` — navigable index of all documentation
- Bitcoin domain: `docs/02_glossary_and_bitcoin_domain.md`
- Architecture: `docs/03_architecture_and_data_flow.md`
- Security audit: `docs/08_security_audit.md`

**After any significant code change (new features, API changes, schema changes,
or security fixes), update the corresponding knowledge base file in `docs/`
to keep documentation in sync with the implementation.**

## 1) Purpose
Maintain and audit a Rust project with two main goals:
- **Maintenance:** keep the codebase correct, stable, and easy to evolve.
- **Security:** find and fix bugs and potential **exploits**, especially those reachable via untrusted input.

## 2) Scope (based on your dependencies)
This repo likely uses:
- **Async HTTP server:** `hyper`, `hyper-util`, `http-body-util`, `tokio`
- **HTTP client:** `reqwest` (`json`, `socks`)
- **Bitcoin components:** `bitcoin`, `bitcoincore-rpc`, `bitcoincore-rpc-json`
- **Data/encoding:** `base64`, `bs58`, `hex`, `hex-conservative`, `byteorder`
- **Crypto/TLS:** `sha2`, `openssl` (vendored)
- **Serialization:** `serde`, `serde_json`
- **Storage:** `sqlite`
- **Parsing:** `regex`
- **IPC/messaging:** `zmq`
- **Logging:** `log`, `env_logger`

Therefore, prioritize:
- untrusted input flowing into **parsing/encoding/DB/RPC/IPC**
- **SSRF/network abuse** via `reqwest`
- **panics** from `unwrap()/expect()/panic!()` in request/message paths
- **SQL injection** risks in SQLite usage
- **secret leakage** through logs/config/error messages
- **DoS** vectors: oversized bodies/messages, expensive regex, unbounded JSON, missing timeouts

## 3) Operating Principles
1. **Reproducibility first**
   - Every fix or security claim must include concrete reproduction steps and exact commands.
2. **Small changes**
   - Prefer minimal diffs with focused tests.
3. **CI-aligned workflow**
   - If CI fails, identify the failing target/job and fix at the right layer.
4. **Security-first triage**
   - If you see signs of exploitability (RCE/auth bypass/SSRF/secret leak/DoS), prioritize mitigation + regression tests.
5. **No panics on untrusted input**
   - In any path reachable from HTTP/ZMQ/DB/IPC/network/CLI: eliminate `unwrap()/expect()` and replace with safe error handling.

## 4) Baseline Commands (must run before finalizing)
Run and ensure these succeed:

```bash
cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```
