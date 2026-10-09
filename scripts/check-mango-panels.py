#!/usr/bin/env python3
"""Exercise layer visibility in a disposable Mango compositor."""

import argparse
import json
import os
import re
import select
import shlex
import signal
import subprocess
import tempfile
import time
from pathlib import Path


def build_pointer(root, repo):
    cargo_home = Path(os.environ.get("CARGO_HOME", str(Path.home() / ".cargo")))
    protocols = list(
        cargo_home.glob(
            "registry/src/*/wayland-protocols-wlr-*/wlr-protocols/unstable/wlr-virtual-pointer-unstable-v1.xml"
        )
    )
    if not protocols:
        raise RuntimeError("Build Kajitsu first so Cargo caches the wlr virtual-pointer protocol")
    protocol = max(protocols)
    for mode, filename in [("client-header", "pointer-protocol.h"), ("private-code", "pointer-protocol.c")]:
        subprocess.run(["wayland-scanner", mode, str(protocol), str(root / filename)], check=True)
    flags = subprocess.check_output(["pkg-config", "--cflags", "--libs", "wayland-client"], text=True)
    binary = root / "mango-pointer"
    subprocess.run(
        [
            "cc",
            "-I",
            str(root),
            str(repo / "scripts/mango-pointer.c"),
            str(root / "pointer-protocol.c"),
            "-o",
            str(binary),
            *shlex.split(flags),
        ],
        check=True,
    )
    return binary


