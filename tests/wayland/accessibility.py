"""Inspect AT-SPI application roots while Terminal's Find dialog is focused."""
import sys
import time
import gi

if "--control" in sys.argv:
    gi.require_version("Gtk", "3.0")
    from gi.repository import GLib, Gtk
    GLib.set_prgname("mundane-probe-control")
    app = Gtk.Window(title="AT-SPI control")
    app.set_accept_focus(False)
    app.add(Gtk.Button(label="Probe button"))
    app.show_all()
    Gtk.main()
else:
    gi.require_version("Atspi", "2.0")
    from gi.repository import Atspi
    Atspi.init()
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        desktop = Atspi.get_desktop(0)
        roots = [desktop.get_child_at_index(i) for i in range(desktop.get_child_count())]
        names = [app.get_name() for app in roots]
        if "mundane-probe-control" in names:
            break
        time.sleep(0.1)
    assert "mundane-probe-control" in names, names
    terminal_pid = int(sys.argv[1])
    assert all(app.get_process_id() != terminal_pid for app in roots), names
    def tree(node):
        yield node.get_role_name(), node.get_name()
        for i in range(node.get_child_count()):
            yield from tree(node.get_child_at_index(i))
    nodes = list(tree(roots[names.index("mundane-probe-control")]))
    assert ("push button", "Probe button") in nodes or ("button", "Probe button") in nodes, nodes
    print("AT-SPI roots:", names, "control tree:", nodes)
