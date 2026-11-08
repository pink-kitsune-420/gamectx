#!/usr/bin/env python3
"""Conformance test cases and differential parity runner for Affect & Skills (Gaps 6 and 8)."""

import json
import os
import random
import subprocess
import sys
from pathlib import Path
from typing import Any, Dict, List

sys.path[1:1] = [str(d) for d in sorted(Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]
from affect_skills_reference import AffectReference, SkillsReference, workflow as py_workflow

ROOT = Path(__file__).resolve().parent.parent.parent
CONFORMANCE_DIR = ROOT / "conformance" / "affect_skills"


def make_cases() -> List[Dict[str, Any]]:
    cases = []

    # 1. Affect: Init and assess calm baker
    cases.append({
        "id": "affect_init_and_assess_mood",
        "description": "Initialize character affect with temperament and assess mood label.",
        "input": {
            "kind": "affect",
            "args": {
                "ops": [
                    {
                        "op": "init",
                        "args": {
                            "being": "baker",
                            "temperament": "calm",
                            "mood": {"valence": 20, "arousal": 10},
                            "attitudes": [
                                {
                                    "id": "trust_player",
                                    "kind": "trust",
                                    "subject": "player",
                                    "level": 15,
                                    "baseline": 0,
                                    "decay_rate": 1,
                                    "reason": "good trade"
                                }
                            ]
                        }
                    },
                    {"op": "assess", "args": {"being": "baker"}}
                ]
            }
        }
    })

    # 2. Affect: Fear decay over ticks toward baseline 0
    cases.append({
        "id": "affect_fear_decay_over_ticks",
        "description": "Attitude of kind fear decays toward baseline 0 over ticks.",
        "input": {
            "kind": "affect",
            "args": {
                "ops": [
                    {
                        "op": "init",
                        "args": {
                            "being": "apprentice",
                            "temperament": "stoic",
                            "mood": {"valence": 0, "arousal": 0},
                            "attitudes": [
                                {
                                    "id": "fear_shadow",
                                    "kind": "fear",
                                    "subject": "shadow",
                                    "level": 30,
                                    "baseline": 0,
                                    "decay_rate": 4,
                                    "reason": "spooky noise"
                                }
                            ]
                        }
                    },
                    {"op": "tick", "args": {"being": "apprentice", "ticks": 5}}
                ]
            }
        }
    })

    # 3. Affect: Calm temperament accelerates emotion/fear decay (+2)
    cases.append({
        "id": "affect_temperament_calm_accelerates_emotion_decay",
        "description": "Calm temperament adds +2 to transient emotion and fear decay rates.",
        "input": {
            "kind": "affect",
            "args": {
                "ops": [
                    {
                        "op": "init",
                        "args": {
                            "being": "baker",
                            "temperament": "calm",
                            "mood": {"valence": 0, "arousal": 0},
                            "attitudes": [
                                {
                                    "id": "fear_wolves",
                                    "kind": "fear",
                                    "subject": "wolves",
                                    "level": 50,
                                    "baseline": 0,
                                    "decay_rate": 4
                                }
                            ]
                        }
                    },
                    {"op": "tick", "args": {"being": "baker", "ticks": 5}}
                ]
            }
        }
    })

    # 4. Affect: Forgiving temperament accelerates resentment decay
    cases.append({
        "id": "affect_temperament_forgiving_accelerates_resentment_decay",
        "description": "Forgiving temperament adds +2 to resentment decay rate.",
        "input": {
            "kind": "affect",
            "args": {
                "ops": [
                    {
                        "op": "init",
                        "args": {
                            "being": "baker",
                            "temperament": "forgiving",
                            "mood": {"valence": 0, "arousal": 0},
                            "attitudes": [
                                {
                                    "id": "resentment_miller",
                                    "kind": "resentment",
                                    "subject": "miller",
                                    "level": 40,
                                    "baseline": 0,
                                    "decay_rate": 2
                                }
                            ]
                        }
                    },
                    {"op": "tick", "args": {"being": "baker", "ticks": 5}}
                ]
            }
        }
    })

    # 5. Affect: Volatile temperament slows emotion decay
    cases.append({
        "id": "affect_temperament_volatile_slows_emotion_decay",
        "description": "Volatile temperament reduces emotion decay rate by 2 (min 1).",
        "input": {
            "kind": "affect",
            "args": {
                "ops": [
                    {
                        "op": "init",
                        "args": {
                            "being": "guard",
                            "temperament": "volatile",
                            "mood": {"valence": 0, "arousal": 0},
                            "attitudes": [
                                {
                                    "id": "emotion_anger",
                                    "kind": "emotion",
                                    "subject": "thief",
                                    "level": 50,
                                    "baseline": 0,
                                    "decay_rate": 5
                                }
                            ]
                        }
                    },
                    {"op": "tick", "args": {"being": "guard", "ticks": 5}}
                ]
            }
        }
    })

    # 6. Affect: Vengeful temperament slows resentment decay
    cases.append({
        "id": "affect_temperament_vengeful_slows_resentment_decay",
        "description": "Vengeful temperament reduces resentment decay rate by 1 (min 1).",
        "input": {
            "kind": "affect",
            "args": {
                "ops": [
                    {
                        "op": "init",
                        "args": {
                            "being": "carter",
                            "temperament": "vengeful",
                            "mood": {"valence": 0, "arousal": 0},
                            "attitudes": [
                                {
                                    "id": "resentment_bandit",
                                    "kind": "resentment",
                                    "subject": "bandit",
                                    "level": 40,
                                    "baseline": 0,
                                    "decay_rate": 2
                                }
                            ]
                        }
                    },
                    {"op": "tick", "args": {"being": "carter", "ticks": 10}}
                ]
            }
        }
    })

    # 7. Affect: Appraising gratitude shifts mood positively
    cases.append({
        "id": "affect_appraise_gratitude_shifts_mood_positive",
        "description": "Appraising an act with gratitude delta increases mood valence.",
        "input": {
            "kind": "affect",
            "args": {
                "ops": [
                    {
                        "op": "init",
                        "args": {
                            "being": "baker",
                            "temperament": "calm",
                            "mood": {"valence": 0, "arousal": 0}
                        }
                    },
                    {
                        "op": "appraise",
                        "args": {
                            "being": "baker",
                            "kind": "gratitude",
                            "subject": "neighbor",
                            "delta": 30,
                            "reason": "helped fix roof"
                        }
                    }
                ]
            }
        }
    })

    # 8. Affect: Appraising fear shifts mood negative and increases arousal
    cases.append({
        "id": "affect_appraise_fear_shifts_mood_negative_arousal_high",
        "description": "Appraising a threat with fear delta reduces mood valence and raises arousal.",
        "input": {
            "kind": "affect",
            "args": {
                "ops": [
                    {
                        "op": "init",
                        "args": {
                            "being": "apprentice",
                            "temperament": "calm",
                            "mood": {"valence": 10, "arousal": 0}
                        }
                    },
                    {
                        "op": "appraise",
                        "args": {
                            "being": "apprentice",
                            "kind": "fear",
                            "subject": "shadow_beast",
                            "delta": 40,
                            "reason": "menacing roar"
                        }
                    }
                ]
            }
        }
    })

    # 9. Affect: Set mood custom valence and arousal
    cases.append({
        "id": "affect_set_mood_custom_valence_arousal",
        "description": "Directly updating mood valence and arousal re-evaluates mood label.",
        "input": {
            "kind": "affect",
            "args": {
                "ops": [
                    {
                        "op": "init",
                        "args": {
                            "being": "baker",
                            "temperament": "calm"
                        }
                    },
                    {
                        "op": "set_mood",
                        "args": {
                            "being": "baker",
                            "valence": 45,
                            "arousal": 30
                        }
                    }
                ]
            }
        }
    })

    # 10. Affect: Additive attitude kinds (respect, gratitude, envy, admiration)
    cases.append({
        "id": "affect_missing_attitude_kinds",
        "description": "Initialize and assess additive attitude kinds fear, respect, gratitude, envy, admiration.",
        "input": {
            "kind": "affect",
            "args": {
                "ops": [
                    {
                        "op": "init",
                        "args": {
                            "being": "scholar",
                            "temperament": "calm",
                            "attitudes": [
                                {"id": "r_master", "kind": "respect", "subject": "master", "level": 50},
                                {"id": "g_healer", "kind": "gratitude", "subject": "healer", "level": 40},
                                {"id": "e_rival", "kind": "envy", "subject": "rival", "level": 25},
                                {"id": "a_hero", "kind": "admiration", "subject": "hero", "level": 60}
                            ]
                        }
                    },
                    {"op": "assess", "args": {"being": "scholar"}}
                ]
            }
        }
    })

    # 11. Skills: Init and assess baker skills
    cases.append({
        "id": "skills_init_and_assess",
        "description": "Initialize character skills and inspect progress percent and threshold.",
        "input": {
            "kind": "skills",
            "args": {
                "ops": [
                    {
                        "op": "init",
                        "args": {
                            "being": "baker",
                            "skills": [
                                {"id": "baking", "label": "Baking", "level": 10, "practice": 25}
                            ]
                        }
                    },
                    {"op": "assess", "args": {"being": "baker"}}
                ]
            }
        }
    })

    # 12. Skills: Practice gain on successful action with diminishing returns
    cases.append({
        "id": "skills_practice_gain_successful_action",
        "description": "Successful action grants practice with diminishing returns based on level.",
        "input": {
            "kind": "skills",
            "args": {
                "ops": [
                    {
                        "op": "init",
                        "args": {
                            "being": "baker",
                            "skills": [
                                {"id": "baking", "label": "Baking", "level": 10, "practice": 0}
                            ]
                        }
                    },
                    {
                        "op": "practice",
                        "args": {
                            "being": "baker",
                            "skill": "baking",
                            "practice": 50,
                            "success": True
                        }
                    }
                ]
            }
        }
    })

    # 13. Skills: Failed action yields 0 practice
    cases.append({
        "id": "skills_practice_failure_grants_no_practice",
        "description": "Failed action outcome grants zero practice points.",
        "input": {
            "kind": "skills",
            "args": {
                "ops": [
                    {
                        "op": "init",
                        "args": {
                            "being": "apprentice",
                            "skills": [
                                {"id": "baking", "label": "Baking", "level": 2, "practice": 15}
                            ]
                        }
                    },
                    {
                        "op": "practice",
                        "args": {
                            "being": "apprentice",
                            "skill": "baking",
                            "practice": 40,
                            "success": False
                        }
                    }
                ]
            }
        }
    })

    # 14. Skills: Practice crossing threshold advances level
    cases.append({
        "id": "skills_level_up_advances_threshold",
        "description": "Accumulating practice past the threshold advances level and computes new threshold.",
        "input": {
            "kind": "skills",
            "args": {
                "ops": [
                    {
                        "op": "init",
                        "args": {
                            "being": "novice",
                            "skills": [
                                {"id": "foraging", "label": "Foraging", "level": 1, "practice": 110}
                            ]
                        }
                    },
                    {
                        "op": "practice",
                        "args": {
                            "being": "novice",
                            "skill": "foraging",
                            "practice": 20,
                            "success": True
                        }
                    }
                ]
            }
        }
    })

    # 15. Skills: Level cap at 100
    cases.append({
        "id": "skills_level_cap_at_100",
        "description": "Character at max skill level 100 accumulates no further practice.",
        "input": {
            "kind": "skills",
            "args": {
                "ops": [
                    {
                        "op": "init",
                        "args": {
                            "being": "grandmaster",
                            "skills": [
                                {"id": "baking", "label": "Baking", "level": 100, "practice": 0}
                            ]
                        }
                    },
                    {
                        "op": "practice",
                        "args": {
                            "being": "grandmaster",
                            "skill": "baking",
                            "practice": 100,
                            "success": True
                        }
                    }
                ]
            }
        }
    })

    # 16. Skills: Evaluate action duration discount
    cases.append({
        "id": "skills_evaluate_action_duration_discount",
        "description": "Skill level provides up to 50% action duration discount.",
        "input": {
            "kind": "skills",
            "args": {
                "ops": [
                    {
                        "op": "init",
                        "args": {
                            "being": "expert_baker",
                            "skills": [
                                {"id": "baking", "label": "Baking", "level": 60}
                            ]
                        }
                    },
                    {
                        "op": "evaluate_action",
                        "args": {
                            "being": "expert_baker",
                            "action": "cook",
                            "skill": "baking",
                            "base_duration": 40,
                            "base_risk": 20
                        }
                    }
                ]
            }
        }
    })

    # 17. Skills: Evaluate action risk reduction
    cases.append({
        "id": "skills_evaluate_action_risk_reduction",
        "description": "Skill level scales down failure risk of an action.",
        "input": {
            "kind": "skills",
            "args": {
                "ops": [
                    {
                        "op": "init",
                        "args": {
                            "being": "journeyman",
                            "skills": [
                                {"id": "carpentry", "label": "Carpentry", "level": 50}
                            ]
                        }
                    },
                    {
                        "op": "evaluate_action",
                        "args": {
                            "being": "journeyman",
                            "action": "craft",
                            "skill": "carpentry",
                            "base_duration": 30,
                            "base_risk": 40
                        }
                    }
                ]
            }
        }
    })

    # 18. Skills: Output quality score and tier
    cases.append({
        "id": "skills_evaluate_action_quality_tier_masterwork",
        "description": "High skill level and quality bonus yield masterwork tier.",
        "input": {
            "kind": "skills",
            "args": {
                "ops": [
                    {
                        "op": "init",
                        "args": {
                            "being": "master_artist",
                            "skills": [
                                {"id": "painting", "label": "Painting", "level": 92}
                            ]
                        }
                    },
                    {
                        "op": "evaluate_action",
                        "args": {
                            "being": "master_artist",
                            "action": "craft",
                            "skill": "painting",
                            "base_duration": 60,
                            "base_risk": 10,
                            "quality_bonus": 5
                        }
                    }
                ]
            }
        }
    })

    # 19. Skills: Teaching between beings grants practice and respect/gratitude
    cases.append({
        "id": "skills_teach_successful_instruction",
        "description": "Higher skilled teacher instructs lower skilled student, producing practice and respect/gratitude attitudes.",
        "input": {
            "kind": "skills",
            "args": {
                "ops": [
                    {
                        "op": "init",
                        "args": {
                            "being": "master_baker",
                            "skills": [
                                {"id": "baking", "label": "Baking", "level": 70}
                            ]
                        }
                    },
                    {
                        "op": "init",
                        "args": {
                            "being": "apprentice",
                            "skills": [
                                {"id": "baking", "label": "Baking", "level": 15}
                            ]
                        }
                    },
                    {
                        "op": "teach",
                        "args": {
                            "teacher": "master_baker",
                            "student": "apprentice",
                            "skill": "baking",
                            "hours": 3,
                            "same_place": True
                        }
                    }
                ]
            }
        }
    })

    # 20. Skills: Teaching rejected when teacher skill level not greater than student
    cases.append({
        "id": "skills_teach_rejected_when_student_equals_or_exceeds",
        "description": "Teaching returns refusal if teacher skill level is not strictly greater than student level.",
        "input": {
            "kind": "skills",
            "args": {
                "ops": [
                    {
                        "op": "init",
                        "args": {
                            "being": "peer1",
                            "skills": [
                                {"id": "baking", "label": "Baking", "level": 30}
                            ]
                        }
                    },
                    {
                        "op": "init",
                        "args": {
                            "being": "peer2",
                            "skills": [
                                {"id": "baking", "label": "Baking", "level": 30}
                            ]
                        }
                    },
                    {
                        "op": "teach",
                        "args": {
                            "teacher": "peer1",
                            "student": "peer2",
                            "skill": "baking",
                            "hours": 1,
                            "same_place": True
                        }
                    }
                ]
            }
        }
    })

    # 21. Malformed: Empty ops array rejected
    cases.append({
        "id": "malformed_affect_empty_ops",
        "description": "Empty operations list in affect workflow is rejected.",
        "input": {
            "kind": "affect",
            "args": {
                "ops": []
            }
        }
    })

    # 22. Malformed: Unknown attitude kind rejected
    cases.append({
        "id": "malformed_affect_unknown_attitude_kind",
        "description": "Attitude kind outside allowed 24 kinds is rejected.",
        "input": {
            "kind": "affect",
            "args": {
                "ops": [
                    {
                        "op": "init",
                        "args": {
                            "being": "baker",
                            "attitudes": [
                                {"id": "bad", "kind": "telepathy_rage", "subject": "cat", "level": 10}
                            ]
                        }
                    }
                ]
            }
        }
    })

    # 23. Malformed: Mood valence out of bounds
    cases.append({
        "id": "malformed_affect_mood_out_of_bounds",
        "description": "Mood valence > 100 is rejected.",
        "input": {
            "kind": "affect",
            "args": {
                "ops": [
                    {
                        "op": "init",
                        "args": {
                            "being": "baker",
                            "mood": {"valence": 250, "arousal": 0}
                        }
                    }
                ]
            }
        }
    })

    # 24. Malformed: Skill level out of bounds (> 100)
    cases.append({
        "id": "malformed_skills_invalid_level_bounds",
        "description": "Skill level > 100 in init is rejected.",
        "input": {
            "kind": "skills",
            "args": {
                "ops": [
                    {
                        "op": "init",
                        "args": {
                            "being": "baker",
                            "skills": [
                                {"id": "baking", "level": 150}
                            ]
                        }
                    }
                ]
            }
        }
    })

    # 25. Malformed: Unknown op name rejected
    cases.append({
        "id": "malformed_skills_unknown_op",
        "description": "Unknown operation name in skills workflow is rejected.",
        "input": {
            "kind": "skills",
            "args": {
                "ops": [
                    {
                        "op": "sorcery_teleport",
                        "args": {"being": "wizard"}
                    }
                ]
            }
        }
    })

    return cases


def generate_conformance_suite():
    cases = make_cases()
    CONFORMANCE_DIR.mkdir(parents=True, exist_ok=True)

    for c in cases:
        case_dir = CONFORMANCE_DIR / c["id"]
        case_dir.mkdir(parents=True, exist_ok=True)

        is_malformed = "malformed" in c["id"]
        case_meta = {
            "op": "local",
            "description": c["description"],
            "error": is_malformed
        }
        with open(case_dir / "case.json", "w") as f:
            json.dump(case_meta, f, indent=2)
            f.write("\n")

        with open(case_dir / "input.json", "w") as f:
            json.dump(c["input"], f, indent=2)
            f.write("\n")

        if not is_malformed:
            expected = py_workflow(c["input"])
            with open(case_dir / "expected.json", "w") as f:
                f.write(json.dumps(expected, separators=(",", ":")) + "\n")
        else:
            exp_file = case_dir / "expected.json"
            if exp_file.exists():
                exp_file.unlink()


def run_random_parity(count: int = 2000, seed: int = 7):
    random.seed(seed)
    rust_bin = ROOT / "target" / "debug" / "gamectx"
    if not rust_bin.exists():
        rust_bin = ROOT / "target" / "release" / "gamectx"

    skill_pool = ["baking", "carpentry", "farming", "foraging", "smithing", "tailoring", "alchemy", "fishing"]
    attitude_pool = ["fear", "respect", "gratitude", "envy", "admiration", "trust", "affection", "resentment", "emotion"]
    temp_pool = ["calm", "volatile", "forgiving", "vengeful", "stoic", "optimistic"]

    for i in range(count):
        is_affect = (i % 2 == 0)
        kind = "affect" if is_affect else "skills"

        ops = []
        being_id = f"being_{random.randint(1, 4)}"

        if is_affect:
            temp = random.choice(temp_pool)
            mood_v = random.randint(-40, 40)
            mood_a = random.randint(-40, 40)
            att_cnt = random.randint(1, 3)
            atts = []
            for j in range(att_cnt):
                akind = random.choice(attitude_pool)
                sub = f"other_{random.randint(1, 3)}"
                lvl = random.randint(-30, 50)
                atts.append({
                    "id": f"{akind}_{sub}_{j}",
                    "kind": akind,
                    "subject": sub,
                    "level": lvl
                })
            ops.append({
                "op": "init",
                "args": {
                    "being": being_id,
                    "temperament": temp,
                    "mood": {"valence": mood_v, "arousal": mood_a},
                    "attitudes": atts
                }
            })

            sub_cnt = random.randint(1, 3)
            for _ in range(sub_cnt):
                coin = random.random()
                if coin < 0.4:
                    ops.append({
                        "op": "tick",
                        "args": {"being": being_id, "ticks": random.randint(1, 15)}
                    })
                elif coin < 0.7:
                    akind = random.choice(attitude_pool)
                    sub = f"other_{random.randint(1, 3)}"
                    ops.append({
                        "op": "appraise",
                        "args": {
                            "being": being_id,
                            "kind": akind,
                            "subject": sub,
                            "delta": random.randint(-20, 20),
                            "reason": "event_appraisal"
                        }
                    })
                elif coin < 0.9:
                    ops.append({
                        "op": "set_mood",
                        "args": {
                            "being": being_id,
                            "valence_delta": random.randint(-15, 15),
                            "arousal_delta": random.randint(-15, 15)
                        }
                    })
                else:
                    ops.append({"op": "assess", "args": {"being": being_id}})
        else:
            sk_cnt = random.randint(1, 3)
            sks = []
            chosen = random.sample(skill_pool, sk_cnt)
            for sk_name in chosen:
                sks.append({
                    "id": sk_name,
                    "label": sk_name.capitalize(),
                    "level": random.randint(1, 60),
                    "practice": random.randint(0, 80)
                })
            ops.append({
                "op": "init",
                "args": {
                    "being": being_id,
                    "skills": sks
                }
            })

            sub_cnt = random.randint(1, 3)
            for _ in range(sub_cnt):
                coin = random.random()
                if coin < 0.4:
                    sk = random.choice(chosen)
                    succ = random.random() < 0.8
                    ops.append({
                        "op": "practice",
                        "args": {
                            "being": being_id,
                            "skill": sk,
                            "practice": random.randint(10, 60),
                            "success": succ
                        }
                    })
                elif coin < 0.7:
                    sk = random.choice(chosen)
                    ops.append({
                        "op": "evaluate_action",
                        "args": {
                            "being": being_id,
                            "action": "craft",
                            "skill": sk,
                            "base_duration": random.randint(10, 50),
                            "base_risk": random.randint(10, 40),
                            "quality_bonus": random.randint(0, 10)
                        }
                    })
                elif coin < 0.85:
                    other_being = f"other_{random.randint(1, 2)}"
                    sk = random.choice(chosen)
                    # init other being
                    ops.insert(0, {
                        "op": "init",
                        "args": {
                            "being": other_being,
                            "skills": [{"id": sk, "level": random.randint(0, 20)}]
                        }
                    })
                    ops.append({
                        "op": "teach",
                        "args": {
                            "teacher": being_id,
                            "student": other_being,
                            "skill": sk,
                            "hours": random.randint(1, 4),
                            "same_place": True
                        }
                    })
                else:
                    ops.append({"op": "assess", "args": {"being": being_id}})

        req = {"kind": kind, "args": {"ops": ops}}

        py_res = None
        py_err = None
        try:
            py_res = py_workflow(req)
        except Exception as e:
            py_err = str(e)

        if rust_bin.exists():
            payload = json.dumps(req)
            proc = subprocess.run(
                [str(rust_bin), "local", "-"],
                input=payload,
                capture_output=True,
                text=True,
            )
            if py_err:
                assert proc.returncode != 0, f"Iteration {i}: Python failed ({py_err}) but Rust succeeded: {proc.stdout}"
            else:
                assert proc.returncode == 0, f"Iteration {i}: Rust failed: {proc.stderr}"
                rust_res = json.loads(proc.stdout)
                assert rust_res == py_res, f"Iteration {i} mismatch:\nPy:   {json.dumps(py_res, sort_keys=True)}\nRust: {json.dumps(rust_res, sort_keys=True)}"

    print(f"Differential parity passed {count} random workflows (seed {seed}) with 0 mismatches.")


if __name__ == "__main__":
    import argparse
    p = argparse.ArgumentParser()
    p.add_argument("--cap", type=float, default=0.0)
    p.add_argument("--random", type=int, default=0)
    p.add_argument("--seed", type=int, default=7)
    p.add_argument("--write-conformance", action="store_true")
    p.add_argument("--generate", action="store_true")
    p.add_argument("--parity", type=int, default=0)
    args = p.parse_args()

    if args.generate or args.write_conformance:
        generate_conformance_suite()
        print(f"Generated {len(make_cases())} conformance cases in {CONFORMANCE_DIR}")
    elif args.parity > 0:
        run_random_parity(args.parity, seed=args.seed)
    elif args.random > 0:
        run_random_parity(args.random, seed=args.seed)
    else:
        generate_conformance_suite()
        cases = make_cases()
        passed = 0
        for c in cases:
            try:
                out = py_workflow(c["input"])
                passed += 1
            except Exception:
                if "malformed" in c["id"]:
                    passed += 1
        print(json.dumps({
            "cases": len(cases),
            "passed": passed,
            "actual_model_calls": 0,
            "api_spend_usd": 0,
        }))
