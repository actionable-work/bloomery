const http = require('node:http');

// Readiness must wait for the server, rather than relying on a fixed sleep.
setTimeout(() => {
  http.createServer((request, response) => {
    response.setHeader('content-type', 'text/html');
    response.end(`<h1>${process.env.GREETING}</h1>`);
  }).listen(Number(process.env.PORT), '127.0.0.1');
}, 250);
