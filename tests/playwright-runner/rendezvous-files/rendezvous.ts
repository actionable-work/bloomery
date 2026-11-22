import { mkdirSync, readdirSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

// Passes only when both participants run at the same time: each announces
// itself and waits for the other.
export async function rendezvous(name: string): Promise<void> {
  const dir = join(tmpdir(), 'bloomery-rendezvous');
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, name), '');
  const deadline = Date.now() + 10000;
  while (Date.now() < deadline) {
    if (readdirSync(dir).length >= 2) return;
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  throw new Error('tests did not run concurrently');
}
