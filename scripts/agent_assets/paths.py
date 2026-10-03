import os


def default_root(category):
    return f"./.agent-assets/{category}"


def normalize_root(category, root):
    if root is None:
        return default_root(category)
    return root


def is_under(path, root):
    rel = os.path.relpath(path, root)
    return not rel.startswith("..") and not os.path.isabs(rel)


def normalize_import(imp):
    if isinstance(imp, str):
        return {"name": imp}
    return dict(imp)


def merge_injections(category_injections, import_injections):
    cat = (category_injections or {}).get("frontmatter") or {}
    imp = (import_injections or {}).get("frontmatter") or {}
    return deep_merge_dicts(cat, imp)


def deep_merge_dicts(a, b):
    out = {}
    for k, v in (a or {}).items():
        out[k] = v
    for k, v in (b or {}).items():
        if k in out and isinstance(out[k], dict) and isinstance(v, dict):
            out[k] = deep_merge_dicts(out[k], v)
        else:
            out[k] = v
    return out