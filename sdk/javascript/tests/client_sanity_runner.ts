import * as fs from 'fs';
import { execute, createDispatcher } from '../src/http_client';

async function runSanity() {
  // Read input from stdin
  const inputStr = fs.readFileSync(0, 'utf8');
  const input = JSON.parse(inputStr);

  const { scenario_id, source_id, request: reqData, proxy, client_timeout_ms, client_response_timeout_ms } = input;

  const request = { ...reqData };
  request.headers = {
    ...(request.headers || {}),
    'x-source': source_id,
    'x-scenario-id': scenario_id
  };

  if (typeof request.body === 'string' && request.body.startsWith('base64:')) {
    request.body = Uint8Array.from(Buffer.from(request.body.replace('base64:', ''), 'base64'));
  }

  const opts: any = {};
  if (client_timeout_ms != null) {
    opts.totalTimeoutMs = client_timeout_ms;
  }
  if (client_response_timeout_ms != null) {
    opts.responseTimeoutMs = client_response_timeout_ms;
  }

  let dispatcher: any = undefined;
  const dispatcherConfig: any = { ...opts };
  if (proxy?.http_url) {
    dispatcherConfig.proxy = { httpUrl: proxy.http_url };
  }
  try {
    dispatcher = createDispatcher(dispatcherConfig);
  } catch (e: any) {
    const code = e?.errorCode ?? (typeof e?.code === 'string' ? e.code : 'UNKNOWN_ERROR');
    console.log(JSON.stringify({ error: { code, message: e?.message || String(e) } }));
    return;
  }

  const output: any = {};
  try {
    const sdkResponse = await execute(request, opts, dispatcher);
    
    const ct = (sdkResponse.headers['content-type'] || '').toLowerCase();
    const bodyStr = ct.includes('application/octet-stream')
      ? Buffer.from(sdkResponse.body).toString('base64')
      : new TextDecoder().decode(sdkResponse.body);

    output.response = {
      statusCode: sdkResponse.statusCode,
      headers: sdkResponse.headers,
      body: bodyStr,
    };
  } catch (e: any) {
    const code = e?.errorCode ?? (typeof e?.code === 'string' ? e.code : 'UNKNOWN_ERROR');
    output.error = { code, message: e?.message || String(e) };
  }

  console.log(JSON.stringify(output));
}

runSanity().catch(err => {
  console.error(err);
  process.exit(1);
});
