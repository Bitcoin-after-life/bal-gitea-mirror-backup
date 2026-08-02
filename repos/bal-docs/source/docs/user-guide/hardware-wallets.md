# :material-key-chain-variant: Hardware wallets

BAL works with **any hardware wallet supported by Electrum** — Ledger, Trezor, Coldcard, BitBox02, Jade, KeepKey and others.

![Confirming on the hardware wallet](../img/hardware-wallet-signing.png){ .screenshot }
*The plugin builds the transaction; your device signs it.*

The division of labour is the same as for a normal Electrum transaction:

- the **plugin** builds the inheritance transaction,
- **Electrum** submits it for signature,
- **your device** signs — with your physical confirmation.

The plugin never has access to your keys and never signs on your behalf.

!!! tip "Cold signing setup"
    For maximum security, use a dedicated computer for signing. Note that with BAL you will need to sign more than once over time: every refresh of the will requires a new signature — see [Keeping the will up to date](updating.md).
