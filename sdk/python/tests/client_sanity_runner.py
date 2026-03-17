import json
import sys
import os
import asyncio
import base64

# Add SDK src and generated proto dirs to import path.
# Some generated *_pb2.py files use bare imports (e.g., "import payment_pb2"),
# so include the generated directory explicitly for local/CI robustness.
sdk_src = os.path.abspath(os.path.join(os.path.dirname(__file__), '..', 'src'))
sdk_generated = os.path.join(sdk_src, 'payments', 'generated')
sys.path.insert(0, sdk_generated)
sys.path.insert(0, sdk_src)

from payments.http_client import execute, HttpRequest, create_client, merge_http_config, DEFAULT_HTTP_CONFIG
from payments.generated import sdk_config_pb2

async def run_sanity():
    # Read input from stdin
    input_data = json.loads(sys.stdin.read())
    
    scenario_id = input_data['scenario_id']
    source_id = input_data['source_id']
    req_data = input_data['request']
    proxy = input_data.get('proxy')
    client_timeout_ms = input_data.get('client_timeout_ms')
    client_response_timeout_ms = input_data.get('client_response_timeout_ms')

    # 1. Setup Client (base config from proxy; proto defaults for timeouts)
    client_config = None
    if proxy and proxy.get('http_url'):
        client_config = sdk_config_pb2.HttpConfig(
            proxy=sdk_config_pb2.ProxyOptions(http_url=proxy['http_url'])
        )

    try:
        client = create_client(client_config)
    except Exception as e:
        code = getattr(e, 'error_code', None) or getattr(e, 'errorCode', None) or 'UNKNOWN_ERROR'
        print(json.dumps({'error': {'code': str(code), 'message': str(e)}}))
        return

    # 2. Setup Request
    headers = (req_data.get('headers') or {}).copy()
    headers['x-source'] = source_id
    headers['x-scenario-id'] = scenario_id

    body = req_data.get('body')
    if isinstance(body, str) and body.startswith('base64:'):
        body = base64.b64decode(body.replace('base64:', ''))
    elif isinstance(body, str):
        body = body.encode('utf-8')

    request = HttpRequest(
        url=req_data['url'],
        method=req_data['method'],
        headers=headers,
        body=body
    )

    # 3. Merge override with base (before execute); execute receives merged values
    override_config = None
    if client_timeout_ms is not None:
        override_config = sdk_config_pb2.HttpConfig(total_timeout_ms=client_timeout_ms)
    elif client_response_timeout_ms is not None:
        override_config = sdk_config_pb2.HttpConfig(response_timeout_ms=client_response_timeout_ms)

    if override_config:
        base = client_config if client_config else DEFAULT_HTTP_CONFIG
        merged = merge_http_config(base, override_config)
        resolved_ms = (merged.total_timeout_ms, merged.connect_timeout_ms, merged.response_timeout_ms)
    else:
        resolved_ms = None

    output = {}
    try:
        sdk_response = await execute(request, client, resolved_ms)
        
        # Format Response
        ct = sdk_response.headers.get('content-type', '').lower()
        if 'application/octet-stream' in ct:
            body_str = base64.b64encode(sdk_response.body).decode('utf-8')
        else:
            body_str = sdk_response.body.decode('utf-8', errors='replace')
            
        output['response'] = {
            'statusCode': sdk_response.status_code,
            'headers': sdk_response.headers,
            'body': body_str
        }
    except Exception as e:
        code = getattr(e, 'error_code', None) or getattr(e, 'errorCode', None) or 'UNKNOWN_ERROR'
        output['error'] = {'code': str(code), 'message': str(e)}

    print(json.dumps(output))

if __name__ == "__main__":
    asyncio.run(run_sanity())
