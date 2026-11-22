"""Read the published AccessKit trees and activate controls on the private test buses."""
import os
from pathlib import Path
import subprocess
import time

import gi
gi.require_version("Atspi", "2.0")
from gi.repository import Atspi, Gio, GLib


def enable(stack, launch, until, runtime):
    launch(stack, "/usr/lib/at-spi-bus-launcher", "--launch-immediately", log="design-bus.log")
    bus = Gio.bus_get_sync(Gio.BusType.SESSION)
    def address():
        try:
            return bus.call_sync("org.a11y.Bus", "/org/a11y/bus", "org.a11y.Bus", "GetAddress",
                                 None, None, 0, 1000, None).unpack()[0]
        except GLib.Error:
            return None
    addr = until(address, "private accessibility bus")
    assert Path(addr.split("unix:path=",1)[1].split(",")[0]).is_relative_to(runtime)
    os.environ["AT_SPI_BUS_ADDRESS"] = addr
    bus.call_sync("org.a11y.Bus", "/org/a11y/bus", "org.freedesktop.DBus.Properties", "Set",
                  GLib.Variant("(ssv)", ("org.a11y.Status", "IsEnabled", GLib.Variant("b", True))),
                  None, 0, 1000, None)
    launch(stack, "/usr/lib/at-spi2-registryd", log="design-registry.log")
    Atspi.init()


def verify(group, pid, key):
    def walk(node):
        yield node
        for i in range(node.get_child_count()):
            if child := node.get_child_at_index(i):
                yield from walk(child)
    def wait(check, msg):
        end = time.monotonic() + 6
        while time.monotonic() < end:
            while GLib.MainContext.default().pending():
                GLib.MainContext.default().iteration(False)
            if result := check():
                return result
            time.sleep(0.03)
        raise AssertionError(msg)
    def tree():
        desktop = Atspi.get_desktop(0)
        for i in range(desktop.get_child_count()):
            a = desktop.get_child_at_index(i)
            if a and a.get_process_id() == pid:
                return list(walk(a))
        return []
    def named(name):
        return wait(lambda: next((n for n in tree() if n.get_name() == name), None), name)
    def click(name):
        n = named(name)
        for i in range(n.get_n_actions()):
            if n.get_action_name(i) == "click":
                assert n.do_action(i)
                return
        raise AssertionError(("missing Click",name))
    def focused(name):
        def check():
            n=named(name)
            n.clear_cache()
            return n.get_state_set().contains(Atspi.StateType.FOCUSED)
        wait(check, "focus on " + name)
    if group == "panel":
        focused("Overview")
        key("Return")
        named("Commands activated: 1")
        assert "Running" in named("Editor").get_description()
        assert "Occupied" in named("Workspace 2").get_description()
        assert named("Unavailable command").get_n_actions() == 0
        click("Show labels")
        assert named("Count").get_role() == Atspi.Role.STATUS_BAR
        assert named("Show keyboard help").get_role() == Atspi.Role.TOOL_TIP
        click("Editor")
        named("Commands activated: 2")
    elif group == "menus":
        focused("Filter options")
        subprocess.run(["wtype","south"],check=True)
        key("Down")
        focused("South quay")
        key("Return")
        named("Selected: South quay")
        focused("Locations")
        click("Actions")  # Second activation closes the initially open menu.
        focused("Actions")
        click("Actions")
        focused("Open")
        key("Down")
        focused("Delete")  # Disabled Rename is skipped.
        key("Escape")
        focused("Actions")
        click("Details")
        focused("Details")
        click("Details")
        focused("Dismiss details")
        key("Escape")
        focused("Details")
    elif group == "settings":
        click("Connection")
        wait(lambda: "Disconnected" in named("Connection").get_description(), "tile state from owner")
        click("Quiet mode")
        switch = named("Quiet mode")
        assert switch.get_role() in (Atspi.Role.TOGGLE_BUTTON,Atspi.Role.CHECK_BOX,getattr(Atspi.Role,"SWITCH",Atspi.Role.TOGGLE_BUTTON))
        assert named("Updates").get_n_actions() == 0
        slider = named("Level")
        assert slider.get_role() == Atspi.Role.SLIDER
        assert (slider.get_minimum_value(),slider.get_maximum_value(),slider.get_minimum_increment()) == (0,100,5)
        assert slider.set_current_value(64)
        wait(lambda: named("Level").get_current_value() == 65, "snapped AT-SPI value")
        assert slider.grab_focus()
        key("Right")
        wait(lambda: named("Level").get_current_value() == 70, "keyboard step")
        click("Reset preferences")
        focused("Cancel")
        key("Escape")
        focused("Reset preferences")
        click("Reset preferences")
        key("Tab")
        focused("Reset")
        key("Return")
        focused("Reset preferences")
    else:
        raise AssertionError(f"missing contract for {group}")
    print(f"Passed {group} control contract")
