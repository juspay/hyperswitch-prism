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

import org.killbill.billing.payment.api.TransactionType;
import org.killbill.billing.payment.plugin.api.PaymentPluginStatus;
import org.testng.Assert;
import org.testng.annotations.Test;

/** Pure unit test — no SDK, network, or DB. */
public class TestPrismStatusMapper {

    @Test(groups = "fast")
    public void testAuthorizeStatuses() {
        Assert.assertEquals(PrismStatusMapper.map("AUTHORIZED", TransactionType.AUTHORIZE), PaymentPluginStatus.PROCESSED);
        Assert.assertEquals(PrismStatusMapper.map("AUTHORIZING", TransactionType.AUTHORIZE), PaymentPluginStatus.PENDING);
        Assert.assertEquals(PrismStatusMapper.map("AUTHORIZATION_FAILED", TransactionType.AUTHORIZE), PaymentPluginStatus.ERROR);
        // VOIDED under an AUTHORIZE is a cancellation, not a success.
        Assert.assertEquals(PrismStatusMapper.map("VOIDED", TransactionType.AUTHORIZE), PaymentPluginStatus.CANCELED);
    }

    @Test(groups = "fast")
    public void testCaptureAndPurchaseStatuses() {
        Assert.assertEquals(PrismStatusMapper.map("CHARGED", TransactionType.CAPTURE), PaymentPluginStatus.PROCESSED);
        Assert.assertEquals(PrismStatusMapper.map("CHARGED", TransactionType.PURCHASE), PaymentPluginStatus.PROCESSED);
        Assert.assertEquals(PrismStatusMapper.map("PARTIAL_CHARGED", TransactionType.PURCHASE), PaymentPluginStatus.PROCESSED);
        Assert.assertEquals(PrismStatusMapper.map("CAPTURE_INITIATED", TransactionType.CAPTURE), PaymentPluginStatus.PENDING);
        Assert.assertEquals(PrismStatusMapper.map("CAPTURE_FAILED", TransactionType.CAPTURE), PaymentPluginStatus.ERROR);
    }

    @Test(groups = "fast")
    public void testVoidStatuses() {
        // VOIDED under a VOID op is success.
        Assert.assertEquals(PrismStatusMapper.map("VOIDED", TransactionType.VOID), PaymentPluginStatus.PROCESSED);
        Assert.assertEquals(PrismStatusMapper.map("VOID_FAILED", TransactionType.VOID), PaymentPluginStatus.ERROR);
    }

    @Test(groups = "fast")
    public void testRefundStatuses() {
        Assert.assertEquals(PrismStatusMapper.map("REFUND_SUCCESS", TransactionType.REFUND), PaymentPluginStatus.PROCESSED);
        Assert.assertEquals(PrismStatusMapper.map("REFUND_PENDING", TransactionType.REFUND), PaymentPluginStatus.PENDING);
        Assert.assertEquals(PrismStatusMapper.map("REFUND_FAILURE", TransactionType.REFUND), PaymentPluginStatus.ERROR);
        Assert.assertEquals(PrismStatusMapper.map("REFUND_TRANSACTION_FAILURE", TransactionType.REFUND), PaymentPluginStatus.ERROR);
    }

    @Test(groups = "fast")
    public void testUnknownAndNull() {
        Assert.assertEquals(PrismStatusMapper.map(null, TransactionType.AUTHORIZE), PaymentPluginStatus.UNDEFINED);
        Assert.assertEquals(PrismStatusMapper.map("SOMETHING_NEW", TransactionType.AUTHORIZE), PaymentPluginStatus.UNDEFINED);
        Assert.assertEquals(PrismStatusMapper.map("PAYMENT_STATUS_UNSPECIFIED", TransactionType.CAPTURE), PaymentPluginStatus.UNDEFINED);
    }
}
