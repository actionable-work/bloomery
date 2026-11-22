import { test } from '@playwright/test';
import { rendezvous } from './rendezvous';

test('second file meets its sibling', async () => {
  await rendezvous('second');
});
