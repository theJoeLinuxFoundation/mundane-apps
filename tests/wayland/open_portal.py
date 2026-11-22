"""A private OpenURI service for exercising FD passing and portal cancellation."""
import os
from pathlib import Path
import subprocess

from gi.repository import Gio, GLib

service = "org.freedesktop.portal.Desktop"
interface = "org.freedesktop.portal.OpenURI"
desktop = "/org/freedesktop/portal/desktop"
node = Gio.DBusNodeInfo.new_for_xml(f"""<node><interface name="{interface}">
<method name="OpenFile"><arg type="s" direction="in"/><arg type="h" direction="in"/>
<arg type="a{{sv}}" direction="in"/><arg type="o" direction="out"/></method>
<property name="version" type="u" access="read"/></interface></node>""")
runtime = Path(os.environ["XDG_RUNTIME_DIR"])


def open_file(bus, sender, path, iface, method, args, call):
    _, index, options = args.unpack()
    with os.fdopen(call.get_message().get_unix_fd_list().get(index), "rb") as stream:
        assert stream.read() == b"Field notes\n"
        source = os.readlink(f"/proc/self/fd/{stream.fileno()}")
    handle = f"{desktop}/request/{sender[1:].replace('.', '_')}/{options['handle_token']}"
    call.return_value(GLib.Variant("(o)", (handle,)))
    cancel = (runtime / "cancel-open").exists()
    if not cancel:
        subprocess.run(["gio", "open", source], check=True)
    bus.emit_signal(sender, handle, "org.freedesktop.portal.Request", "Response",
                    GLib.Variant("(ua{sv})", (1 if cancel else 0, {})))
    (runtime / ("cancelled" if cancel else "accepted")).touch()


def on_bus(bus, name):
    bus.register_object(desktop, node.interfaces[0], open_file,
                        lambda *_: GLib.Variant("u", 5), None)


Gio.bus_own_name(Gio.BusType.SESSION, service, Gio.BusNameOwnerFlags.NONE, on_bus,
                 lambda *_: (runtime / "portal-ready").touch(), None)
GLib.MainLoop().run()
