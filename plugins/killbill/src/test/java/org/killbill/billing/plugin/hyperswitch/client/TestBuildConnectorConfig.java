/*
 * Copyright 2024 Juspay
 *
 * Licensed under the Apache License, version 2.0 (the "License"); you may not
 * use this file except in compliance with the License. You may obtain a copy
 * of the License at http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS, WITHOUT
 * WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied. See the
 * License for the specific language governing permissions and limitations
 * under the License.
 */
package org.killbill.billing.plugin.hyperswitch.client;

import java.util.Properties;

import org.killbill.billing.plugin.hyperswitch.HyperswitchConfigProperties;
import org.testng.Assert;
import org.testng.annotations.Test;

// NOTE (P0 verification): asserts against the SDK proto types (payments.*, types.Payment.*). Builds proto
// messages only (no native lib / DB), so it runs once the io.hyperswitch:prism proto classes resolve on the
// classpath — the same P0 package-name reconciliation as the rest of the module.
import payments.ConnectorConfig;
import payments.ConnectorSpecificConfig;
import payments.Environment;

/**
 * Verifies the parity connector set (stripe, adyen, braintree, cybersource, paypal, forte) each map to the
 * correct {@code ConnectorSpecificConfig} oneof case with credentials populated, and that an out-of-scope
 * connector (a KillBill gateway Prism lacks) is rejected.
 */
public class TestBuildConnectorConfig {

    private static final String PREFIX = "org.killbill.billing.plugin.hyperswitch.";

    private static ConnectorConfig build(final String connector, final String... kv) throws PrismClientException {
        final Properties p = new Properties();
        p.setProperty(PREFIX + "connector", connector);
        for (int i = 0; i < kv.length; i += 2) {
            p.setProperty(PREFIX + connector + "." + kv[i], kv[i + 1]);
        }
        final HyperswitchConfigProperties config = new HyperswitchConfigProperties(p, "test");
        return new SdkPrismClient(null).buildConnectorConfig(config);
    }

    @Test(groups = "fast")
    public void testStripe() throws Exception {
        final ConnectorSpecificConfig c = build("stripe", "apiKey", "sk_test_123").getConnectorConfig();
        Assert.assertTrue(c.hasStripe());
        Assert.assertEquals(c.getStripe().getApiKey().getValue(), "sk_test_123");
    }

    @Test(groups = "fast")
    public void testAdyen() throws Exception {
        final ConnectorSpecificConfig c = build("adyen", "apiKey", "k", "merchantAccount", "MyMerchant").getConnectorConfig();
        Assert.assertTrue(c.hasAdyen());
        Assert.assertEquals(c.getAdyen().getApiKey().getValue(), "k");
        Assert.assertEquals(c.getAdyen().getMerchantAccount().getValue(), "MyMerchant");
    }

    @Test(groups = "fast")
    public void testBraintree() throws Exception {
        final ConnectorSpecificConfig c = build("braintree", "publicKey", "pub", "privateKey", "priv").getConnectorConfig();
        Assert.assertTrue(c.hasBraintree());
        Assert.assertEquals(c.getBraintree().getPublicKey().getValue(), "pub");
        Assert.assertEquals(c.getBraintree().getPrivateKey().getValue(), "priv");
    }

    @Test(groups = "fast")
    public void testCybersource() throws Exception {
        final ConnectorSpecificConfig c = build("cybersource", "apiKey", "k", "merchantAccount", "m", "apiSecret", "s").getConnectorConfig();
        Assert.assertTrue(c.hasCybersource());
        Assert.assertEquals(c.getCybersource().getApiKey().getValue(), "k");
        Assert.assertEquals(c.getCybersource().getMerchantAccount().getValue(), "m");
        Assert.assertEquals(c.getCybersource().getApiSecret().getValue(), "s");
    }

    @Test(groups = "fast")
    public void testPaypal() throws Exception {
        final ConnectorSpecificConfig c = build("paypal", "clientId", "cid", "clientSecret", "csecret").getConnectorConfig();
        Assert.assertTrue(c.hasPaypal());
        Assert.assertEquals(c.getPaypal().getClientId().getValue(), "cid");
        Assert.assertEquals(c.getPaypal().getClientSecret().getValue(), "csecret");
    }

    @Test(groups = "fast")
    public void testForte() throws Exception {
        final ConnectorSpecificConfig c = build("forte",
                                                "apiAccessId", "aid", "organizationId", "org",
                                                "locationId", "loc", "apiSecretKey", "sk").getConnectorConfig();
        Assert.assertTrue(c.hasForte());
        Assert.assertEquals(c.getForte().getApiAccessId().getValue(), "aid");
        Assert.assertEquals(c.getForte().getOrganizationId().getValue(), "org");
        Assert.assertEquals(c.getForte().getLocationId().getValue(), "loc");
        Assert.assertEquals(c.getForte().getApiSecretKey().getValue(), "sk");
    }

    @Test(groups = "fast")
    public void testProductionEnvironment() throws Exception {
        final Properties p = new Properties();
        p.setProperty(PREFIX + "connector", "stripe");
        p.setProperty(PREFIX + "environment", "PRODUCTION");
        p.setProperty(PREFIX + "stripe.apiKey", "sk_live_1");
        final ConnectorConfig cc = new SdkPrismClient(null).buildConnectorConfig(new HyperswitchConfigProperties(p, "test"));
        Assert.assertEquals(cc.getOptions().getEnvironment(), Environment.PRODUCTION);
    }

    // A KillBill gateway with no Prism equivalent must be rejected (the documented parity boundary).
    @Test(groups = "fast", expectedExceptions = PrismClientException.class)
    public void testUnsupportedConnectorThrows() throws Exception {
        build("gocardless", "apiKey", "x");
    }

    @Test(groups = "fast", expectedExceptions = PrismClientException.class)
    public void testMissingConnectorThrows() throws Exception {
        new SdkPrismClient(null).buildConnectorConfig(new HyperswitchConfigProperties(new Properties(), "test"));
    }
}
