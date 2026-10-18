"""
Tests for the "Invalidation fee policy" settings-dialog row (ADVANCED only).

Covers:
  - The row is hidden in BASIC mode and visible in ADVANCED mode (like the
    other advanced-only rows).
  - The Electrum fee widget (``FeeSlider`` + ``FeeComboBox``) is bound to the
    ``INVALIDATION_FEE_POLICY`` config: choosing a Feerate / ETA / Mempool
    policy persists its descriptor (``"<method>:<value>"``).
  - "Reset to Default Setting" restores the policy to its default ("eta:2").

Run:
    source "$BAL_HOME/electrum/env/bin/activate"
    QT_QPA_PLATFORM=offscreen python3 tests/test_settings_invalidation_fee.py
"""

import sys
import tempfile

sys.path.insert(0, __file__.rsplit("/", 2)[0])

from unittest.mock import patch

from PyQt6.QtWidgets import QApplication, QComboBox, QLabel, QPushButton

_app = QApplication.instance() or QApplication(sys.argv)

from electrum.fee_policy import FeeMethod, FeePolicy
from electrum.gui.qt.fee_slider import FeeComboBox, FeeSlider
from electrum.simple_config import SimpleConfig

import bal.gui.qt.plugin as plugin_mod
from bal.gui.qt.plugin import Plugin

LABEL_TEXT = "Invalidation fee policy"


def _isolated_config(**overrides):
    """An in-memory SimpleConfig that never touches the real Electrum config."""
    opts = {"electrum_path": tempfile.mkdtemp(prefix="bal_test_")}
    opts.update(overrides)
    return SimpleConfig(opts)


def _build_dialog(user_type, config=None, **config_overrides):
    """Build the plugin settings dialog for *user_type* and return it.

    *config* (optional) lets a test pass a pre-populated SimpleConfig; overrides
    are used to build one otherwise. Note that overrides become command-line
    options, which ``set()`` refuses to modify - use *config* when the test needs
    a mutable stored value.
    """
    cfg = config if config is not None else _isolated_config(**config_overrides)
    plugin = Plugin(None, cfg, "bal")
    plugin.get_window_title = lambda s: s
    plugin.read_file = lambda *a: b""
    plugin.broadcast_transactions = lambda *a, **k: None
    plugin.update_all = lambda *a, **k: None
    plugin.USER_TYPE.set(user_type)
    captured = {}
    with patch.object(
        plugin_mod, "show_modal", lambda d: captured.setdefault("d", d) or True
    ):
        plugin.settings_dialog(None)
    return plugin, captured["d"]


def _find_label(dialog):
    """Return the "Invalidation fee policy" QLabel (wrapped by _hide_if_basic)."""
    for lbl in dialog.findChildren(QLabel):
        if lbl.text() == LABEL_TEXT:
            return lbl
    raise AssertionError("invalidation fee policy label not found")


# ------------------------------------------------------------------ #
# Visibility
# ------------------------------------------------------------------ #

def test_row_hidden_in_basic_and_visible_in_advanced():
    _plugin, basic = _build_dialog("basic")
    assert _find_label(basic).isHidden() is True
    basic.close()

    _plugin, advanced = _build_dialog("advanced")
    assert _find_label(advanced).isHidden() is False
    advanced.close()


def _user_type_combo(dialog):
    """Return the BASIC/ADVANCED selector combo of the settings dialog."""
    for combo in dialog.findChildren(QComboBox):
        if combo.itemText(0) == "BASIC" and combo.itemText(1) == "ADVANCED":
            return combo
    raise AssertionError("user type combo not found")


