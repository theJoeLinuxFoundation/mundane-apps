"""Run inside the joe image. Only session and journal probes need the root console."""
import json
import os
from pathlib import Path
import pwd
import re
import shutil
import signal
import subprocess
import sys
import time


def output(*args):
    return subprocess.check_output(args, text=True).strip()


def session():
    joe = pwd.getpwnam("joe")
    for line in output("loginctl", "list-sessions", "--no-legend").splitlines():
        sid = line.split()[0]
        properties = dict(row.split("=", 1) for row in output(
            "loginctl", "show-session", sid, "-p", "Name", "-p", "Type", "-p", "Active"
        ).splitlines())
        if properties == {"Name": "joe", "Type": "wayland", "Active": "yes"}:
            break
    else:
        raise RuntimeError("Waiting for joe's active Wayland session")
    shell = output("pgrep", "-u", str(joe.pw_uid), "-x", "gnome-shell")
    runtime = Path(f"/run/user/{joe.pw_uid}")
    # Use the session manager's exported compositor socket; Shell itself may
    # have started before WAYLAND_DISPLAY was set.
    os.initgroups("joe", joe.pw_gid)
    os.setgid(joe.pw_gid)
    os.setuid(joe.pw_uid)
    os.chdir(joe.pw_dir)
    os.environ.clear()
    os.environ.update(HOME=joe.pw_dir, USER="joe", LOGNAME="joe", PATH="/usr/bin:/bin",
                      SHELL=joe.pw_shell, XDG_RUNTIME_DIR=str(runtime), LANG="C.UTF-8",
                      DBUS_SESSION_BUS_ADDRESS=f"unix:path={runtime}/bus",
                      PYTHONDONTWRITEBYTECODE="1")
    manager = dict(row.split("=", 1) for row in output(
        "systemctl", "--user", "show-environment").splitlines() if "=" in row)
    for key in ("WAYLAND_DISPLAY", "XDG_CURRENT_DESKTOP", "XDG_SESSION_DESKTOP",
                "XDG_DATA_DIRS", "XDG_CONFIG_DIRS"):
        if key in manager:
            os.environ[key] = manager[key]
    os.environ.setdefault("WAYLAND_DISPLAY", "wayland-0")
    os.environ.setdefault("XDG_CURRENT_DESKTOP", "GNOME")
    if ("GNOME" not in os.environ["XDG_CURRENT_DESKTOP"].upper().split(":")
            or not (runtime / os.environ["WAYLAND_DISPLAY"]).is_socket()):
        raise RuntimeError("GNOME Wayland compositor is unavailable")
    os.environ["XDG_SESSION_TYPE"] = "wayland"
    return {"session": sid, "uid": os.getuid(), "gnome_shell_pid": shell,
            "wayland": os.environ["WAYLAND_DISPLAY"]}


def launch(case, directory):
    from gi.repository import Gio
    app = Gio.DesktopAppInfo.new(case["desktop"])
    if not app or Path(app.get_executable()).name != case["binary"]:
        raise RuntimeError(f'Missing or incorrect installed entry {case["desktop"]}')
    exe = shutil.which(app.get_executable())
    if not exe:
        raise RuntimeError(f'Installed executable missing: {case["binary"]}')
    info = {"entry": app.get_filename(), "executable": exe}
    if Path(info["entry"]) != Path("/usr/share/applications", case["desktop"]) or exe != "/usr/bin/" + case["binary"]:
        raise RuntimeError(f"Unexpected installed entry or binary: {info}")
    import gi
    gi.require_version("Gtk", "4.0")
    from gi.repository import Gdk, Gtk
    Gtk.init()
    toolkit = Gtk.IconTheme.get_for_display(Gdk.Display.get_default())
    # GNOME Shell uses the desktop preference. A GTK client started from the
    # serial probe can independently fall back to Adwaita.
    theme = Gtk.IconTheme.new()
    theme.set_search_path(toolkit.get_search_path())
    desktop_theme = Gio.Settings.new("org.gnome.desktop.interface").get_string("icon-theme")
    theme.set_theme_name(desktop_theme)
    icon = app.get_icon()
    info["icon"] = icon.to_string() if icon else None
    info["icon_found"] = bool(icon and theme.has_gicon(icon))
    info["icon_theme"] = desktop_theme
    info["gtk_theme"] = toolkit.get_theme_name()
    info["gtk_icon_found"] = bool(icon and toolkit.has_gicon(icon))
    if case["app"] == "clocks":
        autostart = Path("/etc/xdg/autostart/org.joelinux.mundane.Clocks.Alarms.desktop")
        info["alarm_autostart"] = autostart.is_file()
    if not info["icon_found"]:
        alternatives = {
            "document-viewer": ["application-pdf", "x-office-document"],
            "image-viewer": ["image-x-generic"],
            "music": ["audio-x-generic"],
            "system-monitor": ["applications-system", "utilities-system-monitor-symbolic"],
        }
        info["available_icons"] = [i for i in alternatives.get(case["app"], []) if theme.has_icon(i)]
    args = ["gio", "launch", app.get_filename()]
    if case["mode"] != "desktop":
        path = directory / case["file"]
        if case["mode"] == "mime":
            file = Gio.File.new_for_path(str(path))
            mime = file.query_info("standard::content-type", 0, None).get_content_type()
            default = Gio.AppInfo.get_default_for_type(mime, False)
            actual = default.get_id() if default else None
            registered = {a.get_id() for a in Gio.AppInfo.get_all_for_type(mime)}
            declares = any(Gio.content_type_is_a(mime, kind) for kind in app.get_supported_types() or [])
            info.update(mime=mime, default=actual, registered=case["desktop"] in registered,
                        declares=declares)
            if actual == case["desktop"]:
                args = ["gio", "open"]
                info["route"] = "default"
            elif info["registered"] and declares:
                # Open With uses the user's selected GAppInfo and file. Leave
                # the packaged GNOME session's default association untouched.
                info["route"] = "open-with"
            else:
                info["route"] = "entry-only"
        args.append(str(path))
    subprocess.run(["systemd-cat", "-t", "joe-smoke-" + case["app"], *args], check=True)
    return info


