"""Read the exact file descriptor passed to the private Print portal."""
from gi.repository import Gio, GLib
import os
from pathlib import Path

iface = Gio.DBusNodeInfo.new_for_xml('''<node><interface name="org.freedesktop.portal.Print">
<method name="Print"><arg type="s" direction="in"/><arg type="s" direction="in"/>
<arg type="h" direction="in"/><arg type="a{sv}" direction="in"/><arg type="o" direction="out"/></method>
<property name="version" type="u" access="read"/></interface></node>''').interfaces[0]


def handle(bus, sender, path, interface, method, args, call):
    _, title, index, options = args.unpack()
    assert title.endswith(".pdf") and "token" not in options
    runtime = Path(os.environ["XDG_RUNTIME_DIR"])
    address = f"{path}/request/{sender[1:].replace('.', '_')}/{options['handle_token']}"
    with os.fdopen(call.get_message().get_unix_fd_list().get(index), "rb") as f:
        (runtime / "print-input.pdf").write_bytes(f.read())
    call.return_value(GLib.Variant("(o)", (address,)))
    def respond():
        bus.emit_signal(sender, address, "org.freedesktop.portal.Request", "Response",
                        GLib.Variant("(ua{sv})", (1 if (runtime / "cancel-print").exists() else 0, {})))
        return GLib.SOURCE_REMOVE
    GLib.timeout_add(100, respond)
