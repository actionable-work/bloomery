const fs = require('node:fs');
const path = require('node:path');

// Publish declaration identities and outcomes. Native reports stay in the build
// directory: timings, stdout, attachments and host configuration vary per run.
function summarize(report) {
  const tests = [];
  function visit(suite, parents) {
    const titles = [...parents, suite.title];
    for (const spec of suite.specs || []) {
      for (const test of spec.tests) {
        tests.push({
          title: [...titles, spec.title],
          file: path.isAbsolute(spec.file)
            ? path.relative(report.config.rootDir, spec.file)
            : spec.file,
          line: spec.line,
          column: spec.column,
          project: test.projectName,
          tags: [...spec.tags].sort(),
          expectedStatus: test.expectedStatus,
          status: test.status,
        });
      }
    }
    for (const child of suite.suites || []) visit(child, titles);
  }
  for (const suite of report.suites) visit(suite, []);
  // Execution and worker completion order do not affect the published report.
  tests.sort((a, b) => {
    const left = JSON.stringify(a);
    const right = JSON.stringify(b);
    return left < right ? -1 : left > right ? 1 : 0;
  });
  const { expected, unexpected, skipped, flaky } = report.stats;
  return { version: 1, stats: { expected, unexpected, skipped, flaky }, tests };
}

function escapeHtml(value) {
  return String(value).replace(/[&<>"']/g, character => ({
    '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;',
  })[character]);
}

function renderHtml(report) {
  const rows = report.tests.map(test => {
    const cells = [
      test.title.join(' › '),
      test.project,
      `${test.file}:${test.line}:${test.column}`,
      test.expectedStatus,
      test.status,
    ];
    return `<tr>${cells.map(cell => `<td>${escapeHtml(cell)}</td>`).join('')}</tr>`;
  }).join('\n');
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Playwright results</title>
<style>body{font:16px system-ui;margin:2rem}table{border-collapse:collapse;width:100%}td,th{padding:.5rem;text-align:left;border-bottom:1px solid #ccc}</style>
</head>
<body>
<h1>Playwright results</h1>
<p>${report.stats.expected} expected · ${report.stats.unexpected} unexpected · ${report.stats.skipped} skipped · ${report.stats.flaky} flaky</p>
<table><thead><tr><th>Test</th><th>Project</th><th>Location</th><th>Expected</th><th>Outcome</th></tr></thead><tbody>
${rows}
</tbody></table>
</body>
</html>
`;
}

if (require.main === module) {
  const [input, output] = process.argv.slice(2);
  const report = summarize(JSON.parse(fs.readFileSync(input, 'utf8')));
  fs.mkdirSync(path.join(output, 'html'), { recursive: true });
  fs.writeFileSync(path.join(output, 'results.json'), JSON.stringify(report, null, 2) + '\n');
  fs.writeFileSync(path.join(output, 'html/index.html'), renderHtml(report));
}

module.exports = { summarize, renderHtml };