def package_runtime():
    # Exercise the same unprivileged JSON contract as the installed app.
    result = {"polkit_action_installed": Path("/usr/share/polkit-1/actions/io.joelinux.dave.policy").is_file()}
    for args in (["list", "--json"], ["info", "dave"], ["info", "mundane-apps"]):
        response = subprocess.run(["dave", *args], capture_output=True, text=True, timeout=15)
        entry = {"exit": response.returncode, "stderr": response.stderr[:800]}
        if args[0] == "list":
            try:
                data = json.loads(response.stdout)
                entry.update(schema=data.get("schema"), count=len(data.get("packages", [])))
                if data.get("message"):
                    entry["message"] = data["message"]
            except (ValueError, AttributeError):
                entry["stdout"] = response.stdout[:800]
        else:
            entry["stdout"] = response.stdout[:1200]
        result["dave " + " ".join(args)] = entry
    return result


def window(case):
    import gi
    gi.require_version("Atspi", "2.0")
    from gi.repository import Atspi, GLib
    def children(node):
        for i in range(node.get_child_count()):
            child = node.get_child_at_index(i)
            if child:
                yield child

    def contents(node):
        yield node
        for child in children(node):
            yield from contents(child)

    desktop = Atspi.get_desktop(0)
    expected = Path(shutil.which(case["binary"])).resolve()
    last = "waiting for a focused frame"
    deadline = time.monotonic() + 35
    while time.monotonic() < deadline:
        while GLib.MainContext.default().pending():
            GLib.MainContext.default().iteration(False)
        try:
            for app in children(desktop):
                pid = app.get_process_id()
                if Path(f"/proc/{pid}/exe").resolve() != expected:
                    continue
                for frame in children(app):
                    frame.clear_cache()
                    if not frame.get_state_set().contains(Atspi.StateType.ACTIVE):
                        continue
                    names = []
                    evidence = case.get("evidence")
                    found = not evidence
                    playback = False
                    sized_image = False
                    for node in contents(frame):
                        label = node.get_name() or ""
                        if label:
                            names.append(label)
                        if evidence and evidence in label:
                            found = True
                        if evidence and case["app"] in ("text-editor", "document-viewer"):
                            if "Text" in node.get_interfaces():
                                found |= evidence in (Atspi.Text.get_text(node, 0, -1) or "")
                        if label == "Playback status":
                            desc = node.get_description() or ""
                            playback = any(state in desc for state in
                                           ("Playing", "Paused", "Ended", "Queue finished"))
                        if evidence and label == evidence and case["app"] == "image-viewer":
                            sized_image |= "480 × 300" in (node.get_description() or "")
                    last = str(names[:40])
                    if evidence and case["app"] in ("music", "video"):
                        found &= playback
                    if evidence and case["app"] == "image-viewer":
                        found &= sized_image
                    if case["app"] == "packages" and "Loading…" in names:
                        continue
                    if found and len(names) >= 3:
                        return {"pid": pid, "title": frame.get_name(), "controls": names[:40],
                                "content": evidence}
        except (FileNotFoundError, ProcessLookupError, PermissionError, GLib.Error):
            pass  # A transient system window or a rebuilding app tree.
        time.sleep(.2)
    raise RuntimeError(f'{case["app"]}: {last}')


