# `docs/` Knowledge Base

## Quick Guide for Contributors

- **I am a developer and want to understand the project** → Start with [`01_project_overview.md`](01_project_overview.md)
- **I am a developer and need to know how the system works** → Read [`03_architecture_and_data_flow.md`](03_architecture_and_data_flow.md)
- **I am a developer working on a specific module** → See [`04_modules_detail.md`](04_modules_detail.md)
- **I am a security auditor** → Go straight to [`08_security_audit.md`](08_security_audit.md)
- **I am deploying or operating this software** → Check [`07_deployment_and_ops.md`](07_deployment_and_ops.md)
- **I need to integrate with the API** → Reference [`05_api_reference.md`](05_api_reference.md)
- **I need to understand the database** → Use [`06_database_schema.md`](06_database_schema.md)
- **I need to understand Bitcoin concepts** → See [`02_glossary_and_bitcoin_domain.md`](02_glossary_and_bitcoin_domain.md)
- **I need source code references or external links** → Check [`09_references_and_links.md`](09_references_and_links.md)

## Files Overview

| # | File | Purpose |
|---|------|---------|
| 1 | [`01_project_overview.md`](01_project_overview.md) | Vision, goals, components, and mapping to existing docs |
| 2 | [`02_glossary_and_bitcoin_domain.md`](02_glossary_and_bitcoin_domain.md) | Bitcoin domain knowledge: BIP-84, locktime, P2WPKH, ZMQ, block headers |
| 3 | [`03_architecture_and_data_flow.md`](03_architecture_and_data_flow.md) | High-level architecture, data flow, state machine, error handling |
| 4 | [`04_modules_detail.md`](04_modules_detail.md) | Deep dive into each Rust module and binary |
| 5 | [`05_api_reference.md`](05_api_reference.md) | Complete API docs: HTTP, ZMQ, RPC with examples |
| 6 | [`06_database_schema.md`](06_database_schema.md) | Full SQL schema, tables, queries, data lifecycle |
| 7 | [`07_deployment_and_ops.md`](07_deployment_and_ops.md) | Environment variables, systemd, nginx, scripts, Tor, installation |
| 8 | [`08_security_audit.md`](08_security_audit.md) | Threat model, vulnerability assessment, hardening recommendations |
| 9 | [`09_references_and_links.md`](09_references_and_links.md) | Source code links, Cargo dependencies, existing file mapping |

---

> **Note:** This knowledge base is maintained in parallel to the codebase. After any significant change to the code (new features, API changes, schema changes, or security fixes), update the corresponding file in this directory to keep documentation synchronized.
