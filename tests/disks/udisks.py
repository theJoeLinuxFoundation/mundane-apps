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
clear = root + "/block_devices/clear"
clear_mount = "/run/media/joe/Private notes"
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
    if (work / "powered-off").exists() or (work / "ejected").exists():
        return {}
    disks = {drive: {name+".Drive": {"Vendor": v("s","Coast"), "Model": v("s","Field Recorder"),
             "Id": v("s","Coast-123"), "TimeDetected":v("t",1000), "TimeMediaDetected":v("t",1100),
             "CanPowerOff":v("b",True),"Ejectable":v("b",True),"ConnectionBus": v("s","usb"), "Removable":v("b",True), "Size":v("t",64*gib)}},
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
    if (work / "encrypted").exists():
        vault = disk + "3"
        unlocked = (work / "unlocked").exists()
        disks[vault] = {name+".Block":block("/dev/testdisk3",83,gib//2,"Field vault","crypto_LUKS"),
                        name+".Partition":{"Table":v("o",disk),"Offset":v("t",63*gib+1048576),"Number":v("u",3)},
                        name+".Encrypted":{"CleartextDevice":v("o",clear if unlocked else "/")}}
        if unlocked:
            (work / "listed-unlocked").touch()
            disks[clear]={name+".Block":dict(block("/dev/dm-0",92,gib//2,"Private notes","ext4"),Drive=v("o","/"),CryptoBackingDevice=v("o",vault)),
                          filesystem:{"MountPoints":v("aay",[clear_mount.encode()+b"\0"] if (work/"clear-mounted").exists() else [])}}
    return disks

def call(bus, sender, path, iface, method, args, invocation):
    if method == "GetManagedObjects":
        (work / "listed").touch()
        invocation.return_value(v("(a{oa{sa{sv}}})",(objects(),)))
        return
    flags = invocation.get_message().get_flags()
    interactive = bool(flags & Gio.DBusMessageFlags.ALLOW_INTERACTIVE_AUTHORIZATION)
    if method in ("Unlock","Lock","PowerOff","Eject"):
        if not interactive or (work / "deny").exists():
            invocation.return_dbus_error(name+".Error.NotAuthorized","Authentication refused")
        elif method=="Unlock" and args.unpack()==("survey", {}):
            (work/"unlocked").touch()
            invocation.return_value(v("(o)",(root+"/block_devices/clear",)))
        elif method=="Lock" and args.unpack()==({},) and (work/"unlocked").exists():
            (work/"unlocked").unlink()
            invocation.return_value(v("()",()))
        elif method in ("PowerOff","Eject") and args.unpack()==({},):
            (work/("powered-off" if method=="PowerOff" else "ejected")).touch()
            invocation.return_value(v("()",()))
        else:
            invocation.return_dbus_error(name+".Error.Failed","Could not complete operation")
        return
    with (work / "calls.jsonl").open("a") as log:
        print(json.dumps([path,method,interactive,args.unpack()[0]]), file=log)
    if (path != volume and not (path == clear and (work / "unlocked").exists())) or not interactive or args.unpack() != ({},):
        invocation.return_dbus_error(name+".Error.Failed","Unexpected test request")
        return
    if (work / "deny").exists():
        invocation.return_dbus_error(name+".Error.NotAuthorized","Test policy declined the request.")
        return
    if (work / "remove-during").exists() or (work / "removed").exists():
        (work / "removed").touch()
        invocation.return_dbus_error("org.freedesktop.DBus.Error.UnknownObject","Device was removed.")
        return
    marker = work / ("clear-mounted" if path == clear else "mounted")
    if method == "Mount":
        marker.touch()
        invocation.return_value(v("(s)",(clear_mount if path == clear else mount_at,)))
    elif method == "Unmount":
        marker.unlink()
        invocation.return_value(v("()",()))

xml = f"""<node><interface name="{om}"><method name="GetManagedObjects">
<arg direction="out" type="a{{oa{{sa{{sv}}}}}}"/></method></interface>
<interface name="{filesystem}"><method name="Mount"><arg direction="in" type="a{{sv}}"/>
<arg direction="out" type="s"/></method><method name="Unmount"><arg direction="in" type="a{{sv}}"/>
</method></interface><interface name="{name}.Drive"><method name="PowerOff"><arg direction="in" type="a{{sv}}"/></method>
<method name="Eject"><arg direction="in" type="a{{sv}}"/></method></interface>
<interface name="{name}.Encrypted"><method name="Unlock"><arg direction="in" type="s"/><arg direction="in" type="a{{sv}}"/><arg direction="out" type="o"/></method><method name="Lock"><arg direction="in" type="a{{sv}}"/></method></interface></node>"""
interfaces = Gio.DBusNodeInfo.new_for_xml(xml).interfaces
conn = Gio.DBusConnection.new_for_address_sync(address, Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT |
                                               Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION, None, None)
conn.register_object(root, interfaces[0], call, None, None)
conn.register_object(drive, interfaces[2], call, None, None)
conn.register_object(disk+"3", interfaces[3], call, None, None)
for path in (volume,reserve,clear):
    conn.register_object(path, interfaces[1], call, None, None)
Gio.bus_own_name_on_connection(conn,name,Gio.BusNameOwnerFlags.NONE,
                               lambda *_:(work/"ready").touch(),None)
GLib.MainLoop().run()
