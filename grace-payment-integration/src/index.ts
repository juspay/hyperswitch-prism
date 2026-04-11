import * as http from 'http';
import { authorize, refund, AuthorizeRequest, RefundRequest } from './payments';
import { SERVER_PORT } from './config';

function parseBody(req: http.IncomingMessage): Promise<any> {
  return new Promise((resolve, reject) => {
    const chunks: Buffer[] = [];
    req.on('data', (chunk) => chunks.push(chunk));
    req.on('end', () => {
      try {
        resolve(JSON.parse(Buffer.concat(chunks).toString()));
      } catch {
        reject(new Error('Invalid JSON body'));
      }
    });
    req.on('error', reject);
  });
}

function sendJson(res: http.ServerResponse, statusCode: number, data: unknown) {
  res.writeHead(statusCode, { 'Content-Type': 'application/json' });
  res.end(JSON.stringify(data, null, 2));
}

const server = http.createServer(async (req, res) => {
  try {
    if (req.method === 'POST' && req.url === '/authorize') {
      const body: AuthorizeRequest = await parseBody(req);
      const result = await authorize(body);
      sendJson(res, 200, result);
      return;
    }

    if (req.method === 'POST' && req.url === '/refund') {
      const body: RefundRequest = await parseBody(req);
      const result = await refund(body);
      sendJson(res, 200, result);
      return;
    }

    if (req.method === 'GET' && req.url === '/health') {
      sendJson(res, 200, { status: 'ok' });
      return;
    }

    sendJson(res, 404, { error: 'Not found' });
  } catch (err: any) {
    console.error('Request error:', err);
    sendJson(res, 500, {
      error: err.message || 'Internal server error',
      type: err.constructor?.name,
    });
  }
});

server.listen(SERVER_PORT, () => {
  console.log(`Payment server listening on port ${SERVER_PORT}`);
  console.log('Routes:');
  console.log('  POST /authorize - Create a payment authorization');
  console.log('  POST /refund    - Refund a payment');
  console.log('  GET  /health    - Health check');
  console.log('');
  console.log('Routing rules:');
  console.log('  EUR           -> PayPal');
  console.log('  USD (>$100)   -> Cybersource');
  console.log('  USD (<=100)   -> Adyen');
});
