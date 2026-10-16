#!/usr/bin/env python3
"""Execute PayPal financial contracts through actual UCS gRPC mapping.

Fixture mode uses public inert authentication and a local HTTP response server.
It never labels a local mapping check as a live PayPal outcome. Live modes require
explicit environment inputs, durable journals and read-only reconciliation.
"""
import argparse
import base64
import functools
import copy
import contextlib
import fcntl
import hashlib
import http.server
import json
import os
from pathlib import Path
import re
import subprocess
import threading
import time
import urllib.parse
import uuid

ROOT = Path(__file__).resolve().parents[1]
SPECS = ROOT / 'crates/internal/integration-tests/src/connector_specs/paypal'
PROTECTED_ORDER = '0SG65781FC8888712'
PROTECTED_CAPTURE = '2K728570VG9010042'
PUBLIC_STATE = {'access_token': {'token': {'value': 'PUBLIC_INERT_FIXTURE_TOKEN'},
                                'token_type': 'Bearer', 'expires_in_seconds': 300}}


def digest(path):
    value = hashlib.sha256()
    with Path(path).open('rb') as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b''):
            value.update(chunk)
    return value.hexdigest()


def atomic(path, value):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(path.name + '.tmp')
    with temporary.open('w') as stream:
        stream.write(json.dumps(value, indent=2) + '\n')
        stream.flush()
        os.fsync(stream.fileno())
    os.replace(temporary, path)
    directory = os.open(path.parent, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(directory)
    finally:
        os.close(directory)


def strict_keys(value, allowed, required):
    if not isinstance(value, dict) or set(value) - set(allowed) or set(required) - set(value):
        raise ValueError('Invalid contract shape or unknown fields')


def at(value, path):
    for key in path.split('.'):
        if isinstance(value, list) and key.isdigit():
            index = int(key)
            value = value[index] if index < len(value) else None
        elif isinstance(value, dict):
            value = value.get(key)
        else:
            return None
    return value


def normalize(value):
    if isinstance(value, dict):
        return {re.sub(r'(?<!^)(?=[A-Z])', '_', k).lower(): normalize(v)
                for k, v in value.items()}
    if isinstance(value, list):
        return [normalize(v) for v in value]
    return value


def number(value):
    if isinstance(value, bool):
        raise ValueError('Boolean is not money')
    if isinstance(value, str) and re.fullmatch(r'-?\d+', value):
        value = int(value)
    if not isinstance(value, int) or not -(2 ** 63) <= value < 2 ** 63:
        raise ValueError('Money is not an i64')
    return value


def load_contracts():
    order = json.loads((SPECS / 'order_reconciliation_contract.json').read_text())
    capture = json.loads((SPECS / 'capture_operation_chain.json').read_text())
    closure = json.loads((SPECS / 'financial_closure_chain.json').read_text())
    fixture = json.loads((SPECS / 'financial_reconciliation_fixtures.json').read_text())
    strict_keys(fixture, ['schema', 'provenance', 'cases'], ['schema', 'cases'])
    if fixture['schema'] != 'paypal-financial-reconciliation-fixtures/1':
        raise ValueError('Unknown fixture schema')
    expected = ([c['id'] for c in order['cases']] + [c['id'] for c in capture['source_regressions']]
                + [c['id'] for c in closure.get('refund_source_regressions', [])]
                + [c['id'] for c in closure.get('rsync_source_regressions', [])]
                + [c['id'] for c in closure.get('threeds_source_regressions', [])]
                + [c['id'] for c in closure.get('webhook_source_regressions', [])]
                + [c['id'] for c in closure.get('vault_sync_source_regressions', [])])
    actual = [c['id'] for c in fixture['cases']]
    if len(expected) != len(set(expected)) or len(actual) != len(set(actual)) or set(expected) != set(actual):
        raise ValueError('Missing, duplicate or unmatched contract case IDs')
    for case in fixture['cases']:
        strict_keys(case, ['id', 'rpc', 'request', 'http', 'repeat', 'expect', 'error_contains', 'omit_access_token', 'connector'],
                    ['id', 'rpc', 'request', 'http', 'repeat', 'expect'])
        strict_keys(case['http'], ['method', 'path', 'response', 'response_status', 'calls', 'request_body_contains', 'base_url_suffix', 'steps'],
                    ['method', 'path', 'response', 'response_status', 'calls', 'request_body_contains'])
        if case.get('connector', 'Paypal') not in ['Paypal', 'Stripe']:
            raise ValueError('Unsupported inert fixture connector')
        if case.get('connector') == 'Stripe' and case['rpc'] != 'CompositePaymentService/Authorize':
            raise ValueError('Shared connector control must use actual composite Authorize')
        if case['rpc'] not in ['PaymentService/Authorize', 'RecurringPaymentService/Charge', 'PaymentService/Get', 'PaymentService/Capture', 'PaymentService/Void', 'PaymentService/Refund', 'RefundService/Get',
                               'CompositePaymentService/Authorize', 'PaymentMethodAuthenticationService/PostAuthenticate',
                               'EventService/HandleEvent', 'EventService/ParseEvent']:
            raise ValueError('Unsupported fixture RPC')
        if not isinstance(case.get('omit_access_token', False), bool):
            raise ValueError('Invalid access-token omission flag')
        if 'steps' in case['http']:
            if not isinstance(case['http']['steps'], list) or not 1 <= len(case['http']['steps']) <= 8:
                raise ValueError('Invalid fixture HTTP sequence')
            for step in case['http']['steps']:
                strict_keys(step, ['method', 'path', 'response', 'response_status', 'request_body_contains', 'request_body_equals', 'authorization_matches_fresh_oauth'], ['method', 'path', 'response', 'response_status', 'request_body_contains'])
                if (step['method'] not in ['GET', 'POST'] or not step['path'].startswith(('/v1/oauth2/token', '/v2/', '/v1/notifications/verify-webhook-signature', '/v3/vault/setup-tokens', '/v3/vault/payment-tokens'))
                        or step['response_status'] not in [200, 201, 204, 400, 401, 403, 404, 409, 422, 429, 500, 502, 503]
                        or not isinstance(step['request_body_contains'], dict)
                        or not isinstance(step.get('authorization_matches_fresh_oauth', False), bool)):
                    raise ValueError('Invalid sequence step')
        if case['http'].get('base_url_suffix', '') not in ['', '?fields=payment_source', '#PUBLIC_FRAGMENT']:
            raise ValueError('Unsupported inert fixture base URL suffix')
        if case['http']['method'] not in ['GET', 'POST'] or not case['http']['path'].startswith(('/v2/', '/v3/vault/setup-tokens', '/v3/vault/payment-tokens', '/v1/notifications/verify-webhook-signature', '/v1/payment_intents')):
            raise ValueError('Invalid fixture HTTP contract')
        if (isinstance(case['http']['calls'], bool) or not isinstance(case['http']['calls'], int)
                or not 0 <= case['http']['calls'] <= 8):
            raise ValueError('Invalid HTTP count')
        if (isinstance(case['http']['response_status'], bool)
                or case['http']['response_status'] not in [200, 201, 204, 400, 401, 403, 404, 409, 422, 429, 500, 502, 503]):
            raise ValueError('Invalid HTTP fixture status')
        if not isinstance(case['http']['request_body_contains'], dict):
            raise ValueError('Invalid HTTP body assertions')
        if isinstance(case['repeat'], bool) or case['repeat'] not in [1, 2]:
            raise ValueError('Invalid repeat count')
        if not isinstance(case['expect'], dict) or not case['expect']:
            raise ValueError('Missing response assertions')
        for key, value in case['expect'].items():
            if key not in ['error_present', 'status', 'captured_amount', 'connector_refund_id', 'refund_amount_minor', 'refund_amount_currency', 'error_code', 'error_reason', 'source_verified', 'grpc_error_code', 'grpc_error_status', 'grpc_error_identity'] and not key.startswith(('meta.', 'response.')):
                raise ValueError('Unknown assertion path')
            if key == 'error_present' and not isinstance(value, bool):
                raise ValueError('Invalid error assertion')
            if key == 'status' and not isinstance(value, str):
                raise ValueError('Invalid status assertion')
            if key in ['captured_amount', 'refund_amount_minor'] and value is not None:
                number(value)
        for name in ['amount', 'amount_to_capture', 'refund_amount']:
            if name in case['request']:
                number(case['request'][name]['minor_amount'])
        if 'payment_amount' in case['request']:
            number(case['request']['payment_amount'])
        metadata = json.loads(case['request'].get('connector_feature_data', {}).get('value', '{}'))
        if not case['rpc'].startswith('EventService/') and 'connector_feature_data' not in case['request']:
            raise ValueError('Financial fixture requires payment metadata')
        if metadata.get('order_id') == PROTECTED_ORDER or metadata.get('capture_id') == PROTECTED_CAPTURE:
            raise ValueError('Protected real resource cannot be a fixture')
    return fixture['cases']


OWNING_ROOTS = ['crates', 'scripts', 'config', '.cargo', 'Cargo.toml',
                'Cargo.lock', 'buf.yaml', 'buf.lock', 'buf.gen.yaml']


def owning_path(path):
    return (Path(path).suffix in {'.rs', '.proto', '.py', '.toml', '.lock', '.json', '.yaml', '.yml', '.sh'}
            and path not in {'creds.json', 'creds_dummy.json'} and 'target' not in Path(path).parts)


def git_paths(arguments):
    result = subprocess.run(['git', '-C', str(ROOT)] + arguments + ['--'] + OWNING_ROOTS,
                            capture_output=True, check=True)
    return {p.decode() for p in result.stdout.split(b'\0') if p and owning_path(p.decode())}


def owning_manifest():
    # Retain missing indexed paths explicitly. Dropping a deleted file would
    # otherwise let the remaining subset pretend to match the committed tree.
    paths = git_paths(['ls-files', '--cached', '--others', '--exclude-standard', '-z'])
    return {p: digest(ROOT / p) if (ROOT / p).is_file() else None for p in sorted(paths)}


def bindings(binary=None):
    commit = subprocess.run(['git', '-C', str(ROOT), 'rev-parse', 'HEAD'],
                            capture_output=True, text=True, check=True).stdout.strip()
    manifest = owning_manifest()
    head_paths = git_paths(['ls-tree', '-r', '--name-only', '-z', commit])
    dirty = git_paths(['diff', '--name-only', '-z', commit])
    committed = set(manifest) == head_paths and not dirty and all(manifest.values())
    for path, hashed in manifest.items():
        saved = subprocess.run(['git', '-C', str(ROOT), 'show', commit + ':' + path], capture_output=True)
        committed = bool(committed and saved.returncode == 0
                         and hashlib.sha256(saved.stdout).hexdigest() == hashed)
    return {'commit': commit, 'source_sha256': manifest,
            'binary_sha256': digest(binary) if binary else None,
            'source_matches_commit': bool(committed), 'owning_dirty_paths': sorted(dirty),
            'owning_missing_paths': sorted(p for p, hashed in manifest.items() if hashed is None),
            'head_owning_paths_match': set(manifest) == head_paths}


def verify_build_receipt(path, current, binary):
    if not path or not binary or not current['source_matches_commit']:
        raise ValueError('Committed runtime requires actual build receipt and complete clean source manifest')
    receipt = json.loads(Path(path).read_text())
    if (receipt.get('schema') != 'ucs-actual-build-receipt/1'
            or receipt.get('commit') != current['commit']
            or receipt.get('source_sha256_before') != current['source_sha256']
            or receipt.get('source_sha256_after') != current['source_sha256']
            or receipt.get('binary_sha256') != current['binary_sha256']
            or receipt.get('exit_code') != 0
            or receipt.get('producer') != 'owned_actual_build'
            or not isinstance(receipt.get('build_pid'), int)
            or isinstance(receipt.get('build_pid'), bool)
            or receipt['build_pid'] <= 0):
        raise ValueError('Build receipt does not bind actual successful build to current owning source/binary')
    command = receipt.get('command', [])
    expected = ['cargo', 'build', '-p', 'grpc-server', '--bin', 'grpc-server']
    if (not isinstance(command, list) or command[:6] != expected
            or any(value in command for value in ['--lib', '--bins', '--all-targets'])
            or command.count('--bin') != 1
            or not any(value in command for value in ['--message-format=json', '--message-format=json-render-diagnostics'])):
        raise ValueError('Build receipt requires actual grpc-server binary build with Cargo JSON artifacts')
    log = Path(receipt.get('build_log', ''))
    if not log.is_file() or digest(log) != receipt.get('build_log_sha256'):
        raise ValueError('Build receipt missing immutable actual build log')
    events = []
    for line in log.read_bytes().splitlines():
        try:
            value = json.loads(line)
            if isinstance(value, dict):
                events.append(value)
        except (ValueError, UnicodeError):
            continue
    artifact = any(event.get('reason') == 'compiler-artifact'
                   and event.get('target', {}).get('name') == 'grpc-server'
                   and event.get('target', {}).get('kind') == ['bin']
                   and isinstance(event.get('executable'), str)
                   and Path(event['executable']).is_absolute()
                   and Path(event['executable']).resolve() == Path(binary).resolve()
                   and event['executable'] in event.get('filenames', [])
                   for event in events)
    finished = any(event.get('reason') == 'build-finished' and event.get('success') is True for event in events)
    if not artifact or not finished:
        raise ValueError('Build log lacks successful grpc-server executable compiler artifact')
    start, end = receipt.get('started_at_epoch'), receipt.get('finished_at_epoch')
    if (isinstance(start, bool) or isinstance(end, bool)
            or not isinstance(start, (int, float)) or not isinstance(end, (int, float))
            or start > end or end > time.time() + 5
            or Path(binary).stat().st_mtime > end + 5):
        raise ValueError('Build receipt time/binary provenance is invalid')
    return {'path': str(path), 'sha256': digest(path), 'commit': current['commit'],
            'binary_sha256': current['binary_sha256'], 'build_log_sha256': digest(log)}


@contextlib.contextmanager
def journal_lock(path):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor = os.open(str(path) + '.lock', os.O_CREAT | os.O_RDWR | os.O_NOFOLLOW, 0o600)
    try:
        try:
            fcntl.flock(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            raise ValueError('JOURNAL_BUSY: another invocation owns this journal') from None
        yield
    finally:
        os.close(descriptor)


def verify_runtime(endpoint, binary):
    if not binary:
        raise ValueError('Actual runtime binary is required')
    host, port = endpoint.rsplit(':', 1)
    if host not in ['127.0.0.1', 'localhost'] or not port.isdigit():
        raise ValueError('Runtime binding requires local UCS endpoint')
    listeners = subprocess.run(['ss', '-H', '-ltnp', 'sport = :' + port],
                               capture_output=True, text=True, check=True).stdout
    pids = set(re.findall(r'pid=(\d+)', listeners))
    matches = [int(pid) for pid in pids if Path('/proc/' + pid + '/exe').exists()
               and digest('/proc/' + pid + '/exe') == digest(binary)]
    if len(matches) != 1:
        raise ValueError('Endpoint does not match exactly one supplied runtime binary')
    return matches[0]


@functools.lru_cache(maxsize=1)
def public_error_types():
    """Generate real protobuf descriptors from the exact Git-bound API sources.

    protoc and protobuf are the normal schema tools; no parallel wire decoder is
    maintained here. Descriptor bytes and raw error payloads remain in memory.
    """
    import google.protobuf
    from google.protobuf import descriptor_pb2, descriptor_pool, message_factory
    directory = ROOT / 'crates/types-traits/grpc-api-types/proto'
    sources = sorted(directory.glob('*.proto'))
    commit = subprocess.run(['git', 'rev-parse', 'HEAD'], cwd=ROOT,
                            capture_output=True, text=True, check=True).stdout.strip()
    bindings = {}
    for source in sources:
        relative = str(source.relative_to(ROOT))
        committed = subprocess.run(['git', 'show', commit + ':' + relative], cwd=ROOT,
                                   capture_output=True, check=True).stdout
        if committed != source.read_bytes():
            raise ValueError('Error descriptor source differs from committed schema')
        bindings[relative] = hashlib.sha256(committed).hexdigest()
    generated = subprocess.run(['protoc', '-I', str(directory), '--include_imports',
                                '--descriptor_set_out=/dev/stdout', 'sdk_config.proto', 'services.proto', 'composite_services.proto'],
                               cwd=directory, capture_output=True, check=True).stdout
    descriptor = descriptor_pb2.FileDescriptorSet.FromString(generated)
    pool = descriptor_pool.DescriptorPool()
    pending = list(descriptor.file)
    while pending:
        progress = False
        for item in pending[:]:
            try:
                pool.Add(item)
            except TypeError:
                continue
            pending.remove(item)
            progress = True
        if not progress:
            raise ValueError('Unresolved generated protobuf dependency')
    classes = {name: message_factory.GetMessageClass(pool.FindMessageTypeByName('types.' + name))
               for name in ['ConnectorError', 'IntegrationError', 'ErrorInfo']}
    # Exercise the exact runtime API before any provider call is permitted.
    from google.protobuf import json_format
    probe = classes['IntegrationError'](error_message='PUBLIC_PREFLIGHT', error_code='NOT_SUPPORTED')
    clone = classes['IntegrationError'].FromString(probe.SerializeToString())
    if json_format.MessageToDict(clone, preserving_proto_field_name=True).get('error_code') != 'NOT_SUPPORTED':
        raise ValueError('Incompatible protobuf runtime API')
    return classes, {'commit': commit, 'protobuf_version': google.protobuf.__version__, 'proto_sources': bindings,
                     'descriptor_sha256': hashlib.sha256(generated).hexdigest(),
                     'types': {name: cls.DESCRIPTOR.full_name for name, cls in classes.items()}}


def typed_public_error(statuses, streams):
    """Follow scenario_api.rs: structured ErrorInfo before protobuf trailers."""
    from google.protobuf import json_format
    from google.protobuf.message import DecodeError
    classes, provenance = public_error_types()
    for status in statuses:
        for detail in status.get('details', []):
            if not isinstance(detail, dict):
                continue
            info = detail.get('errorInfo', detail.get('error_info'))
            if info is not None:
                if detail.get('@type') not in [None, 'type.googleapis.com/types.ConnectorError']:
                    raise ValueError('Mismatched structured protobuf error type')
                message = classes['ErrorInfo']()
                try:
                    json_format.ParseDict(info, message, ignore_unknown_fields=False)
                except json_format.ParseError:
                    # Protobuf diagnostics may contain raw provider field values.
                    raise ValueError('Malformed structured protobuf ErrorInfo') from None
                if not message.ListFields():
                    raise ValueError('Empty structured ErrorInfo')
                return {'error': json_format.MessageToDict(message, preserving_proto_field_name=True),
                        'grpc_status': status['code']}, provenance
    for stream in streams:
        for line in stream.splitlines():
            if not line.strip().startswith('grpc-status-details-bin:'):
                continue
            raw = base64.b64decode(line.strip().partition(':')[2].strip(), validate=True)
            if not raw:
                raise ValueError('Empty protobuf status details')
            for name in ['ConnectorError', 'IntegrationError']:
                message = classes[name]()
                try:
                    message.ParseFromString(raw)
                except DecodeError:
                    continue
                # Reject unknown fields and an empty/partial accidental decode.
                clean = classes[name]()
                clean.CopyFrom(message)
                clean.DiscardUnknownFields()
                if clean.SerializeToString() != message.SerializeToString():
                    continue
                if not message.error_code or not message.error_message:
                    continue
                if name == 'ConnectorError':
                    if not message.HasField('error_info') or not message.error_info.ListFields():
                        continue
                    info = message.error_info
                else:
                    info = message
                return {'error': json_format.MessageToDict(info, preserving_proto_field_name=True)}, provenance
    return None, provenance


def parse_grpcurl_output(stdout, stderr):
    """Decode unary verbose output without treating metadata as response JSON.

    All input stays in memory. In particular, verbose request metadata and raw
    connector request/response carriers must never be persisted as diagnostics.
    Only the decoded response is used by the existing allowlisted assertions.
    """
    decoder = json.JSONDecoder()
    messages = []
    for marker in re.finditer(r'^Response contents:\s*\n', stdout, re.MULTILINE):
        payload = stdout[marker.end():].lstrip()
        value, _ = decoder.raw_decode(payload)
        if not isinstance(value, dict):
            raise ValueError('Invalid unary response object')
        messages.append(value)
    if len(messages) > 1:
        raise ValueError('Unexpected multiple unary responses')
    errors = []
    statuses = []
    protocol_errors = []
    for stream in [stdout, stderr]:
        # -format-error emits a standalone JSON error, optionally after ERROR:.
        # Do not scan arbitrary braces in echoed request metadata or trailers.
        candidates = [stream.strip()]
        candidates.extend(stream[m.end():].lstrip()
                          for m in re.finditer(r'^ERROR:\s*\n', stream, re.MULTILINE))
        for candidate in candidates:
            if not candidate.startswith('{'):
                continue
            try:
                value, _ = decoder.raw_decode(candidate)
            except json.JSONDecodeError:
                continue
            if (isinstance(value, dict) and isinstance(value.get('code'), int)
                    and not isinstance(value['code'], bool)
                    and value['code'] != 0 and isinstance(value.get('message'), str)):
                statuses.append(value)
                errors.append(value['message'])
                # grpcurl reports mismatched status-details as a structured
                # Internal error. Corroborate its exact public typed identity;
                # unrelated Internal errors must never satisfy a native refusal.
                message = value['message']
                internal = (value['code'] == 13 and message.startswith(
                    'grpc-status-details-bin mismatch: grpc-status=Internal, grpc-message='))
                typed = re.search(r'grpc-status-details-bin=message:"(RESPONSE_HANDLING_FAILED)"(?:\s|$)', message)
                protocol_errors.append({'code': value['code'],
                                        'status': 'Internal' if internal else None,
                                        'identity': typed.group(1) if internal and typed else None})
    if not messages and not errors:
        raise ValueError('No unary response or structured gRPC error')
    response = normalize(messages[0]) if messages else {}
    if statuses:
        typed, provenance = typed_public_error(statuses, [stdout, stderr])
        if typed:
            if response:
                raise ValueError('Conflicting unary response and typed error')
            response = normalize(typed)
            response['_error_descriptor_binding'] = provenance
    if protocol_errors:
        if any(item != protocol_errors[0] for item in protocol_errors):
            raise ValueError('Conflicting public gRPC errors')
        response['_grpc_error'] = protocol_errors[0]
    # Guard matching sees only structured errors, never echoed credentials or
    # request metadata that happens to contain an expected diagnostic string.
    return response, '\n'.join(errors), bool(errors)


RPC_AUDIT = None


def rpc(endpoint, method, request, config, token, inject_state=True, connector='paypal'):
    audit = RPC_AUDIT
    call_key = audit('started', method, request, None) if audit else None
    try:
        result = raw_rpc(endpoint, method, request, config, token, inject_state, connector)
    except (Exception, KeyboardInterrupt):
        if audit:
            audit('uncertain', method, request, call_key)
        raise
    if audit:
        audit('completed', method, request, (call_key, result))
    return result


def raw_rpc(endpoint, method, request, config, token, inject_state=True, connector='paypal'):
    # Configuration and OAuth remain memory-only. Never emit argv, raw stderr,
    # headers or an unfiltered provider response into an artifact.
    q = copy.deepcopy(request)
    if inject_state and method == 'EventService/HandleEvent':
        q['access_token'] = {'token': {'value': token}, 'token_type': 'Bearer', 'expires_in_seconds': 300}
    elif inject_state and not method.startswith('EventService/'):
        q.setdefault('state', {})['access_token'] = {'token': {'value': token}, 'token_type': 'Bearer',
                                     'expires_in_seconds': 300}
    command = ['grpcurl', '-plaintext', '-max-time', '40', '-format-error', '-emit-defaults', '-v',
               '-expand-headers', '-H', 'x-connector-config: ${PAYPAL_REGRESSION_CONNECTOR_CONFIG}',
               '-H', 'x-connector: ' + connector, '-H', 'x-merchant-id: paypal-regression',
               '-H', 'x-tenant-id: public', '-d', '@', endpoint, 'types.' + method]
    environment = dict(os.environ, PAYPAL_REGRESSION_CONNECTOR_CONFIG=config)
    proc = subprocess.run(command, input=json.dumps(q), capture_output=True, text=True, timeout=45, env=environment)
    response, error_text, protocol_error = parse_grpcurl_output(proc.stdout, proc.stderr)
    error = proc.returncode != 0 or protocol_error or bool(response.get('error'))
    if response.get('error'):
        error_text += '\n' + json.dumps(response['error'])
    return response, error, error_text


def projection(response, error):
    result = {'error_present': error, 'source_verified': response.get('source_verified', False), 'status': response.get('status'),
              'captured_amount': response.get('captured_amount'),
              'connector_refund_id': response.get('connector_refund_id'),
              'refund_amount_minor': at(response, 'refund_amount.minor_amount'),
              'refund_amount_currency': at(response, 'refund_amount.currency'),
              'error_code': at(response, 'error.connector_details.code'),
              'error_reason': at(response, 'error.connector_details.reason'),
              'grpc_error_code': at(response, '_grpc_error.code'),
              'grpc_error_status': at(response, '_grpc_error.status'),
              'grpc_error_identity': at(response, '_grpc_error.identity'),
              'response': response}
    raw = at(response, 'connector_feature_data.value')
    if isinstance(raw, str):
        result['meta'] = json.loads(raw)
    return result


def assert_response(result, expected):
    failures = []
    for path, wanted in expected.items():
        actual = at(result, path)
        if isinstance(wanted, int) and not isinstance(wanted, bool):
            try:
                actual = number(actual)
            except ValueError:
                pass
        if actual != wanted:
            # Artifact contains assertion paths only, never arbitrary values.
            failures.append(path)
    return failures


class FixtureServer(http.server.ThreadingHTTPServer):
    daemon_threads = True


class FixtureHandler(http.server.BaseHTTPRequestHandler):
    def log_message(self, *_args):
        pass

    def respond(self):
        server = self.server
        body = self.rfile.read(int(self.headers.get('Content-Length', '0')))
        try:
            data = json.loads(body) if body else {}
        except json.JSONDecodeError:
            if 'application/x-www-form-urlencoded' in self.headers.get('Content-Type', ''):
                data = {key: values[0] for key, values in urllib.parse.parse_qs(body.decode('ascii')).items()}
            else:
                data = {}
        contract = server.case['http']
        steps = contract.get('steps')
        step_index = len(server.observations)
        expected = steps[min(step_index, len(steps) - 1)] if steps else contract
        failures = ['http.extra_step'] if steps and step_index >= len(steps) else []
        if self.command != expected['method']:
            failures.append('http.method')
        if self.path != expected['path']:
            failures.append('http.path')
        if 'request_body_equals' in expected and data != expected['request_body_equals']:
            failures.append('http.body_exact')
        fresh_matches = None
        if expected.get('authorization_matches_fresh_oauth'):
            fresh = steps[0]['response'].get('access_token') if steps else None
            fresh_matches = isinstance(fresh, str) and bool(fresh) and self.headers.get('Authorization') == 'Bearer ' + fresh
            if not fresh_matches:
                failures.append('http.fresh_oauth_not_propagated')
        for path, wanted in expected['request_body_contains'].items():
            if at(data, path) != wanted:
                failures.append('http.body.' + path)
        server.observations.append({'method': self.command, 'path': self.path,
                                    'body_sha256': hashlib.sha256(body).hexdigest(),
                                    'authorization_present': bool(self.headers.get('Authorization')),
                                    'authorization_value_recorded': False, 'fresh_oauth_matches': fresh_matches, 'failures': failures})
        payload = b'' if expected['response_status'] == 204 else json.dumps(expected['response']).encode()
        self.send_response(expected['response_status'] if not failures else 500)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)

    do_GET = respond
    do_POST = respond


def fixture_mode(args, cases):
    host = args.endpoint.rsplit(':', 1)[0]
    if host not in ['127.0.0.1', 'localhost']:
        raise ValueError('Fixture UCS endpoint must be loopback')
    if not args.binary:
        raise ValueError('Fixture execution requires actual runtime binary for source binding')
    server = FixtureServer(('127.0.0.1', 0), FixtureHandler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    url = f'http://127.0.0.1:{server.server_port}/'
    config = json.dumps({'config': {'Paypal': {'client_id': 'PUBLIC_INERT_CLIENT',
                                              'client_secret': 'PUBLIC_INERT_SECRET', 'base_url': url}}})
    results = []
    try:
        for case in cases:
            server.case = case
            server.observations = []
            failures = []
            connector = case.get('connector', 'Paypal')
            case_config = ({'config': {'Stripe': {'api_key': 'PUBLIC_INERT_STRIPE_API_KEY', 'base_url': url}}}
                           if connector == 'Stripe' else json.loads(config))
            case_config['config'][connector]['base_url'] = url + case['http'].get('base_url_suffix', '')
            for _ in range(case['repeat']):
                try:
                    response, error, text = rpc(args.endpoint, case['rpc'], case['request'],
                                                json.dumps(case_config), PUBLIC_STATE['access_token']['token']['value'],
                                                inject_state=not case.get('omit_access_token', False), connector=connector.lower())
                except (OSError, subprocess.SubprocessError, ValueError):
                    response, error, text = {}, True, ''
                    failures.append('execution_error')
                failures.extend(assert_response(projection(response, error), case['expect']))
                if case.get('error_contains') and case['error_contains'] not in text:
                    failures.append('specific_local_error')
            observed = server.observations
            if len(observed) != case['http']['calls']:
                failures.append('http.call_count')
            for observation in observed:
                failures.extend(observation['failures'])
                if not observation['authorization_present']:
                    failures.append('http.authorization_missing')
            results.append({'case_id': case['id'], 'local_mapping_oracle': 'FAIL' if failures else 'PASS',
                            'failed_assertion_paths': sorted(set(failures)), 'wire': observed,
                            'evidence_state': 'IMPLEMENTED', 'provider_wire_graded': False,
                            'provider_live_verified': False})
    finally:
        server.shutdown()
        server.server_close()
    return {'mode': 'fixture', 'pass': all(r['local_mapping_oracle'] == 'PASS' for r in results),
            'case_count': len(results), 'cases': results, 'provider_calls': 0,
            'execution': 'Actual production UCS gRPC and HTTP mapping, local inert fixtures only'}


def financial_metadata(response):
    raw = at(response, 'connector_feature_data.value')
    parsed = json.loads(raw) if isinstance(raw, str) else {}
    allowed = ['order_id', 'authorize_id', 'capture_id', 'incremental_authorization_id',
               'psync_flow', 'reference_id']
    result = {key: value for key, value in parsed.items() if key in allowed}
    context = parsed.get('capture_context')
    if isinstance(context, dict):
        keys = ['capture_history_complete', 'authorization_closed', 'authorized_amount',
                'capture_amounts', 'capture_method', 'invoice_id', 'soft_descriptor']
        result['capture_context'] = {key: value for key, value in context.items() if key in keys}
    return result


def live_mode(args):
    with journal_lock(args.journal):
        return locked_live_mode(args)


def locked_live_mode(args):
    config = os.environ.get('UCS_CONNECTOR_CONFIG')
    token = os.environ.get('PAYPAL_ACCESS_TOKEN')
    if not config or not token:
        raise ValueError('OPERATOR_GAP: required live configuration environment missing')
    cfg = json.loads(config)
    paypal = at(cfg, 'config.Paypal')
    if not isinstance(paypal, dict):
        raise ValueError('Invalid live connector configuration shape')
    endpoint = urllib.parse.urlparse(paypal.get('base_url', 'https://api-m.sandbox.paypal.com/'))
    if endpoint.scheme != 'https' or endpoint.hostname not in ['api-m.sandbox.paypal.com', 'api.sandbox.paypal.com']:
        raise ValueError('Live regression modes require explicit PayPal sandbox host')
    journal = json.loads(args.journal.read_text()) if args.journal.exists() else {
        'schema': 'paypal-financial-regression-journal/1', 'operations': {}, 'run_identity': str(uuid.uuid4())}
    if journal.get('schema') != 'paypal-financial-regression-journal/1':
        raise ValueError('Unknown journal schema')

    cached_operations = []
    protected=set(PROTECTED_FINANCIAL_IDS) | set(getattr(args,'protected_financial_ids',[]))
    current_source = args.execution_binding

    def save():
        atomic(args.journal, journal)

    def get_order(metadata, identity):
        q = {'connector_transaction_id': identity, 'connector_feature_data': {'value': json.dumps(metadata)},
             'amount': {'minor_amount': 6000, 'currency': 'USD'}, 'capture_method': 'MANUAL_MULTIPLE',
             'merchant_request_id': journal['run_identity'], 'test_mode': True}
        return rpc(args.endpoint, 'PaymentService/Get', q, config, token)

    def perform(name, method, request):
        if any(identity in json.dumps(request) for identity in protected):
            raise ValueError('Protected historical resource is GET-only')
        operations = journal['operations']
        existing = operations.get(name)
        request_sha = hashlib.sha256(json.dumps(request, sort_keys=True).encode()).hexdigest()
        if existing:
            provenance = existing.get('source_binding')
            valid = (existing.get('request_sha256') == request_sha
                     and existing.get('method') == method
                     and isinstance(provenance, dict) and provenance.get('commit')
                     and provenance.get('source_sha256') and provenance.get('binary_sha256'))
            metadata = existing.get('reconcile_metadata') or financial_metadata(existing.get('response', {}))
            if metadata and metadata.get('order_id'):
                response, error, _ = get_order(metadata, metadata['order_id'])
                existing['read_only_reconciliation'] = {'error': error,
                    'observed_metadata': financial_metadata(response),
                    'captured_amount': response.get('captured_amount'),
                    'current_source_binding': current_source}
                save()
                if error:
                    raise ValueError('Read-only reconciliation failed; completed financial operation never replayed')
            elif existing.get('state') == 'completed':
                raise ValueError('Completed operation lacks actual identity for read-only reconciliation')
            if not valid:
                raise ValueError('JOURNAL_INPUT_OR_PROVENANCE_MISMATCH: preserve original; no financial replay')
            if existing.get('state') == 'completed':
                cached_operations.append({'operation': name, 'original_source_binding': provenance,
                                          'current_source_same': provenance == current_source,
                                          'current_mapping_verified': False})
                return existing['response']
            raise ValueError('AMBIGUOUS_OPERATION: read-only reconciliation retained; financial retry refused')
        metadata = financial_metadata(request)
        if (request.get('connector_transaction_id') in [PROTECTED_ORDER, PROTECTED_CAPTURE]
                or request.get('connector_order_id') == PROTECTED_ORDER
                or metadata.get('order_id') == PROTECTED_ORDER or metadata.get('capture_id') == PROTECTED_CAPTURE):
            raise ValueError('Protected completed capture must never be replayed')
        operations[name] = {'state': 'started', 'request_sha256': hashlib.sha256(
            json.dumps(request, sort_keys=True).encode()).hexdigest(), 'reconcile_metadata': metadata,
                            'method': method, 'source_binding': current_source}
        save()
        response, error, _ = rpc(args.endpoint, method, request, config, token)
        if error:
            operations[name]['state'] = 'uncertain'
            save()
            raise ValueError('OPERATION_ERROR: reconcile before any financial retry')
        safe = {'status': response.get('status'), 'captured_amount': response.get('captured_amount'),
                'connector_transaction_id': response.get('connector_transaction_id'),
                'connector_refund_id': response.get('connector_refund_id'), 'refund_amount':response.get('refund_amount'),
                'connector_feature_data': {'value': json.dumps(financial_metadata(response))}}
        operations[name].update(state='completed', response=safe)
        save()
        return safe

    if args.mode == 'live-read-only':
        if not args.context:
            raise ValueError('Read-only mode requires original financial context file')
        metadata = json.loads(args.context.read_text())
        if metadata.get('order_id') != PROTECTED_ORDER:
            raise ValueError('Read-only reconciliation requires protected original order identity')
        response, error, _ = get_order(metadata, PROTECTED_ORDER)
        observed = financial_metadata(response)
        amount = at(observed, 'capture_context.capture_amounts.' + PROTECTED_CAPTURE + '.value')
        passed = not error and observed.get('order_id') == PROTECTED_ORDER and amount == '30.00'
        return {'mode': args.mode, 'pass': passed, 'evidence_state': 'VERIFIED' if passed else 'IMPLEMENTED',
                'provider_live_verified': passed, 'financial_posts': 0, 'protected_capture_replayed': False,
                'status': response.get('status'), 'captured_amount': response.get('captured_amount'),
                'actual_financial_metadata': observed, 'hs_reconciliation': 'NOT_RUN: separate actual HS before/after oracle required'}
    if not args.authorize and not getattr(args,'authorize_request',None):
        raise ValueError('Shared chain requires original public sandbox Authorize request')
    request = copy.deepcopy(args.authorize_request) if getattr(args,'authorize_request',None) else json.loads(args.authorize.read_text())
    # Shared-chain mode creates a fresh raw-card authorization. It cannot be used
    # to resume a wallet/vault/native order or mutate the protected original order.
    payment_method = request.get('payment_method', {})
    if not isinstance(payment_method, dict) or not set(payment_method).intersection(['card', 'card_with_no_cvc']):
        raise ValueError('Shared chain requires fresh raw-card sandbox request')
    original_metadata = json.loads(at(request, 'connector_feature_data.value') or '{}')
    if (request.get('connector_order_id') or original_metadata.get('continuation')
            or any(original_metadata.get(key) for key in ['order_id', 'authorize_id', 'capture_id'])):
        raise ValueError('Shared chain cannot reuse an existing financial order')
    identity = journal['run_identity']
    request.update(merchant_order_id=identity, merchant_request_id=identity, capture_method='MANUAL_MULTIPLE',
                   amount={'minor_amount': 6000, 'currency': 'USD'})
    if isinstance(request.get('l2_l3_data'), dict):
        request['l2_l3_data'].setdefault('order_info', {})['merchant_order_reference_id'] = identity
    previous = perform('authorize', 'PaymentService/Authorize', request)
    if previous.get('status') != 'AUTHORIZED':
        raise ValueError('Authorization outcome oracle failed')
    auth = financial_metadata(previous)
    if at(auth, 'capture_context.authorized_amount.value') != '60.00':
        raise ValueError('Actual authorized amount oracle failed')
    if args.mode == 'closure-chain':
        capture_request = {'connector_transaction_id':previous['connector_transaction_id'],
            'connector_feature_data':previous['connector_feature_data'],
            'amount_to_capture':{'minor_amount':3000,'currency':'USD'},'capture_method':'MANUAL_MULTIPLE',
            'merchant_capture_id':identity+'-partial','merchant_request_id':identity+'-partial','test_mode':True,
            'multiple_capture_data':{'capture_sequence':1,'capture_reference':identity+'-partial'}}
        captured=perform('capture_partial','PaymentService/Capture',capture_request)
        captured_meta=financial_metadata(captured)
        actual_capture=captured_meta.get('capture_id')
        if (captured.get('status')!='PARTIAL_CHARGED_AND_CHARGEABLE' or number(captured.get('captured_amount'))!=3000
                or not actual_capture or at(captured_meta,'capture_context.capture_amounts.'+actual_capture+'.value')!='30.00'):
            raise ValueError('Actual partial capture identity/money oracle failed')
        voided=perform('void_remainder','PaymentService/Void',{'connector_transaction_id':captured_meta['order_id'],
            'connector_feature_data':captured['connector_feature_data'],'merchant_void_id':identity+'-void',
            'merchant_request_id':identity+'-void','cancellation_reason':'requested_by_customer','test_mode':True})
        vm=financial_metadata(voided)
        if (voided.get('status')!='PENDING' or at(vm,'capture_context.authorization_closed') is not True
                or at(vm,'capture_context.capture_history_complete') is not False
                or at(vm,'capture_context.capture_amounts.'+actual_capture+'.value')!='30.00'):
            raise ValueError('Void Pending/retained actual ledger oracle failed; no public Void Money inferred')
        synced,error,_=get_order(vm,vm['order_id'])
        sm=financial_metadata(synced)
        if (error or synced.get('status')!='PARTIAL_CHARGED' or number(synced.get('captured_amount'))!=3000
                or at(sm,'capture_context.authorization_closed') is not True
                or at(sm,'capture_context.capture_history_complete') is not True
                or at(sm,'capture_context.capture_amounts.'+actual_capture+'.value')!='30.00'):
            raise ValueError('Authoritative closed partial Order GET oracle failed')
        refunded=perform('refund_collected','PaymentService/Refund',{'connector_transaction_id':actual_capture,
            'connector_feature_data':synced['connector_feature_data'],'refund_amount':{'minor_amount':3000,'currency':'USD'},
            'payment_amount':6000,'merchant_refund_id':identity+'-refund','merchant_request_id':identity+'-refund','test_mode':True})
        refund_id=refunded.get('connector_refund_id')
        if (refunded.get('status')!='REFUND_SUCCESS' or not refund_id or number(at(refunded,'refund_amount.minor_amount'))!=3000
                or at(refunded,'refund_amount.currency')!='USD'):
            raise ValueError('Actual captured-money refund oracle failed')
        refund_sync,error,_=rpc(args.endpoint,'RefundService/Get',{'connector_transaction_id':actual_capture,
            'connector_refund_id':refund_id,'refund_id':refund_id,'connector_feature_data':synced['connector_feature_data'],
            'refund_amount':{'minor_amount':3000,'currency':'USD'},'test_mode':True},config,token)
        if error or refund_sync.get('status')!='REFUND_SUCCESS' or refund_sync.get('connector_refund_id')!=refund_id:
            raise ValueError('Actual refund GET terminal/identity oracle failed')
        return {'mode':args.mode,'pass':not cached_operations,'evidence_state':'VERIFIED' if not cached_operations else 'IMPLEMENTED',
                'provider_live_verified':not cached_operations,'provider_wire_graded':False,'cached_operations':cached_operations,
                'actual_capture_id':actual_capture,'actual_refund_id':refund_id,'captured_amount':3000,
                'authorization_closed':True,'capturable_remaining':0,'void_public_money_asserted':False,
                'provider_void_http_status':'NOT_OBSERVED: not inferred from UCS status'}
    first_request = None
    for sequence in [1, 2]:
        q = {'connector_transaction_id': previous['connector_transaction_id'],
             'connector_feature_data': previous['connector_feature_data'],
             'amount_to_capture': {'minor_amount': 3000, 'currency': 'USD'},
             'capture_method': 'MANUAL_MULTIPLE', 'merchant_capture_id': identity + f'-{sequence}',
             'merchant_request_id': identity + f'-{sequence}', 'test_mode': True,
             'multiple_capture_data': {'capture_sequence': sequence, 'capture_reference': identity + f'-{sequence}'}}
        if sequence == 1:
            first_request = q
        previous = perform(f'capture_{sequence}', 'PaymentService/Capture', q)
        expected_status = 'PARTIAL_CHARGED_AND_CHARGEABLE' if sequence == 1 else 'CHARGED'
        if previous.get('status') != expected_status or number(previous.get('captured_amount')) != 3000:
            raise ValueError('Actual capture operation amount/status oracle failed')
        if not financial_metadata(previous).get('capture_id'):
            raise ValueError('Actual capture identity missing')
    metadata = financial_metadata(previous)
    first_metadata = financial_metadata(journal['operations']['capture_1']['response'])
    if first_metadata.get('capture_id') == metadata.get('capture_id'):
        raise ValueError('Distinct captures returned the same identity')
    observed, error, _ = get_order(metadata, metadata['order_id'])
    if error or number(observed.get('captured_amount')) != 6000:
        raise ValueError('Authoritative order aggregate oracle failed')
    # An explicit fresh-chain idempotency row, never an automatic crash retry.
    retry = perform('intentional_same_operation_retry', 'PaymentService/Capture', first_request)
    first = financial_metadata(journal['operations']['capture_1']['response'])
    if (financial_metadata(retry).get('capture_id') != first.get('capture_id')
            or number(retry.get('captured_amount')) != 3000):
        raise ValueError('Idempotency returned a different capture identity')
    final, error, _ = get_order(metadata, metadata['order_id'])
    passed = not error and number(final.get('captured_amount')) == 6000
    return {'mode': args.mode, 'pass': passed, 'evidence_state': 'VERIFIED' if passed and not cached_operations else 'IMPLEMENTED',
            'provider_live_verified': passed and not cached_operations,
            'cached_operations': cached_operations,
            'current_mapping_verified': passed and not cached_operations,
            'cached_provider_result_is_current_verified': False,
            'captured_amount': final.get('captured_amount'),
            'actual_financial_metadata': financial_metadata(final), 'protected_capture_replayed': False,
            'provider_wire_graded': False,
            'literal_http_body_bytes_verified': False,
            'remaining_wire_oracles': ['actual emitted PayPal-Request-Id identity and literal HTTP body byte equality'],
            'pass_scope': 'live financial outcome oracles; emitted-wire proof must be reported separately'}


MATRIX = SPECS / 'regression_matrix.json'
GLOBAL_SUITES = ROOT / 'crates/internal/integration-tests/src/global_suites'
PROTECTED_FINANCIAL_IDS = {PROTECTED_ORDER, PROTECTED_CAPTURE}


def merge_scenario(base, patch):
    if not isinstance(base, dict) or not isinstance(patch, dict):
        return copy.deepcopy(patch)
    result = copy.deepcopy(base)
    for key, value in patch.items():
        if value is None:
            result.pop(key, None)
        else:
            result[key] = merge_scenario(result.get(key), value)
    return result


def assertion_patch(base, patch):
    # Match connector_override::apply_assertion_patch: replace the entire rule.
    result = copy.deepcopy(base or {})
    for field, rule in patch.items():
        if rule is None:
            result.pop(field, None)
        else:
            result[field] = copy.deepcopy(rule)
    return result


def patched_scenario(base, patch):
    result = merge_scenario(base, {key:value for key,value in patch.items() if key != 'assert'})
    if 'assert' in patch:
        result['assert'] = assertion_patch(base.get('assert', {}), patch['assert'])
    return result


def suite_scenarios(suite):
    path = GLOBAL_SUITES / suite.replace('/', '_') / 'scenario.json'
    global_cases = json.loads(path.read_text()) if path.exists() else {}
    custom = json.loads((SPECS / 'connector_specific_scenarios.json').read_text()).get(suite, {})
    overrides = json.loads((SPECS / 'override.json').read_text()).get(suite, {})
    return {name: patched_scenario(custom.get(name, global_cases.get(name, {})), overrides.get(name, {}))
            for name in sorted(set(global_cases) | set(custom) | set(overrides))}


def canonical_matrix():
    manifest = json.loads(MATRIX.read_text())
    strict_keys(manifest, ['schema', 'generator', 'legacy_identity_provenance', 'recipes', 'rows', 'sources'],
                ['schema', 'legacy_identity_provenance', 'recipes'])
    if manifest['schema'] != 'paypal-production-regression-matrix/1':
        raise ValueError('Unknown production matrix schema')
    sources = {str(Path(__file__).resolve().relative_to(ROOT)):digest(Path(__file__))}
    for path in sorted(SPECS.glob('*.json')):
        if path != MATRIX:
            sources[str(path.relative_to(ROOT))] = digest(path)
    for path in sorted(GLOBAL_SUITES.glob('*/scenario.json')):
        sources[str(path.relative_to(ROOT))] = digest(path)
    rows = []
    def append(case_id, unit, method, request, assertions, source, provisioning=None, **extra):
        rows.append(dict(case_id=case_id, unit=unit, method=method, request=request,
                         assertions=assertions, source=source, provisioning=provisioning,
                         evidence_state='IMPLEMENTED', **extra))
    # Recipes preserve migration identities, not the old shadow runner's assertions.
    # Every request and assertion is reloaded from the current committed suite.
    pm_names = {'Card':'no3ds_auto_capture_credit_card',
                'CardWithNoCvc':'paypal_card_without_cvc_supported',
                'Wallet::ApplePay(PSP)':'paypal_applepay_psp_final',
                'Wallet::ApplePay(Hyperswitch)':'paypal_applepay_hs_final',
                'Wallet::GooglePay(PSP)':'paypal_googlepay_psp_api_unsupported',
                'Wallet::GooglePay(Hyperswitch)':'paypal_googlepay_hs_final',
                'NetworkToken':'paypal_networktoken_cit_no_trid',
                'PaymentMethodToken(untyped)':'paypal_untyped_token_refused'}
    for recipe in manifest['recipes']:
        strict_keys(recipe, ['case_id','unit','payment_method','kind','requirement_ref','suite','provisioning','contract','scenario','request_patch','additional_assertions','prerequisite_bindings','wire_oracle','payment_method_source'],
                    ['case_id','unit','payment_method','kind','suite'])
        suite = recipe['suite']
        scenarios = suite_scenarios(suite)
        name = recipe.get('scenario') or (pm_names.get(recipe['payment_method']) if suite == 'PaymentService/Authorize' else None)
        if recipe.get('requirement_ref') == 'R04': name = 'no3ds_manual_capture_credit_card'
        if recipe.get('requirement_ref') == 'R10': name = 'paypal_external_rawcard_3ds_refused'
        if name is None and suite != 'PaymentService/Authorize' and len(scenarios) == 1:
            name = next(iter(scenarios))
        # Multiple valid suite variants need an explicit recipe; do not silently
        # choose the first happy input for a decline, webhook, or financial chain.
        gap = recipe.get('provisioning')
        if recipe['kind'] == 'decline':
            # Keep a typed baseline available, but never execute it as a decline
            # without an explicit documented negative binding and error oracle.
            gap = 'DECLINE_BINDING_REQUIRED: documented negative input and decline oracle required'
        source = {'suite':suite, 'scenario':name, 'requirement_ref':recipe.get('requirement_ref'), 'payment_method_source':recipe.get('payment_method_source')}
        if name and name not in scenarios: raise ValueError('Unknown explicit current suite scenario')
        scenario = copy.deepcopy(scenarios.get(name, {}))
        if scenario:
            scenario['grpc_req']=merge_scenario(scenario.get('grpc_req',{}),recipe.get('request_patch',{}))
            scenario['assert']=assertion_patch(scenario.get('assert',{}),recipe.get('additional_assertions',{}))
            if recipe.get('payment_method_source'):
                instrument_source=recipe['payment_method_source']
                instrument=suite_scenarios(instrument_source['suite'])[instrument_source['scenario']]['grpc_req']['payment_method']
                scenario['grpc_req']['payment_method']=copy.deepcopy(instrument)
        if not scenario:
            gap = gap or 'SOURCE_RECIPE_REQUIRED: no unambiguous current committed suite variant for this coverage identity'
        append(recipe['case_id'], recipe['unit'], suite, scenario.get('grpc_req'),
               scenario.get('assert', {}), source, gap, kind=recipe['kind'],
               payment_method=recipe['payment_method'], contract=recipe.get('contract'),
               prerequisite_bindings=recipe.get('prerequisite_bindings',[]),wire_oracle=recipe.get('wire_oracle'))
    closure = json.loads((SPECS/'financial_closure_chain.json').read_text())
    groups = [('order_reconciliation_contract.json','cases','PSync','resource_id'),
              ('capture_operation_chain.json','source_regressions','Capture','resource_id')]
    groups += [('financial_closure_chain.json', name, unit, arm) for name,unit,arm in [
        ('refund_source_regressions','Refund','resource_id'),('rsync_source_regressions','RSync','resource_id'),
        ('threeds_source_regressions','ThreeDS','Card'),('webhook_source_regressions','IncomingWebhook','resource_id'),
        ('vault_sync_source_regressions','PSync','resource_id')]]
    fixtures = {case['id']:case for case in load_contracts()}
    for filename, key, unit, arm in groups:
        for case in json.loads((SPECS/filename).read_text()).get(key, []):
            fixture = fixtures[case['id']]
            append(unit+'/'+arm+'/'+case['id'], unit, fixture['rpc'], fixture['request'],
                   fixture['expect'], {'contract':filename,'case_id':case['id']},
                   'LOCAL_FIXTURE_MAPPING_ONLY: provider outcome is a separate oracle', fixture_id=case['id'])
    custom = json.loads((SPECS/'connector_specific_scenarios.json').read_text())
    overrides = json.loads((SPECS/'override.json').read_text())
    units = {'PaymentService/Authorize':'Authorize','PaymentService/Capture':'Capture','PaymentService/Void':'Void',
             'PaymentService/Refund':'Refund','RefundService/Get':'RSync','PaymentService/Get':'PSync',
             'PaymentService/SetupRecurring':'SetupMandate','RecurringPaymentService/Charge':'RepeatPayment',
             'PaymentService/CompleteAuthorize':'ThreeDS','PaymentMethodAuthenticationService/Authenticate':'ThreeDS',
             'PaymentMethodAuthenticationService/PostAuthenticate':'ThreeDS','EventService/HandleEvent':'IncomingWebhook'}
    for suite in sorted(set(custom)|set(overrides)):
        scenarios = suite_scenarios(suite)
        for name in sorted(set(custom.get(suite,{}))|set(overrides.get(suite,{}))):
            case = scenarios[name]
            append(units[suite]+'/committed/'+name, units[suite],
                   'PaymentService/Authorize' if suite=='PaymentService/CompleteAuthorize' else suite,
                   case.get('grpc_req'),case.get('assert',{}),{'suite':suite,'scenario':name}, kind='committed')
    for case in json.loads((SPECS/'hs_refund_reconciliation_regressions.json').read_text())['cases']:
        append(case['case_id'],'RSync','HS forced refund sync',None,case['assert'],
               {'contract':'hs_refund_reconciliation_regressions.json','case_id':case['case_id']},
               'HS_SURFACE: execute with the committed HS consumer, never fake UCS fields')
    for case in json.loads((SPECS/'applepay_manufacturer_wire_contract.json').read_text())['cases']:
        append(case['case_id'],'Authorize','PaymentService/Authorize',case['grpc_req'],case['wire_assert'],
               {'contract':'applepay_manufacturer_wire_contract.json','case_id':case['case_id']},
               'WIRE_OBSERVER_REQUIRED: gRPC wrapper status cannot prove vendor wire shape')
    ids = [row['case_id'] for row in rows]
    legacy = manifest['legacy_identity_provenance']['case_ids']
    if len(ids)!=len(set(ids)) or len(legacy)!=385 or len(set(legacy))!=385 or not set(legacy)<=set(ids):
        raise ValueError('Matrix identity migration contains duplicate or missing canonical cases')
    manifest.update(rows=sorted(rows,key=lambda row:row['case_id']),sources=sources)
    return manifest


def matrix_plan(args):
    generated = canonical_matrix()
    if set(args.case_id or [])-{row['case_id'] for row in generated['rows']}:
        raise ValueError('Unknown canonical matrix selection')
    if args.generate_matrix:
        atomic(MATRIX, generated)
    elif json.loads(MATRIX.read_text()) != generated:
        raise ValueError('Committed matrix and production generator disagree')
    return {'pass':True,'mode':'matrix-plan','case_count':len(generated['rows']),
            'legacy_case_count':385,'new_case_count':len(generated['rows'])-385,
            'provider_calls':0,'provider_live_verified':False,'evidence_state':'IMPLEMENTED',
            'matrix_sha256':digest(MATRIX), 'sources':generated['sources'],
            'rows':[{'case_id':row['case_id'],'state':'NOT_RUN','reason':row['provisioning'] or 'Actual source-bound execution required'} for row in generated['rows']]}


def import_financial_facts(args):
    # Caller supplies an immutable input manifest, not arbitrary response files.
    # Import is GET-only provenance preparation and never executes any RPC.
    proof = json.loads(args.context.read_text())
    if proof.get('schema') == 'paypal-financial-fact-import/2':
        strict_keys(proof,['schema','operations'],['schema','operations'])
        facts=[]
        identities=set()
        for entry in proof['operations']:
            strict_keys(entry,['path','sha256','operation_key'],['path','sha256','operation_key'])
            path=Path(entry['path'])
            if digest(path)!=entry['sha256']: raise ValueError('Financial input digest mismatch')
            source=json.loads(path.read_text())
            operation=source.get('operations',{}).get(entry['operation_key'])
            if not isinstance(operation,dict): raise ValueError('Missing original operation')
            required=['method','request_sha256','source_binding','state']
            if any(not operation.get(key) for key in required) or not operation['source_binding'].get('commit'):
                raise ValueError('Missing original source/request provenance')
            key=entry['operation_key']
            if key in identities: raise ValueError('Duplicate imported operation')
            identities.add(key)
            response=operation.get('response') or {}
            metadata=operation.get('metadata') or operation.get('reconcile_metadata') or financial_metadata(response)
            facts.append({'logical_key':key,'method':operation['method'],'request_sha256':operation['request_sha256'],
                          'original_source_commit':operation['source_binding']['commit'],
                          'metadata':metadata,'status':operation.get('status') or response.get('status'),
                          'connector_transaction_id':operation.get('connector_transaction_id') or response.get('connector_transaction_id'),
                          'connector_refund_id':operation.get('connector_refund_id') or response.get('connector_refund_id'),
                          'request_money':operation.get('request_money'),'financial_post_forbidden':True,
                          'current_mapping_verified':False,'input_binding':entry})
        with journal_lock(args.journal):
            if args.journal.exists(): raise ValueError('Never overwrite imported evidence')
            atomic(args.journal,{'schema':'paypal-imported-financial-facts/1','inputs':proof,
                                'facts':facts,'provider_calls':0,'current_mapping_verified':False})
        return {'pass':True,'mode':'import-financial-facts','fact_count':len(facts),'provider_calls':0,
                'provider_live_verified':False,'current_mapping_verified':False,'evidence_state':'IMPLEMENTED'}
    strict_keys(proof,['schema','files'],['schema','files'])
    if proof['schema'] != 'paypal-financial-fact-import/1': raise ValueError('Invalid import schema')
    required = {'calls','inventory','descriptor','loader','resolution','financial_inventory'}
    if set(proof['files']) != required: raise ValueError('Incomplete financial provenance chain')
    loaded = {}
    for name, entry in proof['files'].items():
        strict_keys(entry,['path','sha256'],['path','sha256'])
        path=Path(entry['path'])
        if digest(path)!=entry['sha256']: raise ValueError('Financial provenance digest mismatch')
        loaded[name]=json.loads(path.read_text())
    inventory=loaded['inventory']; descriptor=loaded['descriptor']; loader=loaded['loader']
    sidecar=inventory.get('recovery_sidecar',{})
    if (descriptor.get('financial_post_retried') is not False
        or proof['files']['descriptor']['sha256'] != sidecar.get('sha256')
        or not inventory.get('shadow_114_results_not_gate_passes')
        or loader.get('prohibited_future_financial_calls') is not True):
        raise ValueError('Descriptor or paused-loader ownership mismatch')
    for name in ['calls','descriptor','loader','financial_inventory']:
        entry=proof['files'][name]
        owners=[value for path,value in inventory.get('ownership_manifest',{}).items()
                if Path(path).name==Path(entry['path']).name]
        if len(owners)!=1 or owners[0].get('sha256')!=entry['sha256']:
            raise ValueError('Financial input not owned by the immutable inventory')
    resolution=loaded['resolution']
    if (resolution.get('status')!='EXTERNALLY_GONE'
        or resolution.get('paused_inventory_sha256')!=proof['files']['inventory']['sha256']
        or resolution.get('descriptor_sidecar_sha256')!=proof['files']['descriptor']['sha256']
        or resolution.get('new_durable_post_pause_progress_or_financial_journal_advancement') is not False):
        raise ValueError('Original parent resolution or immutable inventory binding mismatch')
    inflight=list(loaded['calls'].get('inflight',{}).values())
    if len(inflight)!=1 or any(inflight[0].get(key)!=descriptor.get(other) for key,other in [
        ('request_id','request_id'),('request_sha256','pending_request_sha256'),('key','logical_operation')]):
        raise ValueError('Descriptor sidecar does not own the interrupted operation')
    facts=[]
    for request_sha, operation in loaded['calls']['calls'].items():
        if operation.get('logical_key')=='oauth': continue
        request=operation.get('request')
        if not isinstance(request,dict) or not operation.get('source_commit') or not operation.get('request_id'):
            raise ValueError('Financial call missing original source or request identity')
        # The legacy journal key is not a request hash; retain both identities.
        method=re.search(r'types\.([A-Za-z]+Service/[A-Za-z]+)',operation.get('command',''))
        if not method: raise ValueError('Original RPC identity unavailable')
        expected_key=hashlib.sha256(json.dumps([method.group(1),request,operation['logical_key']],sort_keys=True).encode()).hexdigest()
        if expected_key != request_sha: raise ValueError('Original method/request/logical identity mismatch')
        response=normalize(operation.get('response') or {})
        facts.append({'logical_key':operation['logical_key'],'method':method.group(1),
                      'legacy_operation_sha256':request_sha,
                      'request_sha256':hashlib.sha256(json.dumps(request,sort_keys=True).encode()).hexdigest(),
                      'request_id':operation['request_id'],'original_source_commit':operation['source_commit'],
                      'request_money':request.get('amount_to_capture') or request.get('refund_amount') or request.get('amount'),
                      'metadata':financial_metadata(response),'status':response.get('status'),
                      'captured_amount':response.get('captured_amount'),
                      'connector_transaction_id':response.get('connector_transaction_id'),
                      'connector_refund_id':response.get('connector_refund_id'),
                      'current_mapping_verified':False,'financial_post_forbidden':True})
    financial_inventory=loaded['financial_inventory']
    if len(facts)!=financial_inventory.get('completed_call_count_excluding_oauth') or len(facts)!=resolution.get('financial_inventory_count'):
        raise ValueError('Original non-OAuth financial inventory count mismatch')
    indexed={row['logical_key']:row for row in financial_inventory['rows']}
    if len(indexed)!=len(facts): raise ValueError('Duplicate or missing imported ownership identity')
    for fact in facts:
        prior=indexed.get(fact['logical_key'],{})
        if prior.get('request_id')!=fact['request_id'] or prior.get('source_commit')!=fact['original_source_commit']:
            raise ValueError('Financial fact source/request ownership mismatch')
    provider=descriptor['actual_provider_response_projection']
    actual=descriptor['actual_response_projection']
    if (provider.get('id')!=actual.get('connector_transaction_id') or provider.get('id')!=sidecar.get('actual_order_id')
            or len(provider.get('captures',[]))!=1 or provider['captures'][0].get('id')!=sidecar.get('actual_provider_capture_id')):
        raise ValueError('Recovered descriptor resource ownership mismatch')
    facts.append({'logical_key':descriptor['logical_operation'],'method':'PaymentService/Authorize',
                  'request_id':descriptor['request_id'],'request_sha256':descriptor['pending_request_sha256'],
                  'original_source_commit':descriptor['source_commit'],
                  'order_id':provider['id'],'capture_id':provider['captures'][0]['id'],
                  'current_mapping_verified':False,'financial_post_forbidden':True})
    with journal_lock(args.journal):
        if args.journal.exists(): raise ValueError('Import destination already exists; never overwrite financial evidence')
        atomic(args.journal,{'schema':'paypal-imported-financial-facts/1','inputs':proof['files'],
                            'facts':facts,'provider_calls':0,'current_mapping_verified':False})
    return {'pass':True,'mode':'import-financial-facts','fact_count':len(facts),'provider_calls':0,
            'provider_live_verified':False,'current_mapping_verified':False,'evidence_state':'IMPLEMENTED'}


def original_payment_gap_reason(gap):
    """Persist only declared provenance codes, never exception/provider text."""
    allowed = {
        'ORIGINAL_PAYMENT_ORDER_LINEAGE_MISSING',
        'ORIGINAL_PAYMENT_AUTHORIZATION_INPUT_MISSING',
        'ORIGINAL_PAYMENT_INPUT_DIGEST_CONFLICT',
        'ORIGINAL_PAYMENT_SOURCE_LINEAGE_CONFLICT',
        'ORIGINAL_PAYMENT_CASE_IDENTITY_MISSING',
        'ORIGINAL_PAYMENT_RECIPE_MISSING',
        'ORIGINAL_PAYMENT_REQUEST_SHA_CONFLICT',
        'ORIGINAL_PAYMENT_INPUT_INCOMPLETE',
        'ORIGINAL_PAYMENT_MONEY_CONFLICT',
        'ORIGINAL_PAYMENT_AUTHORIZATION_INPUT_CONFLICT',
    }
    if type(gap) is ValueError and len(gap.args) == 1 and type(gap.args[0]) is str and gap.args[0] in allowed:
        return gap.args[0]
    return 'PROVENANCE_UNRESOLVED'


def original_payment_input(fact, imported, imported_path, journal):
    """Recover original authorization input; saved responses cannot supply money."""
    metadata = fact.get('metadata') or {}
    order = metadata.get('order_id') or fact.get('order_id')
    if not order:
        raise ValueError('ORIGINAL_PAYMENT_ORDER_LINEAGE_MISSING')
    candidates = [item for item in imported + list(journal['operations'].values())
                  if item.get('method') in ['PaymentService/Authorize','CompositePaymentService/Authorize',
                                              'RecurringPaymentService/Charge','PaymentMethodAuthenticationService/Authenticate']
                  and ((item.get('metadata') or {}).get('order_id') or item.get('order_id')) == order]
    if fact.get('method') in ['PaymentService/Authorize','CompositePaymentService/Authorize',
                                              'RecurringPaymentService/Charge','PaymentMethodAuthenticationService/Authenticate'] and fact not in candidates:
        candidates.append(fact)
    if not candidates:
        raise ValueError('ORIGINAL_PAYMENT_AUTHORIZATION_INPUT_MISSING')
    recovered = []
    for original in candidates:
        commit = original.get('original_source_commit') or at(original,'source_binding.commit')
        request = None
        provenance = {'commit':commit,'request_sha256':original.get('request_sha256'),'order_id':order}
        if original.get('legacy_operation_sha256'):
            data = json.loads(Path(imported_path).read_text())
            entry = data['inputs']['calls']
            if digest(entry['path']) != entry['sha256']:
                raise ValueError('ORIGINAL_PAYMENT_INPUT_DIGEST_CONFLICT')
            call = json.loads(Path(entry['path']).read_text())['calls'][original['legacy_operation_sha256']]
            request = call['request']
            if call.get('source_commit') != commit or call.get('logical_key') != original.get('logical_key'):
                raise ValueError('ORIGINAL_PAYMENT_SOURCE_LINEAGE_CONFLICT')
            provenance.update(input_path=entry['path'],input_sha256=entry['sha256'])
        else:
            # Exact historical Git recipe plus immutable session identity is usable
            # only if the resulting FULL request hash matches the original journal.
            source = str(MATRIX.relative_to(ROOT))
            raw = subprocess.run(['git','show',commit+':'+source],cwd=ROOT,capture_output=True,check=True).stdout
            manifest = json.loads(raw)
            identities = [key for key,item in journal['operations'].items() if item is original]
            if len(identities) != 1:
                raise ValueError('ORIGINAL_PAYMENT_CASE_IDENTITY_MISSING')
            identity = identities[0]
            rows = [row for row in manifest['rows'] if row['case_id']==identity]
            if len(rows) != 1 or not journal.get('session_seed'):
                raise ValueError('ORIGINAL_PAYMENT_RECIPE_MISSING')
            request = materialize_request(rows[0]['request'],'paypal-'+hashlib.sha256((journal['session_seed']+':'+identity).encode()).hexdigest()[:24])
            provenance.update(case_id=identity,git_matrix_sha256=hashlib.sha256(raw).hexdigest())
        if hashlib.sha256(json.dumps(request,sort_keys=True).encode()).hexdigest() != original.get('request_sha256'):
            raise ValueError('ORIGINAL_PAYMENT_REQUEST_SHA_CONFLICT')
        amount = request.get('amount')
        method = request.get('capture_method')
        if (not isinstance(amount,dict) or set(amount)!={'minor_amount','currency'}
                or not isinstance(amount.get('currency'),str) or not re.fullmatch('[A-Z]{3}',amount['currency'])
                or method not in ['AUTOMATIC','MANUAL','MANUAL_MULTIPLE','SCHEDULED','SEQUENTIAL_AUTOMATIC']):
            raise ValueError('ORIGINAL_PAYMENT_INPUT_INCOMPLETE')
        number(amount['minor_amount'])
        if original.get('request_money') is not None and original['request_money'] != amount:
            raise ValueError('ORIGINAL_PAYMENT_MONEY_CONFLICT')
        recovered.append(({'amount':copy.deepcopy(amount),'capture_method':method},provenance))
    if any(item[0] != recovered[0][0] for item in recovered):
        raise ValueError('ORIGINAL_PAYMENT_AUTHORIZATION_INPUT_CONFLICT')
    return recovered[0]


def materialize_request(value, identity, path=''):
    if isinstance(value,dict): return {key:materialize_request(item,identity,path+'.'+key) for key,item in value.items()}
    if isinstance(value,list): return [materialize_request(item,identity,path) for item in value]
    if value=='auto_generate':
        if 'email' in path: return 'paypal-regression@example.com'
        if 'zip_code' in path: return '95131'
        if 'phone_number' in path: return '6505550100'
        if any(name in path for name in ['first_name','last_name','card_holder_name','customer.name']): return 'Public Customer'
        if 'city' in path: return 'San Jose'
        if 'line' in path: return '2211 N First Street'
        return identity
    return value


def assertion_preflight(row):
    assertions = row.get('assertions')
    wire_surface = (row.get('source', {}).get('contract') == 'applepay_manufacturer_wire_contract.json'
                    or (row.get('provisioning') or '').startswith('WIRE_OBSERVER_REQUIRED')
                    or bool(row.get('wire_oracle')))
    if wire_surface:
        # No current-runtime provider wire observer is implemented here. Preserve
        # wire assertions exactly; never evaluate them against a gRPC response.
        return 'WIRE_OBSERVER_NOT_PROVISIONED'
    if not isinstance(assertions, dict):
        return 'UNSUPPORTED_CURRENT_ASSERTION_SCHEMA'
    for path, rules in assertions.items():
        if (not isinstance(path, str) or not isinstance(rules, dict) or not rules
                or set(rules) - {'equals','contains','must_exist','must_not_exist','one_of'}):
            return 'UNSUPPORTED_CURRENT_ASSERTION_SCHEMA'
        if any(rules.get(key) is not True for key in ['must_exist','must_not_exist'] if key in rules):
            return 'UNSUPPORTED_CURRENT_ASSERTION_SCHEMA'
        if 'one_of' in rules and not isinstance(rules['one_of'], list):
            return 'UNSUPPORTED_CURRENT_ASSERTION_SCHEMA'
    return None


def response_field_descriptor(method, path):
    """Resolve the public RPC output field, never infer types from field names."""
    classes, _ = public_error_types()
    pool = classes['ErrorInfo'].DESCRIPTOR.file.pool
    service, operation = method.split('/')
    try:
        descriptor = pool.FindServiceByName('types.' + service).methods_by_name[operation].output_type
    except KeyError:
        return None
    field = None
    for part in path.removeprefix('res.').split('.'):
        if part.isdigit() and field is not None and field.is_repeated:
            continue
        if descriptor is None or part not in descriptor.fields_by_name:
            return None
        field = descriptor.fields_by_name[part]
        descriptor = field.message_type
    return field


def descriptor_equal(value, wanted, method, path):
    field = response_field_descriptor(method, path) if method else None
    if field is not None and field.type == field.TYPE_INT64:
        def canonical(item):
            if isinstance(item, str) and not re.fullmatch(r'(?:0|-[1-9][0-9]*|[1-9][0-9]*)', item):
                raise ValueError('Noncanonical int64 projection')
            return number(item)
        try:
            return canonical(value) == canonical(wanted)
        except ValueError:
            return False
    if field is not None and field.message_type is not None and isinstance(value,dict) and isinstance(wanted,dict):
        return set(value)==set(wanted) and all(descriptor_equal(value[key],wanted[key],method,path+'.'+key) for key in wanted)
    return type(value) is type(wanted) and value == wanted


def source_assertions(response, assertions, method=None):
    failures=[]
    for path, rules in assertions.items():
        value=at(response,path.removeprefix('res.'))
        if not isinstance(rules,dict) or set(rules)-{'equals','contains','must_exist','must_not_exist','one_of'}:
            raise ValueError('Unsupported current committed assertion rule')
        for operation,wanted in rules.items():
            valid = (descriptor_equal(value,wanted,method,path) if operation=='equals' else
                     any(descriptor_equal(value,item,method,path) for item in wanted) if operation=='one_of' else
                     (value is not None) if operation=='must_exist' and wanted is True else
                     (value is None) if operation=='must_not_exist' and wanted is True else
                     (isinstance(value,(str,list,dict)) and wanted in value) if operation=='contains' else False)
            if not valid: failures.append(path+'.'+operation)
    return failures


class DurableRows(list):
    """Checkpoint every completed row without replacing previous-run evidence."""
    def __init__(self, args, journal):
        super().__init__()
        self.args = args
        self.journal = journal
        self.previous = json.loads(args.out.read_text()) if args.out.exists() else None

    def append(self, row):
        super().append(row)
        self.journal.setdefault('row_results', {})[row['case_id']] = row
        atomic(self.args.journal, self.journal)
        report = {'mode': self.args.mode, 'pass': False, 'complete': False,
                  'rows': list(self), 'bindings': self.args.execution_binding,
                  'provider_live_verified': False}
        if self.previous:
            report['previous_report'] = self.previous
        atomic(self.args.out, report)


def validate_imported_facts(path):
    """Rebuild the import from immutable inputs and compare every original fact."""
    data = json.loads(Path(path).read_text())
    if data.get('schema') != 'paypal-imported-financial-facts/1':
        raise ValueError('Invalid imported financial fact journal')
    import tempfile
    with tempfile.TemporaryDirectory(prefix='paypal-import-proof-') as directory:
        proof = {'schema': 'paypal-financial-fact-import/1', 'files': data['inputs']}
        if isinstance(data['inputs'], dict) and data['inputs'].get('schema'):
            proof = data['inputs']
        context = Path(directory) / 'context.json'
        context.write_text(json.dumps(proof))
        destination = Path(directory) / 'journal.json'
        import_financial_facts(argparse.Namespace(context=context, journal=destination))
        rebuilt = json.loads(destination.read_text())
        if rebuilt != data:
            raise ValueError('Imported original financial provenance mismatch')
    return data['facts']



def child_chain_uncertainty(args, journal, identity):
    """Read only the exact child journal, corroborated by parent RPC lineage."""
    if not identity.endswith(('/operation_chain', '/financial_closure_chain')):
        return None
    path = args.journal.with_name(args.journal.name + '.' + hashlib.sha256(identity.encode()).hexdigest()[:16])
    if not path.exists():
        return None
    try:
        raw = path.read_bytes()
        child = json.loads(raw)
        if (child.get('schema') != 'paypal-financial-regression-journal/1'
                or not isinstance(child.get('run_identity'), str) or not child['run_identity']
                or not isinstance(child.get('operations'), dict)):
            raise ValueError('Invalid child lineage')
        pending = []
        lineage_verified = True
        for name, operation in child['operations'].items():
            if operation.get('state') not in ['started', 'uncertain']:
                continue
            binding = operation.get('source_binding')
            if (not isinstance(binding, dict)
                    or not all(binding.get(key) for key in ['commit', 'source_sha256', 'binary_sha256'])
                    or not re.fullmatch(r'[0-9a-f]{64}', operation.get('request_sha256', ''))):
                raise ValueError('Invalid child provenance')
            calls = [call for call in journal.get('rpc_calls', {}).values()
                     if call.get('case_id') == identity
                     and call.get('method') == operation.get('method')
                     and call.get('request_sha256') == operation['request_sha256']]
            if calls and not any(call.get('case_id') == identity
                               and call.get('method') == operation.get('method')
                               and call.get('request_sha256') == operation['request_sha256']
                               and call.get('source_binding') == binding
                               for call in calls):
                raise ValueError('Uncorroborated child lineage')
            if not calls:
                lineage_verified = False
            pending.append({'operation': name, 'state': operation['state'], 'source_binding': binding})
        return ({'reason': 'AMBIGUOUS_CHILD_FINANCIAL_GET_ONLY', 'path': str(path),
                 'sha256': hashlib.sha256(raw).hexdigest(), 'operations': pending,
                 'lineage_verified': lineage_verified,
                 'parent_audit': 'CORROBORATED' if lineage_verified else 'PARENT_AUDIT_ABSENT',
                 'provenance': 'CORROBORATED' if lineage_verified else 'UNAVAILABLE'} if pending else None)
    except (OSError, ValueError, TypeError, AttributeError):
        return {'reason': 'CHILD_FINANCIAL_LINEAGE_MISMATCH_GET_ONLY'}


def matrix_execute(args):
    # Dependencies/source/type compatibility must fail BEFORE a journal or RPC.
    _, descriptor_binding = public_error_types()
    manifest=canonical_matrix()
    if json.loads(MATRIX.read_text())!=manifest: raise ValueError('Regenerate canonical matrix before execution')
    context=json.loads(args.context.read_text()) if args.context else {'cases':{}}
    strict_keys(context,['cases','imported_facts'],['cases'])
    imported=[]
    imported_path=None
    if context.get('imported_facts'):
        imported_path=Path(context['imported_facts'])
        imported=validate_imported_facts(imported_path)
    selected=set(args.case_id or [])
    known={row['case_id'] for row in manifest['rows']}
    if selected-known: raise ValueError('Unknown canonical case selection')
    cases=load_contracts()
    fixture_ids={row['fixture_id'] for row in manifest['rows'] if row.get('fixture_id') and (not selected or row['case_id'] in selected)}
    fixture_result=fixture_mode(args,[case for case in cases if case['id'] in fixture_ids]) if fixture_ids and args.mode=='matrix-fixture' else {'rows':[]}
    # Fixture execution and provider execution are separate modes and provenance.
    if args.mode=='matrix-fixture':
        observed={case['case_id']:case for case in fixture_result.get('cases',[])}
        outcomes=[]
        for row in manifest['rows']:
            observation=observed.get(row.get('fixture_id'))
            outcomes.append({'case_id':row['case_id'],
                             'state':observation['local_mapping_oracle'] if observation else 'NOT_RUN',
                             'evidence_state':'IMPLEMENTED','current_local_mapping_verified':bool(observation and observation['local_mapping_oracle']=='PASS'),
                             'provider_live_verified':False,'failed_assertion_paths':observation.get('failed_assertion_paths',[]) if observation else [],
                             'reason':None if observation else row['provisioning'] or 'Not selected or not a fixture surface'})
        return {'pass':bool(observed) and fixture_result.get('pass',False),'mode':args.mode,'case_count':len(manifest['rows']),
                'fixture_mapping':fixture_result,'provider_calls':0,'provider_live_verified':False,'rows':outcomes}
    config=os.environ.get('UCS_CONNECTOR_CONFIG'); token=os.environ.get('PAYPAL_ACCESS_TOKEN')
    cfg=json.loads(config or '{}'); host=urllib.parse.urlparse(at(cfg,'config.Paypal.base_url') or '')
    if not token or host.scheme!='https' or host.hostname not in ['api-m.sandbox.paypal.com','api.sandbox.paypal.com']:
        raise ValueError('Explicit sandbox configuration required')
    if not args.journal: raise ValueError('Durable matrix journal required')
    with journal_lock(args.journal):
        journal=json.loads(args.journal.read_text()) if args.journal.exists() else {'schema':'paypal-production-matrix-journal/1','operations':{},'session_seed':str(uuid.uuid4())}
        if journal.get('schema')!='paypal-production-matrix-journal/1': raise ValueError('Invalid matrix journal schema')
        if not isinstance(journal.get('session_seed'),str) or not journal['session_seed']:
            raise ValueError('Existing journal missing immutable session seed; preserve and reconcile')
        atomic(args.journal,journal)
        result=DurableRows(args, journal)
        active_row = [None]
        def audit(state, method, request, payload):
            calls = journal.setdefault('rpc_calls', {})
            if state == 'started':
                key = str(uuid.uuid4())
                calls[key] = {'state': state, 'method': method, 'case_id': active_row[0],
                              'request_sha256': hashlib.sha256(json.dumps(request, sort_keys=True).encode()).hexdigest(),
                              'source_binding': args.execution_binding, 'started_at_epoch': time.time()}
            else:
                key = payload[0] if state == 'completed' else payload
                record = calls[key]
                record.update(state=state, finished_at_epoch=time.time())
                if state == 'completed':
                    response, error, _ = payload[1]
                    record.update(error_present=error, status=response.get('status'),
                                  connector_transaction_id=response.get('connector_transaction_id'),
                                  connector_refund_id=response.get('connector_refund_id'),
                                  metadata=financial_metadata(response), captured_amount=response.get('captured_amount'))
            atomic(args.journal, journal)
            return key
        global RPC_AUDIT
        if RPC_AUDIT is not None:
            raise ValueError('Concurrent matrix execution is forbidden')
        RPC_AUDIT = audit
        try:
            for row in manifest['rows']:
                active_row[0] = row['case_id']
                try:
                    identity=row['case_id']; setup=context['cases'].get(identity)
                    if selected and identity not in selected:
                        result.append({'case_id':identity,'state':'NOT_RUN','reason':'Not selected'})
                        continue
                    if row.get('fixture_id') or str(row.get('method', '')).startswith(('HS/', 'HS ')) or row.get('surface') in ['hs', 'hyperswitch', 'fixture']:
                        result.append({'case_id':identity,'state':'NOT_RUN',
                                       'reason':'SEPARATE_LOCAL_FIXTURE_CONSUMER' if row.get('fixture_id') else 'SEPARATE_HS_CONSUMER',
                                       'provider_live_verified':False,'provider_wire_graded':False})
                        continue
                    preflight_reason=assertion_preflight(row)
                    if preflight_reason:
                        result.append({'case_id':identity,'state':'NOT_RUN','reason':preflight_reason,
                                       'provider_live_verified':False,'provider_wire_graded':False})
                        continue
                    sequential=identity.endswith(('/operation_chain','/financial_closure_chain'))
                    if sequential and setup and setup.get('allow_financial_write') is True:
                        chain_facts=[fact for fact in imported if fact['logical_key'].startswith('closure-' if identity.endswith('/financial_closure_chain') else 'capture-operation-')]
                        if chain_facts:
                            # A completed write is immutable history. Retest the current
                            # terminal GET contract, never the historical write mapping.
                            orders={}
                            for fact in chain_facts:
                                meta=fact.get('metadata') or {}
                                if meta.get('order_id'): orders[meta['order_id']]=meta
                            observations=[]
                            closure=identity.endswith('/financial_closure_chain')
                            expected_amount=3000 if closure else 6000
                            for order,meta in orders.items():
                                try:
                                    original_input, original_provenance = original_payment_input(
                                        {'metadata':meta},imported,imported_path,journal)
                                except (ValueError, KeyError, subprocess.CalledProcessError):
                                    observations.append({'error_present':False,'failed_assertion_paths':[],
                                        'original_input_gap':True,'reason':'ORIGINAL_PAYMENT_INPUT_PROVENANCE_UNRESOLVED'})
                                    continue
                                observed,error,_=rpc(args.endpoint,'PaymentService/Get',{'connector_transaction_id':order,
                                    'connector_feature_data':{'value':json.dumps(meta)},**original_input,'test_mode':True},config,token)
                                expected={'status':{'equals':'PARTIAL_CHARGED' if closure else 'CHARGED'},
                                          'captured_amount':{'equals':expected_amount}}
                                failures=source_assertions(observed,expected,'PaymentService/Get')
                                actual_meta=financial_metadata(observed)
                                if actual_meta.get('order_id')!=order: failures.append('owned_order_identity')
                                if closure and at(actual_meta,'capture_context.authorization_closed') is not True:
                                    failures.append('authorization_closed')
                                observations.append({'error_present':error,'status':observed.get('status'),
                                    'captured_amount':observed.get('captured_amount'),'failed_assertion_paths':failures,
                                    'original_payment_input_provenance':original_provenance})
                            current_pass=bool(observations) and all(not o['error_present'] and not o['failed_assertion_paths'] and not o.get('original_input_gap') for o in observations)
                            result.append({'case_id':identity,'state':'RECONCILED_READ_ONLY' if current_pass else 'NOT_RUN' if any(o.get('original_input_gap') for o in observations) else 'FAIL',
                                'current_mapping_verified':False,'current_terminal_get_verified':current_pass,
                                'provider_live_verified':False,'original_write_mapping_verified':False,
                                'reason':'Prior financial writes retained; only current terminal GET contract retested',
                                'observations':observations})
                            continue
                        child=copy.copy(args);child.mode='closure-chain' if identity.endswith('/financial_closure_chain') else 'shared-chain'
                        child.journal=args.journal.with_name(args.journal.name+'.'+hashlib.sha256(identity.encode()).hexdigest()[:16])
                        child.authorize_request=materialize_request(suite_scenarios('PaymentService/Authorize')['no3ds_manual_capture_credit_card']['grpc_req'],
                            journal['session_seed']+'-'+identity.replace('/','-'))
                        child.protected_financial_ids=[value for fact in imported for key,value in fact.items()
                            if key in ['order_id','capture_id','connector_transaction_id','connector_refund_id'] and isinstance(value,str)]
                        chain_result=live_mode(child)
                        result.append({'case_id':identity,'state':'VERIFIED' if chain_result.get('provider_live_verified') else 'FAIL',
                            'current_mapping_verified':chain_result.get('provider_live_verified',False),'chain':chain_result})
                        continue
                    if (not setup or not row.get('request') or row.get('fixture_id')
                            or (row.get('provisioning') or '').startswith(('SEQUENTIAL_CONTRACT_REQUIRED','API_UNSUPPORTED','PUBLIC_VARIANT_REQUIRED','PROVIDER_OUTCOME_REQUIRED','PROVIDER_DECLINE_REQUIRED'))):
                        result.append({'case_id':identity,'state':'NOT_RUN','reason':row['provisioning'] or 'OPERATOR_CONTEXT_REQUIRED'})
                        continue
                    strict_keys(setup,['bindings','env_bindings','allow_financial_write','decline_evidence','wire_observation','dependency_bindings'],['bindings'])
                    if setup.get('wire_observation'):
                        raise ValueError('Authored context is not a production wire observation')
                    if row.get('kind')=='decline':
                        negative=setup.get('decline_evidence') or {}
                        if (not negative.get('documentation_url','').startswith('https://developer.paypal.com/')
                                or not negative.get('binding_paths')
                                or not set(negative['binding_paths']) <= set(setup.get('bindings',{})) | set(setup.get('env_bindings',{}))
                                or not negative.get('assertions')
                                or not any('error' in path or 'status' in path for path in negative['assertions'])):
                            result.append({'case_id':identity,'state':'NOT_RUN','reason':'DOCUMENTED_DECLINE_BINDING_AND_ORACLE_REQUIRED'});continue
                        row=copy.deepcopy(row)
                        row['assertions']=negative['assertions']
                        preflight_reason=assertion_preflight(row)
                        if preflight_reason:
                            result.append({'case_id':identity,'state':'NOT_RUN','reason':preflight_reason});continue
                    request=materialize_request(row['request'],'paypal-'+hashlib.sha256((journal['session_seed']+':'+identity).encode()).hexdigest()[:24])
                    setup=copy.deepcopy(setup)
                    for target,dependency in setup.get('dependency_bindings',{}).items():
                        strict_keys(dependency,['case_id','path'],['case_id','path'])
                        prior=journal['operations'].get(dependency['case_id'])
                        if (not prior or prior.get('state')!='completed' or prior.get('failures')
                                or not prior.get('source_binding',{}).get('commit')):
                            raise ValueError('Dependency has no successful owned actual response')
                        if dependency['path']=='connector_feature_data.value':
                            value=json.dumps(prior.get('metadata') or {})
                        else:
                            if dependency['path'] not in ['connector_transaction_id','connector_refund_id','metadata.order_id','metadata.capture_id','metadata.authorize_id']:
                                raise ValueError('Dependency path is not an allowlisted financial identity')
                            value=at(prior,dependency['path'])
                        if not value: raise ValueError('Unresolved actual prerequisite identity')
                        if target in setup['bindings']: raise ValueError('Conflicting prerequisite sources')
                        setup['bindings'][target]=value
                    for path,value in setup['bindings'].items():
                        if any(word in path.lower() for word in ['token','secret','credential','card','cvc','cryptogram','signature']):
                            raise ValueError('Sensitive context must be resolved in memory, not persisted bindings')
                        parts=path.split('.'); target=request
                        for part in parts[:-1]:
                            if part not in target or not isinstance(target[part],dict): raise ValueError('Unknown prerequisite binding')
                            target=target[part]
                        if parts[-1] not in target: raise ValueError('Unknown prerequisite field')
                        target[parts[-1]]=value
                    for path,variable in setup.get('env_bindings',{}).items():
                        if not isinstance(variable,str) or not re.fullmatch(r'[A-Z][A-Z0-9_]+',variable) or variable not in os.environ:
                            raise ValueError('Missing in-memory operator binding')
                        parts=path.split('.'); target=request
                        for part in parts[:-1]:
                            if part not in target or not isinstance(target[part],dict): raise ValueError('Unknown environment prerequisite binding')
                            target=target[part]
                        if parts[-1] not in target: raise ValueError('Unknown environment prerequisite field')
                        target[parts[-1]]=os.environ[variable]
                    method=row['method']; readonly=method in ['PaymentService/Get','RefundService/Get']
                    # Required bindings must be explicit; materializing auto_generate is
                    # appropriate for merchant IDs, never proof of an owned resource.
                    required=row.get('prerequisite_bindings',[])
                    supplied=set(setup['bindings']) | set(setup.get('env_bindings',{}))
                    if set(required)-supplied:
                        result.append({'case_id':identity,'state':'NOT_RUN','reason':'EXPLICIT_PREREQUISITE_BINDINGS_REQUIRED','paths':sorted(set(required)-supplied)});continue
                    request_sha=hashlib.sha256(json.dumps(request,sort_keys=True).encode()).hexdigest()
                    existing=journal['operations'].get(identity)
                    old_fact=next((fact for fact in imported if fact['logical_key']==identity),None)
                    if existing and (existing.get('method')!=method or existing.get('request_sha256')!=request_sha
                                     or not existing.get('source_binding',{}).get('commit')):
                        raise ValueError('Journal method/request/source identity mismatch; no financial replay')
                    if old_fact and readonly and old_fact.get('request_sha256') != request_sha:
                        raise ValueError('Imported current readonly request binding mismatch')
                    if old_fact or existing:
                        fact=old_fact or existing
                        if old_fact and old_fact.get('method')!=method:
                            raise ValueError('Imported operation method mismatch; original request provenance is immutable')
                        metadata=fact.get('metadata') or {}; order=metadata.get('order_id') or fact.get('order_id')
                        if not order and not readonly:
                            result.append({'case_id':identity,'state':'NOT_RUN','reason':'PRIOR_FINANCIAL_OPERATION_GET_ONLY_IDENTITY_UNRESOLVED','current_mapping_verified':False}); continue
                        original_provenance=None
                        original_input={}
                        if not readonly:
                            try:
                                original_input,original_provenance=original_payment_input(fact,imported,imported_path,journal)
                            except (ValueError, KeyError, subprocess.CalledProcessError) as gap:
                                result.append({'case_id':identity,'state':'NOT_RUN','reason':original_payment_gap_reason(gap),
                                    'current_mapping_verified':False,'financial_post_forbidden':True});continue
                        query=request if readonly else {**original_input,'connector_transaction_id':order,'connector_feature_data':{'value':json.dumps(metadata)},'test_mode':True}
                        response,error,_=rpc(args.endpoint,method if readonly else 'PaymentService/Get',query,config,token)
                        current_get_failures=source_assertions(response,row['assertions'],method) if readonly else []
                        if not readonly and (response.get('connector_transaction_id') != order
                                             or financial_metadata(response).get('order_id') != order):
                            current_get_failures.append('owned_order_identity')
                        result.append({'case_id':identity,'state':'FAIL' if error or current_get_failures else 'VERIFIED' if readonly else 'RECONCILED_READ_ONLY',
                                       'current_mapping_verified':readonly and not error and not current_get_failures,
                                       'original_write_mapping_verified':False,'current_get_assertion_failures':current_get_failures,
                                       'original_method':fact.get('method'),'original_request_sha256':fact.get('request_sha256'),
                                       'original_source_commit':fact.get('original_source_commit') or at(fact,'source_binding.commit'),
                                       'new_recipe_request_sha256':request_sha,
                                       'historical_failures':fact.get('failures',[]),
                                       'original_write_status':fact.get('status'),'original_payment_input_provenance':original_provenance,
                                       'metadata':financial_metadata(response),'captured_amount':response.get('captured_amount')})
                        continue
                    if identity.endswith('/no3ds_fail_payment'):
                        result.append({'case_id':identity,'state':'NOT_RUN','reason':'SPEC_GAP/PROVIDER_DECLINE_NOT_PROVISIONED',
                                       'provider_live_verified':False})
                        continue
                    imported_ids=set(PROTECTED_FINANCIAL_IDS)
                    for fact in imported:
                        for key in ['order_id','capture_id','connector_transaction_id','connector_refund_id']:
                            if fact.get(key): imported_ids.add(fact[key])
                        meta=fact.get('metadata') or {}
                        imported_ids.update(value for key,value in meta.items() if key in ['order_id','authorize_id','capture_id','incremental_authorization_id'] and isinstance(value,str))
                        imported_ids.update((meta.get('capture_context') or {}).get('capture_amounts') or {})
                    required=row.get('prerequisite_bindings',[])
                    unresolved=[path for path in required if not at(request,path) or at(request,path)=='auto_generate']
                    if unresolved:
                        result.append({'case_id':identity,'state':'NOT_RUN','reason':'UNRESOLVED_PREREQUISITES','paths':unresolved});continue
                    if not readonly and method in ['PaymentService/Capture','PaymentService/Void','PaymentService/Refund']:
                        metadata=financial_metadata(request)
                        saved=request.get('connector_transaction_id')
                        owned=[record for record in list(journal['operations'].values())+imported
                               if saved and (saved==record.get('connector_transaction_id')
                                   or saved in {value for key,value in (record.get('metadata') or {}).items()
                                                if key in ['order_id','authorize_id','capture_id','incremental_authorization_id'] and isinstance(value,str)}
                                   or saved in ((record.get('metadata') or {}).get('capture_context') or {}).get('capture_amounts',{}))]
                        # Actual identity plus exact metadata, currency and known successful
                        # source are prerequisites; a caller-supplied amount/ID is not proof.
                        owned=[record for record in owned if record.get('metadata')==metadata
                               and (record.get('source_binding',{}).get('commit') or record.get('original_source_commit'))
                               and record.get('status') in ['AUTHORIZED','PARTIAL_CHARGED_AND_CHARGEABLE','PARTIAL_CHARGED','CHARGED']]
                        if not owned:
                            result.append({'case_id':identity,'state':'NOT_RUN','reason':'JOURNAL_OWNED_FINANCIAL_PREREQUISITE_REQUIRED'});continue
                        money=request.get('amount_to_capture') or request.get('refund_amount') or request.get('amount')
                        authoritative=at(metadata,'capture_context.authorized_amount')
                        if money and (not authoritative or money.get('currency')!=authoritative.get('currency_code')):
                            result.append({'case_id':identity,'state':'NOT_RUN','reason':'OWNED_CURRENCY_PROOF_REQUIRED'});continue
                    if not readonly and (setup.get('allow_financial_write') is not True or any(item in json.dumps(request) for item in imported_ids)):
                        result.append({'case_id':identity,'state':'NOT_RUN','reason':'FINANCIAL_WRITE_NOT_PROVISIONED_OR_PROTECTED_RESOURCE'});continue
                    if not row['assertions']:
                        result.append({'case_id':identity,'state':'NOT_RUN','reason':'MISSING_CURRENT_OUTCOME_ORACLE'});continue
                    journal['operations'][identity]={'method':method,'request_sha256':request_sha,'source_binding':args.execution_binding,'state':'started',
                        'request_money':request.get('amount_to_capture') or request.get('refund_amount') or request.get('amount')}
                    atomic(args.journal,journal)
                    response,error,_=rpc(args.endpoint,method,request,config,token)
                    failures=source_assertions(response,row['assertions'],method)
                    record=journal['operations'][identity]
                    record.update(state='completed',metadata=financial_metadata(response),status=response.get('status'),captured_amount=response.get('captured_amount'),
                                  connector_transaction_id=response.get('connector_transaction_id'),connector_refund_id=response.get('connector_refund_id'),failures=failures)
                    atomic(args.journal,journal)
                    wire_failures=[]
                    wire_oracle=row.get('wire_oracle')
                    wire_observed=False
                    if wire_oracle:
                        observation=setup.get('wire_observation') or {}
                        # Only an actual current-runtime observer can establish wire proof.
                        # Expected JSON supplied in context is never accepted as observation.
                        if observation:
                            raise ValueError('Wire observations require a production observer binding; authored context is not evidence')
                        wire_failures=['WIRE_OBSERVER_NOT_PROVISIONED']
                    local_refusal=error or row.get('kind') in ['refusal','decline'] or bool(at(row['assertions'],'error.must_exist'))
                    actual_status=response.get('status')
                    outcome_oracle=not local_refusal and (
                        (actual_status=='AUTHORIZED' and 'status' in row['assertions'] and bool(response.get('connector_transaction_id')))
                        or (actual_status in ['CHARGED','PARTIAL_CHARGED','PARTIAL_CHARGED_AND_CHARGEABLE']
                            and 'status' in row['assertions'] and request.get('amount_to_capture',request.get('amount',{})).get('minor_amount') is not None
                            and number(response.get('captured_amount'))==number(request.get('amount_to_capture',request.get('amount'))['minor_amount']))
                        or (actual_status=='REFUND_SUCCESS' and 'status' in row['assertions'] and bool(response.get('connector_refund_id'))
                            and response.get('refund_amount')==request.get('refund_amount'))
                        or (method=='EventService/HandleEvent' and response.get('source_verified') is True and 'source_verified' in row['assertions']))
                    verified=not failures and outcome_oracle
                    result.append({'case_id':identity,'state':'FAIL' if failures else 'OUTCOME_VERIFIED_WIRE_NOT_RUN' if wire_failures and verified else 'VERIFIED' if verified else 'LOCAL_ORACLE_PASS',
                                   'provider_live_verified':verified,'provider_wire_graded':wire_observed,'wire_oracle_state':'NOT_RUN' if wire_failures else 'NOT_APPLICABLE',
                                   'wire_oracle_failures':wire_failures,'current_mapping_verified':not failures,'failures':failures,
                                   'status':response.get('status'),'captured_amount':response.get('captured_amount'),'error_present':error})
                except (Exception, KeyboardInterrupt) as failure:
                    pending = [entry for entry in journal.get('rpc_calls', {}).values()
                               if entry.get('case_id') == active_row[0] and entry.get('state') in ['started', 'uncertain']]
                    child_pending = child_chain_uncertainty(args, journal, active_row[0])
                    result.append({'case_id': active_row[0], 'state': 'FAIL',
                                   'reason': child_pending['reason'] if child_pending else 'AMBIGUOUS_RPC_GET_ONLY' if pending else 'EXECUTION_OR_CONTRACT_FAILURE',
                                   'exception_type': type(failure).__name__, 'provider_live_verified': False,
                                   'dependent_chain_blocked': bool(pending or child_pending),
                                   'child_financial_uncertainty': child_pending})
                    if isinstance(failure, KeyboardInterrupt):
                        raise
        finally:
            RPC_AUDIT = None
        return {'pass':all(row['state']=='VERIFIED' for row in result),'mode':args.mode,'rows':result,
                'provider_live_verified':bool(result) and all(row['state']=='VERIFIED' for row in result),
                'case_count':len(manifest['rows']),'fixture_mapping':fixture_result,
                'previous_report':result.previous,'error_descriptor_binding':descriptor_binding,'complete':True}


def main():
    parser = argparse.ArgumentParser(description=__doc__, epilog='Runtime prerequisites: protoc on PATH and Python google.protobuf (message_factory.GetMessageClass API). Schema sources must match Git HEAD; dependency/type/source checks run before RPCs.')
    parser.add_argument('mode', choices=['validate-contracts', 'fixture', 'live-read-only', 'shared-chain', 'closure-chain', 'matrix-plan', 'import-financial-facts', 'matrix-fixture', 'matrix-live'])
    parser.add_argument('--case-id', action='append')
    parser.add_argument('--generate-matrix', action='store_true')
    parser.add_argument('--endpoint', default='127.0.0.1:8001')
    parser.add_argument('--binary', type=Path)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--journal', type=Path)
    parser.add_argument('--context', type=Path)
    parser.add_argument('--authorize', type=Path)
    parser.add_argument('--require-committed', action='store_true')
    parser.add_argument('--build-receipt', type=Path)
    args = parser.parse_args()
    try:
        if args.mode in ['matrix-plan','import-financial-facts']:
            result = matrix_plan(args) if args.mode=='matrix-plan' else import_financial_facts(args)
            atomic(args.out,result)
            print(json.dumps({key:result[key] for key in ['mode','pass','provider_calls']}))
            return 0
        if args.mode != 'validate-contracts':
            try:
                public_error_types()
            except (ImportError, AttributeError, ValueError, OSError, subprocess.SubprocessError):
                retained = json.loads(args.out.read_text()) if args.out.exists() else {}
                retained.update(mode=args.mode, **{'pass': False,
                                  'error': 'PROTOBUF_RUNTIME_DEPENDENCY_SOURCE_OR_TYPE_PREFLIGHT_FAILED',
                                  'provider_calls': 0, 'provider_live_verified': False})
                atomic(args.out, retained)
                print('Protobuf preflight failed before RPC: install protoc/Python protobuf and verify committed schema sources.')
                return 1
        cases = load_contracts()
        before = bindings(args.binary)
        if args.mode != 'validate-contracts' and not args.require_committed:
            raise ValueError('All runtime execution requires --require-committed')
        receipt = verify_build_receipt(args.build_receipt, before, args.binary) if args.require_committed else None
        args.execution_binding = dict(before, actual_build_receipt=receipt)
        runtime_pid = verify_runtime(args.endpoint, args.binary) if args.mode != 'validate-contracts' else None
        if args.mode == 'validate-contracts':
            result = {'mode': args.mode, 'pass': True, 'contract_cases': len(cases),
                      'executed_cases': 0, 'evidence_state': 'IMPLEMENTED',
                      'provider_live_verified': False, 'provider_calls': 0}
        elif args.mode == 'fixture':
            result = fixture_mode(args, cases)
        elif args.mode in ['matrix-fixture','matrix-live']:
            result = matrix_execute(args)
        else:
            if not args.journal:
                raise ValueError('Live execution requires a durable journal')
            result = live_mode(args)
        after_pid = verify_runtime(args.endpoint, args.binary) if runtime_pid else None
        if after_pid != runtime_pid:
            raise ValueError('Serving process changed during execution')
        after = bindings(args.binary)
        if before != after:
            raise ValueError('Source or runtime binary changed during execution')
        result['bindings'] = after
        result['runtime_pid'] = runtime_pid
        result['actual_build_receipt'] = receipt
        atomic(args.out, result)
        print(json.dumps({'mode': args.mode, 'pass': result['pass'],
                          'cases': result.get('case_count', result.get('contract_cases')),
                          'provider_live_verified': result.get('provider_live_verified', False)}))
        return 0 if result['pass'] else 1
    except (ValueError, OSError, subprocess.SubprocessError, KeyError, TypeError):
        # Never serialize exception values: transports can include authentication.
        retained = json.loads(args.out.read_text()) if args.out.exists() else {}
        retained.update(mode=args.mode, **{'pass': False, 'error': 'EXECUTION_OR_CONTRACT_FAILURE',
                          'evidence_state': 'IMPLEMENTED', 'provider_live_verified': False})
        atomic(args.out, retained)
        print('Execution/contract failure; no response or credential values emitted.')
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
