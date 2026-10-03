"""Normalise consumer-supplied config into a canonical in-memory shape.

The config can arrive in many forms (strings shorthand for imports,
omitted `root`, optional injections). Normalisation:
  - expands string shorthand in `imports` to `{name = ...}`
  - fills in the default `root` per category
  - returns a fresh dict so callers can mutate freely.
"""

from __future__ import annotations

from .constants import DEFAULT_ROOT_TEMPLATE


def normalize_import(imp) -> dict:
    if isinstance(imp, str):
        return {"name": imp}
    return dict(imp)


def normalize_category(category: str, cfg: dict | None) -> dict | None:
    """Return a normalised category config or `None` if the consumer
    omitted the category entirely."""
    if not cfg:
        return None
    out = dict(cfg)
    out.setdefault("root", DEFAULT_ROOT_TEMPLATE.format(category=category))
    out["imports"] = [normalize_import(i) for i in out.get("imports") or []]
    out.setdefault("injections", {})
    return out


def normalize_config(config: dict | None) -> dict:
    """Return a normalised full config (every known category present,
    even if empty)."""
    config = config or {}
    out = {}
    for category in ("rules", "skills", "agents"):
        cat = normalize_category(category, config.get(category))
        if cat is not None:
            out[category] = cat
    return out