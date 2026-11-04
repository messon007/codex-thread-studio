#!/usr/bin/env python3
"""Accepted native continuation -> durable Router monitoring, private providers only.

The fixture records an already accepted turn inside its owned fake provider.
The production endpoint must read that receipt, adopt monitoring, and never send
either a routing inference or another source turn. Python is test tooling only.
"""
import ast
import concurrent.futures
import importlib.util
import json
from pathlib import Path
import sqlite3
import sys
import tempfile
import time

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location(
    "handoff_runtime", Path(__file__).with_name("smoke-gnome-router-runtime.py")
)
runtime = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runtime)

CONTROL = r'''
def apply_control():
    control=root/'accepted-control.json'
    if not control.exists():return
    payload=json.loads(control.read_text());control.unlink()
    with lock:
        history=turns.setdefault('target',[])
        for turn in history:
            if turn['id'] in payload.get('complete',[]):turn['status']='completed'
        history.extend(payload.get('append',[]));save()
'''
READ = r'''
    elif method=='thread/turns/list' and session=='target':
        apply_control()
        with lock:value={'data':list(reversed(turns.get(session,[])))[:4],'nextCursor':None}
        if (root/'hold-monitor').exists() and not (root/'monitor-held').exists():
            (root/'monitor-held').touch()
            def held(identity=identity,value=value):
                while not (root/'release-monitor').exists():time.sleep(.01)
                answer(identity,value)
            threading.Thread(target=held,daemon=True).start()
        else:answer(identity,value)
'''


def backend():
    source = runtime.CODEX.replace("for raw in sys.stdin:", CONTROL + "\nfor raw in sys.stdin:")
    source = source.replace("    elif method=='thread/read':", "    elif method=='thread/read':\n        apply_control()")
    source = source.replace("    elif method=='thread/turns/list':", READ + "\n    elif method=='thread/turns/list':")
    ast.parse(source)
    return source


FOLLOW = "/studio/native/router/follow"
READY = "/studio/native/router/prepare-supervised"
PROMPT = "[Studio turn supervisor]\nFinish the remaining work 中文😀"


def accepted(root, previous, identity="accepted-next", status="inProgress", complete_previous=True):
    turn = {
        "id": identity,
        "status": status,
        "items": [
            {"id": identity + "-user", "type": "userMessage", "content": [{"type": "text", "text": PROMPT}]},
            {"id": identity + "-answer", "type": "agentMessage", "text": "Only the NEW accepted reply 中文😀"},
        ],
    }
    (root / "accepted-control.json").write_text(json.dumps({"complete": [previous] if complete_previous else [], "append": [turn]}))
    return {"backend": "codex", "threadId": "target", "cwd": str(root / "project"), "previousTurnId": previous, "turnId": identity, "prompt": PROMPT}


def close(gateway, root):
    gateway.close()
    for name in ["backend.pid", "opencode.pid"]:
        path = root / name
        if path.exists():
            identity = path.read_text().strip()
            runtime.until(lambda: not Path("/proc", identity).exists(), seconds=8)


