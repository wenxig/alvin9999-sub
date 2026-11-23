"""Compare native release executables against identical loopback HTTP pages."""

import argparse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import os
from pathlib import Path
import platform
from statistics import median, quantiles
import subprocess
import tarfile
from tempfile import TemporaryDirectory
from threading import Thread
from time import sleep


ROOT = Path(__file__).resolve().parents[1]
BASELINE = "ca58186d27168e87ecbc2ee80173c2fbae8abcd6"


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_args):
        pass

    def do_GET(self):
        body = self.server.pages.get(self.path)
        if body is None:
            self.send_error(404)
            return
        sleep(self.server.delay)
        self.send_response(200)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)


def run(command, cwd, env=None):
    subprocess.run(command, cwd=cwd, env=env, check=True)


def build_baseline(directory, revision):
    archive = directory / "baseline.tar"
    with archive.open("wb") as output:
        subprocess.run(
            ["git", "archive", revision, "src", "package.json"],
            cwd=ROOT,
            stdout=output,
            check=True,
        )
    with tarfile.open(archive) as source:
        source.extractall(directory, filter="data")
    for name, variable in [("ss", "SS_SOURCE_URL"), ("v2ray", "V2RAY_SOURCE_URL")]:
        file = directory / "src" / "sources" / f"{name}.ts"
        text = file.read_text()
        target = "const source ="
        if target not in text:
            raise ValueError(f"Cannot set fixture URL for {file}")
        file.write_text(text.replace(target, f"const source = process.env.{variable} ??", 1))
    executable = directory / "bun-sub"
    run(
        ["bun", "build", "--compile", "--minify", "--bytecode", "--format=esm", "src/index.ts",
         "--outfile", str(executable)],
        directory,
    )
    return executable


def measure(timer, executable, directory, env):
    # A small native parent keeps the Python HTTP server out of the child's peak RSS.
    sample = subprocess.check_output([timer, executable], cwd=directory, env=env, text=True)
    wall, cpu, rss = map(float, sample.split())
    rss_units = 1024**2 if platform.system() == "Darwin" else 1024
    return wall, cpu, rss / rss_units


def verify_outputs(directories):
    expected = b"\n".join((ROOT / "tests/fixtures" / f"{name}.txt").read_bytes() for name in ("ss", "v2ray"))
    for directory in directories:
        if (directory / "data/sub.txt").read_bytes() != expected:
            raise ValueError(f"{directory.name}: sub.txt failed output verification")

    # The current release embeds the fixture nodes. Historical releases used an HTTP provider.
    inline = (ROOT / "tests/fixtures/sub.yaml").read_bytes()
    baseline, current = ((directory / "data/sub.yaml").read_bytes() for directory in directories)
    if current != inline:
        raise ValueError("current sub.yaml failed inline fixture verification")
    legacy_url = b"https://raw.githubusercontent.com/wenxig/alvin9999-sub/main/data/sub.txt"
    if baseline != inline and legacy_url not in baseline:
        raise ValueError("baseline sub.yaml contains neither the expected inline config nor the legacy URL")


def compare(server, timer, executables, directories, env, names, delay, runs):
    server.pages = {"/ss": (ROOT / "tests/fixtures/ss.html").read_bytes(), "/v2ray": (ROOT / "tests/fixtures/v2ray.html").read_bytes()}
    server.delay = delay
    for executable, directory in zip(executables, directories):
        for _ in range(3):
            run([executable], directory, env)
    verify_outputs(directories)
    samples = [[], []]
    for round_number in range(runs):
        for index in (0, 1) if round_number % 2 == 0 else (1, 0):
            samples[index].append(measure(timer, executables[index], directories[index], env))
    verify_outputs(directories)
    print(f"\nTwo {len(server.pages['/ss']):,}/{len(server.pages['/v2ray']):,}-byte pages, "
          f"{delay * 1000:g} ms delay per request, {runs} runs per executable")
    print(f"{'Runtime':<8} {'Wall p50 (ms)':>14} {'Wall p95 (ms)':>14} "
          f"{'CPU p50 (ms)':>14} {'Peak RSS p50 (MiB)':>19} {'Binary (MiB)':>13}")
    for name, executable, measurements in zip(names, executables, samples):
        wall, cpu, rss = zip(*measurements)
        p95 = quantiles(wall, n=100, method="inclusive")[94]
        size = executable.stat().st_size / 1024**2
        print(f"{name:<8} {median(wall):14.2f} {p95:14.2f} {median(cpu):14.2f} "
              f"{median(rss):19.2f} {size:13.2f}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--runs", type=int, default=50, help="Samples per version and workload")
    parser.add_argument("--baseline", default=BASELINE, help="Commit containing the original Bun app")
    parser.add_argument("--rust-baseline", type=Path, help="Existing release binary of the previous Rust version")
    args = parser.parse_args()
    if args.runs < 2:
        parser.error("--runs must be at least 2")
    if platform.system() not in ("Darwin", "Linux"):
        parser.error("the benchmark requires macOS or Linux for wait4 resource usage")
    with TemporaryDirectory(prefix="dongtai-bench-") as temporary:
        directory = Path(temporary)
        target = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target")).resolve()
        run(["cargo", "build", "--release", "--locked", "--target-dir", str(target)], ROOT)
        timer = directory / "measure"
        run(["cc", "-O2", "-Wall", "-Wextra", "-o", str(timer), "scripts/measure.c"], ROOT)
        baseline = args.rust_baseline.resolve() if args.rust_baseline else build_baseline(directory, args.baseline)
        names = ("Old Rust" if args.rust_baseline else "Bun", "Rust")
        executables = [baseline, target / "release" / "alvin9999-sub"]
        directories = [directory / "bun-run", directory / "rust-run"]
        for path in directories:
            path.mkdir()
        with ThreadingHTTPServer(("127.0.0.1", 0), Handler) as server:
            worker = Thread(target=server.serve_forever)
            worker.start()
            try:
                base = f"http://127.0.0.1:{server.server_port}"
                env = dict(os.environ, SS_SOURCE_URL=f"{base}/ss", V2RAY_SOURCE_URL=f"{base}/v2ray",
                           GITHUB_REPOSITORY="wenxig/alvin9999-sub", NO_PROXY="127.0.0.1",
                           no_proxy="127.0.0.1")
                print(platform.platform())
                for tool in (("rustc",) if args.rust_baseline else ("bun", "rustc")):
                    print(tool, subprocess.check_output([tool, "--version"], text=True).strip())
                for delay in (0, 0.1):
                    compare(server, timer, executables, directories, env, names, delay, args.runs)
            finally:
                server.shutdown()
                worker.join()


if __name__ == "__main__":
    main()
