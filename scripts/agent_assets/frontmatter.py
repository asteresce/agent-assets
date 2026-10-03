import yaml


def split_frontmatter(source):
    if not source.startswith("---"):
        return {}, source
    rest = source[3:]
    end = rest.find("\n---")
    if end < 0:
        return {}, source
    head = rest[:end]
    body = rest[end + 4:]
    if body.startswith("\n"):
        body = body[1:]
    if body.startswith("\r\n"):
        body = body[2:]
    parsed = yaml.safe_load(head) if head.strip() else {}
    if parsed is None:
        parsed = {}
    if not isinstance(parsed, dict):
        parsed = {}
    return parsed, body


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
        seen = []
        merged = []
        for item in a + b:
            if item not in seen:
                seen.append(item)
                merged.append(item)
        return merged
    return b


def render(frontmatter, body):
    if not frontmatter:
        return body
    yaml_text = yaml.safe_dump(
        frontmatter,
        default_flow_style=False,
        allow_unicode=True,
        sort_keys=True,
        width=4096,
    )
    return f"---\n{yaml_text}---\n{body}"


def apply(source, category_frontmatter, import_frontmatter):
    source_fm, body = split_frontmatter(source)
    merged = deep_merge(deep_merge({}, source_fm), deep_merge(category_frontmatter or {}, import_frontmatter or {}))
    return render(merged, body)