def test_row_follows_live_mode_switch():
    """The row must show/hide when the mode is toggled at runtime (not only
    the initial creation-time visibility)."""
    # BASIC -> ADVANCED (with the confirmation phrase).
    plugin, dialog = _build_dialog("basic")
    combo = _user_type_combo(dialog)
    with patch(
        "PyQt6.QtWidgets.QInputDialog.getText",
        return_value=("at My Risk", True),
    ):
        combo.setCurrentIndex(1)
    assert plugin.USER_TYPE.get() == "advanced"
    assert _find_label(dialog).isHidden() is False
    dialog.close()

    # ADVANCED -> BASIC (no confirmation needed).
    plugin, dialog = _build_dialog("advanced")
    combo = _user_type_combo(dialog)
    combo.setCurrentIndex(0)
    assert plugin.USER_TYPE.get() == "basic"
    assert _find_label(dialog).isHidden() is True
    dialog.close()


# ------------------------------------------------------------------ #
# Persistence
# ------------------------------------------------------------------ #

def test_default_policy_is_eta_two():
    plugin, dialog = _build_dialog("advanced")
    assert plugin.INVALIDATION_FEE_POLICY.default == "eta:2"
    assert plugin.INVALIDATION_FEE_POLICY.get() == "eta:2"
    dialog.close()


def test_hand_edited_policy_is_not_rewritten_on_open():
    """Opening the dialog must not normalize (snap/persist) a hand-edited
    descriptor; only an explicit user change should do that."""
    # "eta:3" is not a slider target (FEE_ETA_TARGETS = [1008, 144, 25, 10,
    # 5, 2, 1]), so a naive slider.update() would snap it to "eta:2". The value
    # is written to the isolated config (not passed as an override, which would
    # become a non-modifiable command-line option and hide the bug).
    config = _isolated_config()
    config.set_key("bal_invalidation_fee_policy", "eta:3")
    plugin, dialog = _build_dialog("advanced", config=config)
    assert plugin.INVALIDATION_FEE_POLICY.get() == "eta:3"
    dialog.close()


def test_choosing_a_fixed_feerate_is_persisted():
    plugin, dialog = _build_dialog("advanced")
    combo = dialog.findChildren(FeeComboBox)[0]
    slider = dialog.findChildren(FeeSlider)[0]

    combo.setCurrentIndex(FeeMethod.slider_index_of_method(FeeMethod.FEERATE))
    slider.fee_policy.value = 5000  # sat/kvB
    slider.update()

    assert plugin.INVALIDATION_FEE_POLICY.get() == "feerate:5000"
    dialog.close()


def test_choosing_an_eta_target_is_persisted():
    plugin, dialog = _build_dialog("advanced")
    combo = dialog.findChildren(FeeComboBox)[0]
    slider = dialog.findChildren(FeeSlider)[0]

    combo.setCurrentIndex(FeeMethod.slider_index_of_method(FeeMethod.ETA))
    slider.fee_policy.value = 10  # blocks (valid FixedTargets entry)
    slider.update()

    assert plugin.INVALIDATION_FEE_POLICY.get() == "eta:10"
    dialog.close()


# ------------------------------------------------------------------ #
# Reset to default
# ------------------------------------------------------------------ #

def test_reset_to_default_restores_eta_two():
    plugin, dialog = _build_dialog("advanced")
    combo = dialog.findChildren(FeeComboBox)[0]
    slider = dialog.findChildren(FeeSlider)[0]

    combo.setCurrentIndex(FeeMethod.slider_index_of_method(FeeMethod.FEERATE))
    slider.fee_policy.value = 5000
    slider.update()
    assert plugin.INVALIDATION_FEE_POLICY.get() == "feerate:5000"

    reset_btn = [
        b for b in dialog.findChildren(QPushButton)
        if b.text() == "Reset to Default Setting"
    ][0]
    reset_btn.click()

    assert plugin.INVALIDATION_FEE_POLICY.get() == "eta:2"
    assert slider.fee_policy.get_descriptor() == "eta:2"
    assert FeePolicy("eta:2").method == FeeMethod.ETA
    dialog.close()


# ------------------------------------------------------------------ #
# Main
# ------------------------------------------------------------------ #

if __name__ == "__main__":
    for name in sorted(dir()):
        if name.startswith("test_"):
            globals()[name]()
            print(f"  [OK] {name}")
    print("[OK] All invalidation fee settings tests passed")
