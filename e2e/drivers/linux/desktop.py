"""Linux e2e driver for a real, logged-in desktop session (roadmap PR3c):
types every scenario into a GTK entry on the VM's own screen, through the
session's own IBus (GNOME) or Fcitx5 (KDE) running the test-mode build, and
keeps a screenshot per step as evidence (never asserted on).

Runs over ssh on the VM, as the logged-in user:

  python3 tools/e2e/linux/desktop.py --framework ibus --platform linux-gnome-ibus \\
      --prefix <prefix> --out <run-dir>

The VM's own install and user data are never touched: the framework is
pointed at the test build in <prefix> and at a private config / data / cache
tree, and put back afterwards (`--restore`, also run at every start, undoes
an interrupted run from the marker file written before anything changes).
Keys come from a uinput keyboard (uinput.py, needs passwordless sudo).
"""

from __future__ import annotations

import argparse
import html
import json
import os
import re
import shlex
import shutil
import subprocess
import sys
import time
from pathlib import Path

import driver
from driver import INPUT_METHOD_NAME, STARTUP_TIMEOUT_S, ScenarioError, Session, wait_until

STATE_DIR = Path.home() / "taigi-e2e" / "desktop"
RESTORE_MARKER = STATE_DIR / "restore.json"
# The IBus component's <exec> reads the running scenario's work dir from
# here, so the component file stays the same for the whole run.
CURRENT_WORK = STATE_DIR / "current-work"
COMPONENT_DIR = STATE_DIR / "ibus-component"
ENGINE_LAUNCHER = STATE_DIR / "ibus-engine.sh"
CACHE_DIR = STATE_DIR / "cache"
IBUS_UNIT = "org.freedesktop.IBus.session.GNOME.service"
# The user-manager variables the IBus swap sets; their prior values go into the marker.
IBUS_SWAP_VARIABLES = ("IBUS_COMPONENT_PATH", "XDG_CACHE_HOME")
SYSTEM_COMPONENT_DIR = Path("/usr/share/ibus/component")
UINPUT_SCRIPT = Path(__file__).with_name("uinput.py")
SESSION_WAIT_S = 90


def parse_assignments(entries: list[str]) -> dict[str, str]:
    """`NAME=value` entries (loginctl properties, an environment) as a dict."""
    return dict(entry.split("=", 1) for entry in entries if "=" in entry)


def user_bus_environment() -> dict[str, str]:
    runtime = f"/run/user/{os.getuid()}"
    return {"XDG_RUNTIME_DIR": runtime, "DBUS_SESSION_BUS_ADDRESS": f"unix:path={runtime}/bus"}


def systemctl(*arguments: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["systemctl", "--user", *arguments], env={**os.environ, **user_bus_environment()},
        capture_output=True, text=True, check=False, timeout=STARTUP_TIMEOUT_S,
    )


def graphical_session() -> tuple[dict[str, str] | None, str]:
    """(the session's environment, "") for this user's active, unlocked
    seat0 session, else (None, why not)."""
    listing = subprocess.run(["loginctl", "list-sessions", "--no-legend"], capture_output=True, text=True, check=False)
    user = os.environ.get("USER", "")
    for line in listing.stdout.splitlines():
        session_id = line.split()[0]
        shown = subprocess.run(
            ["loginctl", "show-session", session_id, "-p", "Name", "-p", "Seat", "-p", "Type", "-p", "Active", "-p", "LockedHint"],
            capture_output=True, text=True, check=False,
        ).stdout
        props = parse_assignments(shown.splitlines())
        if props.get("Name") != user or props.get("Seat") != "seat0" or props.get("Type") not in ("wayland", "x11"):
            continue
        if props.get("Active") != "yes":
            return None, f"{props['Type']} session {session_id} is not active"
        if props.get("LockedHint") == "yes":
            return None, f"{props['Type']} session {session_id} is locked"
        shown_env = systemctl("show-environment").stdout
        environment = parse_assignments(shown_env.splitlines())
        return {**environment, **user_bus_environment()}, ""
    return None, f"no graphical seat0 session for {user}"


