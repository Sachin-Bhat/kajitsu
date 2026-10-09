#!/usr/bin/env python3
"""Check layout labels and pointer controls in an isolated two-output Mango session."""

import argparse
import hashlib
import importlib.util
import json
import os
import re
import select
import subprocess
import tempfile
import time
from pathlib import Path


def request(env, *args):
    result = subprocess.run(["mmsg", *args], env=env, capture_output=True, text=True, check=True, timeout=5)
    return json.loads(result.stdout)


def layouts(env):
    return {monitor["name"]: monitor["layout_index"] for monitor in request(env, "get", "all-monitors")["monitors"]}


def pill_image(env, monitor):
    data = subprocess.check_output(["grim", "-t", "ppm", "-o", monitor["name"], "-"], env=env, timeout=5)
    header = re.match(rb"P6\s+(\d+)\s+(\d+)\s+255\s", data)
    if not header:
        raise RuntimeError("Unexpected screenshot format")
    width, _ = map(int, header.groups())
    scale = width / monitor["width"]
    # Keep the crop inside the icon as the pill now follows its text width.
    start, end = int(70 * scale), int(89 * scale)
    pixels = b"".join(
        data[header.end() + (y * width + start) * 3 : header.end() + (y * width + end) * 3]
        for y in range(int(11 * scale), int(29 * scale))
    )
    return hashlib.sha256(pixels).digest()


def point(device, action, amount=0):
    device.stdin.write(f"110 20 {action} {amount}\n")
    device.stdin.flush()
    if not select.select([device.stdout], [], [], 5)[0] or device.stdout.readline().strip() != "clicked":
        raise RuntimeError("Virtual pointer did not acknowledge the input")


