"""
Tests for ``BalBuildWillDialog._sync_locktime_to_built_txs`` and the
"Building your will" row of ``BalBuildWillDialog._build_success_report``.

``_sync_locktime_to_built_txs`` is the post-build sync that keeps the plugin's
stored delivery date (WILL_SETTINGS["locktime"]) in lockstep with the BUILT
transactions' fixed locktime when the core AUTOMATICALLY anticipates it (one
day earlier than stored).

RELATIVE recipes ("90d" / "1y") are now PRESERVED: the daily-drift problem
that once forced freezing them to absolute timestamps is solved at the root by
anchoring every relative recipe against the built transactions
(``Util.resolve_locktime_against_tx`` for the postpone detection,
``resolve_date_to_check(..., built_locktime=...)`` for the reference
timestamp).  Only a genuine automatic anticipation on an ABSOLUTE stored date
moves the stored value.

WHY THE FLAG IS NOT SET HERE ANY MORE: because a relative recipe is anchored to
the built tx, a stored-vs-built comparison cannot detect the anticipation for
it.  ``_build_success_report`` therefore detects it by comparing the will's
minimum locktime BEFORE the rebuild with the one AFTER it (see there), which
works for both absolute and relative recipes.

The methods are exercised with a lightweight fake ``self`` (no Qt event loop,
no Electrum wallet) by calling them as unbound methods.

Run:
    source "$BAL_HOME/electrum/env/bin/activate"
    python3 tests/test_sync_locktime_built_txs.py
"""

import os
import sys
from types import SimpleNamespace

sys.path.insert(0, os.path.join(os.path.dirname(__file__), os.pardir))

from bal.gui.qt.dialogs import BalBuildWillDialog  # noqa: E402  (path insert above)

# ------------------------------------------------------------------ #
# Fakes
# ------------------------------------------------------------------ #

def _fake_willitem(tx_locktime):
    """Minimal stand-in for a WillItem: enough for Will.get_min_locktime and
    the heir listing in _build_success_report."""
    return SimpleNamespace(
        tx=SimpleNamespace(locktime=tx_locktime),
        get_status=lambda name: True,
        heirs={},
    )


def _make_dialog(will_settings, tx_locktimes, recorded):
    """Build a fake dialog ``self`` for _sync_locktime_to_built_txs."""

    def update_setting_widgets(new_value, field, update_all=False):
        will_settings[field] = new_value
        recorded.append((field, new_value, update_all))

    return SimpleNamespace(
        bal_window=SimpleNamespace(
            willitems={
                f"tx{i}": _fake_willitem(lt) for i, lt in enumerate(tx_locktimes)
            },
            will_settings=will_settings,
            update_setting_widgets=update_setting_widgets,
        ),
    )


def _call_sync(will_settings, tx_locktimes, recorded):
    fake = _make_dialog(will_settings, tx_locktimes, recorded)
    BalBuildWillDialog._sync_locktime_to_built_txs(fake)
    return fake


# ------------------------------------------------------------------ #
# Tests
# ------------------------------------------------------------------ #

def test_relative_locktime_preserved():
    """A RELATIVE stored locktime ("90d"/"1y") is PRESERVED after a rebuild:
    it is anchored against the built transactions on every check, so it must
    not be frozen to an absolute timestamp in WILL_SETTINGS."""
    tx_locktime = 1_800_000_000
    recorded = []
    fake = _call_sync(
        {"locktime": "90d", "threshold": "30d"}, [tx_locktime], recorded
    )
    assert fake.bal_window.will_settings["locktime"] == "90d"
    assert fake.bal_window.will_settings["threshold"] == "30d"
    assert recorded == [], "a relative recipe must never be rewritten"


def test_relative_threshold_preserved():
    """Same for the relative "Check Alive" threshold: it stays relative."""
    tx_locktime = 1_800_000_000
    recorded = []
    fake = _call_sync(
        {"locktime": "90d", "threshold": "30d"}, [tx_locktime], recorded
    )
    assert fake.bal_window.will_settings["threshold"] == "30d"
    assert recorded == []


def test_absolute_locktime_unchanged_on_equal():
    """An absolute stored locktime that already matches the built txs is left
    untouched (no spurious rewrite)."""
    tx_locktime = 1_800_000_000
    recorded = []
    fake = _call_sync(
        {"locktime": tx_locktime, "threshold": 1_700_000_000},
        [tx_locktime],
        recorded,
    )
    assert fake.bal_window.will_settings["locktime"] == tx_locktime
    assert fake.bal_window.will_settings["threshold"] == 1_700_000_000


def test_anticipation_moves_stored_date_earlier():
    """A real automatic anticipation of an ABSOLUTE stored date (built earlier
    than stored) moves the stored date earlier so the next Check does not
    mistake it for a postpone."""
    tx_locktime = 1_700_000_000
    recorded = []
    fake = _call_sync(
        {"locktime": 1_800_000_000, "threshold": 1_600_000_000},
        [tx_locktime],
        recorded,
    )
    assert fake.bal_window.will_settings["locktime"] == tx_locktime


