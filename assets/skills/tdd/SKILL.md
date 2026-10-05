## Description

Test-driven development: write a failing test first, then the least code that
passes it, then restructure with the suite green. The loop is red, green,
refactor.

## When to Use

When implementing new behaviour, and when fixing a bug — there, begin with a
test that reproduces it. Skip the loop for throwaway experiments and for
changes that genuinely cannot be observed by a test.

## Prerequisites

A test command that completes in seconds, and a way to run a single test.
Without a fast feedback loop the discipline degrades into guessing.

## Inputs

The behaviour to produce, stated as something observable from the outside: a
return value, a side effect, an error. Also the command that runs the tests.

## Steps

1. **Red.** Write one test for the next piece of behaviour. Run it and watch it
   fail. If it passes, the test is not yet measuring the behaviour — fix the
   test before writing any production code.
2. **Green.** Write the least code that makes that test pass. Resist improving
   anything the test does not demand; the next cycle will ask for it.
3. **Refactor.** With the suite green, remove duplication and sharpen names.
   No new behaviour, and the suite must stay green.

Repeat until the behaviour is complete, one test at a time.

## Output

A passing suite, the implementation, and a test that pins the behaviour so a
later change cannot silently drop it.

## Verification

The full suite is green, and the new test fails when the change it covers is
reverted — that is what proves the test is load-bearing.

## Pitfalls

- Writing several failing tests before the first green: the implementation then
  chases a specification instead of a single step.
- Asserting on internal calls instead of observable behaviour: the test then
  resists refactoring rather than enabling it.
- Mocking code you own. Mocks belong at boundaries — clocks, network,
  filesystem — and nowhere else.
- Skipping the refactor phase: the code reaches green and stays awkward, and
  the next cycle costs more than the one before it.

## Examples

Red: `parse("")` currently raises, so write `assert parse("") == []` and watch
it fail. Green: return `[]` for empty input and nothing else. Refactor: the
early return now resembles the guard in `parse_line`, so extract the shared
check and run the suite.

## Bundled Files

`checklist.md` — the cycle and its stop conditions on one page.

## References

- Kent Beck, *Test-Driven Development by Example* (2002).
