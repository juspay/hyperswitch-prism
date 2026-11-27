/** Client-side handoff using metadata from ONE Prism Authorize call with checkout_mode=sdk.
 * Keep secret_api_key on the server. Native apps open the returned universal link;
 * browser checkouts should use the redirection flow instead.
 */
export async function getUniversalLink(session, { appReturnUrl, source } = {}, fetcher = fetch) {
  const allowed = ['https://checkout.revolut.com', 'https://sandbox-checkout.revolut.com'];
  if (!allowed.includes(session.checkout_base_url) || !session.order_token || !session.public_key) {
    throw new Error('Missing or invalid Revolut Pay session');
  }
  if (Boolean(appReturnUrl) === Boolean(source)) {
    throw new Error('Supply appReturnUrl for same-device, or source for cross-device payment');
  }
  if (source && !source.startsWith('INPERSON_')) throw new Error('source must start with INPERSON_');
  const url = new URL('/get-universal-link', session.checkout_base_url);
  url.searchParams.set('orderToken', session.order_token);
  url.searchParams.set('publicKey', session.public_key);
  if (appReturnUrl) url.searchParams.set('appReturnUrl', appReturnUrl);
  if (source) url.searchParams.set('src', source);
  const response = await fetcher(url.toString(), { method: 'GET', credentials: 'omit' });
  if (!response.ok) throw new Error(`Revolut universal-link request failed: ${response.status}`);
  const result = await response.json();
  if (typeof result.universal_link !== 'string') throw new Error('Missing universal_link');
  const link = new URL(result.universal_link);
  if (link.protocol !== 'https:') throw new Error('Invalid universal_link protocol');
  return link.toString();
}
