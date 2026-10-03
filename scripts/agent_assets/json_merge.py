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