"""
Tests for will invalidation (cancellation) in ``bal.core.will``.

Covers:
* Will.invalidate_will() - building the invalidation transaction
* Will.set_invalidate() - marking will items as invalidated (status cascade)

The invalidation ("cancellation") transaction spends the same UTXOs that were
committed to the time-locked will, making the original will transactions
unspendable.  This is the mechanism used when:
  * The will expires (locktime in the past)
  * The owner postpones a signed/sent will to a later date
  * The check-alive threshold is passed (dead-man's switch)

Run:
    QT_QPA_PLATFORM=offscreen PYTHONPATH=electrum-src \
        python3 -m pytest tests/test_core_will_invalidate.py -q
"""

import os
import sys
from unittest.mock import MagicMock, patch

sys.path.insert(0, os.path.join(os.path.dirname(__file__), os.pardir))

# Patch Transaction.add_info_from_wallet so WillItem can parse the tx hex
# without a live Electrum wallet connection.
from electrum.transaction import Transaction

from bal.core.will import Will, WillItem

_patcher = patch.object(Transaction, "add_info_from_wallet")
_patcher.start()

# A valid serialized Bitcoin transaction hex (1 input + 1 P2PKH output,
# version 2).  Reused across multiple test suites.
_VALID_TX_HEX = (
    "01000000012a5c9a94fcde98f5581cd00162c60a13936ceb75389ea65b"
    "f38633b424eb4031000000006c493046022100a82bbc57a0136751e543"
    "3f41cf000b3f1a99c6744775e76ec764fb78c54ee100022100f9e80b7d"
    "e89de861dc6fb0c1429d5da72c2b6b2ee2406bc9bfb1beedd729d98501"
    "2102e61d176da16edd1d258a200ad9759ef63adf8e14cd97f53227bae3"
    "5cdb84d2f6ffffffff0140420f00000000001976a914230ac37834073a"
    "42146f11ef8414ae929feaafc388ac00000000"
)

# The prevout string that _VALID_TX_HEX spends (input 0).
_PREVOUT_STR = "3140eb24b43386f35ba69e3875eb6c93130ac66201d01c58f598defc949a5c2a:0"

# A second transaction identical to _VALID_TX_HEX but spending a DIFFERENT
# prevout. It stands for a superseded (REPLACED) will transaction that commits
# different coins than the current valid one.
_REPLACED_TX_HEX = "0100000001ff" + _VALID_TX_HEX[len("01000000012a"):]
_REPLACED_PREVOUT_STR = _PREVOUT_STR[:-4] + "ff:0"

# A 2-input transaction: version 2 with two parallel inputs. Built from
# _VALID_TX_HEX by duplicating its (298-hex) input with a DIFFERENT prevout
# (first byte "ff", so the second input spends _REPLACED_PREVOUT_STR) and
# keeping the unchanged outputs.
_INPUT_LEN = 298
_TWO_INPUT_TX_HEX = (
    "0100000002"
    + _VALID_TX_HEX[10:10 + _INPUT_LEN]
    + "ff" + _VALID_TX_HEX[10 + 2:10 + _INPUT_LEN]
    + _VALID_TX_HEX[10 + _INPUT_LEN:]
)

# Change address for the invalidation output.
_CHANGE_ADDR = "14CHYaaByjJZpx4oHBpfDMdqhTyXnZ3kVs"


# ------------------------------------------------------------------ #
# Helpers
# ------------------------------------------------------------------ #

def _make_willitem(value_sats=1000000, valid=True, extra_heirs=None,
                   tx_hex=_VALID_TX_HEX):
    """Create a WillItem from ``tx_hex`` with a known input value.

    The input's ``_trusted_value_sats`` is set so that
    ``invalidate_will`` can read the balance from it.
    """
    heirs = {"alice": ["addr_alice", 5000, "30d"]}
    if extra_heirs:
        heirs.update(extra_heirs)
    item = WillItem({
        "tx": tx_hex,
        "heirs": heirs,
        "willexecutor": None,
        "status": "",
        "description": "",
        "time": 0,
        "change": "",
        "baltx_fees": 100,
    })
    item.STATUS = WillItem.copy_status_table(WillItem.STATUS_DEFAULT)
    # Set the input value so the balance calculation works.
    # Use the name-mangled attribute because tx_from_any creates a
    # Transaction whose inputs are TxInput objects; TxInput.value_sats()
    # reads __value_sats, not _trusted_value_sats.
    item.tx.inputs()[0]._TxInput__value_sats = value_sats
    if not valid:
        item.set_status("INVALIDATED", True)
    return item


