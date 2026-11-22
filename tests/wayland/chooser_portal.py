"""Private FileChooser replies for native Open and Save As dialogs."""
import os
import sys
from pathlib import Path
from gi.repository import Gio, GLib

runtime = Path(os.environ["XDG_RUNTIME_DIR"])
service = "org.freedesktop.portal.Desktop"
desktop = "/org/freedesktop/portal/desktop"
methods = "".join(f'<method name="{method}"><arg type="s" direction="in"/><arg type="s" direction="in"/>'
                  '<arg type="a{sv}" direction="in"/><arg type="o" direction="out"/></method>'
                  for method in ("OpenFile", "SaveFile"))
node = Gio.DBusNodeInfo.new_for_xml(f'<node><interface name="org.freedesktop.portal.FileChooser">'
                                    f'{methods}<property name="version" type="u" access="read"/></interface></node>')


open_choices = iter(sys.argv[1:])


def choose(bus, sender, path, iface, method, args, call):
    _, _, options = args.unpack()
    handle = f"{desktop}/request/{sender[1:].replace('.', '_')}/{options['handle_token']}"
    call.return_value(GLib.Variant("(o)", (handle,)))
    dest = Path.home() / ("Saved copy.txt" if method == "SaveFile" else "Second.txt")
    if method == "OpenFile" and len(sys.argv) > 1:
        dest = Path(next(open_choices, sys.argv[-1]))
    cancel = (runtime / "cancel-choice").exists()
    # Portal requests reply with a handle before the user's subsequent choice.
    def respond():
        bus.emit_signal(sender, handle, "org.freedesktop.portal.Request", "Response",
                        GLib.Variant("(ua{sv})", (1, {}) if cancel else (0, {"uris": GLib.Variant("as", [dest.as_uri()])})))
        (runtime / method).touch()
        return GLib.SOURCE_REMOVE
    GLib.timeout_add(100, respond)


def start(bus, _):
    if os.environ.get("MUNDANE_TEST_PRINT"):
        import print_fixture
        bus.register_object(desktop, print_fixture.iface, print_fixture.handle, lambda *_: GLib.Variant("u", 4), None)
    bus.register_object(desktop, node.interfaces[0], choose, lambda *_: GLib.Variant("u", 4), None)


Gio.bus_own_name(Gio.BusType.SESSION, service, Gio.BusNameOwnerFlags.NONE, start,
                 lambda *_: (runtime / "chooser-ready").touch(), None)
GLib.MainLoop().run()
