import test from 'node:test';
import assert from 'node:assert/strict';
import { 
  NetworkError, 
  resolveProxyUrl, 
  generateProxyCacheKey,
  NetworkErrorCode
} from '../src/http_client';
// @ts-ignore
import { types } from '../src/payments/generated/proto';

test('NetworkError initialization and properties', () => {
  const err = new NetworkError(
    'Connection failed',
    types.NetworkErrorCode.CONNECT_TIMEOUT_EXCEEDED,
    504
  );
  
  assert.equal(err.message, 'Connection failed');
  assert.equal(err.code, types.NetworkErrorCode.CONNECT_TIMEOUT_EXCEEDED);
  assert.equal(err.statusCode, 504);
  assert.equal(err.errorCode, 'CONNECT_TIMEOUT_EXCEEDED');
  assert.equal(err.name, 'NetworkError');
});

test('NetworkError default values', () => {
  const err = new NetworkError('Simple error');
  
  assert.equal(err.message, 'Simple error');
  assert.equal(err.code, types.NetworkErrorCode.NETWORK_ERROR_CODE_UNSPECIFIED);
  assert.equal(err.statusCode, undefined);
  assert.equal(err.errorCode, 'NETWORK_ERROR_CODE_UNSPECIFIED');
});

test('resolveProxyUrl returns null when no proxy provided', () => {
  assert.equal(resolveProxyUrl('https://api.stripe.com'), null);
  assert.equal(resolveProxyUrl('https://api.stripe.com', null), null);
});

test('resolveProxyUrl returns httpUrl when httpsUrl is missing', () => {
  const proxy = { httpUrl: 'http://proxy.local:8080' };
  assert.equal(resolveProxyUrl('https://api.stripe.com', proxy), 'http://proxy.local:8080');
});

test('resolveProxyUrl returns httpsUrl when both are provided', () => {
  const proxy = { 
    httpUrl: 'http://proxy.local:8080',
    httpsUrl: 'https://proxy.local:8443'
  };
  assert.equal(resolveProxyUrl('https://api.stripe.com', proxy), 'https://proxy.local:8443');
});

test('resolveProxyUrl honors bypassUrls', () => {
  const proxy = { 
    httpsUrl: 'https://proxy.local:8443',
    bypassUrls: ['https://api.stripe.com']
  };
  assert.equal(resolveProxyUrl('https://api.stripe.com', proxy), null);
  assert.equal(resolveProxyUrl('https://api.adyen.com', proxy), 'https://proxy.local:8443');
});

test('generateProxyCacheKey handles empty proxy', () => {
  assert.equal(generateProxyCacheKey(), '');
  assert.equal(generateProxyCacheKey(null), '');
});

test('generateProxyCacheKey combines URLs predictably', () => {
  const proxy = { 
    httpUrl: 'http://proxy.local:8080',
    httpsUrl: 'https://proxy.local:8443'
  };
  assert.equal(generateProxyCacheKey(proxy), 'http://proxy.local:8080|https://proxy.local:8443|');
});

test('generateProxyCacheKey sorts bypassUrls for stable keys', () => {
  const proxy1 = { 
    httpUrl: 'http://proxy.local:8080',
    bypassUrls: ['https://api.b.com', 'https://api.a.com']
  };
  const proxy2 = { 
    httpUrl: 'http://proxy.local:8080',
    bypassUrls: ['https://api.a.com', 'https://api.b.com']
  };
  
  const key1 = generateProxyCacheKey(proxy1);
  const key2 = generateProxyCacheKey(proxy2);
  
  assert.equal(key1, key2);
  assert.equal(key1, 'http://proxy.local:8080||https://api.a.com,https://api.b.com');
});
