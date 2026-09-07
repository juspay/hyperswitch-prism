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
package org.killbill.billing.plugin.hyperswitch.model;

import java.util.List;
import java.util.Map;
import java.util.UUID;

import org.killbill.billing.payment.api.PluginProperty;
import org.killbill.billing.plugin.api.PluginProperties;
import org.killbill.billing.plugin.api.payment.PluginPaymentMethodPlugin;
import org.killbill.billing.plugin.hyperswitch.dao.HyperswitchDao;
import org.killbill.billing.plugin.hyperswitch.dao.gen.tables.records.HyperswitchPaymentMethodsRecord;

/**
 * Rebuilds a KillBill {@code PaymentMethodPlugin} from a persisted {@code hyperswitch_payment_methods} row.
 * {@code hyperswitch_id} (the connector token) is surfaced as the external payment-method id; card metadata,
 * mandate id, etc. come from {@code additional_data}. Mirrors {@code StripePaymentMethodPlugin}.
 */
public class HyperswitchPaymentMethodPlugin extends PluginPaymentMethodPlugin {

    public static HyperswitchPaymentMethodPlugin build(final HyperswitchPaymentMethodsRecord record) {
        final Map<?, ?> additionalData = HyperswitchDao.fromAdditionalData(record.getAdditionalData());
        return new HyperswitchPaymentMethodPlugin(UUID.fromString(record.getKbPaymentMethodId()),
                                                  record.getHyperswitchId(),
                                                  record.getIsDefault() != null && record.getIsDefault() == HyperswitchDao.DB_TRUE,
                                                  PluginProperties.buildPluginProperties(additionalData));
    }

    public HyperswitchPaymentMethodPlugin(final UUID kbPaymentMethodId,
                                          final String externalPaymentMethodId,
                                          final boolean isDefaultPaymentMethod,
                                          final List<PluginProperty> properties) {
        super(kbPaymentMethodId, externalPaymentMethodId, isDefaultPaymentMethod, properties);
    }
}
