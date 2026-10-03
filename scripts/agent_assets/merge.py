"""Recursive merge with pluggable list policy.

Used by both frontmatter injection (where lists are deduped) and JSON
injection (where lists are concatenated). Other data shapes fall through
unchanged.
"""

from __future__ import annotations
from typing import Literal

ListPolicy = Literal["concat", "dedupe"]


def deep_merge(a, b, *, list_policy: ListPolicy = "concat"):
    """Recursively merge `b` into `a`.

    - dicts: keys are merged recursively.
    - lists: concatenated (or deduped when `list_policy="dedupe"`).
    - everything else: `b` wins.
    """
    if isinstance(a, dict) and isinstance(b, dict):
        out = dict(a)
        for k, v in b.items():
            if k in out:
                out[k] = deep_merge(out[k], v, list_policy=list_policy)
            else:
                out[k] = v
        return out
    if isinstance(a, list) and isinstance(b, list):
        if list_policy == "dedupe":
            seen = []
            merged = []
            for item in a + b:
                if item not in seen:
                    seen.append(item)
                    merged.append(item)
            return merged
        return a + b
    return b