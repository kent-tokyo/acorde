import datetime, hashlib, json, pathlib, subprocess, sys

R = pathlib.Path("/private/tmp/acorde-print-v3")
OUT = pathlib.Path(sys.argv[1])
sha = lambda p: hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()
git = lambda *a: subprocess.run(["git", "-C", str(R), *a], capture_output=True, text=True, check=True).stdout
mscore = "/Applications/MuseScore 4.app/Contents/MacOS/mscore"

assert OUT.exists() is False, "freeze already exists"
assert git("status", "--porcelain", "--untracked-files=no") == "", "worktree has tracked changes"
sel = sha(R / "benchmarks/print/parity-holdout-selection-v3.json")
assert sel == "657e1334b8f1ab888fd97336bd3474f5e44e7fe8547b79eca60715cb9e54b218", sel
files = sorted(git("ls-files", "crates").split()) + ["Cargo.toml", "Cargo.lock"]
artifacts = [R / "target/release/examples/print_mxl", R / "target/release/examples/measured_print",
             R / "crates/pdf/tests/fixtures/Tuffy.ttf"]
version = subprocess.run([mscore, "--version"], capture_output=True, text=True).stdout.strip().splitlines()[-1]
freeze = {
    "classification": "v3 final holdout candidate, frozen before sources are opened",
    "frozen_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
    "source_commit": git("rev-parse", "HEAD").strip(),
    "selection_sha256": sel,
    "source_files_sha256": {f: sha(R / f) for f in files},
    "artifacts_sha256": {str(p): sha(p) for p in artifacts},
    "native_cli_audit_tool": {"path": str(R / "target/release/acorde"), "sha256": sha(R / "target/release/acorde")},
    "verovio": "6.4.1",
    "staff_space_mm": 1.8,
    "matched_measures_per_system_cap": 8,
    "musescore": {"version": version, "binary": mscore, "binary_sha256": sha(mscore)},
    "holdout_status": "unopened",
}
OUT.write_text(json.dumps(freeze, indent=2) + "\n")
print(OUT, sha(OUT), len(files), "source files")