def exercise(executable, root, mode):
    runtime.create_profile(root)
    runtime.fixture.BACKEND = backend()
    runtime.fixture.OPENCODE_BACKEND = runtime.OPENCODE
    gateway = runtime.fixture.Gateway(executable, root, "opencode")
    try:
        for path in [FOLLOW, READY]:
            assert gateway.api(path, {}, authorized=False)[0] == 401
            assert gateway.api(path, {})[0] == 400
        ready = {"backend": "codex", "threadId": "target", "cwd": str(root / "project"), "turnId": "unrouted"}
        assert gateway.api(READY, ready) == (200, {"ready": True, "routed": False})
        if mode in ("active", "stopped"):
            (root / "hold-target").touch()
            (root / "hold-monitor").touch()
        identity = runtime.submit(gateway, runtime.body(gateway, "Original routed work"))
        if mode in ("active", "stopped"):
            runtime.until(lambda: (root / "monitor-held").exists())
            current = runtime.get(gateway, identity)
            assert current["status"] == "running", current
        else:
            current = runtime.finished(gateway, identity)
        previous = current["targetTurnId"]
        before = len(runtime.starts(root))
        if mode == "stopped":
            assert gateway.api("/studio/native/router/stop", {"id": identity})[0] == 202
        request = accepted(root, previous, status="completed" if mode == "terminal" else "inProgress")
        for bad in [
            {**request, "cwd": str(root / "foreign-project")},
            {**request, "prompt": PROMPT + " changed"},
        ]:
            status, value = gateway.api(FOLLOW, bad)
            assert status == 409, (status, value)
            assert runtime.get(gateway, identity)["targetTurnId"] == previous
        status, receipt = gateway.api(FOLLOW, request)
        assert status == 200 and receipt == {"followed": True, "id": identity, "controller": "codex:router", "targetTurnId": "accepted-next"}, (status, receipt)
        adopted = runtime.get(gateway, identity)
        assert adopted["targetTurnId"] == "accepted-next"
        assert adopted["supervision"] == {"previousTurnId": previous, "prompt": PROMPT}
        assert adopted["prompt"] == "Original routed work"
        assert "Native routed target response" not in adopted["response"]
        assert all(item["id"] != previous + "-response" for item in adopted["responseItems"])
        assert gateway.api(FOLLOW, request) == (200, receipt)
        assert len(runtime.starts(root)) == before
        with sqlite3.connect(root / "config/codex-thread-studio/studio.sqlite3") as database:
            mirrors = [json.loads(row[0]) for row in database.execute("SELECT dispatch FROM router_dispatches WHERE controller='codex:router'")]
        mirror = next(value for value in mirrors if value.get("nativeJobId") == identity)
        assert mirror["targetTurnId"] == "accepted-next" and mirror["decision"]["forwardedPrompt"] == PROMPT
        if mode in ("active", "stopped"):
            (root / "release-monitor").touch()
        if mode == "stopped":
            final = runtime.finished(gateway, identity, "stopped")
            assert final["stopRequested"] is True
            interrupts = [r for r in runtime.requests(root) if r.get("method") == "turn/interrupt"]
            assert len(interrupts) == 1 and interrupts[0]["params"]["turnId"] == "accepted-next", interrupts
            assert gateway.api(READY, {**ready, "turnId": "accepted-next"})[0] == 409
        elif mode == "restart":
            close(gateway, root)
            gateway = runtime.fixture.Gateway(executable, root, "opencode")
            # Race an ordinary recovery read with a newly accepted receipt.
            # Both may request recovery, but only one monitor may adopt it.
            second = accepted(root, "accepted-next", "accepted-after-restart")
            with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
                restoring = pool.submit(runtime.get, gateway, identity)
                following = pool.submit(gateway.api, FOLLOW, second)
                restoring.result(timeout=10)
                status, restored = following.result(timeout=10)
                assert status == 200 and restored["targetTurnId"] == "accepted-after-restart", (status, restored)
            assert runtime.get(gateway, identity)["targetTurnId"] == "accepted-after-restart"
            (root / "accepted-control.json").write_text(json.dumps({"complete": ["accepted-after-restart"]}))
            runtime.finished(gateway, identity)
        elif mode == "active":
            (root / "accepted-control.json").write_text(json.dumps({"complete": ["accepted-next"]}))
            runtime.finished(gateway, identity)
        else:
            runtime.finished(gateway, identity)
        if mode != "stopped":
            final = runtime.get(gateway, identity)
            assert final["response"] == "Only the NEW accepted reply 中文😀", final
            assert [item["id"] for item in final["responseItems"]] == [("accepted-after-restart" if mode == "restart" else "accepted-next") + "-answer"]
        assert len(runtime.starts(root)) == before, "Router handoff replayed an inference/source mutation"
        return {"mode": mode, "noReplay": True, "exactSource": True, "durableMonitoring": True}
    finally:
        (root / "release-monitor").touch()
        close(gateway, root)


def map_readiness(executable, root):
    spec = importlib.util.spec_from_file_location("handoff_map_runtime", Path(__file__).with_name("smoke-gnome-router-map.py"))
    mapped = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mapped)
    mapped.runtime.create_profile(root)
    mapped.runtime.fixture.BACKEND = mapped.codex()
    mapped.runtime.fixture.OPENCODE_BACKEND = mapped.runtime.OPENCODE
    gateway = mapped.runtime.fixture.Gateway(executable, root, "opencode")
    try:
        original = mapped.create_map(gateway)
        identity = mapped.runtime.submit(gateway, mapped.runtime.body(gateway, "MAP_HUMAN"))
        runtime.until(lambda: (root / "MAP_HUMAN-held").exists())
        runtime.until(lambda: mapped.runtime.get(gateway, identity)["status"] == "running")
        current = mapped.runtime.get(gateway, identity)
        request = {"backend": "codex", "threadId": "target", "cwd": str(root / "project"), "turnId": current["targetTurnId"]}
        with concurrent.futures.ThreadPoolExecutor(max_workers=1) as pool:
            pending = pool.submit(gateway.api, READY, request)
            time.sleep(.25)
            assert not pending.done(), "Next native Map preparation bypassed the unfinished routed Map scope"
            (root / "MAP_HUMAN-release").touch()
            assert pending.result(timeout=8) == (200, {"ready": True, "routed": True})
        updated = mapped.read_map(gateway)
        assert updated["id"] == original["id"] and updated["revision"] == original["revision"] + 1
        mapped.runtime.finished(gateway, identity)
        assert len(mapped.runtime.starts(root, "target")) == 1
        return {"mode": "map-readiness", "waitedForPreviousMap": True, "newPreparationSeesSavedRevision": True, "noReplay": True}
    finally:
        (root / "MAP_HUMAN-release").touch()
        close(gateway, root)


def main():
    executable = Path(sys.argv[1]).resolve()
    base = Path(__file__).resolve().parents[1] / "target/gnome-native-smoke"
    base.mkdir(parents=True, exist_ok=True)
    directory = Path(tempfile.mkdtemp(prefix="supervision-handoff-", dir=base))
    results = [exercise(executable, directory / mode, mode) for mode in ["terminal", "active", "stopped", "restart"]]
    results.append(map_readiness(executable, directory / "map-readiness"))
    report = {"ok": True, "artifacts": str(directory), "scenarios": results, "privateProvidersOnly": True, "ownedProvidersExited": True}
    (directory / "result.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report))


if __name__ == "__main__":
    main()