def _make_utxo(prevout_str=None, value_sats=1000000, is_coinbase=False):
    """Create a minimal mock UTXO (wallet-side) matching a will input."""
    if prevout_str is None:
        prevout_str = _PREVOUT_STR
    utxo = MagicMock()
    utxo.prevout.to_str.return_value = prevout_str
    utxo.is_coinbase_output.return_value = is_coinbase
    utxo.block_height = 1
    utxo.block_txpos = 0
    utxo.value_sats.return_value = value_sats
    return utxo


def _mock_wallet(utxos, change_addr=_CHANGE_ADDR):
    """Create a mock wallet with the given UTXOs and change address.

    ``network`` is left ``None`` so ``Will.invalidate_will`` uses the
    ``fees_per_byte`` FALLBACK instead of Electrum's live estimate: these tests
    exercise the pure fee arithmetic (``fee = size * fees_per_byte``). The
    policy-based rate selection has its own tests in
    ``tests/test_core_invalidation_fee.py``.
    """
    wallet = MagicMock()
    wallet.get_utxos.return_value = utxos
    wallet.get_change_addresses_for_new_transaction.return_value = [change_addr]
    wallet.network = None
    return wallet


def _run_invalidate(will, wallet, fees_per_byte=10, current_height=800000):
    """Run ``Will.invalidate_will`` with mocked Electrum tx building.

    Returns ``(result, mock_from_io, mock_out)`` so tests can inspect
    the calls to ``PartialTransaction.from_io`` and
    ``PartialTxOutput.from_address_and_value``.
    """
    mock_output = MagicMock()
    mock_output.value = 0
    mock_output.is_change = False

    mock_tx = MagicMock()
    mock_tx.txid.return_value = "invalidation_txid"
    mock_tx.estimated_size.return_value = 200

    with patch("bal.core.will.Util.get_current_height", return_value=current_height), \
         patch("electrum.transaction.PartialTxOutput.from_address_and_value",
               return_value=mock_output) as mock_out, \
         patch("electrum.transaction.PartialTransaction.from_io",
               return_value=mock_tx) as mock_from_io:
        result = Will.invalidate_will(will, wallet, fees_per_byte)
        return result, mock_from_io, mock_out


# ================================================================== #
# Will.invalidate_will - building the cancellation transaction
# ================================================================== #