def wait_for_graphical_session() -> tuple[dict[str, str] | None, str]:
    """A freshly booted VM logs in automatically; give it a moment."""
    deadline = time.monotonic() + SESSION_WAIT_S
    while True:
        environment, reason = graphical_session()
        if environment is not None or time.monotonic() > deadline:
            return environment, reason
        time.sleep(3)


# --- IBus (GNOME): the session's own daemon, pointed at the test component ---

def engine_launcher(prefix: Path) -> str:
    """The script the test component execs: the engine with the running
    scenario's private XDG dirs. IBUS_ADDRESS is resolved first, with the
    real XDG_CONFIG_HOME: the engine otherwise looks for the bus socket file
    under the private one (taigikeyboard-ibus/src/bus.rs)."""
    return (
        "#!/bin/sh\n"
        f"work=$(cat {shlex.quote(str(CURRENT_WORK))})\n"
        'IBUS_ADDRESS=$(ibus address) XDG_CONFIG_HOME="$work/config" XDG_DATA_HOME="$work/data" \\\n'
        f"    TAIGIKEYBOARD_DATA_DIR={shlex.quote(str(prefix / 'share' / 'taigikeyboard'))} \\\n"
        f"    exec {shlex.quote(str(prefix / 'libexec' / 'ibus-engine-taigikeyboard'))} --ibus\n"
    )


def ibus_component_xml(prefix: Path) -> str:
    """The test build's component, its <exec> pointed at ENGINE_LAUNCHER."""
    xml = (prefix / "share" / "ibus" / "component" / "taigikeyboard.xml").read_text(encoding="utf-8")
    command = html.escape(f"{shlex.quote(str(ENGINE_LAUNCHER))} --ibus")
    # The file's leading comment mentions `<exec>` too; only the element matches.
    return re.sub(r"<exec>[^<]*</exec>", lambda _: f"<exec>{command}</exec>", xml)


def ibus_engine(set_to: str | None = None) -> str:
    """The global engine, after switching to `set_to` when given. The switch
    exits 1 whenever `setxkbmap` finds no X display, even though the engine
    did change, so only the read-back counts."""
    environment = {**os.environ, **user_bus_environment()}
    if set_to:
        subprocess.run(["ibus", "engine", set_to], env=environment, capture_output=True, check=False)
    return subprocess.run(["ibus", "engine"], env=environment, capture_output=True, text=True, check=False).stdout.strip()


def restart_ibus() -> None:
    result = systemctl("restart", IBUS_UNIT)
    if result.returncode != 0:
        raise ScenarioError(f"restarting {IBUS_UNIT} failed: {result.stderr.strip()}")


def swap_ibus(prefix: Path) -> None:
    """Every other system component stays listed, so GNOME's other engines
    keep working; the registry cache goes to a private XDG_CACHE_HOME so the
    user's own cache is never rebuilt or deleted."""
    manager_environment = parse_assignments(systemctl("show-environment").stdout.splitlines())
    write_marker({
        "framework": "ibus",
        "engine": ibus_engine(),
        "environment": {name: manager_environment.get(name) for name in IBUS_SWAP_VARIABLES},
    })
    shutil.rmtree(COMPONENT_DIR, ignore_errors=True)
    shutil.rmtree(CACHE_DIR, ignore_errors=True)
    COMPONENT_DIR.mkdir(parents=True)
    for component in SYSTEM_COMPONENT_DIR.glob("*.xml"):
        if component.name != "taigikeyboard.xml":
            shutil.copyfile(component, COMPONENT_DIR / component.name)
    ENGINE_LAUNCHER.write_text(engine_launcher(prefix), encoding="utf-8")
    ENGINE_LAUNCHER.chmod(0o755)
    (COMPONENT_DIR / "taigikeyboard.xml").write_text(ibus_component_xml(prefix), encoding="utf-8")
    systemctl("set-environment", f"IBUS_COMPONENT_PATH={COMPONENT_DIR}", f"XDG_CACHE_HOME={CACHE_DIR}")


