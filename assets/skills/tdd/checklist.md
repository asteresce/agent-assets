# Red, green, refactor

## Red
- Write exactly one failing test.
- Run it. It must fail, and for the right reason.
- If it passes, fix the test. Do not write production code yet.

## Green
- Write the least code that makes it pass.
- Fix nothing the test does not demand.
- Run it. It must pass.

## Refactor
- Remove duplication; improve names and structure.
- Add no behaviour.
- Run the whole suite. It must stay green.

Then repeat, one test at a time, until the behaviour is complete.