class TestInvalidateWill:
    """Tests for ``Will.invalidate_will()``: the cancellation transaction."""

    def test_basic_returns_tx(self):
        """A single valid will item with a matching wallet UTXO produces an
        invalidation transaction."""
        item = _make_willitem(value_sats=1000000)
        will = {"willtxid1": item}
        wallet = _mock_wallet([_make_utxo()])

        result, mock_from_io, _ = _run_invalidate(will, wallet, fees_per_byte=10)

        assert result is not None, "should return a transaction"

    def test_basic_rbf_enabled(self):
        """The invalidation tx has RBF (Replace-By-Fee) enabled."""
        item = _make_willitem(value_sats=1000000)
        will = {"willtxid1": item}
        wallet = _mock_wallet([_make_utxo()])

        result, _, _ = _run_invalidate(will, wallet)

        result.set_rbf.assert_called_with(True)

    def test_basic_locktime_is_current_height(self):
        """The invalidation tx locktime equals the current block height."""
        item = _make_willitem(value_sats=1000000)
        will = {"willtxid1": item}
        wallet = _mock_wallet([_make_utxo()])
        current_height = 750000

        _, mock_from_io, _ = _run_invalidate(will, wallet, current_height=current_height)

        # from_io(inputs, outputs, locktime=<height>, version=2)
        _, kwargs = mock_from_io.call_args
        assert kwargs["locktime"] == current_height

    def test_basic_version_2(self):
        """The invalidation tx uses Bitcoin transaction version 2."""
        item = _make_willitem(value_sats=1000000)
        will = {"willtxid1": item}
        wallet = _mock_wallet([_make_utxo()])

        _, mock_from_io, _ = _run_invalidate(will, wallet)

        _, kwargs = mock_from_io.call_args
        assert kwargs["version"] == 2

    def test_basic_output_value_deducts_fee(self):
        """The invalidation output value is balance minus fee.

        Fee = estimated_size * fees_per_byte.
        """
        item = _make_willitem(value_sats=1000000)
        will = {"willtxid1": item}
        wallet = _mock_wallet([_make_utxo()])
        fees_per_byte = 10

        _, mock_from_io, mock_out = _run_invalidate(will, wallet, fees_per_byte=fees_per_byte)

        # The second call to from_address_and_value uses balance - fee.
        # estimated_size returns 200, so fee = 200 * 10 = 2000.
        # Expected output value = 1000000 - 2000 = 998000.
        second_call_value = mock_out.call_args_list[1][0][1]
        assert second_call_value == 998000

    def test_basic_spends_correct_utxos(self):
        """The invalidation tx spends the same UTXOs as the will."""
        item = _make_willitem(value_sats=1000000)
        will = {"willtxid1": item}
        wallet = _mock_wallet([_make_utxo()])

        _, mock_from_io, _ = _run_invalidate(will, wallet)

        # First positional arg is the list of UTXOs to spend.
        spent_utxos = mock_from_io.call_args[0][0]
        assert len(spent_utxos) == 1
        assert spent_utxos[0].prevout.to_str() == _PREVOUT_STR

    def test_no_matching_utxos_returns_none(self):
        """When wallet UTXOs don't match any will inputs, returns None."""
        item = _make_willitem(value_sats=1000000)
        will = {"willtxid1": item}
        wallet = _mock_wallet([_make_utxo(prevout_str="aaaa:1")])

        result, _, _ = _run_invalidate(will, wallet)
        assert result is None

    def test_no_valid_items_returns_none(self):
        """When all will items are INVALIDATED, returns None."""
        item = _make_willitem(valid=False)
        will = {"willtxid1": item}
        wallet = _mock_wallet([_make_utxo()])

        result, _, _ = _run_invalidate(will, wallet)
        assert result is None

    def test_empty_will_returns_none(self):
        """An empty will dictionary returns None."""
        wallet = _mock_wallet([_make_utxo()])

        result, _, _ = _run_invalidate({}, wallet)
        assert result is None

    def test_skips_young_coinbase(self):
        """Coinbase UTXOs younger than current_height + 100 are skipped."""
        item = _make_willitem(value_sats=1000000)
        will = {"willtxid1": item}
        # Coinbase UTXO: block_height = 800050, current_height = 800000
        # 800050 < 800000 + 100 => skipped
        utxo = _make_utxo(value_sats=1000000, is_coinbase=True)
        utxo.block_height = 800050
        wallet = _mock_wallet([utxo])

        result, _, _ = _run_invalidate(will, wallet, current_height=800000)
        assert result is None

    def test_includes_mature_coinbase(self):
        """Coinbase UTXOs at or above current_height + 100 are included."""
        item = _make_willitem(value_sats=1000000)
        will = {"willtxid1": item}
        utxo = _make_utxo(value_sats=1000000, is_coinbase=True)
        utxo.block_height = 800150  # >= 800000 + 100
        wallet = _mock_wallet([utxo])

        result, _, _ = _run_invalidate(will, wallet, current_height=800000)
        assert result is not None

    def test_fee_exceeds_balance_returns_none(self):
        """When the fee exceeds the balance, returns None.

        estimated_size (200) * fees_per_byte (100) = 20000 > balance (100).
        """
        item = _make_willitem(value_sats=100)
        will = {"willtxid1": item}
        wallet = _mock_wallet([_make_utxo(value_sats=100)])

        result, mock_from_io, _ = _run_invalidate(will, wallet, fees_per_byte=100)

        assert result is None
        # from_io is still called once (for fee estimation), but the
        # result is discarded because balance - fee <= 0.
        assert mock_from_io.call_count == 1

    def test_only_valid_items_contribute_balance(self):
        """INVALIDATED will items are excluded from the balance."""
        valid_item = _make_willitem(value_sats=1000000, valid=True)
        invalid_item = _make_willitem(value_sats=2000000, valid=False)
        will = {"valid": valid_item, "invalid": invalid_item}
        wallet = _mock_wallet([_make_utxo()])

        _, mock_from_io, mock_out = _run_invalidate(will, wallet, fees_per_byte=10)

        # Balance = 1000000 (valid only), fee = 200 * 10 = 2000
        # Output value = 998000
        second_call_value = mock_out.call_args_list[1][0][1]
        assert second_call_value == 998000

    def test_replaced_item_inputs_are_invalidated(self):
        """A REPLACED (superseded) item's funding UTXO is also spent.

        Regression: a will rebuilt after an heir change supersedes the old
        signed transaction, which is kept but marked REPLACED (VALID cleared).
        The old transaction may have been pushed to the will-executors and may
        spend DIFFERENT coins than the current valid one, so its UTXO must be
        included in the invalidation. Otherwise it would stay unspent and a
        will-executor could still broadcast the superseded transaction.
        """
        valid_item = _make_willitem(value_sats=1000000)
        replaced_item = _make_willitem(
            value_sats=2000000, tx_hex=_REPLACED_TX_HEX
        )
        replaced_item.set_status("REPLACED", True)
        assert replaced_item.get_status("VALID") is False

        will = {"valid": valid_item, "replaced": replaced_item}
        wallet = _mock_wallet([
            _make_utxo(),
            _make_utxo(prevout_str=_REPLACED_PREVOUT_STR, value_sats=2000000),
        ])

        result, mock_from_io, _ = _run_invalidate(will, wallet)

        assert result is not None
        spent = [u.prevout.to_str() for u in mock_from_io.call_args[0][0]]
        assert _PREVOUT_STR in spent
        assert _REPLACED_PREVOUT_STR in spent

    def test_replaced_item_contributes_balance(self):
        """The balance of a REPLACED item is included in the invalidation."""
        valid_item = _make_willitem(value_sats=1000000)
        replaced_item = _make_willitem(
            value_sats=2000000, tx_hex=_REPLACED_TX_HEX
        )
        replaced_item.set_status("REPLACED", True)

        will = {"valid": valid_item, "replaced": replaced_item}
        wallet = _mock_wallet([
            _make_utxo(),
            _make_utxo(prevout_str=_REPLACED_PREVOUT_STR, value_sats=2000000),
        ])

        _, _, mock_out = _run_invalidate(will, wallet, fees_per_byte=10)

        # Balance = 1000000 + 2000000 = 3000000, fee = 200 * 10 = 2000
        second_call_value = mock_out.call_args_list[1][0][1]
        assert second_call_value == 2998000

    def test_invalidated_item_inputs_are_not_invalidated(self):
        """A purely INVALIDATED item (already neutralised) is still excluded."""
        valid_item = _make_willitem(value_sats=1000000)
        invalid_item = _make_willitem(
            value_sats=2000000, tx_hex=_REPLACED_TX_HEX, valid=False
        )
        assert invalid_item.get_status("INVALIDATED") is True

        will = {"valid": valid_item, "invalid": invalid_item}
        wallet = _mock_wallet([
            _make_utxo(),
            _make_utxo(prevout_str=_REPLACED_PREVOUT_STR, value_sats=2000000),
        ])

        result, mock_from_io, _ = _run_invalidate(will, wallet)

        assert result is not None
        spent = [u.prevout.to_str() for u in mock_from_io.call_args[0][0]]
        assert _PREVOUT_STR in spent
        assert _REPLACED_PREVOUT_STR not in spent

    def test_first_from_io_uses_full_balance(self):
        """The first from_io call uses the full balance (before fee deduction)
        to estimate the fee."""
        item = _make_willitem(value_sats=1000000)
        will = {"willtxid1": item}
        wallet = _mock_wallet([_make_utxo()])

        _, mock_from_io, mock_out = _run_invalidate(will, wallet, fees_per_byte=10)

        # First from_address_and_value call: value = balance (1000000)
        first_call_value = mock_out.call_args_list[0][0][1]
        assert first_call_value == 1000000

    def test_output_address_is_change_address(self):
        """The invalidation output goes to the wallet's change address."""
        item = _make_willitem(value_sats=1000000)
        will = {"willtxid1": item}
        wallet = _mock_wallet([_make_utxo()])

        _, _, mock_out = _run_invalidate(will, wallet)

        # Both calls to from_address_and_value use the change address.
        for call in mock_out.call_args_list:
            assert call[0][0] == _CHANGE_ADDR

    def test_multiple_utxos_all_matched(self):
        """Multiple matching UTXOs are all included in the invalidation."""
        item1 = _make_willitem(value_sats=500000)
        item2 = _make_willitem(value_sats=300000)
        will = {"tx1": item1, "tx2": item2}

        # Two UTXOs with different prevouts matching the two will items.
        # Since both items use the same _VALID_TX_HEX, their prevout is the
        # same.  To test multiple UTXOs, we need a second tx hex with a
        # different input.
        #
        # However, get_all_inputs deduplicates by prevout_str, so even with
        # two items sharing the same prevout, only one entry is added to
        # prevout_to_spend.  The first matching UTXO is what matters.
        utxos = [_make_utxo()]
        wallet = _mock_wallet(utxos)

        result, mock_from_io, _ = _run_invalidate(will, wallet)
        assert result is not None
        # Only 1 UTXO spent (deduplication of shared prevout)
        spent_utxos = mock_from_io.call_args[0][0]
        assert len(spent_utxos) == 1

    def test_zero_fees_per_byte(self):
        """With zero fee rate, the full balance goes to the output."""
        item = _make_willitem(value_sats=1000000)
        will = {"willtxid1": item}
        wallet = _mock_wallet([_make_utxo()])

        _, mock_from_io, mock_out = _run_invalidate(will, wallet, fees_per_byte=0)

        assert mock_from_io.call_count == 2  # two calls (both succeed)
        # Output value = balance - 0 = 1000000
        second_call_value = mock_out.call_args_list[1][0][1]
        assert second_call_value == 1000000

    def test_invalidation_spends_oldest_input_per_tx(self):
        """For each will tx, only its OLDEST funding coin is spent.

        A will item whose transaction has two inputs (old and new coin) must
        spend only the coin that has been in the wallet longest, leaving the
        newer coin unspent for the rebuilt (postponed) will.
        """
        item = _make_willitem(value_sats=1000000, tx_hex=_TWO_INPUT_TX_HEX)
        will = {"willtxid1": item}
        old_utxo = _make_utxo(prevout_str=_PREVOUT_STR, value_sats=1000000)
        old_utxo.block_height = 1
        new_utxo = _make_utxo(prevout_str=_REPLACED_PREVOUT_STR, value_sats=2000000)
        new_utxo.block_height = 500  # newer than the old coin
        wallet = _mock_wallet([old_utxo, new_utxo])

        result, mock_from_io, _ = _run_invalidate(will, wallet)

        assert result is not None
        spent = [u.prevout.to_str() for u in mock_from_io.call_args[0][0]]
        assert spent == [_PREVOUT_STR]

    def test_invalidation_fallback_appends_next_oldest(self):
        """If the oldest coin cannot cover the fee, the next-oldest is appended.

        The invalidation must not be dropped just because the single oldest
        coin is small: it keeps adding will coins, oldest first, until the
        balance covers the estimated fee.
        """
        item = _make_willitem(value_sats=1000000, tx_hex=_TWO_INPUT_TX_HEX)
        will = {"willtxid1": item}
        old_utxo = _make_utxo(prevout_str=_PREVOUT_STR, value_sats=1000)
        old_utxo.block_height = 1
        new_utxo = _make_utxo(prevout_str=_REPLACED_PREVOUT_STR, value_sats=1000000)
        new_utxo.block_height = 500
        wallet = _mock_wallet([old_utxo, new_utxo])

        # estimated_size (200) * fees_per_byte (10) = 2000 > balance (1000)
        result, mock_from_io, mock_out = _run_invalidate(will, wallet, fees_per_byte=10)

        assert result is not None
        spent = [u.prevout.to_str() for u in mock_from_io.call_args[0][0]]
        assert _PREVOUT_STR in spent
        assert _REPLACED_PREVOUT_STR in spent
        # balance = 1000 + 1000000, fee = 2000
        final_value = mock_out.call_args[0][1]
        assert final_value == 999000

    def test_invalidation_treats_unconfirmed_as_newest(self):
        """An unconfirmed (unknown-height) coin is never chosen as the oldest."""
        item = _make_willitem(value_sats=1000000, tx_hex=_TWO_INPUT_TX_HEX)
        will = {"willtxid1": item}
        confirmed = _make_utxo(prevout_str=_PREVOUT_STR, value_sats=1000000)
        confirmed.block_height = 10
        unconfirmed = _make_utxo(prevout_str=_REPLACED_PREVOUT_STR, value_sats=2000000)
        unconfirmed.block_height = None
        wallet = _mock_wallet([unconfirmed, confirmed])

        result, mock_from_io, _ = _run_invalidate(will, wallet)

        assert result is not None
        spent = [u.prevout.to_str() for u in mock_from_io.call_args[0][0]]
        assert spent == [_PREVOUT_STR]

    def test_invalidation_same_height_order_is_deterministic(self):
        """Coins at the same height are ordered by block position, not list order."""
        item = _make_willitem(value_sats=1000000, tx_hex=_TWO_INPUT_TX_HEX)
        will = {"willtxid1": item}
        later = _make_utxo(prevout_str=_PREVOUT_STR, value_sats=1000000)
        later.block_height = 10
        later.block_txpos = 5
        earlier = _make_utxo(prevout_str=_REPLACED_PREVOUT_STR, value_sats=2000000)
        earlier.block_height = 10
        earlier.block_txpos = 1
        wallet = _mock_wallet([later, earlier])

        result, mock_from_io, _ = _run_invalidate(will, wallet)

        assert result is not None
        spent = [u.prevout.to_str() for u in mock_from_io.call_args[0][0]]
        assert spent == [_REPLACED_PREVOUT_STR]  # block_txpos 1 < 5 => older

    def test_sub_dust_change_returns_none(self):
        """A change output below the dust limit is never created.

        Regression: previously any positive change (balance - fee > 0) was
        accepted, which could produce a non-standard (unrelayable) output and
        leave the will un-invalidated.
        """
        item = _make_willitem(value_sats=2200)
        will = {"willtxid1": item}
        wallet = _mock_wallet([_make_utxo(value_sats=2200)])
        wallet.dust_threshold.return_value = 546

        # fee = estimated_size (200) * fees_per_byte (10) = 2000;
        # balance - fee = 200 < dust (546) -> no transaction.
        result, _, _ = _run_invalidate(will, wallet, fees_per_byte=10)

        assert result is None

    def test_change_equal_to_dust_is_allowed(self):
        """Change exactly at the dust threshold is accepted (>= comparison)."""
        item = _make_willitem(value_sats=2546)
        will = {"willtxid1": item}
        wallet = _mock_wallet([_make_utxo(value_sats=2546)])
        wallet.dust_threshold.return_value = 546

        # balance - fee = 2546 - 2000 = 546 == dust -> transaction.
        result, _, mock_out = _run_invalidate(will, wallet, fees_per_byte=10)

        assert result is not None
        assert mock_out.call_args_list[1][0][1] == 546

    def test_fallback_stops_when_change_still_dust(self):
        """Coins are appended until the change clears dust; else returns None."""
        item = _make_willitem(value_sats=1000000, tx_hex=_TWO_INPUT_TX_HEX)
        will = {"willtxid1": item}
        old_utxo = _make_utxo(prevout_str=_PREVOUT_STR, value_sats=1000)
        old_utxo.block_height = 1
        new_utxo = _make_utxo(prevout_str=_REPLACED_PREVOUT_STR, value_sats=1200)
        new_utxo.block_height = 500
        wallet = _mock_wallet([old_utxo, new_utxo])
        wallet.dust_threshold.return_value = 546

        # fee = 2000; total balance = 1000 + 1200 = 2200;
        # change = 200 < dust (546) and no coin is left -> None.
        result, _, _ = _run_invalidate(will, wallet, fees_per_byte=10)

        assert result is None

    def test_fallback_succeeds_once_change_clears_dust(self):
        """Appending the next coin yields a change above dust -> transaction."""
        item = _make_willitem(value_sats=1000000, tx_hex=_TWO_INPUT_TX_HEX)
        will = {"willtxid1": item}
        old_utxo = _make_utxo(prevout_str=_PREVOUT_STR, value_sats=1000)
        old_utxo.block_height = 1
        new_utxo = _make_utxo(prevout_str=_REPLACED_PREVOUT_STR, value_sats=2546)
        new_utxo.block_height = 500
        wallet = _mock_wallet([old_utxo, new_utxo])
        wallet.dust_threshold.return_value = 546

        # fee = 2000; total balance = 3546; change = 1546 >= dust -> transaction.
        result, _, mock_out = _run_invalidate(will, wallet, fees_per_byte=10)

        assert result is not None
        assert mock_out.call_args_list[-1][0][1] == 1546


