import os
import shutil

from . import frontmatter, hashing, lock


# Mirrors lib/schemas/formats.nix.
CATEGORIES = ("rules", "skills", "agents")
FILE_BASED = {"rules", "agents"}


def normalize_import(imp):
    if isinstance(imp, str):
        return {"name": imp}
    return dict(imp)


def resolve_out_path(category, imp, root):
    rename = imp.get("rename") or imp["name"]
    destination = imp.get("destination") or ""
    rel_dir = os.path.join(root, destination) if destination else root
    if category in FILE_BASED:
        return os.path.join(rel_dir, f"{rename}.md")
    return os.path.join(rel_dir, imp["name"])


def build(config, src_dir, out_dir):
    os.makedirs(out_dir, exist_ok=True)
    entries = []

    for category in CATEGORIES:
        cat_cfg = config.get(category)
        if not cat_cfg:
            continue
        root = cat_cfg["root"]
        category_injections = cat_cfg.get("injections") or {}
        category_frontmatter = category_injections.get("frontmatter") or {}

        for imp in cat_cfg["imports"]:
            imp = normalize_import(imp)
            name = imp["name"]
            if category in FILE_BASED:
                source = os.path.join(src_dir, category, f"{name}.md")
                if not os.path.exists(source):
                    raise FileNotFoundError(f"asset not found: {source}")
                rel_target = resolve_out_path(category, imp, root)
                target = os.path.join(out_dir, rel_target)
                with open(source, "r", encoding="utf-8") as f:
                    source_text = f.read()
                import_injections = imp.get("injections") or {}
                import_frontmatter = import_injections.get("frontmatter") or {}
                rendered = frontmatter.apply(source_text, category_frontmatter, import_frontmatter)
                os.makedirs(os.path.dirname(target) or ".", exist_ok=True)
                with open(target, "w", encoding="utf-8") as f:
                    f.write(rendered)
                digest = hashing.sha256_file(target)
                hash_value = hashing.to_nix_hash(digest)
                entries.append(lock.entry(category, name, rel_target, hash_value))
            else:
                source = os.path.join(src_dir, category, name)
                if not os.path.isdir(source):
                    raise FileNotFoundError(f"skill not found: {source}")
                rel_target = resolve_out_path(category, imp, root)
                target = os.path.join(out_dir, rel_target)
                if os.path.exists(target):
                    shutil.rmtree(target)
                shutil.copytree(source, target)
                skills_md = os.path.join(target, "SKILLS.md")
                if os.path.exists(skills_md):
                    with open(skills_md, "r", encoding="utf-8") as f:
                        source_text = f.read()
                    import_injections = imp.get("injections") or {}
                    import_frontmatter = import_injections.get("frontmatter") or {}
                    rendered = frontmatter.apply(source_text, category_frontmatter, import_frontmatter)
                    with open(skills_md, "w", encoding="utf-8") as f:
                        f.write(rendered)
                registry_files = hashing.list_registry_files(source)
                hash_value = hashing.dir_hash(target, registry_files)
                entries.append(lock.entry(category, name, rel_target, hash_value))

    lock_path = os.path.join(out_dir, "agent-assets.lock")
    lock.write_lock(lock_path, entries)
    return {"entries": entries, "lock_path": lock_path}