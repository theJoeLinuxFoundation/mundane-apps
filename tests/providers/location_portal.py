"""One-shot XDG Location portal for the disposable session bus."""
from pathlib import Path
import os
from gi.repository import Gio, GLib

work = Path(os.environ["XDG_RUNTIME_DIR"])
desktop = "/org/freedesktop/portal/desktop"
iface = "org.freedesktop.portal.Location"
bus = Gio.bus_get_sync(Gio.BusType.SESSION)
def interface(name, methods):
    return Gio.DBusNodeInfo.new_for_xml(f'<node><interface name="{name}">{methods}'
        '<property name="version" type="u" access="read"/></interface></node>').interfaces[0]
def props(*_):
    return GLib.Variant("u", 1)
def close(_, sender, path, iface, method, args, invocation):
    (work / "location-closed").touch()
    invocation.return_value(GLib.Variant("()", ()))
def call(_, sender, path, interface_name, method, args, invocation):
    session_id = sender[1:].replace(".", "_")
    if method == "CreateSession":
        opts, = args.unpack()
        session = f"{desktop}/session/{session_id}/{opts['session_handle_token']}"
        bus.register_object(session, interface("org.freedesktop.portal.Session", '<method name="Close"/>'), close, props, None)
        invocation.return_value(GLib.Variant("(o)", (session,)))
    else:
        session, parent, opts = args.unpack()
        request = f"{desktop}/request/{session_id}/{opts['handle_token']}"
        invocation.return_value(GLib.Variant("(o)", (request,)))
        def send():
            denied = (work / "location-deny").exists()
            bus.emit_signal(sender, request, "org.freedesktop.portal.Request", "Response",
                            GLib.Variant("(ua{sv})", (1 if denied else 0, {})))
            if not denied:
                point = {k:GLib.Variant("d", v) for k,v in dict(Accuracy=150., Altitude=0., Heading=-1.,
                    Speed=-1., Latitude=59.91, Longitude=10.75).items()}
                point.update(Description=GLib.Variant("s", "test fix"), Timestamp=GLib.Variant("(tt)", (1,0)))
                bus.emit_signal(sender, desktop, iface, "LocationUpdated", GLib.Variant("(oa{sv})", (session, point)))
            return False
        GLib.timeout_add(100, send)
bus.register_object(desktop, interface(iface, '<method name="CreateSession"><arg type="a{sv}" direction="in"/>'
    '<arg type="o" direction="out"/></method><method name="Start"><arg type="o" direction="in"/>'
    '<arg type="s" direction="in"/><arg type="a{sv}" direction="in"/><arg type="o" direction="out"/></method>'
    '<signal name="LocationUpdated"><arg type="o"/><arg type="a{sv}"/></signal>'), call, props, None)
Gio.bus_own_name_on_connection(bus, "org.freedesktop.portal.Desktop", Gio.BusNameOwnerFlags.NONE,
                               lambda *_: (work / "location-ready").touch(), None)
GLib.MainLoop().run()
