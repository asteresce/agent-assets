## Summary

Commit messages follow the Conventional Commits specification.

## Scope

Every commit the agent creates, on any branch. It does not govern merge
commits, history imported from elsewhere, or a message a human already wrote
before the agent was asked to help.

## Guidelines

Write the header as `<type>[optional scope][optional !]: <description>`.

Types, and what each means here:

- `feat` — new behaviour for whoever consumes the code.
- `fix` — a defect in existing behaviour.
- `docs` — documentation only.
- `style` — formatting only; no behaviour change.
- `refactor` — restructuring that changes no behaviour.
- `perf` — a performance improvement.
- `test` — adding or correcting tests.
- `build` — build system or dependency changes.
- `ci` — continuous-integration configuration.
- `chore` — maintenance that fits nothing above.
- `revert` — reverts a previous commit.

Header rules:

- `scope` is a noun naming the area touched (`parser`, `api`, `docs`). Omit it
  when the change spans the repository.
- Mark a breaking change with `!` before the colon — `feat(api)!: ...` — and
  describe it in a `BREAKING CHANGE:` footer.
- `description` is imperative mood, lowercase, no trailing period, and at most
  72 characters.

An optional body explains the motivation and contrasts it with the previous
behaviour. Optional footers carry metadata: `Refs: #123`,
`Co-authored-by: Name <email>`, and `BREAKING CHANGE: <what breaks and how to
migrate>`.

## Exceptions

- Merge commits and conflict-resolution commits carry no type.
- `revert` of a single commit uses the type with the original subject and a
  `This reverts commit <sha>.` body.
- `fixup!` and `squash!` commits during a session are allowed and must be
  squashed before the branch lands.
- The first commit of a repository may use `chore: initial commit`.

## Examples

Good:

    feat(parser): support quoted scopes
    fix(api)!: remove the deprecated list endpoint

    BREAKING CHANGE: `list` is gone; use `search` with an empty query.

    refactor(engine): hoist the config loader
    test(queue): cover the retry ceiling

Bad:

- `fixed stuff` — no type, no description discipline.
- `feat: Added New Thing.` — past tense, capitalised, trailing period.
- `update` — not a Conventional Commits type.

## Rationale

Typed history makes commits machine-readable: changelogs generate themselves,
semantic versions infer from the types (`feat` is a minor bump, `fix` a patch),
and `git bisect` can skip commits that provably change nothing. A uniform
subject line is also the cheapest possible review surface.

## References

- Conventional Commits 1.0.0: https://www.conventionalcommits.org/en/v1.0.0/
- Semantic Versioning 2.0.0: https://semver.org/