def restore_ibus(marker: dict) -> None:
    for name, value in marker.get("environment", {}).items():
        if value is None:
            systemctl("unset-environment", name)
        else:
            systemctl("set-environment", f"{name}={value}")
    restart_ibus()
    engine = marker.get("engine", "")
    if engine:
        wait_until(lambda: ibus_engine(set_to=engine) == engine, STARTUP_TIMEOUT_S, f"ibus to take back {engine}")


# --- Fcitx5 (KDE): the session's daemon, replaced by one on the test build ---

def running_fcitx5() -> int | None:
    found = subprocess.run(["pgrep", "-u", str(os.getuid()), "-x", "fcitx5"], capture_output=True, text=True, check=False)
    return int(found.stdout.split()[0]) if found.stdout.strip() else None


def stop_fcitx5() -> None:
    pid = running_fcitx5()
    if pid is None:
        return
    os.kill(pid, 15)
    wait_until(lambda: running_fcitx5() is None, STARTUP_TIMEOUT_S, "the running fcitx5 to exit")


def swap_fcitx5() -> None:
    """The original daemon's command line and environment go into the
    marker (mode 0600, never printed) so `--restore` restarts it as the
    session started it (im-config `run_im fcitx5`)."""
    pid = running_fcitx5()
    marker: dict = {"framework": "fcitx5"}
    if pid is not None:
        marker["argv"] = Path(f"/proc/{pid}/cmdline").read_bytes().decode().rstrip("\0").split("\0")
        marker["environ"] = parse_assignments(Path(f"/proc/{pid}/environ").read_bytes().decode().split("\0"))
    write_marker(marker)
    stop_fcitx5()


