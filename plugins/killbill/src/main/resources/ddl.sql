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

/*! SET default_storage_engine=INNODB */;

-- One row per KillBill transaction attempt (AUTHORIZE / CAPTURE / PURCHASE / VOID / REFUND / CREDIT).
-- hyperswitch_id holds the Prism connector_transaction_id (the key for capture/void/refund/PSync).
-- Everything else Prism returns (numeric status, mandate/recurring id, refund id, error, raw response)
-- is stashed in additional_data as JSON — same convention as killbill-stripe-plugin's stripe_responses.
create table hyperswitch_responses (
  record_id serial
, kb_account_id char(36) not null
, kb_payment_id char(36) not null
, kb_payment_transaction_id char(36) not null
, transaction_type varchar(32) not null
, amount numeric(15,9)
, currency char(3)
, hyperswitch_id varchar(255) default null
, additional_data longtext default null
, created_date datetime not null
, kb_tenant_id char(36) not null
, primary key(record_id)
) /*! CHARACTER SET utf8 COLLATE utf8_bin */;
create index hyperswitch_responses_kb_payment_id on hyperswitch_responses(kb_payment_id);
create index hyperswitch_responses_kb_payment_transaction_id on hyperswitch_responses(kb_payment_transaction_id);
create index hyperswitch_responses_hyperswitch_id on hyperswitch_responses(hyperswitch_id);

-- One row per KillBill payment method.
-- hyperswitch_id holds the primary reusable reference (connector payment-method token, e.g. Stripe pm_xxx);
-- the mandate / connectorRecurringPaymentId, connector customer id, card metadata, etc. live in additional_data.
create table hyperswitch_payment_methods (
  record_id serial
, kb_account_id char(36) not null
, kb_payment_method_id char(36) not null
, hyperswitch_id varchar(255) default null
, is_default smallint not null default 0
, is_deleted smallint not null default 0
, additional_data longtext default null
, created_date datetime not null
, updated_date datetime not null
, kb_tenant_id char(36) not null
, primary key(record_id)
) /*! CHARACTER SET utf8 COLLATE utf8_bin */;
create unique index hyperswitch_payment_methods_kb_payment_id on hyperswitch_payment_methods(kb_payment_method_id);
create index hyperswitch_payment_methods_hyperswitch_id on hyperswitch_payment_methods(hyperswitch_id);
