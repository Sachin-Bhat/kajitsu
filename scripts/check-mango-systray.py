#!/usr/bin/env python3
"""Verify controlled native tray calls in an isolated two-output Mango session."""

import argparse
import hashlib
import importlib.util
import json
import os
import re
import select
import shlex
import subprocess
import tempfile
import time
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
panels = REPO / "scripts/check-mango-panels.py"
spec = importlib.util.spec_from_file_location("panels", panels)
if spec is None or spec.loader is None:
    raise RuntimeError(f"Cannot load {panels}")
helper = importlib.util.module_from_spec(spec)
spec.loader.exec_module(helper)


def build_keyboard(root):
    cargo = Path(os.environ.get("CARGO_HOME", str(Path.home() / ".cargo")))
    protocols = list(cargo.glob("registry/src/*/wayland-protocols-misc-*/protocols/virtual-keyboard-unstable-v1.xml"))
    if not protocols:
        raise RuntimeError("Build Kajitsu first to cache the virtual keyboard protocol")
    for mode, filename in [("client-header", "keyboard-protocol.h"), ("private-code", "keyboard-protocol.c")]:
        subprocess.run(["wayland-scanner", mode, str(max(protocols)), str(root / filename)], check=True)
    flags = shlex.split(
        subprocess.check_output(["pkg-config", "--cflags", "--libs", "wayland-client", "xkbcommon"], text=True)
    )
    binary = root / "mango-keyboard"
    subprocess.run(
        [
            "cc",
            "-I",
            str(root),
            str(REPO / "scripts/mango-keyboard.c"),
            str(root / "keyboard-protocol.c"),
            "-o",
            str(binary),
            *flags,
        ],
        check=True,
    )
    return binary


def command(device, text, expected):
    device.stdin.write(text + "\n")
    device.stdin.flush()
    if not select.select([device.stdout], [], [], 5)[0] or device.stdout.readline().strip() != expected:
        raise RuntimeError(f"Input was not acknowledged: {text}")


def build_outputs(root):
    cargo = Path(os.environ.get("CARGO_HOME", str(Path.home() / ".cargo")))
    protocols = list(
        cargo.glob(
            "registry/src/*/wayland-protocols-wlr-*/wlr-protocols/unstable/wlr-output-management-unstable-v1.xml"
        )
    )
    if not protocols:
        raise RuntimeError("Missing cached output-management protocol")
    for mode, filename in [("client-header", "outputs-protocol.h"), ("private-code", "outputs-protocol.c")]:
        subprocess.run(["wayland-scanner", mode, str(max(protocols)), str(root / filename)], check=True)
    flags = shlex.split(subprocess.check_output(["pkg-config", "--cflags", "--libs", "wayland-client"], text=True))
    binary = root / "mango-outputs"
    subprocess.run(
        [
            "cc",
            "-I",
            str(root),
            str(REPO / "scripts/mango-outputs.c"),
            str(root / "outputs-protocol.c"),
            "-o",
            str(binary),
            *flags,
        ],
        check=True,
    )
    return binary


def point(device, x, y, action="left", amount=0):
    command(device, f"{int(x)} {int(y)} {action} {amount}", "clicked")


def key(device, name):
    command(device, name, "keyed")
    time.sleep(0.08)


def logs(path):
    if not path.exists():
        return []
    result = []
    for line in path.read_text().splitlines():
        try:
            result.append(json.loads(line))
        except json.JSONDecodeError:
            pass
    return result


def call_count(path, method, item=None):
    return sum(row["method"] == method and (item is None or row["item"] == item) for row in logs(path))


def image(env, output):
    data = subprocess.check_output(["grim", "-t", "ppm", "-o", output, "-"], env=env, timeout=5)
    header = re.match(rb"P6\s+(\d+)\s+(\d+)\s+255\s", data)
    if not header:
        raise RuntimeError("Invalid screenshot")
    width, height = map(int, header.groups())
    return width, height, data[header.end() :]