def restore_fcitx5(marker: dict) -> None:
    stop_fcitx5()
    if "argv" in marker:
        subprocess.Popen(marker["argv"], env=marker["environ"], start_new_session=True,
                         stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        wait_until(lambda: running_fcitx5() is not None, STARTUP_TIMEOUT_S, "the original fcitx5 to start")


# --- restore marker ---

def write_marker(marker: dict) -> None:
    STATE_DIR.mkdir(parents=True, exist_ok=True)
    RESTORE_MARKER.touch(mode=0o600)
    # touch() keeps an existing file's mode; the fcitx5 marker holds an environment.
    RESTORE_MARKER.chmod(0o600)
    RESTORE_MARKER.write_text(json.dumps(marker), encoding="utf-8")


def restore() -> None:
    """Undo a swap if one is recorded; the marker goes only once it's undone."""
    if not RESTORE_MARKER.exists():
        return
    marker = json.loads(RESTORE_MARKER.read_text(encoding="utf-8"))
    if marker["framework"] == "ibus":
        restore_ibus(marker)
    else:
        restore_fcitx5(marker)
    RESTORE_MARKER.unlink()


class DesktopSession(Session):
    """One scenario in the logged-in session: the framework restarted on the
    scenario's settings, the host window focused by the compositor, keys
    from uinput, a screenshot after every step."""

    def __init__(self, framework: str, prefix: Path, scenario: dict, work: Path, out: Path, session_env: dict[str, str]) -> None:
        # The session's own environment: the host window and the framework
        # tools talk to the real display and bus, with the real HOME.
        super().__init__(framework, prefix, scenario, work, env={**os.environ, **session_env})
        self.prefix = prefix
        self.out = out
        self.focus_file = work / "host-focused"
        self.keyboard: subprocess.Popen | None = None

    def start(self) -> None:
        if self.framework == "ibus":
            CURRENT_WORK.write_text(str(self.work), encoding="utf-8")
            restart_ibus()
            wait_until(lambda: INPUT_METHOD_NAME in self.run(["ibus", "list-engine"]).stdout,
                       STARTUP_TIMEOUT_S, "ibus-daemon listing the engine")
        else:
            (self.work / "config" / "fcitx5").mkdir(parents=True)
            (self.work / "config" / "fcitx5" / "profile").write_text(driver.fcitx5_profile(), encoding="utf-8")
            self.spawn(["fcitx5", "--replace"], env={
                **self.env,
                **driver.prefix_environment(self.prefix),
                "XDG_CONFIG_HOME": str(self.work / "config"),
                "XDG_DATA_HOME": str(self.work / "data"),
            })
            wait_until(self.fcitx5_owns_its_name, STARTUP_TIMEOUT_S, "fcitx5 to own org.fcitx.Fcitx5")
        self.host = subprocess.Popen([sys.executable, str(driver.HOST_SCRIPT), str(self.host_out), str(self.focus_file)], env=self.env)
        self.processes.append(self.host)
        wait_until(self.focus_file.exists, STARTUP_TIMEOUT_S, "the host window to get keyboard focus")
        wait_until(self.activate, STARTUP_TIMEOUT_S, "the input method to be active on the focused field")
        self.keyboard = subprocess.Popen(["sudo", "-n", sys.executable, str(UINPUT_SCRIPT)],
                                         stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
        self.processes.append(self.keyboard)

    def type(self, command: str) -> None:
        self.keyboard.stdin.write(command + "\n")
        self.keyboard.stdin.flush()
        answer = self.keyboard.stdout.readline().strip()
        if answer != "ok":
            raise ScenarioError(f"uinput keyboard answered {answer or 'nothing (did sudo -n fail?)'} to {command!r}")

    def send_text(self, text: str) -> None:
        self.type(f"text {text}")
        self.keys_sent += len(text)

    def send_key(self, name: str) -> None:
        self.type(f"key {name}")
        self.keys_sent += 1

    def checkpoint(self, step_index: int) -> None:
        """Evidence only: a failed screenshot is noted beside the others and
        never changes the scenario's outcome."""
        self.settle()
        time.sleep(0.5)  # let the candidate window paint
        shot = self.out / f"shot-{step_index}.png"
        try:
            taken = self.run(["gnome-screenshot", "-f", str(shot)])
            problem = "" if taken.returncode == 0 and shot.exists() else taken.stderr or "no file written"
        except (OSError, subprocess.SubprocessError) as error:
            problem = str(error)
        if problem:
            shot.with_suffix(".missing").write_text(problem, encoding="utf-8")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--framework", choices=("fcitx5", "ibus"))
    parser.add_argument("--platform", help="run-dir name, e.g. linux-gnome-ibus")
    parser.add_argument("--prefix", type=Path, help="where `make install E2E=1 PREFIX=` put the test-mode build")
    parser.add_argument("--out", type=Path, help="run dir; results go to <out>/<platform>/")
    parser.add_argument("--scenarios", type=Path, default=driver.analyze.REPO_ROOT / "e2e" / "scenarios")
    parser.add_argument("--only", help="run just this scenario id")
    parser.add_argument("--skip", metavar="REASON", help="drive nothing; record every scenario as skipped")
    parser.add_argument("--restore", action="store_true", help="only undo an interrupted run's framework swap")
    args = parser.parse_args(argv)

    if args.restore:
        restore()
        return 0
    if not (args.framework and args.platform and args.out):
        parser.error("--framework, --platform and --out are required")
    if args.skip:
        driver.run_scenarios(args.platform, args.scenarios, args.only, args.out, skip=args.skip)
        return 0
    if args.prefix is None:
        parser.error("--prefix is required unless --skip or --restore")

    restore()
    session_env, reason = wait_for_graphical_session()
    if session_env is None:
        driver.run_scenarios(args.platform, args.scenarios, args.only, args.out, skip=reason)
        return 0
    prefix = args.prefix.resolve()
    try:
        if args.framework == "ibus":
            swap_ibus(prefix)
        else:
            swap_fcitx5()
        driver.run_scenarios(
            args.platform, args.scenarios, args.only, args.out,
            drive_one=lambda scenario, out: driver.drive(
                lambda work, out: DesktopSession(args.framework, prefix, scenario, work, out, session_env), scenario, out
            ),
        )
    finally:
        restore()
    return 0


if __name__ == "__main__":
    sys.exit(main())
