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

import org.killbill.billing.osgi.libs.killbill.OSGIKillbillAPI;
import org.killbill.billing.plugin.api.notification.PluginTenantConfigurableConfigurationHandler;

/**
 * Resolves {@link HyperswitchConfigProperties} per tenant. Global defaults come from
 * {@code hyperswitch.properties}; tenants override via the KillBill tenant config-upload API.
 */
public class HyperswitchConfigPropertiesConfigurationHandler extends PluginTenantConfigurableConfigurationHandler<HyperswitchConfigProperties> {

    private final String region;

    public HyperswitchConfigPropertiesConfigurationHandler(final String pluginName,
                                                           final OSGIKillbillAPI osgiKillbillAPI,
                                                           final String region) {
        super(pluginName, osgiKillbillAPI);
        this.region = region;
    }

    @Override
    protected HyperswitchConfigProperties createConfigurable(final Properties properties) {
        return new HyperswitchConfigProperties(properties, region);
    }
}
