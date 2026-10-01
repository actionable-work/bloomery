# Manual review catalogue

`bloomery review` selects exactly the records for which `manual = true`. It
does not include automated records merely because they lack coverage; those are
errors owned by `bloomery check`.

Each selected item contains:

- service, feature, and group identity;
- the full requirement ID and title;
- the generated EARS statement;
- the resolved design reference, including an anchor when supplied.

## Ordering

The catalogue is deterministic and hierarchical:

1. service directory name, ascending;
2. feature directory name, ascending;
3. group name, ascending;
4. sequence, ascending numerically (`001` before `010`).

The original zero-padded sequence remains in the displayed ID. Sorting does not
alter identifiers or source documents.

Review reads the same validated requirement model as check. If the input model
is malformed, review reports parsing and linkage errors rather than presenting
a partially interpreted catalogue.
