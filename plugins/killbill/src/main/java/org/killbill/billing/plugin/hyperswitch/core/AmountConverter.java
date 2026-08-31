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
package org.killbill.billing.plugin.hyperswitch.core;

import java.math.BigDecimal;
import java.math.RoundingMode;

import org.killbill.billing.catalog.api.Currency;

/**
 * Converts between KillBill's {@link BigDecimal} major-unit amounts and Prism's {@code minor_amount} (int64).
 * Uses the ISO-4217 fraction digits from {@link java.util.Currency} (JPY/KRW = 0, most = 2, BHD/KWD = 3), so no
 * hand-maintained exponent table is needed. Java port of Medusa's {@code toMinorAmount}/{@code fromMinorAmount}.
 */
public final class AmountConverter {

    private AmountConverter() {
    }

    public static long toMinor(final BigDecimal amount, final Currency currency) {
        return amount.movePointRight(fractionDigits(currency)).setScale(0, RoundingMode.HALF_UP).longValueExact();
    }

    public static BigDecimal fromMinor(final long minorAmount, final Currency currency) {
        return BigDecimal.valueOf(minorAmount).movePointLeft(fractionDigits(currency));
    }

    private static int fractionDigits(final Currency currency) {
        try {
            final int digits = java.util.Currency.getInstance(currency.name()).getDefaultFractionDigits();
            return digits < 0 ? 2 : digits;
        } catch (final IllegalArgumentException e) {
            return 2;
        }
    }
}
