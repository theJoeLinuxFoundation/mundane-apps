"""Interleaved benchmark processes sharing the isolated test compositor and registry."""
import os
import random
import subprocess


def measure(directory, set_enabled):
    address = os.environ["AT_SPI_BUS_ADDRESS"]
    rng = random.Random(56065)
    count = int(os.environ.get("COST_TRIALS", "10"))
    for trial in range(count):
        order = ["may", "inactive", "active"]
        rng.shuffle(order)
        for condition in order:
            enabled = condition == "active"
            set_enabled(enabled)
            binary = directory / ("may-bin" if condition == "may" else "pin-bin")
            env = {**os.environ, "COST_OUTPUT": str(directory / f"{condition}-{trial}.csv")}
            if enabled:
                env["EXPECT_ACTIVE"] = "1"
            subprocess.run([binary], env=env, check=True, timeout=90)
        print(f"Completed cost trial {trial+1}/{count}", flush=True)
    if (directory / "trace-bin").is_file():
        for enabled in (False, True):
            condition = "active" if enabled else "inactive"
            env = {**os.environ, "COST_TRACE": str(directory / f"trace-{condition}.csv"),
                   "COST_OUTPUT": str(directory / f"trace-timings-{condition}.csv")}
            if enabled:
                env["EXPECT_ACTIVE"] = "1"
            set_enabled(enabled)
            with (directory / f"bus-{condition}.pcap").open("wb") as stream:
                capture = subprocess.Popen(["dbus-monitor", "--address", address, "--pcap",
                    "type='signal',interface='org.a11y.atspi.Event.Object'"], stdout=stream)
                try:
                    subprocess.run([directory / "trace-bin"], env=env, check=True, timeout=90)
                finally:
                    capture.terminate()
                    capture.wait(timeout=3)
        for trial in range(int(os.environ.get("ACTIVATION_TRIALS", "10"))):
            env = {**os.environ, "COST_ACTIVATION_ONLY": "1", "EXPECT_ACTIVE": "1",
                   "COST_OUTPUT": str(directory / f"activation-{trial}.ns"),
                   "COST_TRACE": str(directory / "activation")}
            subprocess.run([directory / "trace-bin"], env=env, check=True, timeout=15)
