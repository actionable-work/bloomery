# Playwright scanner

## Reference syntax

Playwright tests associate requirements through the configured tag prefix:

```typescript
test('renders the lockout banner', {
  tag: ['@bloomery:CLI-EXAMPLE-UI-001'],
}, async ({ page }) => {
  // test body is never executed by Bloomery
});
```

The scanner parses configured TypeScript or JavaScript files with
`oxc_parser`. It inspects `test(...)` and `test.describe(...)` call expressions,
then scans the options object for string tag values beginning with
`tag_prefix`. The prefix is removed before the ID is entered in the common
registry.

Tags in unrelated calls, comments, or arbitrary string literals are not
references. A malformed source file or invalid tag value is reported with its
source location.
