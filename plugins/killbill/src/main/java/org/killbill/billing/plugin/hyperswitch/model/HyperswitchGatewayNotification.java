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

import org.killbill.billing.plugin.api.payment.PluginGatewayNotification;

/**
 * The acknowledgement body returned to the connector after an inbound webhook is processed. Mirrors
 * {@code StripeGatewayNotification}: a thin wrapper carrying the response entity Kill Bill hands back.
 */
public class HyperswitchGatewayNotification extends PluginGatewayNotification {

    public HyperswitchGatewayNotification(final String entity) {
        super(entity);
    }

    public static HyperswitchGatewayNotification ack(final String entity) {
        return new HyperswitchGatewayNotification(entity);
    }
}
