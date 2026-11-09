"""Independent original reference for the pragmatic parser, mixed-initiative agenda and
conversational stance (pass 3, stage 3). Python standard library only; nothing here calls a model.

Parser (F1): `parse` splits a player turn at sentence punctuation (. ? ! ; and the em dash) into at
most 4 clauses, reads each clause's words (lower-cased runs of ASCII letters and digits), and
classifies it with an original phrase lexicon in a fixed precedence (threat, accusation, apology,
offer or transfer, request for help, greeting, farewell, compliment, then a question about facts
or about the listener, else small talk). Authors add phrases with `lexicon`. A subtext rule set
reads urgency, sarcasm (praise for something the context says failed), testing honesty (asking
about something the speaker witnessed), guilt-tripping, bargaining, evasion and probing; the
default is sincere. Each clause reports whether it needs a state update (an apology to appraise,
an item to receive) and whether it needs a reply. `validate` checks clauses a model proposed
instead: known acts and subtexts, and texts found in the utterance in order.

Agenda (F2): `agenda` blends a turn's obligations with the character's own: a motive scoring 90 or
more overrides polite routing (updates are still applied, replies deferred); a strong own item is
raised after the answers (answer then counter-ask); a grievance of priority 80 or more withholds
answers to non-urgent questions until it is addressed (conditional gate). The thread stack
(`open`, `interrupt`, `resume`, `close`, `threads`) keeps up to 4 conversation threads per being so an
interrupted one can be resumed with its unanswered question and how long ago it stopped.

Stance (F3): `stance` gives cooperativeness from trust, affection and resentment, clamps a
confidential reply to a whisper or a deflection when an untrusted bystander is in earshot, and
shortens replies as task urgency rises.
"""
import pathlib
import sys
from typing import Any, Dict, List, Optional, Tuple

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from beliefs_speech_reference import ident, keys, need, opt_bool, opt_uint, text, uint
from frame_reference import time_label
from growth_reference import run_limited
from limits_reference import Limits

MAX_AT = 1_000_000_000_000
SPEC = [("beings", 1024, 1_000_000), ("threads", 4, 16), ("phrases", 64, 4096), ("own", 8, 64)]
MAX_CLAUSES = 4
ALNUM = set("abcdefghijklmnopqrstuvwxyz0123456789")
BOUNDARY = {".", "?", "!", ";", "—"}
LEADING = ["by the way", "and also", "also", "anyway", "and", "but", "so", "oh"]
PRECEDENCE = ["threat", "accusation", "apology", "offer_or_transfer", "request_help", "greeting", "farewell",
              "compliment"]
ACTS = PRECEDENCE + ["question_fact", "question_personal", "small_talk"]
SUBTEXTS = ["sincere", "probing", "evasive", "sarcastic", "urgent", "guilt_tripping", "bargaining",
            "testing_honesty"]
LEXICON = {
    "threat": ["or else", "you will regret", "you ll regret", "watch yourself", "i will hurt you", "i ll hurt you",
               "last warning"],
    "accusation": ["you stole", "you took", "you lied", "you broke", "you cheated", "liar", "thief", "your fault"],
    "apology": ["sorry", "apologize", "apologise", "apologies", "forgive me", "my fault", "my mistake"],
    "offer_or_transfer": ["here is", "here s", "here are", "here it is", "take this", "i brought", "this is for you",
                          "a gift for you",
                          "as promised", "i have your"],
    "request_help": ["help me", "can you help", "could you", "would you", "please help", "i need you to",
                     "lend me"],
    "greeting": ["hello", "hi", "hey", "good morning", "good evening", "greetings", "well met"],
    "farewell": ["goodbye", "farewell", "good night", "see you", "until tomorrow"],
    "compliment": ["well done", "thank you", "thanks", "fine work", "good work", "beautiful", "you look",
                   "nicely done", "great job"],
}
QUESTION_START = {"who", "what", "where", "when", "why", "how", "is", "are", "do", "does", "did", "can", "could",
                  "will", "would", "have", "has"}
YOU = {"you", "your", "yours", "yourself"}
URGENT = ["now", "quick", "quickly", "hurry", "immediately", "at once"]
GUILT = ["after all i", "after everything", "i thought you", "you never", "you always"]
BARGAIN = ["if you", "in exchange", "for a price", "a deal", "what s in it"]
EVASIVE = ["maybe", "perhaps", "don t know", "never mind", "whatever", "not sure"]
PROBE = ["really", "true", "honestly", "be honest"]
UPDATE = {"apology": "appraise_apology", "offer_or_transfer": "receive_item", "accusation": "appraise_accusation",
          "threat": "appraise_threat", "compliment": "appraise_praise"}
OWN_KINDS = ["ask", "followup", "grievance"]
GATED = {"question_fact", "question_personal", "request_help", "small_talk"}


