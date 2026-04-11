import { authorize, refund } from './payments';

async function runTests() {
  console.log('=== Payment Integration Tests ===\n');

  // Test 1: USD > $100 -> Cybersource
  console.log('--- Test 1: USD $150 -> Cybersource (authorize + refund) ---');
  try {
    const auth1 = await authorize({
      currency: 'USD',
      minorAmount: 15000, // $150.00
      cardNumber: '4111111111111111',
      cardExpMonth: '12',
      cardExpYear: '2027',
      cardCvc: '123',
      cardHolderName: 'Test Cybersource',
    });
    console.log('Auth result:', JSON.stringify(auth1, null, 2));

    if (auth1.connector !== 'cybersource') {
      console.error('FAIL: Expected cybersource, got', auth1.connector);
    } else {
      console.log('PASS: Routed to cybersource');
    }

    if (auth1.connectorTransactionId && (auth1.status === 8 || auth1.status === 6)) {
      console.log('\nRefunding...');
      const ref1 = await refund({
        connectorTransactionId: auth1.connectorTransactionId,
        refundMinorAmount: 15000,
        originalMinorAmount: 15000,
        currency: 'USD',
        reason: 'OTHER',
        connectorFeatureData: auth1.connectorFeatureData,
      });
      console.log('Refund result:', JSON.stringify(ref1, null, 2));
      if (ref1.status === 3 || ref1.status === 4) {
        console.log('PASS: Refund succeeded/pending');
      } else {
        console.log('WARN: Refund status:', ref1.statusName);
      }
    }
  } catch (err) {
    console.error('Test 1 error:', err);
  }

  console.log('\n--- Test 2: USD $50 -> Adyen (authorize + refund) ---');
  try {
    const auth2 = await authorize({
      currency: 'USD',
      minorAmount: 5000, // $50.00
      cardNumber: '4111111111111111',
      cardExpMonth: '03',
      cardExpYear: '2030',
      cardCvc: '737', // Adyen sandbox requires 737
      cardHolderName: 'Test Adyen',
    });
    console.log('Auth result:', JSON.stringify(auth2, null, 2));

    if (auth2.connector !== 'adyen') {
      console.error('FAIL: Expected adyen, got', auth2.connector);
    } else {
      console.log('PASS: Routed to adyen');
    }

    if (auth2.connectorTransactionId && (auth2.status === 8 || auth2.status === 6)) {
      console.log('\nRefunding...');
      const ref2 = await refund({
        connectorTransactionId: auth2.connectorTransactionId,
        refundMinorAmount: 5000,
        originalMinorAmount: 5000,
        currency: 'USD',
        reason: 'OTHER',
        connectorFeatureData: auth2.connectorFeatureData,
      });
      console.log('Refund result:', JSON.stringify(ref2, null, 2));
      if (ref2.status === 3 || ref2.status === 4) {
        console.log('PASS: Refund succeeded/pending');
      } else {
        console.log('WARN: Refund status:', ref2.statusName);
      }
    }
  } catch (err) {
    console.error('Test 2 error:', err);
  }

  console.log('\n--- Test 3: EUR 25 -> PayPal (authorize + refund) ---');
  try {
    const auth3 = await authorize({
      currency: 'EUR',
      minorAmount: 2500, // EUR 25.00
      cardNumber: '4111111111111111',
      cardExpMonth: '12',
      cardExpYear: '2027',
      cardCvc: '123',
      cardHolderName: 'Test PayPal',
    });
    console.log('Auth result:', JSON.stringify(auth3, null, 2));

    if (auth3.connector !== 'paypal') {
      console.error('FAIL: Expected paypal, got', auth3.connector);
    } else {
      console.log('PASS: Routed to paypal');
    }

    if (auth3.connectorTransactionId && (auth3.status === 8 || auth3.status === 6)) {
      console.log('\nRefunding...');
      const ref3 = await refund({
        connectorTransactionId: auth3.connectorTransactionId,
        refundMinorAmount: 2500,
        originalMinorAmount: 2500,
        currency: 'EUR',
        reason: 'OTHER',
        connectorFeatureData: auth3.connectorFeatureData,
      });
      console.log('Refund result:', JSON.stringify(ref3, null, 2));
      if (ref3.status === 3 || ref3.status === 4) {
        console.log('PASS: Refund succeeded/pending');
      } else {
        console.log('WARN: Refund status:', ref3.statusName);
      }
    }
  } catch (err) {
    console.error('Test 3 error:', err);
  }

  console.log('\n=== Tests Complete ===');
}

runTests().catch(console.error);
