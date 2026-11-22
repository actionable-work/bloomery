import { test } from '@playwright/test';
import { rendezvous } from './rendezvous';

test('first file meets its sibling', async () => {
  await rendezvous('first');
});
