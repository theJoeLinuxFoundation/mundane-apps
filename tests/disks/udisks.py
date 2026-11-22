#!/usr/bin/env python3
"""Small UDisks service on an explicitly supplied private message bus."""
import json
import sys
from pathlib import Path
from gi.repository import Gio, GLib

address, directory = sys.argv[1:]
work = Path(directory)
name = "org.freedesktop.UDisks2"
root = "/org/freedesktop/UDisks2"
drive = root + "/drives/Coast_Field_Recorder"
disk = root + "/block_devices/testdisk"
volume = disk + "1"
reserve = disk + "2"
mount_at = "/run/media/joe/Survey"
om = "org.freedesktop.DBus.ObjectManager"
filesystem = name + ".Filesystem"
v = GLib.Variant
gib = 2**30

def block(device, number, size, title="", fs=""):
    return {"Drive": v("o",drive), "PreferredDevice": v("ay",device.encode()+b"\0"),
            "Size": v("t",size), "DeviceNumber": v("t",number), "IdLabel": v("s",title),
            "IdType": v("s",fs), "IdUUID": v("s", "replacement" if (work / "replace").exists() else f"uuid-{number}")}

def objects():
    disks = {drive: {name+".Drive": {"Vendor": v("s","Coast"), "Model": v("s","Field Recorder"),
             "Id": v("s","Coast-123"), "TimeDetected":v("t",1000), "TimeMediaDetected":v("t",1100),
             "ConnectionBus": v("s","usb"), "Removable":v("b",True), "Size":v("t",64*gib)}},
             disk: {name+".Block":block("/dev/testdisk",80,64*gib),
                    name+".PartitionTable":{"Type":v("s","gpt")}}}
    for path, num, size, title, fs, offset in [(volume,1,48*gib,"Survey","ext4",1048576),
                                              (reserve,2,15*gib,"Transfer","vfat",48*gib+1048576)]:
        if path == volume and (work / "removed").exists():
            continue
        mounts = [mount_at.encode()+b"\0"] if path == volume and (work / "mounted").exists() else []
        disks[path] = {name+".Block":block(f"/dev/testdisk{num}",80+num,size,title,fs),
                       name+".Partition":{"Table":v("o",disk), "Offset":v("t",offset),
                                          "Number":v("u",num),"UUID":v("s",f"part-{num}")},
                       filesystem:{"MountPoints":v("aay",mounts)}}
    return disks

def call(bus, sender, path, iface, method, args, invocation):
    if method == "GetManagedObjects":
        (work / "listed").touch()
        invocation.return_value(v("(a{oa{sa{sv}}})",(objects(),)))
        return
    flags = invocation.get_message().get_flags()
    interactive = bool(flags & Gio.DBusMessageFlags.ALLOW_INTERACTIVE_AUTHORIZATION)
    with (work / "calls.jsonl").open("a") as log:
        print(json.dumps([path,method,interactive,args.unpack()[0]]), file=log)
    if path != volume or not interactive or args.unpack() != ({},):
        invocation.return_dbus_error(name+".Error.Failed","Unexpected test request")
        return
    if (work / "deny").exists():
        invocation.return_dbus_error(name+".Error.NotAuthorized","Test policy declined the request.")
        return
    if (work / "remove-during").exists() or (work / "removed").exists():
        (work / "removed").touch()
        invocation.return_dbus_error("org.freedesktop.DBus.Error.UnknownObject","Device was removed.")
        return
    if method == "Mount":
        (work / "mounted").touch()
        invocation.return_value(v("(s)",(mount_at,)))
    elif method == "Unmount":
        (work / "mounted").unlink()
        invocation.return_value(v("()",()))

xml = f"""<node><interface name="{om}"><method name="GetManagedObjects">
<arg direction="out" type="a{{oa{{sa{{sv}}}}}}"/></method></interface>
<interface name="{filesystem}"><method name="Mount"><arg direction="in" type="a{{sv}}"/>
<arg direction="out" type="s"/></method><method name="Unmount"><arg direction="in" type="a{{sv}}"/>
</method></interface></node>"""
interfaces = Gio.DBusNodeInfo.new_for_xml(xml).interfaces
conn = Gio.DBusConnection.new_for_address_sync(address, Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT |
                                               Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION, None, None)
conn.register_object(root, interfaces[0], call, None, None)
for path in (volume,reserve):
    conn.register_object(path, interfaces[1], call, None, None)
Gio.bus_own_name_on_connection(conn,name,Gio.BusNameOwnerFlags.NONE,
                               lambda *_:(work/"ready").touch(),None)
GLib.MainLoop().run()
