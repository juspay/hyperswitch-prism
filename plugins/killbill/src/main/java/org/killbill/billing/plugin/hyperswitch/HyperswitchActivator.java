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

import java.util.Hashtable;

import javax.servlet.Servlet;
import javax.servlet.http.HttpServlet;

import org.killbill.billing.osgi.api.OSGIPluginProperties;
import org.killbill.billing.osgi.libs.killbill.KillbillActivatorBase;
import org.killbill.billing.payment.plugin.api.PaymentPluginApi;
import org.killbill.billing.plugin.api.notification.PluginConfigurationEventHandler;
import org.killbill.billing.plugin.core.config.PluginEnvironmentConfig;
import org.killbill.billing.plugin.core.resources.jooby.PluginApp;
import org.killbill.billing.plugin.core.resources.jooby.PluginAppBuilder;
import org.killbill.billing.plugin.hyperswitch.client.PrismClient;
import org.killbill.billing.plugin.hyperswitch.client.SdkPrismClient;
import org.killbill.billing.plugin.hyperswitch.store.HyperswitchStateStore;
import org.killbill.billing.plugin.hyperswitch.webhook.HyperswitchServlet;
import org.osgi.framework.BundleContext;

public class HyperswitchActivator extends KillbillActivatorBase {

    public static final String PLUGIN_NAME = "killbill-hyperswitch";

    private HyperswitchConfigPropertiesConfigurationHandler configHandler;

    @Override
    public void start(final BundleContext context) throws Exception {
        super.start(context);

        // DB-less: connector state lives in KillBill's own storage (custom fields + core Payment API).
        final HyperswitchStateStore store = new HyperswitchStateStore(killbillAPI, clock.getClock());

        final String region = PluginEnvironmentConfig.getRegion(configProperties.getProperties());
        configHandler = new HyperswitchConfigPropertiesConfigurationHandler(PLUGIN_NAME, killbillAPI, region);
        final HyperswitchConfigProperties defaults = configHandler.createConfigurable(configProperties.getProperties());
        configHandler.setDefaultConfigurable(defaults);

        // The Prism client is stateless w.r.t. transport; it builds a per-tenant ConnectorConfig + client bundle
        // on demand from the resolved configuration (same as Medusa constructs its PaymentClient per options).
        final PrismClient prismClient = new SdkPrismClient(configHandler);

        // Register the payment plugin.
        final HyperswitchPaymentPluginApi pluginApi = new HyperswitchPaymentPluginApi(configHandler,
                                                                                      prismClient,
                                                                                      store,
                                                                                      killbillAPI,
                                                                                      clock.getClock());
        registerPaymentPluginApi(context, pluginApi);

        // Register the webhook servlet (connectors POST notifications to /plugins/killbill-hyperswitch).
        final PluginApp pluginApp = new PluginAppBuilder(PLUGIN_NAME,
                                                         killbillAPI,
                                                         dataSource,
                                                         super.clock,
                                                         configProperties).withRouteClass(HyperswitchServlet.class)
                                                                          .withService(pluginApi)
                                                                          .withService(prismClient)
                                                                          .withService(clock)
                                                                          .build();
        final HttpServlet servlet = PluginApp.createServlet(pluginApp);
        registerServlet(context, servlet);

        registerHandlers();
    }

    private void registerHandlers() {
        final PluginConfigurationEventHandler handler = new PluginConfigurationEventHandler(configHandler);
        dispatcher.registerEventHandlers(handler);
    }

    private void registerServlet(final BundleContext context, final HttpServlet servlet) {
        final Hashtable<String, String> props = new Hashtable<>();
        props.put(OSGIPluginProperties.PLUGIN_NAME_PROP, PLUGIN_NAME);
        registrar.registerService(context, Servlet.class, servlet, props);
    }

    private void registerPaymentPluginApi(final BundleContext context, final PaymentPluginApi api) {
        final Hashtable<String, String> props = new Hashtable<>();
        props.put(OSGIPluginProperties.PLUGIN_NAME_PROP, PLUGIN_NAME);
        registrar.registerService(context, PaymentPluginApi.class, api, props);
    }
}
