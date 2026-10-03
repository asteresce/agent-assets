def deep_merge(a, b):
    if isinstance(a, dict) and isinstance(b, dict):
        out = dict(a)
        for k, v in b.items():
            if k in out:
                out[k] = deep_merge(out[k], v)
            else:
                out[k] = v
        return out
    if isinstance(a, list) and isinstance(b, list):
        return a + b
    return b


def chunk_present(target, chunk):
    if not isinstance(chunk, dict):
        return target == chunk
    if not isinstance(target, dict):
        return False
    for k, v in chunk.items():
        if k not in target:
            return False
        if isinstance(v, dict):
            if not chunk_present(target[k], v):
                return False
        elif isinstance(v, list):
            tv = target[k]
            if not isinstance(tv, list):
                return False
            for item in v:
                if item not in tv:
                    return False
        else:
            if target[k] != v:
                return False
    return True


def collect(config):
    """Walk every category's `injections.json` and deep-merge by `file` path.

    Returns a dict mapping target file path -> merged content dict.
    """
    grouped = {}
    for category in ("rules", "skills", "agents"):
        cat_cfg = (config or {}).get(category) or {}
        for entry in (cat_cfg.get("injections") or {}).get("json") or []:
            fp = entry["file"]
            content = entry.get("content") or {}
            if fp in grouped:
                grouped[fp] = deep_merge(grouped[fp], content)
            else:
                grouped[fp] = content
    return grouped