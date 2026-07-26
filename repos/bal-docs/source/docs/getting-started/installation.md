# Installation

## Requirements

| | |
|---|---|
| **Electrum** | {{ bal.electrum_latest }} (last stable release tested) |
| **Platform** | Desktop — Linux, Windows, macOS |
| **License** | MIT |
| **Wallet type** | Standard seed-based single-signature, or a [hardware wallet](../user-guide/hardware-wallets.md). See [Compatibility](../user-guide/compatibility.md). |

Install Electrum first, from the official site [electrum.org](https://electrum.org), and verify its signature as you normally would.

## 1. Download the plugin

Get the latest release from the [plugin releases page](https://bitcoin-after.life/gitea/bitcoinafterlife/bal-electrum-plugin/releases).

!!! tip "Verify the archive"
    Check that the zip's SHA-256 matches the value printed by `build_zip.py`. If you built the zip yourself from source, compare it against the published release.

## 2. Install it in Electrum

![Electrum Tools menu with Plugins](../img/install-electrum-plugins-menu.png){ .screenshot }
*Electrum → Tools → Plugins.*

1. Open Electrum and go to **Tools → Plugins**.
2. Choose **install from file** and select the BAL zip.

![Install from file dialog](../img/install-from-file.png){ .screenshot }
*Selecting the plugin archive.*

## 3. Enable and restart

Tick **Bitcoin After Life** in the plugin list, then restart Electrum.

![Plugin list with Bitcoin After Life enabled](../img/install-plugin-enabled.png){ .screenshot }
*The plugin enabled and ready.*

## 4. Check that it loaded

After restarting you will see two new tabs in the Electrum window: **HEIRS** and **WILL**.

![Electrum with the HEIRS and WILL tabs](../img/overview-tabs.png){ .screenshot }
*HEIRS is where you define who inherits; WILL shows the technical state of your inheritance transactions.*

Next: [create your first will](quick-start.md).

## Platform-specific notes

Step-by-step installation notes for specific platforms are kept in their own repositories:

- [Windows installation](https://bitcoin-after.life/gitea/bitcoinafterlife/bal_plugin_windows_installation)
- [Linux installation](https://bitcoin-after.life/gitea/bitcoinafterlife/bal_plugin_linux_installation)
