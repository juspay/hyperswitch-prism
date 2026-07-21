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
package org.killbill.billing.plugin.hyperswitch;

import java.util.Properties;

import javax.annotation.Nullable;

/**
 * Typed view over the per-tenant {@code org.killbill.billing.plugin.hyperswitch.*} configuration.
 *
 * <p>Single connector per tenant, mirroring the Medusa plugin's {@code HyperswitchPrismOptions}
 * ({@code connector} + {@code connectorConfig} + {@code environment} + {@code webhookSecret}). The raw
 * {@link Properties} are retained so {@code PrismClientFactory}/{@code SdkPrismClient} can read the
 * connector-namespaced credential keys (e.g. {@code stripe.apiKey}) when building the Prism
 * {@code ConnectorConfig}.
 */
public class HyperswitchConfigProperties {

    public static final String PROPERTY_PREFIX = "org.killbill.billing.plugin.hyperswitch.";

    private static final String DEFAULT_ENVIRONMENT = "SANDBOX";
    private static final String DEFAULT_CAPTURE_METHOD = "AUTOMATIC";

    private final Properties properties;
    private final String region;

    private final String connector;
    private final String environment;
    private final String captureMethod;
    private final String returnUrl;
    private final String webhookSecret;

    public HyperswitchConfigProperties(final Properties properties, final String region) {
        this.properties = properties;
        this.region = region;
        this.connector = trimToNull(properties.getProperty(PROPERTY_PREFIX + "connector"));
        this.environment = properties.getProperty(PROPERTY_PREFIX + "environment", DEFAULT_ENVIRONMENT);
        this.captureMethod = properties.getProperty(PROPERTY_PREFIX + "captureMethod", DEFAULT_CAPTURE_METHOD);
        this.returnUrl = trimToNull(properties.getProperty(PROPERTY_PREFIX + "returnUrl"));
        this.webhookSecret = trimToNull(properties.getProperty(PROPERTY_PREFIX + "webhookSecret"));
    }

    /** The Prism connector to route to (e.g. {@code stripe}, {@code adyen}) — selects the ConnectorSpecificConfig variant. */
    @Nullable
    public String getConnector() {
        return connector;
    }

    /** {@code SANDBOX} or {@code PRODUCTION} (maps to Prism {@code SdkOptions.environment}). */
    public String getEnvironment() {
        return environment;
    }

    /** Default capture behaviour for {@code purchasePayment} — {@code AUTOMATIC} (sale) or {@code MANUAL} (auth-only). */
    public String getCaptureMethod() {
        return captureMethod;
    }

    @Nullable
    public String getReturnUrl() {
        return returnUrl;
    }

    /** Webhook verification secret passed to Prism {@code EventService.handle_event}. */
    @Nullable
    public String getWebhookSecret() {
        return webhookSecret;
    }

    public String getRegion() {
        return region;
    }

    /**
     * Read a credential key namespaced under the active connector, e.g. {@code getConnectorProperty("apiKey")}
     * resolves {@code org.killbill.billing.plugin.hyperswitch.<connector>.apiKey}.
     */
    @Nullable
    public String getConnectorProperty(final String key) {
        if (connector == null) {
            return null;
        }
        return trimToNull(properties.getProperty(PROPERTY_PREFIX + connector + "." + key));
    }

    /** Raw properties, for building the Prism {@code ConnectorConfig} in the client layer. */
    public Properties getRawProperties() {
        return properties;
    }

    @Nullable
    private static String trimToNull(@Nullable final String value) {
        if (value == null) {
            return null;
        }
        final String trimmed = value.trim();
        return trimmed.isEmpty() ? null : trimmed;
    }
}