def words(s: str) -> List[str]:
    out, cur = [], []
    for ch in s:
        c = ch.lower() if "A" <= ch <= "Z" else ch
        if c in ALNUM:
            cur.append(c)
        elif cur:
            out.append("".join(cur))
            cur = []
    if cur:
        out.append("".join(cur))
    return out


def has(ws: List[str], phrase: List[str]) -> bool:
    n = len(phrase)
    return any(ws[i:i + n] == phrase for i in range(len(ws) - n + 1))


def phrase_of(v: Any) -> str:
    s = text(v, 64)
    ws = words(s)
    need(1 <= len(ws) <= 4 and " ".join(ws) == s)
    return s


def word_list(d: dict, k: str) -> List[str]:
    v = d.get(k)
    v = [] if v is None else v
    need(isinstance(v, list) and len(v) <= 16)
    out = []
    for w in v:
        need(isinstance(w, str) and 1 <= len(w) <= 32 and all(ch in ALNUM for ch in w))
        out.append(w)
    need(len(set(out)) == len(out))
    return out


def split(s: str) -> List[Tuple[str, str]]:
    """(clause text, terminator) pairs; the terminator is '' at the end of the text."""
    out, start = [], 0
    for i, ch in enumerate(s):
        if ch in BOUNDARY:
            out.append((s[start:i], ch))
            start = i + 1
    out.append((s[start:], ""))
    res = []
    for t, term in out:
        t = t.strip(" \t,")
        if words(t):
            res.append((t, term))
    return res


def strip_leading(ws: List[str]) -> List[str]:
    for m in LEADING:
        p = m.split(" ")
        if ws[:len(p)] == p and len(ws) > len(p):
            return ws[len(p):]
    return ws


def obligations(act: str, sub: str) -> dict:
    """A reply is owed for anything but sincere small talk."""
    return {"reply": act != "small_talk" or sub != "sincere", "update": UPDATE.get(act)}


