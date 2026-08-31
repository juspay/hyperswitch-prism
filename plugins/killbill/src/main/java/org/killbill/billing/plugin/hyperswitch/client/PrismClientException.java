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

/**
 * Wraps failures from the Prism SDK ({@code IntegrationError} = request-phase / don't retry,
 * {@code ConnectorError} = response-phase / don't retry, {@code NetworkError} = safe to retry) into a single
 * checked exception the plugin translates to a KillBill {@code PaymentPluginApiException}.
 */
public class PrismClientException extends Exception {

    private static final long serialVersionUID = 1L;

    private final String errorCode;
    private final boolean retryable;

    public PrismClientException(final String message, final String errorCode, final boolean retryable, final Throwable cause) {
        super(message, cause);
        this.errorCode = errorCode;
        this.retryable = retryable;
    }

    public String getErrorCode() {
        return errorCode;
    }

    /** Whether the failure is transient (network) and the same request may be safely retried. */
    public boolean isRetryable() {
        return retryable;
    }
}
