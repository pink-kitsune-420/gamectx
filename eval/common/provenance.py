"""Provenance for evidence recorded before the 0.4 repository layout.

Frozen artifacts keep the paths and SHA-256 digests they recorded. The 0.4 reorganization
moved files (``moves-0.4.json`` maps each old path to its new one) and edited some of them
(module paths in sources, folder depth in eval runners). These helpers map recorded paths to
current files and, for moved files whose bytes changed, check the digest against the file as
committed at the last pre-0.4 commit. They never attribute a measurement to code that was not
measured: a matching historical digest proves only what was measured, not that edited code
would measure the same. Requires git history (CI checks out with full depth).
"""
import hashlib
import json
import pathlib
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[2]
PRE_LAYOUT = '38471d5409df1bc599ee00ae1ea0bc264149f247'
MOVES = json.loads(pathlib.Path(__file__).with_name('moves-0.4.json').read_text())
RECORDED = {new: old for old, new in MOVES.items()}


def current(path):
    """Current repository path of a path recorded before 0.4 (unchanged if not moved)."""
    return MOVES.get(path, path)


def recorded(path):
    """Path under which evidence recorded before 0.4 names a current file."""
    return RECORDED.get(path, path)


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def at_layout(path):
    """Bytes of a pre-0.4 path at the last pre-0.4 commit."""
    return subprocess.run(['git', 'show', f'{PRE_LAYOUT}:{path}'], cwd=ROOT, check=True,
                          capture_output=True).stdout


def matches(recorded_digests):
    """True when every recorded {path: sha256} matches the current file, or, for a path moved
    by the 0.4 layout whose bytes later changed, the file at the last pre-0.4 commit."""
    for path, digest in recorded_digests.items():
        now = ROOT / current(path)
        if now.is_file() and sha256(now.read_bytes()) == digest:
            continue
        if path not in MOVES or sha256(at_layout(path)) != digest:
            return False
    return True