def app_pids(binary):
    executable = shutil.which(binary)
    if not executable:
        return []
    expected = Path(executable).resolve()
    result = []
    for path in Path("/proc").glob("[0-9]*"):
        try:
            if path.stat().st_uid == os.getuid() and (path / "exe").resolve() == expected:
                if b"--watch-alarms" not in (path / "cmdline").read_bytes().split(b"\0"):
                    result.append(int(path.name))
        except (PermissionError, FileNotFoundError, ProcessLookupError):
            continue
    return result


def cleanup(binary, previous):
    # Only failed cases use this path. Never signal a pre-existing login app.
    for pid in set(app_pids(binary)) - set(previous):
        try:
            fd = os.pidfd_open(pid)
        except ProcessLookupError:
            continue
        try:
            if pid in app_pids(binary):
                signal.pidfd_send_signal(fd, signal.SIGTERM)
        except ProcessLookupError:
            pass
        finally:
            os.close(fd)
    deadline = time.monotonic() + 5
    while set(app_pids(binary)) - set(previous):
        if time.monotonic() > deadline:
            raise RuntimeError(f"Could not isolate the next case after {binary} failed")
        time.sleep(.1)
    return True


FIELDS = ("MESSAGE", "MESSAGE_ID", "_EXE", "_PID", "_COMM", "SYSLOG_IDENTIFIER",
          "COREDUMP_EXE", "COREDUMP_COMM", "COREDUMP_PID", "_SYSTEMD_USER_UNIT",
          "USER_UNIT", "UNIT", "EXIT_CODE", "EXIT_STATUS")
CRASH = re.compile(r"segfault|general protection fault|core.dump|status=\d+/(?:SEGV|ABRT|BUS|ILL|FPE|KILL)|"
                   r"code=(?:dumped|killed)|panicked at|fatal runtime error|out of memory: killed process|"
                   r"trap (?:invalid opcode|divide error)", re.I)


def journal(cursor, case):
    subprocess.run(["journalctl", "--sync"], check=True)
    data = output("journalctl", "-q", "-b", "--after-cursor=" + cursor, "--no-pager", "-o", "json",
                  "--output-fields=" + ",".join(FIELDS))
    entries, crashes = [], []
    binaries = {case["binary"], case["binary"][:15], case["desktop"].removesuffix(".desktop")}
    for line in data.splitlines():
        row = json.loads(line)
        msg = str(row.get("MESSAGE", ""))
        fields = " ".join(str(row.get(k, "")) for k in FIELDS)
        tagged = row.get("SYSLOG_IDENTIFIER") == "joe-smoke-" + case["app"]
        if not tagged and not any(b in fields for b in binaries):
            continue
        record = {k: row[k] for k in FIELDS if k in row}
        entries.append(record)
        if row.get("COREDUMP_PID") or row.get("EXIT_CODE") in ("killed", "dumped") or CRASH.search(msg):
            crashes.append(record)
    return {"entries": entries[-60:], "crashes": crashes}


if __name__ == "__main__":
    action, *args = sys.argv[1:]
    if action == "cursor":
        result = json.loads(output("journalctl", "-b", "-n1", "-o", "json"))["__CURSOR"]
    elif action == "journal":
        result = journal(args[0], json.loads(args[1]))
    else:
        context = session()
        if action == "session":
            import gi
            gi.require_version("Atspi", "2.0")
            from gi.repository import Atspi, Gio
            output("gsettings", "set", "org.gnome.desktop.session", "idle-delay", "0")
            output("gdbus", "call", "--session", "--dest", "org.a11y.Bus", "--object-path",
                   "/org/a11y/bus", "--method", "org.freedesktop.DBus.Properties.Set",
                   "org.a11y.Status", "IsEnabled", "<true>")
            result = context
        elif action == "launch":
            result = launch(json.loads(args[0]), Path(__file__).parent)
        elif action == "window":
            result = window(json.loads(args[0]))
        elif action == "packages":
            result = package_runtime()
        elif action == "pids":
            result = app_pids(args[0])
        elif action == "cleanup":
            result = cleanup(args[0], json.loads(args[1]))
        elif action == "closed":
            path = Path(f"/proc/{int(args[0])}/exe")
            deadline = time.monotonic() + 12
            while path.resolve() == Path(shutil.which(args[1])).resolve():
                if time.monotonic() >= deadline:
                    raise RuntimeError(f"Window close left PID {args[0]} running")
                time.sleep(.15)
            result = True
        else:
            raise ValueError(action)
    print(json.dumps(result))
