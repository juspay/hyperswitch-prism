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
package org.killbill.billing.plugin.hyperswitch.webhook;

import java.nio.charset.StandardCharsets;
import java.util.HashMap;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;

import javax.inject.Inject;
import javax.inject.Named;
import javax.inject.Singleton;

import org.jooby.MediaType;
import org.jooby.Request;
import org.jooby.Result;
import org.jooby.Results;
import org.jooby.Status;
import org.jooby.mvc.Local;
import org.jooby.mvc.POST;
import org.jooby.mvc.Path;
import org.killbill.billing.osgi.libs.killbill.OSGIKillbillClock;
import org.killbill.billing.plugin.api.PluginCallContext;
import org.killbill.billing.plugin.core.resources.PluginHealthcheck;
import org.killbill.billing.plugin.hyperswitch.HyperswitchActivator;
import org.killbill.billing.plugin.hyperswitch.HyperswitchPaymentPluginApi;
import org.killbill.billing.plugin.hyperswitch.model.HyperswitchGatewayNotification;
import org.killbill.billing.tenant.api.Tenant;
import org.killbill.billing.util.callcontext.CallContext;

/**
 * Inbound connector webhooks: {@code POST /plugins/killbill-hyperswitch/webhook}. Reads the raw request and
 * hands it to {@link HyperswitchPaymentPluginApi#handleWebhook}, which verifies + applies the event via Prism
 * {@code EventService.handle_event}.
 */
@Singleton
@Path("/webhook")
public class HyperswitchServlet extends PluginHealthcheck {

    private final OSGIKillbillClock clock;
    private final HyperswitchPaymentPluginApi pluginApi;

    @Inject
    public HyperswitchServlet(final OSGIKillbillClock clock, final HyperswitchPaymentPluginApi pluginApi) {
        this.clock = clock;
        this.pluginApi = pluginApi;
    }

    @POST
    public Result webhook(final Request request,
                          @Local @Named("killbill_tenant") final Optional<Tenant> tenant) throws Exception {
        if (!tenant.isPresent()) {
            return Results.with("{\"status\":\"error\",\"reason\":\"missing_tenant\"}", Status.BAD_REQUEST).type(MediaType.json);
        }
        final UUID tenantId = tenant.get().getId();
        final CallContext context = new PluginCallContext(HyperswitchActivator.PLUGIN_NAME, clock.getClock().getUTCNow(), null, tenantId);

        final Map<String, String> headers = new HashMap<>();
        request.headers().forEach((name, value) -> headers.put(name.toLowerCase(), value.toOptional().orElse("")));
        final String raw = request.body(String.class);
        final byte[] body = raw == null ? new byte[0] : raw.getBytes(StandardCharsets.UTF_8);

        final HyperswitchGatewayNotification notification = pluginApi.handleWebhook(tenantId, request.path(), headers, body, context);
        return Results.with(notification.getEntity(), Status.OK).type(MediaType.json);
    }
}