def region(env, monitor, rect):
    width, height, data = image(env, monitor["name"])
    scale = width / monitor["width"]
    x, y, w, h = [int(value * scale) for value in rect]
    x, y = max(0, x), max(0, y)
    w, h = min(w, width - x), min(h, height - y)
    return hashlib.sha256(
        b"".join(data[(row * width + x) * 3 : (row * width + x + w) * 3] for row in range(y, y + h))
    ).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=REPO / "target/release/kajitsu")
    parser.add_argument("--fixture", type=Path, default=REPO / "target/release/examples/systray-fixture")
    args = parser.parse_args()
    binary, fixture_binary = args.binary.resolve(), args.fixture.resolve()
    if not binary.is_file() or not fixture_binary.is_file():
        parser.error("Build Kajitsu and the fixture first")
    root = Path(tempfile.mkdtemp(prefix="kajitsu-systray-check-"))
    print(f"Evidence: {root}", flush=True)
    for name in ["runtime", "home", "config", "state", "cache"]:
        (root / name).mkdir()
    (root / "runtime").chmod(0o700)
    (root / "state/kajitsu").mkdir()
    settings = root / "state/kajitsu/settings"
    defaults = {
        "bar_height": "40",
        "bar_battery": "false",
        "bar_memory": "false",
        "bar_clock": "false",
        "bar_systray": "true",
        "bar_tray": "true",
        "bar_auto_hide": "false",
        "bar_position": "top",
        "bar_layout": "true",
        "wallpaper_shuffle": "false",
    }
    pointer_binary = helper.build_pointer(root, REPO)
    keyboard_binary = build_keyboard(root)
    outputs_binary = build_outputs(root)
    config = root / "mango.conf"
    config.write_text(
        "monitorrule=name:HEADLESS-1,width:0,height:0,scale:1\nmonitorrule=name:HEADLESS-2,width:0,height:0,scale:1.25\nanimations=0\n"
    )
    env = dict(
        os.environ,
        HOME=str(root / "home"),
        XDG_RUNTIME_DIR=str(root / "runtime"),
        XDG_CONFIG_HOME=str(root / "config"),
        XDG_STATE_HOME=str(root / "state"),
        XDG_CACHE_HOME=str(root / "cache"),
        KAJITSU_CONFIG_DIR=str(REPO),
        WLR_BACKENDS="headless",
        WLR_HEADLESS_OUTPUTS="2",
        WLR_LIBINPUT_NO_DEVICES="1",
        WLR_RENDERER="pixman",
    )
    for name in ["WAYLAND_DISPLAY", "MANGO_INSTANCE_SIGNATURE", "DISPLAY", "DBUS_SESSION_BUS_ADDRESS"]:
        env.pop(name, None)
    address = "unix:path=" + str(root / "bus")
    env["DBUS_SESSION_BUS_ADDRESS"] = address
    processes = []
    files = []

    def spawn(argv, log, stdin=False, stdout=False):
        handle = (root / log).open("ab")
        files.append(handle)
        p = subprocess.Popen(
            argv,
            env=env,
            stdin=subprocess.PIPE if stdin else subprocess.DEVNULL,
            stdout=subprocess.PIPE if stdout else handle,
            stderr=handle,
            text=stdin or stdout,
            start_new_session=True,
        )
        processes.append(p)
        return p

    def bus_start():
        p = spawn(
            ["dbus-daemon", "--session", "--nofork", "--address=" + address, "--print-address=1"],
            "bus.log",
            stdout=True,
        )
        stdout = p.stdout
        assert stdout is not None
        assert select.select([stdout], [], [], 5)[0]
        assert stdout.readline().strip().startswith("unix:")
        return p

    def request(*args):
        return json.loads(subprocess.check_output(["mmsg", *args], env=env, timeout=5))

    def layer(name, output=None):
        return any(
            item["name"] == name and (output is None or item["monitor"] == output)
            for item in request("get", "all-layers")["layers"]
        )

    def ids():
        try:
            data = subprocess.check_output(
                [
                    "busctl",
                    "--json=short",
                    "--address=" + address,
                    "get-property",
                    "org.freedesktop.StatusNotifierWatcher",
                    "/StatusNotifierWatcher",
                    "org.freedesktop.StatusNotifierWatcher",
                    "RegisteredStatusNotifierItems",
                ],
                env=env,
                stderr=subprocess.DEVNULL,
                timeout=4,
            )
            return json.loads(data)["data"]
        except (subprocess.CalledProcessError, json.JSONDecodeError):
            return []

    shell = None

    def start_shell(**changes):
        nonlocal shell
        helper.stop(shell)
        defaults.update({key: str(value) for key, value in changes.items()})
        settings.write_text("".join(f"{key}={value}\n" for key, value in defaults.items()))
        shell = spawn([str(binary)], "shell.log")
        helper.wait_for(lambda: (root / "runtime/amane.sock").exists(), shell)
        time.sleep(3.0)
        return shell

    def items_start(name):
        log = root / f"{name}.jsonl"
        p = spawn([str(fixture_binary), "--mode", "items", "--log", str(log)], f"{name}.stderr", stdin=True)
        helper.wait_for(lambda: call_count(log, "Ready"), p)
        helper.wait_for(lambda: len(ids()) == 4, shell)
        time.sleep(0.6)
        return p, log

    def control(p, **values):
        p.stdin.write(json.dumps(values) + "\n")
        p.stdin.flush()
        time.sleep(0.25)

    def expect_call(log, method, device, x, y, action="left", item=None, amount=0):
        count = call_count(log, method, item)
        point(device, x, y, action, amount)
        helper.wait_for(lambda: call_count(log, method, item) > count, shell)
        return next(
            row for row in reversed(logs(log)) if row["method"] == method and (item is None or row["item"] == item)
        )

    try:
        bus = bus_start()
        mango = spawn(["mango", "-c", str(config)], "mango.log")
        sockets = helper.wait_for(lambda: list((root / "runtime").glob("mango-*.sock")), mango)
        displays = helper.wait_for(
            lambda: [p for p in (root / "runtime").glob("wayland-*") if not p.name.endswith(".lock")], mango
        )
        env["MANGO_INSTANCE_SIGNATURE"], env["WAYLAND_DISPLAY"] = str(sockets[0]), displays[0].name
        watcher_log = root / "watcher.jsonl"
        watcher = spawn(
            [str(fixture_binary), "--mode", "watcher", "--log", str(watcher_log)], "watcher.stderr", stdin=True
        )
        helper.wait_for(lambda: call_count(watcher_log, "Ready"), watcher)
        start_shell()
        helper.wait_for(lambda: call_count(watcher_log, "Host"), shell)
        fixture, fixture_log = items_start("items")
        monitors = request("get", "all-monitors")["monitors"]
        assert len(monitors) == 2 and {m["scale"] for m in monitors} == {1.0, 1.25}
        keyboard = spawn([str(keyboard_binary)], "keyboard.log", stdin=True, stdout=True)
        pointers = {
            m["name"]: spawn(
                [str(pointer_binary), m["name"], str(m["width"]), str(m["height"])],
                f"pointer-{m['name']}.log",
                stdin=True,
                stdout=True,
            )
            for m in monitors
        }
        for monitor in monitors:
            output, width = monitor["name"], monitor["width"]
            device = pointers[output]
            other = next(m for m in monitors if m["name"] != output)
            slot = [width - 206 + 30 * i for i in range(4)]
            call = expect_call(fixture_log, "Activate", device, slot[0], 20, item="named")
            assert call["args"] == [monitor["x"] + slot[0], monitor["y"] + 20], call
            expect_call(fixture_log, "SecondaryActivate", device, slot[1], 20, "middle", "raw")
            expect_call(fixture_log, "Scroll", device, slot[1], 20, "scroll", "raw", 15)
            expect_call(fixture_log, "Scroll", device, slot[1], 20, "scroll-x", "raw", -15)
            assert {
                row["args"][1] for row in logs(fixture_log) if row["method"] == "Scroll" and row["item"] == "raw"
            } == {"horizontal", "vertical"}
            expect_call(fixture_log, "ContextMenu", device, slot[1], 20, "right", "raw")
            crop = (width - 435, 44, 427, 245)
            other_crop = (other["width"] - 435, 44, 427, 245)
            before, other_before = region(env, monitor, crop), region(env, other, other_crop)
            for m in [monitor, other]:
                subprocess.run(
                    ["grim", "-o", m["name"], str(root / f"before-{output}-{m['name']}.png")],
                    env=env,
                    check=True,
                    timeout=5,
                )
            expect_call(fixture_log, "GetLayout", device, slot[0], 20, "right", "named")
            helper.wait_for(
                lambda monitor=monitor, crop=crop, before=before: region(env, monitor, crop) != before,
                shell,
            )
            helper.wait_for(lambda output=output: layer("kajitsu-tray-menu", output), shell)
            for m in [monitor, other]:
                subprocess.run(
                    ["grim", "-o", m["name"], str(root / f"after-{output}-{m['name']}.png")],
                    env=env,
                    check=True,
                    timeout=5,
                )
            assert not layer("kajitsu-tray-menu", other["name"]), "Popup appeared on the other output"
            assert region(env, other, other_crop) == other_before
            physical_width, _, pixels = image(env, output)
            scale = physical_width / width
            marker_x, marker_y = int((width - 428 + 12) * scale), int(146 * scale)
            marker_width = int(18 * scale)
            bright = sum(
                all(
                    channel > 170 for channel in pixels[(y * physical_width + x) * 3 : (y * physical_width + x) * 3 + 3]
                )
                for y in range(marker_y, marker_y + marker_width)
                for x in range(marker_x, marker_x + marker_width)
            )
            assert bright > 2, "Checked state disappeared when the menu row also exported an icon"
            event_count = call_count(fixture_log, "Event")
            expect_call(fixture_log, "GetLayout", device, slot[2], 20, item="menu")
            assert [r for r in logs(fixture_log) if r["method"] == "GetLayout"][-1]["item"] == "menu"
            expect_call(fixture_log, "GetLayout", device, slot[0], 20, "right", "named")
            point(device, width - 300, 95)
            time.sleep(0.2)
            assert call_count(fixture_log, "Event") == event_count, "Disabled row sent an event"
            call = expect_call(fixture_log, "Event", device, width - 300, 155, item="named")
            assert call["args"][0] == 4, call
            helper.wait_for(
                lambda monitor=monitor, crop=crop, before=before: region(env, monitor, crop) == before,
                shell,
            )
            expect_call(fixture_log, "GetLayout", device, slot[0], 20, "right", "named")
            point(device, 30, 350, "move")
            key(keyboard, "home")
            key(keyboard, "down")
            key(keyboard, "enter")
            helper.wait_for(
                lambda event_count=event_count: call_count(fixture_log, "Event") > event_count + 1,
                shell,
            )
            assert [r for r in logs(fixture_log) if r["method"] == "Event"][-1]["args"][0] == 4
            expect_call(fixture_log, "GetLayout", device, slot[0], 20, "right", "named")
            point(device, 30, 350, "move")
            key(keyboard, "home")
            for _ in range(3):
                key(keyboard, "down")
            key(keyboard, "right")
            helper.wait_for(
                lambda: any(r["method"] == "GetLayout" and r["args"] == [6] for r in logs(fixture_log)), shell
            )
            key(keyboard, "enter")
            helper.wait_for(
                lambda: any(r["method"] == "Event" and r["args"][0] == 60 for r in logs(fixture_log)), shell
            )
            expect_call(fixture_log, "GetLayout", device, slot[2], 20, item="menu")
            key(keyboard, "escape")
            helper.wait_for(
                lambda monitor=monitor, crop=crop, before=before: region(env, monitor, crop) == before,
                shell,
            )
            expect_call(fixture_log, "GetLayout", device, slot[0], 20, "right", "named")
            point(device, 30, 350)
            helper.wait_for(
                lambda monitor=monitor, crop=crop, before=before: region(env, monitor, crop) == before,
                shell,
            )
            point(device, slot[1], 20, "move")
            time.sleep(0.15)
            assert region(env, monitor, crop) == before, "Tooltip showed before its delay"
            helper.wait_for(
                lambda monitor=monitor, crop=crop, before=before: region(env, monitor, crop) != before,
                shell,
            )
            point(device, 30, 350, "move")
            helper.wait_for(
                lambda monitor=monitor, crop=crop, before=before: region(env, monitor, crop) == before,
                shell,
            )
            subprocess.run(["grim", "-o", output, str(root / f"bar-{output}.png")], env=env, check=True, timeout=5)
            print(
                f"Passed: {output} scale {monitor['scale']}: "
                "captured coordinates, clicks, both scroll axes, "
                "menus, submenu/keyboard, disabled rows, tooltip and dismissal",
                flush=True,
            )

        monitor = monitors[0]
        device = pointers[monitor["name"]]
        width = monitor["width"]
        bar_crop = (width - 240, 0, 150, 40)
        bar_before = region(env, monitor, bar_crop)
        control(fixture, op="icon", id="raw", generation=1)
        helper.wait_for(lambda: region(env, monitor, bar_crop) != bar_before, shell)
        control(fixture, op="icon", id="raw", generation=999)
        expect_call(fixture_log, "Activate", device, width - 176, 20, item="raw")
        control(fixture, op="status", id="raw", value="Passive")
        time.sleep(0.4)
        expect_call(fixture_log, "Activate", device, width - 176, 20, item="named")
        control(fixture, op="status", id="raw", value="Active")
        time.sleep(0.4)
        expect_call(fixture_log, "GetLayout", device, width - 206, 20, "right", "named")
        point(device, 30, 350, "move")
        control(fixture, op="menu_update", id="named")
        key(keyboard, "home")
        key(keyboard, "enter")
        helper.wait_for(lambda: any(r["method"] == "Event" and r["args"][0] == 8 for r in logs(fixture_log)), shell)
        expect_call(fixture_log, "GetLayout", device, width - 206, 20, "right", "named")
        control(fixture, op="menu_bad", id="named")
        key(keyboard, "escape")
        control(fixture, op="long_menu", id="menu")
        expect_call(fixture_log, "GetLayout", device, width - 146, 20, item="menu")
        helper.wait_for(lambda: layer("kajitsu-tray-menu"), shell)
        point(device, 30, 350, "move")
        key(keyboard, "end")
        key(keyboard, "enter")
        helper.wait_for(
            lambda: any(
                r["method"] == "Event" and r["item"] == "menu" and r["args"][0] == 140 for r in logs(fixture_log)
            ),
            shell,
        )
        helper.wait_for(lambda: not layer("kajitsu-tray-menu"), shell)
        expect_call(fixture_log, "GetLayout", device, width - 146, 20, item="menu")
        helper.wait_for(lambda: layer("kajitsu-tray-menu"), shell)
        point(device, width - 300, 65, "scroll", 120)
        time.sleep(0.2)
        call = expect_call(fixture_log, "Event", device, width - 300, 65, item="menu")
        assert call["args"][0] == 102, call
        print("Passed: long menus scroll by pointer and reveal the final keyboard selection", flush=True)
        print(
            "Passed: named/pixel icon updates, malformed pixels, passive/active ordering and live menu updates",
            flush=True,
        )

        helper.stop(fixture)
        helper.wait_for(lambda: not ids(), shell)
        fixture, fixture_log = items_start("restart")
        helper.stop(watcher)
        time.sleep(1.4)
        helper.stop(fixture)
        fixture, fixture_log = items_start("owned")
        print(
            "Passed: external watcher hosting, duplicate registrations, owner restart and watcher replacement",
            flush=True,
        )

        control(fixture, op="stall", id="raw", milliseconds=4000)
        expect_call(fixture_log, "Activate", device, width - 176, 20, item="raw")
        point(device, width - 206, 20, "right")
        helper.wait_for(lambda: layer("kajitsu-tray-menu"), shell)
        started = time.monotonic()
        key(keyboard, "escape")
        helper.wait_for(lambda: not layer("kajitsu-tray-menu"), shell)
        assert time.monotonic() - started < 1.0, "Stalled app blocked the drawing/input thread"
        helper.stop(fixture)
        helper.wait_for(lambda: not ids(), shell)
        fixture, fixture_log = items_start("after-stall")
        time.sleep(3.2)
        assert call_count(fixture_log, "Event") == call_count(fixture_log, "GetLayout") == 0, (
            "Old jobs reached the replacement owner"
        )
        print("Passed: input stays responsive during a method stall; stale jobs skip the replacement owner", flush=True)

        for index in range(24):
            control(fixture, op="add", id=f"extra{index}")
        helper.wait_for(lambda: len(ids()) == 28, shell)
        time.sleep(0.6)
        # Reserve the last complete slot for overflow within the remaining right third.
        inline_slots = min(28, int((width / 3 - 95 - 16 + 4) // 30))
        overflow_x = width - 116
        point(device, overflow_x, 20)
        helper.wait_for(lambda: layer("kajitsu-tray-menu"), shell)
        point(device, width - 130, 65, "left")
        helper.wait_for(lambda: call_count(fixture_log, "Activate") > 0, shell)
        helper.wait_for(lambda: not layer("kajitsu-tray-menu"), shell)
        point(device, overflow_x, 20)
        helper.wait_for(lambda: layer("kajitsu-tray-menu"), shell)
        point(device, 30, 350, "move")
        key(keyboard, "end")
        key(keyboard, "enter")
        helper.wait_for(lambda: call_count(fixture_log, "Activate", "extra23") > 0, shell)
        assert inline_slots > 0
        print("Passed: bounded overflow list and keyboard activation of its last item", flush=True)

        helper.stop(fixture)
        start_shell(bar_systray="false")
        fixture, fixture_log = items_start("hidden")
        point(device, width - 206, 20)
        time.sleep(0.3)
        assert call_count(fixture_log, "Activate") == 0
        start_shell(bar_systray="true", bar_position="bottom", bar_auto_hide="true")
        # Existing fixture re-registers after the shell is replaced.
        helper.stop(fixture)
        fixture, fixture_log = items_start("bottom")
        for monitor in monitors:
            device = pointers[monitor["name"]]
            width, height = monitor["width"], monitor["height"]
            point(device, width - 206, height - 1, "move")
            time.sleep(0.5)
            expect_call(fixture_log, "GetLayout", device, width - 206, height - 20, "right", "named")
            point(device, 30, height / 2, "move")
            time.sleep(0.6)
            expect_call(fixture_log, "Event", device, width - 300, height - 40 - 4 - 222 + 6 + 15, item="named")
            print(f"Passed: {monitor['name']}: bottom placement and auto-hide hold", flush=True)
        helper.stop(fixture)
        helper.stop(bus)
        time.sleep(0.4)
        bus = bus_start()
        time.sleep(5.5)
        fixture, fixture_log = items_start("reconnected")
        monitor = monitors[1]
        device = pointers[monitor["name"]]
        point(device, monitor["width"] - 206, monitor["height"] - 1, "move")
        time.sleep(0.5)
        expect_call(fixture_log, "GetLayout", device, monitor["width"] - 206, monitor["height"] - 20, "right", "named")
        helper.wait_for(lambda: layer("kajitsu-tray-menu", monitor["name"]), shell)
        subprocess.run([str(outputs_binary), monitor["name"], "off"], env=env, check=True, timeout=5)
        helper.wait_for(
            lambda: any(
                m["name"] == monitor["name"] and m["width"] == 0 and m["height"] == 0
                for m in request("get", "all-monitors")["monitors"]
            ),
            shell,
        )
        time.sleep(1.3)
        subprocess.run(
            [str(outputs_binary), monitor["name"], "on", str(monitor["scale"])], env=env, check=True, timeout=5
        )
        helper.wait_for(lambda: sum(m["width"] > 0 for m in request("get", "all-monitors")["monitors"]) == 2, shell)
        time.sleep(0.4)
        assert not layer("kajitsu-tray-menu"), "Removed-output popup returned after re-enabling"
        print("Passed: output removal closes its popup and re-enabling keeps it closed", flush=True)
        assert shell is not None and shell.poll() is None
        assert (root / "shell.log").stat().st_size < 1024 * 1024
        print("Passed: session-bus reconnection and bounded healthy shell log", flush=True)
    except Exception:
        print((root / "shell.log").read_text(errors="replace")[-4000:], flush=True)
        raise
    finally:
        for process in reversed(processes):
            helper.stop(process)
        for handle in files:
            handle.close()


if __name__ == "__main__":
    main()
