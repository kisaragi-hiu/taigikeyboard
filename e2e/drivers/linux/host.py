"""The e2e text field: one GTK 3 entry the Linux driver types into.

Usage: host.py <out-file> [<focus-file>]. On SIGUSR1 it writes the entry's
committed text (never the preedit) to <out-file> and exits. The window title
is `e2ehost`, which the Xvfb driver searches for; the desktop driver instead
waits for <focus-file>, created once the window really has keyboard focus.
"""

from __future__ import annotations

import signal
import sys
from pathlib import Path

import gi

gi.require_version("Gtk", "3.0")
from gi.repository import GLib, Gtk

WINDOW_TITLE = "e2ehost"


def main() -> int:
    out_file = sys.argv[1]
    window = Gtk.Window(title=WINDOW_TITLE)
    entry = Gtk.Entry()
    window.add(entry)
    window.connect("destroy", Gtk.main_quit)
    if len(sys.argv) > 2:
        focus_file = sys.argv[2]

        def mark_focused(*_) -> bool:
            # Window activation and entry focus arrive in either order.
            if window.is_active() and entry.has_focus():
                Path(focus_file).touch()
            return False

        window.connect("notify::is-active", mark_focused)
        entry.connect("notify::has-focus", mark_focused)
    window.show_all()

    def dump() -> bool:
        with open(out_file, "w", encoding="utf-8") as out:
            out.write(entry.get_text())
        Gtk.main_quit()
        return GLib.SOURCE_REMOVE

    GLib.unix_signal_add(GLib.PRIORITY_DEFAULT, signal.SIGUSR1, dump)
    Gtk.main()
    return 0


if __name__ == "__main__":
    sys.exit(main())
