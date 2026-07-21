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

import org.killbill.billing.catalog.api.Currency;
import org.testng.Assert;
import org.testng.annotations.Test;

/** Pure unit test — verifies ISO-4217 minor-unit conversion (2 / 0 / 3 fraction digits). */
public class TestAmountConverter {

    @Test(groups = "fast")
    public void testTwoDecimalCurrency() {
        Assert.assertEquals(AmountConverter.toMinor(new BigDecimal("10.00"), Currency.USD), 1000L);
        Assert.assertEquals(AmountConverter.toMinor(new BigDecimal("10.005"), Currency.EUR), 1001L); // HALF_UP
        Assert.assertEquals(AmountConverter.fromMinor(1000L, Currency.USD), new BigDecimal("10.00"));
    }

    @Test(groups = "fast")
    public void testZeroDecimalCurrency() {
        Assert.assertEquals(AmountConverter.toMinor(new BigDecimal("1000"), Currency.JPY), 1000L);
        Assert.assertEquals(AmountConverter.fromMinor(1000L, Currency.JPY), new BigDecimal("1000"));
    }

    @Test(groups = "fast")
    public void testThreeDecimalCurrency() {
        Assert.assertEquals(AmountConverter.toMinor(new BigDecimal("1.000"), Currency.BHD), 1000L);
        Assert.assertEquals(AmountConverter.fromMinor(1000L, Currency.BHD), new BigDecimal("1.000"));
    }
}
