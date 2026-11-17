const assert = require('node:assert/strict');
const { execFileSync } = require('node:child_process');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { test } = require('node:test');
const reporter = process.env.BLOOMERY_REPORT_MODULE;
const { summarize, renderHtml } = require(reporter);

function nativeReport(root, duration) {
  return {
    config: { rootDir: root, workers: duration, metadata: { host: root } },
    stats: { startTime: new Date(duration).toISOString(), duration, expected: 1, unexpected: 0, skipped: 0, flaky: 0 },
    suites: [{
      title: 'example.spec.ts',
      suites: [{
        title: 'nested suite',
        specs: [{
          title: '<browser & test>', file: `${root}/example.spec.ts`, line: 5, column: 1,
          id: root, tags: ['b', 'a'],
          tests: [{
            projectName: 'chromium', expectedStatus: 'passed', status: 'expected',
            results: [{
              duration, startTime: new Date(duration).toISOString(), workerIndex: duration,
              stdout: [{ text: root }], attachments: [{ path: `${root}/random.png` }],
            }],
          }],
        }],
      }],
    }],
  };
}

test('runtime metadata does not affect published bytes', () => {
  const first = nativeReport('/build/first/source', 100);
  const second = nativeReport('/private/tmp/another/source', 9876);
  second.suites[0].suites[0].specs[0].tags.reverse();
  const left = summarize(first);
  const right = summarize(second);
  assert.equal(JSON.stringify(left), JSON.stringify(right));
  assert.equal(renderHtml(left), renderHtml(right));
  assert.equal(left.tests[0].file, 'example.spec.ts');
  assert.deepEqual(left.tests[0].title, ['example.spec.ts', 'nested suite', '<browser & test>']);
  assert.equal('duration' in left.stats, false);
});

test('declarations and outcomes remain meaningful and HTML is escaped', () => {
  const original = nativeReport('/build/source', 100);
  const changed = structuredClone(original);
  changed.stats.skipped = 1;
  changed.stats.expected = 0;
  const test = changed.suites[0].suites[0].specs[0].tests[0];
  test.expectedStatus = 'skipped';
  test.status = 'skipped';
  const report = summarize(changed);
  assert.notDeepEqual(summarize(original), report);
  assert.equal(report.tests[0].expectedStatus, 'skipped');
  assert.equal(report.tests[0].status, 'skipped');
  assert.match(renderHtml(report), /&lt;browser &amp; test&gt;/);
  assert.doesNotMatch(renderHtml(report), /<browser/);
});

test('suite completion order does not affect the report', () => {
  const first = nativeReport('/build/source', 100);
  const other = structuredClone(first.suites[0]);
  other.title = 'another.spec.ts';
  first.suites.push(other);
  const second = structuredClone(first);
  second.suites.reverse();
  assert.deepEqual(summarize(first), summarize(second));
});

test('CLI writes only stable JSON and HTML summaries', () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'bloomery-report-'));
  try {
    for (const [name, root, duration] of [['first', '/build/source', 100], ['second', '/other/source', 9000]]) {
      const input = path.join(directory, `${name}.json`);
      fs.writeFileSync(input, JSON.stringify(nativeReport(root, duration)));
      execFileSync(process.execPath, [reporter, input, path.join(directory, name)]);
    }
    for (const file of ['results.json', 'html/index.html']) {
      assert.deepEqual(fs.readFileSync(path.join(directory, 'first', file)), fs.readFileSync(path.join(directory, 'second', file)));
    }
    assert.deepEqual(fs.readdirSync(path.join(directory, 'first')).sort(), ['html', 'results.json']);
  } finally {
    fs.rmSync(directory, { recursive: true, force: true });
  }
});