def main():
    repo = Path(__file__).resolve().parent.parent
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=repo / "target/release/kajitsu")
    args = parser.parse_args()
    binary = args.binary.resolve()
    if not binary.is_file():
        parser.error("Build Kajitsu first")

    path = repo / "scripts/check-mango-panels.py"
    spec = importlib.util.spec_from_file_location("panel_check", path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"Cannot load {path}")
    helper = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(helper)

    with tempfile.TemporaryDirectory(prefix="kajitsu-layout-check-") as folder:
        root = Path(folder)
        for name in ["runtime", "home", "config", "state", "cache"]:
            (root / name).mkdir()
        (root / "runtime").chmod(0o700)
        (root / "state/kajitsu").mkdir()
        settings = root / "state/kajitsu/settings"
        settings.write_text("bar_layout=true\n")
        pointer = helper.build_pointer(root, repo)
        config = root / "mango.conf"
        config.write_text(
            "monitorrule=name:HEADLESS-1,width:0,height:0,scale:1\n"
            "monitorrule=name:HEADLESS-2,width:0,height:0,scale:1.25\n"
            "animations=0\n"
            "tagrule=id:1,layout_name:tile\n"
        )
        env = dict(
            os.environ,
            HOME=str(root / "home"),
            XDG_RUNTIME_DIR=str(root / "runtime"),
            XDG_CONFIG_HOME=str(root / "config"),
            XDG_STATE_HOME=str(root / "state"),
            XDG_CACHE_HOME=str(root / "cache"),
            KAJITSU_CONFIG_DIR=str(repo),
            WLR_BACKENDS="headless",
            WLR_HEADLESS_OUTPUTS="2",
            WLR_LIBINPUT_NO_DEVICES="1",
            WLR_RENDERER="pixman",
        )
        for name in ["WAYLAND_DISPLAY", "MANGO_INSTANCE_SIGNATURE", "DISPLAY"]:
            env.pop(name, None)
        shell = None
        with (root / "mango.log").open("wb") as mango_log, (root / "shell.log").open("wb") as shell_log:
            mango = subprocess.Popen(
                ["mango", "-c", str(config)], env=env, stdout=mango_log, stderr=mango_log, start_new_session=True
            )
            try:
                sockets = helper.wait_for(lambda: list((root / "runtime").glob("mango-*.sock")), mango)
                displays = helper.wait_for(
                    lambda: [p for p in (root / "runtime").glob("wayland-*") if not p.name.endswith(".lock")], mango
                )
                env["MANGO_INSTANCE_SIGNATURE"] = str(sockets[0])
                env["WAYLAND_DISPLAY"] = displays[0].name

                def start_shell():
                    process = subprocess.Popen(
                        ["dbus-run-session", "--", str(binary)],
                        env=env,
                        stdout=shell_log,
                        stderr=shell_log,
                        start_new_session=True,
                    )
                    helper.wait_for(lambda: (root / "runtime/amane.sock").exists(), process)
                    time.sleep(2)
                    return process

                shell = start_shell()
                monitors = request(env, "get", "all-monitors")["monitors"]
                assert len(monitors) == 2
                count = len(request(env, "get", "layouts")["layouts"])
                for monitor in monitors:
                    output = monitor["name"]
                    other = next(item["name"] for item in monitors if item["name"] != output)
                    request(env, "dispatch", f"focusmon,^{output}$")
                    request(env, "dispatch", "setlayout,tile")
                    with subprocess.Popen(
                        [str(pointer), output, str(monitor["width"]), str(monitor["height"])],
                        env=env,
                        stdin=subprocess.PIPE,
                        stdout=subprocess.PIPE,
                        text=True,
                    ) as device:
                        point(device, "move")
                        time.sleep(0.3)
                        previous = pill_image(env, monitor)
                        for action, amount, expected in [
                            ("left", 0, 1),
                            ("right", 0, 0),
                            ("scroll", 15, 1),
                            ("scroll", -15, 0),
                        ]:
                            other_layout = layouts(env)[other]
                            point(device, action, amount)
                            helper.wait_for(
                                lambda output=output, expected=expected: layouts(env).get(output) == expected,
                                shell,
                            )
                            helper.wait_for(
                                lambda monitor=monitor, previous=previous: pill_image(env, monitor) != previous,
                                shell,
                            )
                            previous = pill_image(env, monitor)
                            assert layouts(env)[other] == other_layout, "Changed the other output's layout"
                        request(env, "dispatch", "setlayout,grid")
                        helper.wait_for(
                            lambda monitor=monitor, previous=previous: pill_image(env, monitor) != previous,
                            shell,
                        )
                        point(device, "left")
                        helper.wait_for(lambda output=output: layouts(env)[output] == 3, shell)
                        point(device, "right")
                        helper.wait_for(lambda output=output: layouts(env)[output] == 2, shell)
                        request(env, "dispatch", "setlayout,tile")
                        point(device, "right")
                        helper.wait_for(lambda output=output: layouts(env)[output] == count - 1, shell)
                        point(device, "left")
                        helper.wait_for(lambda output=output: layouts(env)[output] == 0, shell)
                    print(
                        f"Passed: {output} at scale {monitor['scale']}: "
                        "clicks, scrolls, external changes, labels, wraparound",
                        flush=True,
                    )

                helper.stop(shell)
                shell = None
                settings.write_text("bar_layout=false\n")
                shell = start_shell()
                monitor = monitors[0]
                request(env, "dispatch", f"focusmon,^{monitor['name']}$")
                request(env, "dispatch", "setlayout,tile")
                with subprocess.Popen(
                    [str(pointer), monitor["name"], str(monitor["width"]), str(monitor["height"])],
                    env=env,
                    stdin=subprocess.PIPE,
                    stdout=subprocess.PIPE,
                    text=True,
                ) as device:
                    point(device, "left")
                    time.sleep(0.3)
                    assert layouts(env)[monitor["name"]] == 0, "Hidden layout switcher still accepted clicks"
                print("Passed: Bar settings can hide the layout switcher", flush=True)
            except Exception:
                print((root / "shell.log").read_bytes()[:4096].decode(errors="replace"))
                raise
            finally:
                helper.stop(shell)
                helper.stop(mango)


if __name__ == "__main__":
    main()
