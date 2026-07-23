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
import java.util.UUID;

import javax.annotation.Nullable;

import org.killbill.billing.payment.api.PluginProperty;
import org.killbill.billing.plugin.api.payment.PluginPaymentMethodPlugin;

/**
 * A KillBill {@code PaymentMethodPlugin} built from plain fields (DB-less). {@code externalPaymentMethodId}
 * carries the connector token (recovered from a KillBill custom field).
 */
public class HyperswitchPaymentMethodPlugin extends PluginPaymentMethodPlugin {

    public HyperswitchPaymentMethodPlugin(final UUID kbPaymentMethodId,
                                          @Nullable final String externalPaymentMethodId,
                                          final boolean isDefaultPaymentMethod,
                                          final List<PluginProperty> properties) {
        super(kbPaymentMethodId, externalPaymentMethodId, isDefaultPaymentMethod, properties);
    }
}