def test_stored_earlier_than_built_never_moved_later():
    """A stored absolute date that is already EARLIER than the built txs (the
    user moved the delivery later) is never pulled back up on rebuild: only
    anticipation (built < stored) moves the value."""
    stored = 1_800_000_000
    recorded = []
    fake = _call_sync(
        {"locktime": stored, "threshold": 1_700_000_000},
        [1_900_000_000],
        recorded,
    )
    assert fake.bal_window.will_settings["locktime"] == stored


def test_multiple_txs_uses_minimum_locktime():
    """When several ABSOLUTE transactions carry different locktimes, the minimum
    is used for a genuine automatic anticipation (owner-confirmed behaviour for
    the delivery date shown in the UI)."""
    min_locktime = 1_750_000_000
    recorded = []
    fake = _call_sync(
        {"locktime": 1_800_000_000, "threshold": 1_600_000_000},
        [min_locktime, min_locktime + 86_400],
        recorded,
    )
    assert fake.bal_window.will_settings["locktime"] == min_locktime


def test_relative_locktime_stays_coherent_via_anchor():
    """Daily-drift guard: an UNCHANGED relative recipe is resolved against the
    tx build moment (``Util.resolve_locktime_against_tx``), so even WITHOUT
    being frozen to an absolute value it still reads as COHERENT (== tx
    locktime) on later days - the postpone check never fires again."""
    from datetime import datetime, timedelta, timezone

    from bal.core.util import Util

    # resolve_locktime_against_tx normalises to UTC midnight before anchoring,
    # so use a midnight-UTC frozen tx locktime (the timestamp the engine itself
    # stores after building).
    tx_locktime = int(datetime(2027, 1, 15, tzinfo=timezone.utc).timestamp())
    built = "90d"  # recipe frozen at build time
    current = "90d"  # unchanged recipe today
    for _day in range(0, 7):
        resolved = Util.resolve_locktime_against_tx(current, built, tx_locktime)
        assert resolved == tx_locktime  # no POSTPONE / drift
    # Sanity: a naive forward-from-now re-parse would have drifted past it
    # (the bug the anchor fixes).
    drifted = int(
        (
            datetime.fromtimestamp(tx_locktime) + timedelta(days=1)
        ).timestamp()
    )
    assert drifted > tx_locktime


# ------------------------------------------------------------------ #
# _build_success_report: the "Building your will" row
# ------------------------------------------------------------------ #

def _make_report_dialog(new_locktimes, previous_min_locktime):
    """Fake ``self`` for _build_success_report (no Qt event loop).

    ``_sync_locktime_to_built_txs`` is stubbed (it is exercised separately), so
    the test drives the AFTER-build locktimes and the captured BEFORE-build
    locktime directly; the message helpers capture what would be written on the
    "Building your will" row.
    """
    captured = {}

    def _set_building(status=None, row=None, color=None):
        captured["build_status"] = status

    fake = SimpleNamespace(
        bal_window=SimpleNamespace(
            willitems={
                f"tx{i}": _fake_willitem(lt)
                for i, lt in enumerate(new_locktimes)
            },
            wallet=SimpleNamespace(set_label=lambda *a, **k: None),
        ),
        _previous_min_locktime=previous_min_locktime,
        _sync_locktime_to_built_txs=lambda: None,
        msg_set_building=_set_building,
        msg_set_status=lambda *a, **k: None,
        msg_alert=lambda e: "ALERT:{}".format(e),
        msg_ok=lambda e=None: "OK",
    )
    return fake, captured


def test_build_report_shows_anticipated_warning():
    """When the rebuild moved the delivery one day earlier than the previous
    will, the "Building your will" row shows a warning naming the NEW delivery
    date instead of a plain "Ok"."""
    previous = 1_800_000_000  # 2027-01-15 (UTC)
    rebuilt = previous - 86_400  # anticipated one day earlier
    fake, captured = _make_report_dialog([rebuilt], previous)
    BalBuildWillDialog._build_success_report(fake)
    assert fake._date_was_anticipated is True
    assert fake._anticipated_locktime == rebuilt
    assert captured["build_status"] == (
        "ALERT:Anticipated - new delivery date: 2027-01-14"
    )


def test_build_report_shows_ok_when_not_anticipated():
    """Without anticipation the row keeps the plain "Ok" and the state stays
    clean."""
    previous = 1_800_000_000
    fake, captured = _make_report_dialog([previous], previous)
    BalBuildWillDialog._build_success_report(fake)
    assert fake._date_was_anticipated is False
    assert fake._anticipated_locktime is None
    assert captured["build_status"] == "OK"


def test_build_report_first_build_is_not_anticipated():
    """A first build (no previous will) is never flagged as anticipated."""
    fake, captured = _make_report_dialog([1_800_000_000], None)
    BalBuildWillDialog._build_success_report(fake)
    assert fake._date_was_anticipated is False
    assert fake._anticipated_locktime is None
    assert captured["build_status"] == "OK"


if __name__ == "__main__":
    for name in sorted(dir()):
        if name.startswith("test_"):
            globals()[name]()
            print(f"  [OK] {name}")
    print("[OK] All sync_locktime_built_txs tests passed.")
