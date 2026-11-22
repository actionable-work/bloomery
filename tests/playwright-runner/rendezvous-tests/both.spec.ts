import { test } from '@playwright/test';
import { rendezvous } from './rendezvous';

test('first test meets its sibling', async () => {
  await rendezvous('first');
});

test('second test meets its sibling', async () => {
  await rendezvous('second');
});
