#!/usr/bin/env python3
"""Runnable starter integration checks; uses only Python's standard library."""
import json
import pathlib
import subprocess
import sys

root = pathlib.Path(__file__).resolve().parent
binary = sys.argv[1] if len(sys.argv) > 1 else 'gamectx'
def run(*args):
    return json.loads(subprocess.check_output([binary, *args], cwd=root, text=True))
assert run('doctor', 'project.json')['warnings'] == []
packet = run('inspect', 'project.json', 'snapshot.json', 'conversation')
assert packet['inspection']['receipt']['total_bytes'] <= 4096
assert 'secret' not in json.dumps(packet['packet'])
report = run('evaluate', 'project.json', 'suite.json')
assert report['runs'][0]['answers_correct'] == 3
assert report['runs'][1]['invalid_queries'] == 1
assert report['runs'][1]['invalid_tool_calls'] == 1
print('starter checks passed')
