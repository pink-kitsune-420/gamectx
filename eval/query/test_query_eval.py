#!/usr/bin/env python3
"""Free regression checks for the measured prototype and token-only spending guards.

Build the release CLI first. No API calls or real key files are used.
"""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder

import contextlib
import io
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import query_eval as q  # noqa: E402

ROOT = Path(__file__).resolve().parents[2]
BIN = str(ROOT / "target" / "release" / "gamectx")


class PrototypeTests(unittest.TestCase):
    def test_shared_conformance_cases(self):
        for path in sorted((ROOT / "conformance" / "query").glob("*/case.json")):
            with self.subTest(case=path.parent.name):
                case = json.loads(path.read_text())
                tables = q.load_tables(path.parent / "input.json")
                if case.get("error"):
                    with self.assertRaises(q.QueryError):
                        q.run_sql(case["args"]["sql"], tables)
                else:
                    result = q.dumps(q.run_sql(case["args"]["sql"], tables))
                    self.assertEqual(result, (path.parent / "expected.json").read_text().rstrip("\n"))

    def test_overflow_and_extreme_literals(self):
        tables = {"t": [{"x": 1e308}]}
        self.assertEqual(q.dumps(q.run_sql("SELECT COUNT(x * 10), MIN(x * 10) FROM t", tables)), "[0,null]")
        self.assertEqual(q.dumps(q.run_sql("SELECT ROUND(x, 12) FROM t", tables)), "[null]")
        huge = "9" * 310
        for sql in [f"SELECT x FROM t LIMIT {huge}", f"SELECT ROUND(x, {huge}) FROM t"]:
            with self.subTest(sql=sql), self.assertRaises(q.QueryError):
                q.run_sql(sql, tables)

    def test_zero_cap_never_calls_chat(self):
        # Exercise each real entry point with cap zero; the former token-only path
        # ignored the cap entirely. Mock network entry points, not the cap logic.
        data = q.m.build_data()
        for module in (q, q.m):
            with self.subTest(module=module.__name__), tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                key = root / "dummy.key"
                key.write_text("dummy-test-key")
                argv = [module.__file__, "--tokens-only", "--cap", "0", "--gamectx", BIN,
                        "--key-file", str(key)]
                if module is q:
                    argv += ["--formats", "direct,tool"]
                else:
                    argv += ["--formats", "gamectx-nested", "--out", str(root / "results")]
                with patch.object(q.m, "ROOT", str(root)), patch.object(sys, "argv", argv), \
                        patch.object(q.m, "build_data", return_value=data), \
                        patch.object(q.m, "get_prices", return_value={}), \
                        patch.object(q.m, "chat", side_effect=AssertionError("cap allowed a request")) as chat, \
                        contextlib.redirect_stdout(io.StringIO()):
                    module.main()
                chat.assert_not_called()
                saved = list(root.rglob("tokens.json"))
                self.assertEqual(len(saved), 1)
                result = json.loads(saved[0].read_text())
                self.assertEqual(result["spent_usd"], 0)
                self.assertTrue(result["stopped"])


if __name__ == "__main__":
    unittest.main()
