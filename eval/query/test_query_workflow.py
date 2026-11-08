#!/usr/bin/env python3
"""Free regression tests for schema descriptions, correction bounds and eval accounting."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder

import argparse
import contextlib
import io
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import query_workflow_eval as w

ROOT = Path(__file__).resolve().parents[2]


class WorkflowTests(unittest.TestCase):
    def test_shared_cases(self):
        for operation in ("query-schema", "query-session"):
            for path in sorted((ROOT / "conformance" / operation).glob("*/case.json")):
                with self.subTest(case=path.parent.name):
                    case = json.loads(path.read_text())
                    tables = w.q.load_tables(path.parent / "input.json")

                    def run():
                        if operation == "query-schema":
                            return w.schema(tables)
                        session = w.Session(case["args"].get("result_bytes", 4096))
                        return [session.apply_call(call, tables) for call in case["args"]["calls"]]

                    if case.get("error"):
                        with self.assertRaises(w.q.QueryError):
                            run()
                    else:
                        self.assertEqual(w.q.dumps(run()), (path.parent / "expected.json").read_text().rstrip("\n"))

    def test_semantic_fields_and_heldout_states(self):
        sizes = []
        for seed in (7, 101, 202, 303):
            tools, legacy, explicit, _ = w.state(seed)
            sizes.append((len(explicit["inventory"]), len(explicit["npcs"])))
            for old, new in zip(legacy["inventory"], explicit["inventory"]):
                self.assertNotIn("weight", new)
                self.assertEqual(old["weight"], new["unit_weight"])
                self.assertEqual(old["count"] * old["weight"], new["stack_weight"])
            self.assertEqual(len(w.questions(tools, legacy, True)), 29)
        self.assertEqual(sizes, [(48, 20), (24, 10), (48, 20), (96, 40)])

    def test_spending_guard_counts_usage_and_missing_usage(self):
        messages = [{"role": "user", "content": "x" * 1000}]
        prices = {"test": {"input": 1.0, "output": 1.0}}
        args = argparse.Namespace(cap=0.002, base_url="unused")
        reply = {"choices": [{"message": {"content": "{}"}}],
                 "usage": {"prompt_tokens": 1000, "completion_tokens": 10}}
        account = {"model": "test", "spent": 0.0}
        with patch.object(w.m, "chat_messages", return_value=reply) as chat:
            w.ask(messages, args, "dummy-test-key", prices, account, 10)
            self.assertAlmostEqual(account["spent"], 0.00101)
            with self.assertRaises(StopIteration):
                w.ask(messages, args, "dummy-test-key", prices, account, 10)
            self.assertEqual(chat.call_count, 1)
        account["spent"] = 0
        del reply["usage"]
        with patch.object(w.m, "chat_messages", return_value=reply):
            w.ask(messages, args, "dummy-test-key", prices, account, 10)
        self.assertAlmostEqual(account["spent"], 0.001522)
        account["spent"] = 0
        with patch.object(w.m, "chat_messages", side_effect=RuntimeError("uncertain request")):
            with self.assertRaises(RuntimeError):
                w.ask(messages, args, "dummy-test-key", prices, account, 10)
        self.assertAlmostEqual(account["spent"], 0.001522)

    def test_malformed_tool_arguments_are_not_accepted(self):
        for text in ['{"action":"query","args":{"sql":3}}',
                     '{"action":"query","args":{"sql":"SELECT 1 FROM t","extra":true}}',
                     '{"action":"other","args":{"sql":"SELECT 1 FROM t"}}']:
            self.assertEqual(w.extract_sql(text), "")

    def test_resume_does_not_repeat_completed_questions(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            key = root / "dummy.key"
            key.write_text("dummy-test-key")
            row = {"model": "test", "seed": 7, "mode": "schema", "question": "count?",
                   "first_ok": True, "executed_ok": True, "ok": True}
            prior = root / "prior.json"
            prior.write_text(json.dumps({"spent_usd": 0.002, "rows": [row]}))
            output = root / "output"
            output.mkdir()
            args = argparse.Namespace(resume=str(prior), key_file=str(key), base_url="unused",
                                      models="test", tokens_only=False, cap=0, seeds="7")
            bundle = (7, "schema", {}, "prompt", [("test", "count?", "float", 1)])
            with patch.object(w.m, "chat_messages", side_effect=AssertionError("repeated completed question")), \
                    contextlib.redirect_stdout(io.StringIO()):
                w.evaluate(args, [bundle], output)
            saved = json.loads((output / "results.json").read_text())
            self.assertEqual(saved["spent_usd"], 0)
            self.assertEqual(saved["cumulative_spent_usd"], 0.002)
            self.assertEqual(saved["rows"], [row])


if __name__ == "__main__":
    unittest.main()