# ================================================================== #
# Will.set_invalidate - status flag cascade
# ================================================================== #

class TestSetInvalidate:
    """Tests for ``Will.set_invalidate()``: marking will items as invalidated."""

    def test_single_item_no_children(self):
        """Invalidating a single will item sets INVALIDATED and clears VALID."""
        item = _make_willitem(valid=True)
        item.children = {}
        will = {"willid1": item}

        Will.set_invalidate("willid1", will)

        assert item.get_status("INVALIDATED") is True
        assert item.get_status("VALID") is False

    def test_cascades_to_direct_children(self):
        """Invalidating a parent cascades INVALIDATED to its children."""
        parent = _make_willitem(valid=True)
        child = _make_willitem(valid=True)

        parent.children = {"child_id": ["child_id", 0, 0]}
        child.children = {}

        will = {"parent_id": parent, "child_id": child}

        Will.set_invalidate("parent_id", will)

        assert parent.get_status("INVALIDATED") is True
        assert parent.get_status("VALID") is False
        assert child.get_status("INVALIDATED") is True
        assert child.get_status("VALID") is False

    def test_cascades_to_grandchildren(self):
        """Invalidating cascades through multiple levels of descendants."""
        root = _make_willitem(valid=True)
        branch = _make_willitem(valid=True)
        leaf = _make_willitem(valid=True)

        root.children = {"branch_id": ["branch_id", 0, 0]}
        branch.children = {"leaf_id": ["leaf_id", 0, 0]}
        leaf.children = {}

        will = {
            "root_id": root,
            "branch_id": branch,
            "leaf_id": leaf,
        }

        Will.set_invalidate("root_id", will)

        for name, item in [("root", root), ("branch", branch), ("leaf", leaf)]:
            assert item.get_status("INVALIDATED") is True, f"{name} should be INVALIDATED"
            assert item.get_status("VALID") is False, f"{name} should not be VALID"

    def test_empty_children_dict(self):
        """A will item with an empty children dict is a leaf (no cascade)."""
        item = _make_willitem(valid=True)
        item.children = {}
        will = {"wid": item}

        Will.set_invalidate("wid", will)

        assert item.get_status("INVALIDATED") is True
        assert item.get_status("VALID") is False

    def test_does_not_affect_siblings(self):
        """Invalidating one item does not affect unrelated siblings."""
        item_a = _make_willitem(valid=True)
        item_b = _make_willitem(valid=True)

        item_a.children = {}
        item_b.children = {}

        will = {"a": item_a, "b": item_b}

        Will.set_invalidate("a", will)

        assert item_a.get_status("INVALIDATED") is True
        assert item_a.get_status("VALID") is False
        assert item_b.get_status("INVALIDATED") is False
        assert item_b.get_status("VALID") is True

    def test_multiple_children(self):
        """Invalidating a parent with multiple children cascades to all of them."""
        parent = _make_willitem(valid=True)
        child1 = _make_willitem(valid=True)
        child2 = _make_willitem(valid=True)

        parent.children = {
            "c1": ["c1", 0, 0],
            "c2": ["c2", 0, 0],
        }
        child1.children = {}
        child2.children = {}

        will = {"p": parent, "c1": child1, "c2": child2}

        Will.set_invalidate("p", will)

        assert parent.get_status("INVALIDATED") is True
        assert child1.get_status("INVALIDATED") is True
        assert child2.get_status("INVALIDATED") is True

    def test_idempotent(self):
        """Setting INVALIDATED twice on the same item is a safe no-op."""
        item = _make_willitem(valid=True)
        item.children = {}
        will = {"wid": item}

        Will.set_invalidate("wid", will)
        Will.set_invalidate("wid", will)

        assert item.get_status("INVALIDATED") is True
        assert item.get_status("VALID") is False


# ------------------------------------------------------------------ #
# Main
# ------------------------------------------------------------------ #

if __name__ == "__main__":
    for name in sorted(dir()):
        if name.startswith("test_"):
            globals()[name]()
            print(f"  [OK] {name}")
    print("[OK] All invalidation tests passed")