class Pragmatics:
    def __init__(self) -> None:
        self.lim = Limits(SPEC)
        self.custom: Dict[str, List[str]] = {a: [] for a in PRECEDENCE}
        self.count = 0
        self.stacks: Dict[str, List[dict]] = {}
        self.clock = 0

    def classify(self, ws: List[str], term: str, witnessed: List[str], failed: List[str]) -> dict:
        ws = strip_leading(ws)
        act, cues = None, []
        for a in PRECEDENCE:
            hits = sorted({p for p in LEXICON[a] + self.custom[a] if has(ws, p.split(" "))})
            if hits:
                act, cues = a, hits
                break
        question = term == "?" or ws[0] in QUESTION_START
        if act is None:
            if question:
                act = "question_personal" if any(w in YOU for w in ws[:3]) else "question_fact"
            else:
                act = "small_talk"
        m = lambda lst: any(has(ws, p.split(" ")) for p in lst)
        is_q = act in ("question_fact", "question_personal")
        if m(URGENT) or (term == "!" and act == "request_help"):
            sub = "urgent"
        elif act == "compliment" and any(w in failed for w in ws):
            sub = "sarcastic"
        elif is_q and any(w in witnessed for w in ws):
            sub = "testing_honesty"
        elif m(GUILT):
            sub = "guilt_tripping"
        elif m(BARGAIN):
            sub = "bargaining"
        elif m(EVASIVE):
            sub = "evasive"
        elif act == "question_personal" or (is_q and m(PROBE)):
            sub = "probing"
        else:
            sub = "sincere"
        return {"act": act, "cues": cues, "question": question, "subtext": sub, **obligations(act, sub)}

    # ---------------------------------------------------------------- parser

    def op_lexicon(self, a: dict) -> dict:
        keys(a, ["act", "phrases"], [])
        act = ident(a["act"])
        need(act in PRECEDENCE)
        rows = a["phrases"]
        need(isinstance(rows, list) and 1 <= len(rows) <= 16)
        ps = [phrase_of(p) for p in rows]
        need(len(set(ps)) == len(ps))
        new = [p for p in ps if p not in self.custom[act] and p not in LEXICON[act]]
        need(self.count + len(new) <= self.lim["phrases"])
        self.custom[act] += new
        self.count += len(new)
        return {"act": act, "added": len(new), "phrases": sorted(LEXICON[act] + self.custom[act])}

    def op_parse(self, a: dict) -> dict:
        keys(a, ["text"], ["speaker", "listener", "context"])
        s = text(a["text"], 500)
        ctx = a.get("context")
        ctx = {} if ctx is None else ctx
        keys(ctx, [], ["witnessed", "failed"])
        witnessed, failed = word_list(ctx, "witnessed"), word_list(ctx, "failed")
        parts = split(s)
        need(len(parts) >= 1)
        clauses = []
        for i, (t, term) in enumerate(parts[:MAX_CLAUSES]):
            clauses.append({"index": i, "text": t, **self.classify(words(t), term, witnessed, failed)})
        return {"clauses": clauses, "listener": None if a.get("listener") is None else ident(a["listener"]),
                "overflow": max(0, len(parts) - MAX_CLAUSES),
                "speaker": None if a.get("speaker") is None else ident(a["speaker"])}

    def op_validate(self, a: dict) -> dict:
        keys(a, ["text", "clauses"], [])
        s = text(a["text"], 500)
        rows = a["clauses"]
        need(isinstance(rows, list) and 1 <= len(rows) <= MAX_CLAUSES)
        pos, out = 0, []
        for i, c in enumerate(rows):
            keys(c, ["text", "act", "subtext"], [])
            t = text(c["text"], 500)
            act, sub = ident(c["act"]), ident(c["subtext"])
            need(act in ACTS and sub in SUBTEXTS)
            at = s.find(t, pos)
            need(at >= 0)
            pos = at + len(t)
            out.append({"act": act, "index": i, "subtext": sub, "text": t, **obligations(act, sub)})
        return {"clauses": out, "valid": True}

    # ---------------------------------------------------------------- agenda

    def op_agenda(self, a: dict) -> dict:
        keys(a, ["clauses"], ["motive", "own"])
        rows = a["clauses"]
        need(isinstance(rows, list) and len(rows) <= MAX_CLAUSES)
        cls = []
        for c in rows:
            keys(c, ["index", "act", "subtext"], [])
            i = uint(c["index"], 0, MAX_CLAUSES - 1)
            act, sub = ident(c["act"]), ident(c["subtext"])
            need(act in ACTS and sub in SUBTEXTS)
            cls.append((i, act, sub))
        need(len({c[0] for c in cls}) == len(cls))
        cls.sort()
        mo = a.get("motive")
        motive = None
        if mo is not None:
            keys(mo, ["id", "score"], [])
            motive = (ident(mo["id"]), uint(mo["score"], 0, 100))
        own_rows = a.get("own")
        own_rows = [] if own_rows is None else own_rows
        need(isinstance(own_rows, list) and len(own_rows) <= self.lim["own"])
        own = []
        for o in own_rows:
            keys(o, ["id", "kind", "priority"], [])
            k = ident(o["kind"])
            need(k in OWN_KINDS)
            own.append((ident(o["id"]), k, uint(o["priority"], 0, 100)))
        need(len({o[0] for o in own}) == len(own))

        def step(kind: str, what: str, clause: Optional[int] = None, own_id: Optional[str] = None) -> dict:
            return {"clause": clause, "kind": kind, "own": own_id, "what": what}

        steps, withheld, deferred = [], [], []
        if motive is not None and motive[1] >= 90:
            steps.append(step("own", "urgent", own_id=motive[0]))
            for i, act, sub in cls:
                if act in UPDATE:
                    steps.append(step("update", UPDATE[act], clause=i))
                if obligations(act, sub)["reply"]:
                    deferred.append(i)
            return {"deferred": deferred, "mode": "interrupt_override", "steps": steps, "withheld": []}
        top = None
        for o in own:
            if o[2] >= 60 and (top is None or (o[2], top[0]) > (top[2], o[0])):
                top = o
        gate = top is not None and top[1] == "grievance" and top[2] >= 80
        answered = False
        for i, act, sub in cls:
            if act in UPDATE:
                steps.append(step("update", UPDATE[act], clause=i))
            if not obligations(act, sub)["reply"]:
                continue
            if gate and act in GATED and sub != "urgent":
                withheld.append(i)
            else:
                steps.append(step("answer", act, clause=i))
                answered = True
        if top is not None:
            steps.append(step("own", top[1], own_id=top[0]))
        mode = ("conditional_gate" if withheld else "answer_then_counter_ask" if top is not None and answered
                else "raise_own" if top is not None else "respond")
        return {"deferred": [], "mode": mode, "steps": steps, "withheld": withheld}

    # ---------------------------------------------------------------- threads

    def op_tick(self, a: dict) -> dict:
        keys(a, ["now"], [])
        now = uint(a["now"], 0, MAX_AT)
        need(now >= self.clock)
        self.clock = now
        return {"now": now}

    def view(self, b: str) -> List[dict]:
        return [dict(t) for t in reversed(self.stacks.get(b, []))]

    def op_open(self, a: dict) -> dict:
        keys(a, ["being", "id", "topic", "speaker"], ["question"])
        b, i = ident(a["being"]), ident(a["id"])
        topic, speaker = ident(a["topic"]), ident(a["speaker"])
        q = None if a.get("question") is None else text(a["question"], 160)
        need(b in self.stacks or len(self.stacks) < self.lim["beings"])
        st = self.stacks.get(b, [])
        need(all(t["id"] != i for t in st))
        st = self.stacks.setdefault(b, [])
        dropped = None
        if len(st) >= self.lim["threads"]:
            dropped = st.pop(0)["id"]
        for t in st:
            if t["status"] == "active":
                t["status"] = "waiting"
        st.append({"cause": None, "id": i, "interrupted_at": None, "opened_at": self.clock, "question": q,
                   "speaker": speaker, "status": "active", "topic": topic})
        return {"being": b, "dropped": dropped, "threads": self.view(b)}

    def op_interrupt(self, a: dict) -> dict:
        keys(a, ["being", "cause"], [])
        b, cause = ident(a["being"]), ident(a["cause"])
        st = self.stacks.get(b, [])
        need(len(st) > 0 and st[-1]["status"] == "active")
        st[-1].update(status="interrupted", cause=cause, interrupted_at=self.clock)
        return {"being": b, "thread": dict(st[-1])}

    def op_resume(self, a: dict) -> dict:
        keys(a, ["being"], ["ticks_per_hour"])
        b = ident(a["being"])
        tph = opt_uint(a, "ticks_per_hour", 1, 3600, 1)
        st = self.stacks.get(b, [])
        need(len(st) > 0 and st[-1]["status"] == "interrupted")
        t = st[-1]
        since = time_label(t["interrupted_at"], self.clock, tph, False)["label"]
        resumed = dict(t)
        t.update(status="active", cause=None, interrupted_at=None)
        return {"being": b, "since": since, "thread": resumed, "unanswered": resumed["question"]}

    def op_close(self, a: dict) -> dict:
        keys(a, ["being", "id"], [])
        b, i = ident(a["being"]), ident(a["id"])
        st = self.stacks.get(b, [])
        need(any(t["id"] == i for t in st))
        was_top = st[-1]["id"] == i
        st[:] = [t for t in st if t["id"] != i]
        if was_top and st and st[-1]["status"] == "waiting":
            st[-1]["status"] = "active"
        return {"being": b, "closed": i, "top": st[-1]["id"] if st else None}

    def op_threads(self, a: dict) -> dict:
        keys(a, ["being"], [])
        b = ident(a["being"])
        return {"being": b, "threads": self.view(b)}

    # ---------------------------------------------------------------- stance

    def op_stance(self, a: dict) -> dict:
        keys(a, ["trust"], ["affection", "resentment", "confidential", "bystanders", "urgency", "deadline_hours"])
        v = a["trust"]
        need(isinstance(v, int) and not isinstance(v, bool) and -100 <= v <= 100)
        trust = v
        aff = opt_uint(a, "affection", 0, 100, 0)
        res = opt_uint(a, "resentment", 0, 100, 0)
        conf = opt_bool(a, "confidential", False)
        urg = opt_uint(a, "urgency", 0, 100, 0)
        dl = None if a.get("deadline_hours") is None else uint(a["deadline_hours"], 0, 1_000_000)
        rows = a.get("bystanders")
        rows = [] if rows is None else rows
        need(isinstance(rows, list) and len(rows) <= 16)
        seen, untrusted = set(), []
        for x in rows:
            keys(x, ["being"], ["trusted", "earshot"])
            w = ident(x["being"])
            need(w not in seen)
            seen.add(w)
            if opt_bool(x, "earshot", True) and not opt_bool(x, "trusted", False):
                untrusted.append(w)
        untrusted.sort()
        score = trust + aff // 2 - res
        coop = ("eager" if score >= 70 else "cooperative" if score >= 20 else "guarded" if score >= -10
                else "reluctant" if score >= -40 else "deflecting" if score >= -70 else "hostile_refusal")
        if not conf or not untrusted:
            privacy = "open"
        elif trust >= 60:
            privacy = "whispered"
        else:
            privacy = "deflect_due_to_bystanders"
        n = 4 if urg < 30 else 3 if urg < 60 else 2 if urg < 85 else 1
        if dl is not None and dl <= 1:
            n = 1
        verbosity = "full" if n == 4 else "brief" if n == 1 else "normal"
        return {"cooperativeness": coop, "max_sentences": n, "privacy": privacy, "score": score,
                "untrusted_in_earshot": untrusted, "verbosity": verbosity}

    def workflow(self, request: Any) -> dict:
        table = {"lexicon": self.op_lexicon, "parse": self.op_parse, "validate": self.op_validate,
                 "agenda": self.op_agenda, "tick": self.op_tick, "open": self.op_open,
                 "interrupt": self.op_interrupt, "resume": self.op_resume, "close": self.op_close,
                 "threads": self.op_threads, "stance": self.op_stance}

        def f(op: str, args: Any) -> dict:
            need(op in table and isinstance(args, dict))
            return table[op](args)
        return run_limited(request, self.lim, f)


def workflow(request: Any) -> dict:
    return Pragmatics().workflow(request)
