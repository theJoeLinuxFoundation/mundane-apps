"""Record countdown notifications on the private test session bus."""
import os
from pathlib import Path
from gi.repository import Gio, GLib

work = Path(os.environ["XDG_RUNTIME_DIR"])
name = "org.freedesktop.Notifications"
xml = f"""<node><interface name="{name}"><method name="Notify">
{''.join(f'<arg direction="in" type="{t}"/>' for t in ('s','u','s','s','s','as','a{sv}','i'))}
<arg direction="out" type="u"/></method><method name="CloseNotification">
<arg direction="in" type="u"/></method></interface></node>"""

def call(bus, sender, path, iface, method, args, invocation):
    values = args.unpack()
    if method == "Notify":
        if values[3] == "Alarm":
            (work / "alarm-notified").write_text(values[4])
        else:
            assert values[3] == "Timer finished"
            (work / "notified").touch()
        invocation.return_value(GLib.Variant("(u)", (42,)))
    else:
        assert values == (42,)
        (work / "dismissed").touch()
        invocation.return_value(GLib.Variant("()", ()))

bus = Gio.bus_get_sync(Gio.BusType.SESSION)
bus.register_object("/org/freedesktop/Notifications",
                    Gio.DBusNodeInfo.new_for_xml(xml).interfaces[0], call, None, None)
Gio.bus_own_name_on_connection(bus, name, Gio.BusNameOwnerFlags.NONE,
                               lambda *_: (work / "notify-ready").touch(), None)
GLib.MainLoop().run()