def panel_pixel(env, output):
    data = subprocess.check_output(["grim", "-t", "ppm", "-o", output, "-"], env=env, timeout=5)
    header = re.match(rb"P6\s+(\d+)\s+(\d+)\s+255\s", data)
    if not header:
        raise RuntimeError("Unexpected screenshot format")
    width, height = map(int, header.groups())
    # A plain calendar background pixel, away from date text and buttons.
    offset = header.end() + ((height * 3 // 4) * width + width - 30) * 3
    return data[offset : offset + 3]


def click(pointer, x, y):
    pointer.stdin.write(f"{x} {y}\n")
    pointer.stdin.flush()
    if not select.select([pointer.stdout], [], [], 5)[0] or pointer.stdout.readline().strip() != "clicked":
        raise RuntimeError("Virtual pointer did not acknowledge the click")


def wait_for(check, process):
    for _ in range(100):
        if process.poll() is not None:
            raise RuntimeError(f"Test process exited with {process.returncode}")
        result = check()
        if result:
            return result
        time.sleep(0.1)
    raise RuntimeError("Test session did not become ready")


def stop(process):
    if process is not None and process.poll() is None:
        os.killpg(process.pid, signal.SIGTERM)
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            process.wait(timeout=5)


def main():
    repo = Path(__file__).resolve().parent.parent
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=repo / "target/release/kajitsu")
    parser.add_argument("--cycles", type=int, default=12)
    args = parser.parse_args()
    binary = args.binary.resolve()
    if not binary.is_file() or args.cycles < 1:
        parser.error("Build the binary first and use at least one cycle")

    with tempfile.TemporaryDirectory(prefix="kajitsu-panel-check-") as folder:
        root = Path(folder)
        for name in ["runtime", "home", "config", "state", "cache"]:
            (root / name).mkdir()
        os.chmod(root / "runtime", 0o700)
        (root / "state/kajitsu").mkdir()
        (root / "state/kajitsu/settings").write_text("click_outside_dismiss=true\n")
        pointer = build_pointer(root, repo)
        config = root / "mango.conf"
        config.write_text(
            "monitorrule=name:HEADLESS-1,width:0,height:0,scale:1\n"
            "monitorrule=name:HEADLESS-2,width:0,height:0,scale:1.25\n"
            "animations=0\n"
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
        # Cava can read the existing audio server without using it for Wayland.
        host_runtime = os.environ.get("XDG_RUNTIME_DIR")
        if host_runtime:
            env["PIPEWIRE_RUNTIME_DIR"] = host_runtime
            env["PULSE_SERVER"] = f"unix:{host_runtime}/pulse/native"
        shell = None
        with (root / "mango.log").open("wb") as mango_log, (root / "shell.log").open("wb") as shell_log:
            mango = subprocess.Popen(
                ["mango", "-c", str(config)], env=env, stdout=mango_log, stderr=mango_log, start_new_session=True
            )
            try:
                sockets = wait_for(lambda: list((root / "runtime").glob("mango-*.sock")), mango)
                displays = wait_for(
                    lambda: [p for p in (root / "runtime").glob("wayland-*") if not p.name.endswith(".lock")], mango
                )
                env["MANGO_INSTANCE_SIGNATURE"] = str(sockets[0])
                env["WAYLAND_DISPLAY"] = displays[0].name
                shell = subprocess.Popen(
                    ["dbus-run-session", "--", str(binary)],
                    env=env,
                    stdout=shell_log,
                    stderr=shell_log,
                    start_new_session=True,
                )
                wait_for(lambda: (root / "runtime/amane.sock").exists(), shell)
                time.sleep(3)
                monitors = subprocess.run(
                    ["mmsg", "get", "all-monitors"], env=env, capture_output=True, text=True, check=True, timeout=5
                )
                assert len(json.loads(monitors.stdout)["monitors"]) == 2
                for cycle in range(args.cycles):
                    for target in ["launcher", "wallpaper", "utility", "control"]:
                        for action in ["show", "hide"]:
                            result = subprocess.run(
                                [str(binary), "ipc", "call", target, action],
                                env=env,
                                check=True,
                                capture_output=True,
                                text=True,
                                timeout=5,
                            )
                            if result.returncode or result.stdout.strip() != "ok":
                                raise RuntimeError(f"Cycle {cycle}, {target} {action}: {result.stderr.strip()}")
                    if shell.poll() is not None or (root / "shell.log").stat().st_size > 65536:
                        raise RuntimeError("The shell exited or entered an error loop")
                time.sleep(1)
                assert shell.poll() is None
                output = "HEADLESS-1"
                monitor = next(item for item in json.loads(monitors.stdout)["monitors"] if item["name"] == output)
                width, height = monitor["width"], monitor["height"]
                hidden = panel_pixel(env, output)
                # Complete a hide/reopen to exercise retirement, not just IPC acceptance.
                for action in ["show", "hide", "show"]:
                    subprocess.run(
                        [str(binary), "ipc", "call", "utility", action],
                        env=env,
                        capture_output=True,
                        check=True,
                        timeout=5,
                    )
                    time.sleep(1)
                opened = panel_pixel(env, output)
                assert opened != hidden, "Utility panel did not appear"
                # Keep the pointer connected: disconnecting synthesizes leave,
                # which intentionally closes hovered panels before the assertion.
                with subprocess.Popen(
                    [str(pointer), output, str(width), str(height)],
                    env=env,
                    stdin=subprocess.PIPE,
                    stdout=subprocess.PIPE,
                    text=True,
                ) as device:
                    # Blank notification space avoids changing a control or hover color.
                    click(device, width - 50, height // 3)
                    time.sleep(1)
                    assert panel_pixel(env, output) == opened, "Clicking inside a reopened panel dismissed it"
                    subprocess.run(
                        [str(binary), "ipc", "call", "utility", "hide"],
                        env=env,
                        capture_output=True,
                        check=True,
                        timeout=5,
                    )
                    time.sleep(1)
                    click(device, width // 2, height // 2)
                    # Open with the pointer already outside: the next click
                    # exercises dismissal rather than the hover-leave behavior.
                    subprocess.run(
                        [str(binary), "ipc", "call", "utility", "show"],
                        env=env,
                        capture_output=True,
                        check=True,
                        timeout=5,
                    )
                    time.sleep(1)
                    click(device, width // 2, height // 2)
                    time.sleep(1)
                    assert panel_pixel(env, output) == hidden, "Clicking outside did not dismiss the panel"
                print(
                    f"Passed: {args.cycles} rapid show/hide cycles on two outputs, plus inside/outside pointer clicks"
                )
            except Exception:
                print((root / "shell.log").read_bytes()[:4096].decode(errors="replace"))
                raise
            finally:
                stop(shell)
                stop(mango)


if __name__ == "__main__":
    main()
