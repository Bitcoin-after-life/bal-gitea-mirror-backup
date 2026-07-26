# FAQ

## The project

??? question "What is the Bitcoin After Life Protocol for?"
    It manages the **inheritance of bitcoin** held in a seed-based wallet, and can also serve as an **emergency backup** if you lose your seed. It uses time-locked transactions held by Will-Executor servers, which broadcast them to the network on the delivery date you set.

??? question "Who created it, and when?"
    It is open-source software, belonging to everyone, like Bitcoin itself. The first public post about Bitcoin After Life appeared on BitcoinTalk on **31 October 2024**, signed *Svātantrya*. Anyone can contribute.

??? question "Why a plugin for Electrum rather than a dedicated wallet?"
    Because Electrum is **heavily tested** and considered the gold standard among Bitcoin wallets. Building a new wallet — or forking one — would put funds at unnecessary risk. The plugin does not sign transactions itself: that is entirely Electrum's job.

??? question "Is BAL 2.0 planned?"
    Yes. The main new feature will be **staggered inheritance**: gradual distributions over time (for example 10% per year) instead of a single lump-sum transfer.

## Using the plugin

??? question "How much does it cost?"
    Two fees, both paid **exclusively in bitcoin** — no subscriptions, no cards:

    - **Miner fee**, chosen by you at signing time. Set it generously so the transaction confirms even under congestion.
    - **Will-Executor fee**, deducted from the inheritance at execution.

??? question "Is it hard to use?"
    The plugin includes a **guided wizard** and was designed for ordinary users, not only technical ones. See [Create your first will](getting-started/quick-start.md).

??? question "Can I use a hardware wallet?"
    Yes — any hardware wallet supported by Electrum (Ledger, Trezor, Coldcard, BitBox02, Jade, KeepKey and others). See [Hardware wallets](user-guide/hardware-wallets.md).

??? question "Is it compatible with every Electrum wallet type?"
    Standard seed-based single-signature wallets and hardware wallets: yes. **Multisig is not yet supported**, and TrustedCoin (2FA) compatibility is **unknown**. See [Compatibility](user-guide/compatibility.md).

??? question "What if my balance changes after setting up the inheritance?"
    The plugin monitors your UTXOs every time you close Electrum and prompts you to update the will. This prevents both new funds being excluded and the old transaction being rejected because its inputs were spent. See [Keeping the will up to date](user-guide/updating.md).

??? question "What if an incoming transaction is still unconfirmed during setup?"
    Not a problem. It only needs to be **visible in the mempool** — the plugin handles this case automatically.

??? question "Can I use the plugin to back up my wallet?"
    Yes. Set a second wallet you control as the beneficiary. If you lose access to the original and stop opening Electrum, the funds are sent automatically to the backup address on the delivery date. See [Wallet recovery use case](user-guide/wallet-recovery.md).

??? question "Do I have to do anything after creating the will, or is it set-and-forget?"
    Not strictly required, but renewing every **2–3 years** is recommended. A rolling deadline — "always five years from today" — refreshed periodically keeps newer Will-Executors in your selection and picks up protocol improvements.

??? question "What is Check Alive?"
    A period of inactivity after which the plugin asks whether to postpone your delivery date. Available in **advanced mode** — see [Check Alive](user-guide/check-alive.md).

## Will-Executors

??? question "Does the plugin use one Will-Executor or several?"
    Several — as many as possible. It downloads the full [WeList](will-executors/welist.md) by default; you can also add servers manually or use alternative lists. For the inheritance to succeed, **only one** of the selected Will-Executors has to still be operating on the delivery date.

??? question "Who receives the fee if several Will-Executors broadcast the same inheritance?"
    The plugin creates a **separate transaction for each Will-Executor**, each paying that specific server. Only one can confirm: the fee goes to whoever broadcast it successfully. A competitive mechanism that rewards servers that are actually operating.

??? question "Why aren't the fees redistributed among all Will-Executors?"
    Three practical reasons: it would require including every active server's address in every inheritance transaction, it would increase transaction size and cost, and it would reward servers that do nothing.

    But the decisive reason is different: **anyone who wants to share proceeds can already do so**, without changing the protocol — see the next question. Writing redistribution into the protocol would impose a policy on everyone; leaving it out keeps it a voluntary choice.

??? question "Does a Will-Executor have to be a single person or company?"
    No. The protocol sees a service plus a Bitcoin address. Several operators can form a consortium, present themselves as one Will-Executor, and use a shared **multisig** address for fees, splitting them by whatever agreement they prefer. The plugin knows nothing about it — it pays an address, as always.

    Two caveats: for a user's redundancy the consortium counts as **one** Will-Executor, and the internal split is outside the protocol, neither arbitrated nor guaranteed by BAL.

??? question "Can a newly joined Will-Executor handle wills created before it existed?"
    No. Transactions are pre-signed and already name the Will-Executor to be paid; back-dating is neither possible nor appropriate. A new server receives wills created from its arrival onwards.

??? question "Do Will-Executors talk to each other?"
    No. Each one is completely independent — no coordination, no shared state. This removes central points of coordination and reduces the attack surface.

??? question "Why not use miners to broadcast inheritance transactions?"
    Technical reasons: the mempool rejects transactions with a future locktime, and a signed transaction cannot be modified to add an "anyonecanspend" output without invalidating it.

    Privacy reasons: Will-Executors are **chosen by the user**. Handing transactions to a network of miners nobody selected would expose wallet data to unwanted observers. A network of independent Will-Executors whose only incentive is broadcasting is also more resistant to regulation and censorship than dependence on large pools.

??? question "Can Will-Executors see the contents of my wallet?"
    The servers you select receive pre-signed transactions that reflect your wallet's state. This is exactly why their selection stays in your hands, and why privacy — together with reliability — is what Will-Executors really compete on.

??? question "What if every Will-Executor I selected shuts down before the delivery date?"
    This is what the [offline backup transaction](user-guide/backup-transaction.md) is for: saved to physical media and handed to your heirs, who can broadcast it themselves. It must be kept up to date as your wallet changes — which is why the better strategy remains selecting as many Will-Executors as possible.

??? question "Who decides the Will-Executor fees?"
    Each server sets its own; the market sets the price. When creating your will you can exclude servers you find too expensive — or suspiciously cheap, since an unsustainable price suggests a service unlikely to last for years.

??? question "Are the miner fees enough if the network is congested?"
    The plugin sets a deliberately generous default fee to ensure confirmation even under congestion. A Will-Executor can additionally use **CPFP (Child Pays For Parent)** to accelerate confirmation.

## Running a server

??? question "Do I need a powerful machine to run a Will-Executor?"
    No — recent versions run on even a small VPS, and at least one active Will-Executor runs on Umbrel OS. What matters is uptime over years, not raw power. See [Running a Will-Executor](will-executors/running-a-server.md).

??? question "Can I add my server to the WeList?"
    Yes. The WeList is open to everyone; listing costs approximately **{{ bal.welist_fee_sats }} satoshis per year** to cover the directory's running costs. Once listed, your server is visible to all BAL users directly inside the plugin. Listing is optional — users can also add servers manually.
