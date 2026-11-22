"""Fake Settings portal. Only the private test bus exports the control interface."""
from pathlib import Path
import os
from gi.repository import Gio, GLib

DESKTOP = "/org/freedesktop/portal/desktop"
SETTINGS = "org.freedesktop.portal.Settings"
CONTROL = "org.joelinux.TestSettings"
NS = "org.freedesktop.appearance"
ready = Path(os.environ["XDG_RUNTIME_DIR"]) / "settings-ready"
values = {"color-scheme": GLib.Variant("u", 2)}
xml = f"""<node><interface name="{SETTINGS}">
  <method name="ReadAll"><arg type="as" direction="in"/><arg type="a{{sa{{sv}}}}" direction="out"/></method>
  <method name="ReadOne"><arg type="s" direction="in"/><arg type="s" direction="in"/><arg type="v" direction="out"/></method>
  <method name="Read"><arg type="s" direction="in"/><arg type="s" direction="in"/><arg type="v" direction="out"/></method>
  <signal name="SettingChanged"><arg type="s"/><arg type="s"/><arg type="v"/></signal>
  <property name="version" type="u" access="read"/>
</interface><interface name="{CONTROL}">
  <method name="Set"><arg type="s" direction="in"/><arg type="v" direction="in"/></method>
</interface></node>"""

def call(bus, sender, path, iface, method, args, reply):
    if method == "Set":
        key = args.get_child_value(0).get_string()
        value = args.get_child_value(1).get_variant()
        values[key] = value
        bus.emit_signal(None, DESKTOP, SETTINGS, "SettingChanged", GLib.Variant("(ssv)", (NS, key, value)))
        reply.return_value(None)
    elif method == "ReadAll":
        namespaces, = args.unpack()
        result = {NS: values} if not namespaces or NS in namespaces else {}
        snapshot = GLib.Variant("(a{sa{sv}})", (result,))
        pending = ready.with_name("change-during-read")
        if namespaces == [NS] and pending.exists():
            pending.unlink()
            values["contrast"] = GLib.Variant("u", 1)
            bus.emit_signal(None, DESKTOP, SETTINGS, "SettingChanged",
                            GLib.Variant("(ssv)", (NS, "contrast", values["contrast"])))
        reply.return_value(snapshot)
    else:
        ns, key = args.unpack()
        if ns != NS or key not in values:
            reply.return_dbus_error("org.freedesktop.portal.Error.NotFound", "Unknown setting")
            return
        value = values[key]
        if method == "Read":
            value = GLib.Variant("v", value)
        reply.return_value(GLib.Variant("(v)", (value,)))

def register(bus, _):
    for iface in Gio.DBusNodeInfo.new_for_xml(xml).interfaces:
        bus.register_object(DESKTOP, iface, call, lambda *_: GLib.Variant("u", 2), None)

Gio.bus_own_name(Gio.BusType.SESSION, "org.freedesktop.portal.Desktop", Gio.BusNameOwnerFlags.NONE,
                 register, lambda *_: ready.touch(), None)
GLib.MainLoop().run()
