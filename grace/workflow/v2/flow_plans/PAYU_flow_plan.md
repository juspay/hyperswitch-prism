# Flow Plan: PAYU

## Summary

- **Connector**: PAYU
- **Techspec**: /home/kanikachaudhary/workflow/hyperswitch-prism/euler-techspec-output/PAYU_spec.md
- **Scope**: 6 core flows (Authorize, PSync, Capture, Void, Refund, RSync)
- **Flows to Implement**: 4 (Capture, Void, Refund, RSync)
- **Existing (skip)**: [PSync]
- **Existing (PM additions pending)**: [Authorize — has UPI (UpiIntent, UpiCollect, UpiQr), needs +Wallet (REDIRECT_WALLET_DEBIT), +Netbanking (NET BANKING)]
- **Not Supported**: []

---

## 1. Gateway Presence & Scaffolding

- **Status**: EXISTS in connector-service
- **Connector module**: `payu.rs`
- **Connector enum variant**: `ConnectorEnum::Payu`
- **Scaffolding action taken**: None — connector already exists

---

## 2. Tech Spec Summary

- **Gateway**: PAYU
- **Base URL (Txns/Authorize)**: `https://test.payu.in/_payment` (sandbox), `https://secure.payu.in/_payment` (production)
- **Base URL (Postservice)**: `https://test.payu.in/merchant/postservice.php?form=2` (sandbox), `https://info.payu.in/merchant/postservice.php?form=2` (production)
- **Auth mechanism**: HMAC-SHA512 hash embedded in request body field `hash` (not HTTP header). Standard hash: `SHA512(key|txnid|amount|productinfo|firstname|email|udf1..udf10|salt)`. Verify hash: `SHA512(key|command|txnId|salt)`. Capture/Void/Refund hash: `SHA512(key|command|var1|salt)`.
- **Operations found (6 core)**:
  - Authorize → POST `/_payment`
  - PSync → POST `/merchant/postservice.php?form=2` (command=`verify_payment`)
  - Capture → POST `/merchant/postservice.php?form=2` (command=`capture_transaction`)
  - Void → POST `/merchant/postservice.php?form=2` (command=`cancel_refund_transaction`)
  - Refund → POST `/merchant/postservice.php?form=2` (command=`cancel_refund_transaction`)
  - RSync → POST `/merchant/postservice.php?form=2` (command=`getAllRefundsFromTxnIds`)
- **Payment methods found**: Card (CC, DC, RuPay, Amex, Diners, Maestro), UPI (Collect, Intent, QR), Net Banking, Wallet (Google Pay via UPI/TEZ, LazyPay via BNPL), Mandate/SI, EMI, BNPL

---

## 3. Flow Mapping (6 Core Flows)

| # | Tech Spec Operation | Tech Spec Endpoint | Connector-Service Flow | Status | Notes |
|---|---------------------|--------------------|------------------------|--------|-------|
| 1 | `initiateTxn` / `initiateTransaction` (Flow.hs) | POST `/_payment` | Authorize | ✅ (EXISTING_PM_PENDING) | UPI (Collect/Intent/QR) implemented; Wallet and Netbanking missing |
| 2 | `verifyPayment` / `getPayUTxnStatusResponse` (Flow.hs) | POST `/merchant/postservice.php?form=2` command=`verify_payment` | PSync | ✅ | Fully implemented via `macro_connector_implementation!` with `get_url()` override |
| 3 | `captureTxn` (Flow.hs:1453) | POST `/merchant/postservice.php?form=2` command=`capture_transaction` | Capture | ❌ | Stub — empty `ConnectorIntegrationV2<Capture,...>` impl |
| 4 | `voidTxn` (Flow.hs:1478) | POST `/merchant/postservice.php?form=2` command=`cancel_refund_transaction` | Void | ❌ | Stub — empty `ConnectorIntegrationV2<Void,...>` impl |
| 5 | `initPayuRefundRequestApi` / `RefundResponseHandler` (RefundResponseHandler.hs:125-248) | POST `/merchant/postservice.php?form=2` command=`cancel_refund_transaction` | Refund | ❌ | Stub — empty `ConnectorIntegrationV2<Refund,...>` impl |
| 6 | Refund ARN Sync / `getAllRefundsFromTxnIds` (Flow.hs:4971-5007) | POST `/merchant/postservice.php?form=2` command=`getAllRefundsFromTxnIds` | RSync | ❌ | Stub — empty `ConnectorIntegrationV2<RSync,...>` impl |

---

## 4. Payment Method / PMT Mapping

| # | Tech Spec PM | Tech Spec PMT | CS PaymentMethod | CS PaymentMethodType | CS PaymentMethodData Variant | Status | Priority | Refunds | Capture Methods |
|---|--------------|---------------|------------------|----------------------|------------------------------|--------|----------|---------|-----------------|
| 1 | UPI | UPI_PAY (Intent) | `PaymentMethod::Upi` | `PaymentMethodType::UpiIntent` | `PaymentMethodData::Upi(UpiData::UpiIntent(UpiIntentData))` | Implemented in transformer | MANDATORY | Yes (after CHARGED) | AutoCapture only (no pre-auth for UPI) |
| 2 | UPI | UPI_QR | `PaymentMethod::Upi` | `PaymentMethodType::UpiQr` | `PaymentMethodData::Upi(UpiData::UpiQr(UpiQrData))` | Implemented in transformer (treated as Intent flow) | MANDATORY | Yes (after CHARGED) | AutoCapture only |
| 3 | UPI | UPI_COLLECT | `PaymentMethod::Upi` | `PaymentMethodType::UpiCollect` | `PaymentMethodData::Upi(UpiData::UpiCollect(UpiCollectData))` | Implemented in transformer | MANDATORY | Yes (after CHARGED) | AutoCapture only |
| 4 | WALLET | REDIRECT_WALLET_DEBIT | `PaymentMethod::Wallet` | Connector-specific per-wallet variant (e.g., `PaymentMethodType::Wallet`) | `PaymentMethodData::Wallet(WalletData::<WalletName>Redirect(<WalletNameRedirectData>))` | NOT YET — no match arm in transformers | MANDATORY | Yes (after CHARGED) | AutoCapture only |
| 5 | NET BANKING | Netbanking (all bank codes from techspec) | `PaymentMethod::Netbanking` | `PaymentMethodType::Netbanking` | `PaymentMethodData::Netbanking(NetbankingData)` with `bank_code` field | NOT YET — no match arm in transformers | MANDATORY | Yes (after CHARGED) | AutoCapture only |
| 6 | CARD | Card (CC/DC/RuPay/Amex/Diners/Maestro) | `PaymentMethod::Card` | `PaymentMethodType::Card` | `PaymentMethodData::Card(Card<T>)` | NOT YET (techspec documents it via `pg`+`bankcode` routing) | OPTIONAL | Yes | Manual & Auto |

> **Note on NET BANKING**: The mandatory entry `"NET BANKING": []` with empty array means implement all net banking bank codes the techspec mentions. The techspec (Section 8.1) documents NB payments use `pg=NB` with bank-specific `bankcode`. PayU supports standard Indian banks (HDFC, SBI, ICICI, AXIS, etc.). The `NetbankingData.bank_code` maps to PayU's `bankcode` field directly.

> **Note on WALLET**: Techspec shows wallets route via `pg` + `bankcode`. Google Pay routes as `pg=UPI, bankcode=TEZ`. LazyPay routes as `pg=BNPL, bankcode=LAZYPAY`. The REDIRECT_WALLET_DEBIT PMT maps to per-wallet `WalletData` variants per the hard gate reference branch.

---

## 5. API Call Sequence (Ordered)

### 5.1 Payment Flow (Auto-Capture — UPI/Wallet/Netbanking)
1. Authorize → POST `/_payment` (returns `AuthenticationPending` for redirect flows)
2. PSync → POST `/merchant/postservice.php?form=2` (command=`verify_payment`)

### 5.2 Payment Flow (Manual Capture — Card)
1. Authorize → POST `/_payment` (returns `Authorized`)
2. PSync → POST `/merchant/postservice.php?form=2` (command=`verify_payment`)
3. Capture → POST `/merchant/postservice.php?form=2` (command=`capture_transaction`)
4. PSync → POST `/merchant/postservice.php?form=2` (command=`verify_payment`)

### 5.3 Void Flow (Card Pre-Auth)
1. Authorize (manual) → POST `/_payment`
2. Void → POST `/merchant/postservice.php?form=2` (command=`cancel_refund_transaction`)

### 5.4 Refund Flow
1. Refund → POST `/merchant/postservice.php?form=2` (command=`cancel_refund_transaction`, var1=mihpayid, var2=amount, var3=txnId)
2. RSync → POST `/merchant/postservice.php?form=2` (command=`getAllRefundsFromTxnIds`, var1=txnId list)

---

## 6. Gap Analysis

### 6.1 Tech Spec Features Not in Scope
| Feature | Category | Notes |
|---------|----------|-------|
| OTP Submit / Resend OTP | 3DS/OTP | Not in 6 core flows — skipped |
| Mandate/SI Register (`si_transaction`) | SetupMandate | Not in 6 core flows — skipped |
| Execute Mandate | RepeatPayment | Not in 6 core flows — skipped |
| Revoke Mandate Token | MandateRevoke | Not in 6 core flows — skipped |
| Check Mandate Status | Extra | Not in 6 core flows — skipped |
| 3DS Authentication params fetch | PreAuthenticate | Not in 6 core flows — skipped |
| VPA Validation (`validateVPA`) | Extra | Not in 6 core flows — skipped |
| Surcharge check (`get_additional_charge`) | Extra | Not in 6 core flows — skipped |
| Split Settlement (`payment_split`) | Extra | Not in 6 core flows — skipped |
| Settlement Details (`get_settlement_details`) | Gateway only | Not in 6 core flows — skipped |
| EMI Plans / DC EMI eligibility | Gateway only | Not in 6 core flows — skipped |
| Direct Debit (LinkAndPay) | Extra | Not in 6 core flows — skipped |
| Capture/Void Sync (`check_action_status`) | Extra PSync variant | Used internally, maps to PSync flow for capture/void state — covered by existing PSync |
| Refund ARN Sync (`getAllRefundsFromTxnIds`) | RSync | Mapped to RSync flow — to be implemented |
| Pre-debit notification (`pre_debit_SI`) | Mandate | Not in 6 core flows — skipped |
| Token Update (`update_SI`) | Mandate | Not in 6 core flows — skipped |

### 6.2 Missing Info for Core Flows
| Flow | What's Missing | Impact |
|------|---------------|--------|
| Refund | Exact `PayuRefundStatusResponse` structure not fully specified in techspec (only ADT constructors shown). The refund response `status` field can be int or string (`PayuRefundStatusType`). | Low — techspec section 7.11 provides clear mapping rules |
| RSync | `PayuRefundArnSyncRequest` uses command `getAllRefundsFromTxnIds`, var1=txnId list. The exact response structure `PayuRefundArnSyncResponse` is not fully expanded in techspec. | Medium — will need to reference pattern guides and connector patterns |
| Capture | Response structure reuses `PayuCaptureOrVoidSyncResponse` (section 4.2) which is well-documented | Low |
| Void | Same response structure as Capture (`PayuCaptureOrVoidSyncResponse`) | Low |
| Authorize (Wallet) | Per-wallet `WalletData` enum variants not yet in codebase — requires hard gate reference branch | Medium — hard gate reference provided |
| Authorize (Netbanking) | `NetbankingData` struct and match arm not yet in transformers | Medium — hard gate reference provided |

---

## 7. Hard Gate References

### Netbanking (MANDATORY — plan includes NET BANKING)
- **Branch**: https://github.com/juspay/hyperswitch-prism/tree/feat/pmt-netbanking
- **Flow Agent MUST follow**:
  - `PaymentMethodData::Netbanking(NetbankingData)` with `bank_code: String` and `bank_name: Option<String>`
  - `bank_code` maps to PayU `bankcode` field (e.g., `"HDFC"`, `"SBIN"`, `"ICICI"`)
  - `pg` field = `"NB"` for net banking
  - Async redirect pattern: returns `AuthenticationPending` with redirect URL
  - Currency validation: PayU netbanking typically supports INR only
  - Redirect form construction with `RedirectForm::Form`

### Wallet Redirect (MANDATORY — plan includes WALLET with REDIRECT_WALLET_DEBIT)
- **Branch**: https://github.com/juspay/hyperswitch-prism/tree/feat/per-wallet-indian-variants-v2
- **Flow Agent MUST follow**:
  - Per-wallet enum variants in `WalletData` (e.g., `WalletData::PhonePeRedirect(PhonePeRedirectData)`, `WalletData::GooglePayRedirect(GooglePayRedirectData)`, etc.)
  - Empty data structs carrying type identity (e.g., `pub struct PhonePeRedirectData {}`)
  - Connector maps variant to PayU `pg` + `bankcode` (e.g., `WalletData::GooglePayRedirect(_)` → `pg="UPI"`, `bankcode="TEZ"`)
  - Returns `AuthenticationPending` with `redirection_data`
  - Status MUST be `AuthenticationPending` (NOT `Charged`) for redirect wallets

---

## Implementation Order (for Master Agent consumption)

### Mandatory Payment Methods (NON-NEGOTIABLE)

```json
{"UPI": ["UPI_PAY", "UPI_QR", "UPI_COLLECT"], "WALLET": ["REDIRECT_WALLET_DEBIT"], "NET BANKING": []}
```

---

### 1. Authorize (EXISTING_PM_PENDING)

- **Status**: EXISTING_PM_PENDING
- **Techspec Section**: "Flow 1: `initiateTxn` / `initiateTransaction`" (lines 958-982), "3.2 PayuTransactionRequest" (lines 152-199), "3.3 PayuUpiTransactionRequest" (lines 201-224), "Section 8.1 Payment Method Routing" (lines 1843-1863)
- **API Endpoint**: POST `/_payment`
- **HTTP Method**: POST
- **Content-Type**: `application/x-www-form-urlencoded`
- **gRPC Service**: `types.PaymentService/Authorize`
- **Pattern Guide**: `grace/rulesbook/codegen/guides/patterns/pattern_authorize.md`
- **Sub-Pattern Guides**:
  - Wallet: `grace/rulesbook/codegen/guides/patterns/authorize/wallet/pattern_authorize_wallet.md`
  - Net Banking: `grace/rulesbook/codegen/guides/patterns/authorize/net_banking/pattern_authorize_net_banking.md`
- **Existing PMs (already implemented)**:
  - `PaymentMethod::Upi` + `UpiIntent` → `PaymentMethodData::Upi(UpiData::UpiIntent(_))` → `pg="UPI"`, `bankcode="INTENT"`, `txn_s2s_flow="2"`
  - `PaymentMethod::Upi` + `UpiQr` → `PaymentMethodData::Upi(UpiData::UpiQr(_))` → same as UpiIntent
  - `PaymentMethod::Upi` + `UpiCollect` → `PaymentMethodData::Upi(UpiData::UpiCollect(data))` → `pg="UPI"`, `bankcode="UPI"`, `vpa=data.vpa_id`
- **Pending Mandatory PMs**:
  - `PaymentMethod::Wallet` + `REDIRECT_WALLET_DEBIT` → Per-wallet `WalletData` variants → `pg` + `bankcode` per wallet (e.g., GooglePay: `pg="UPI"`, `bankcode="TEZ"`; PhonePe: `pg="UPI"`, `bankcode="PHONEPE"` etc.)
  - `PaymentMethod::Netbanking` + `Netbanking` → `PaymentMethodData::Netbanking(NetbankingData)` → `pg="NB"`, `bankcode=bank_code`
- **Action**: Add match arms for Wallet and Netbanking PaymentMethodData variants to the existing `TryFrom<PayuRouterData<...>> for PayuPaymentRequest` in `transformers.rs`. Do NOT modify existing UPI handling.
- **Key Request Fields**: `key`, `txnid`, `amount`, `productinfo`, `firstname`, `email`, `phone`, `surl`, `furl`, `hash`, `pg`, `bankcode`, `upi_vpa`, `txn_s2s_flow`, `s2s_client_ip`, `s2s_device_info`, `api_version`
- **Key Response Fields**: `status` (Int 1 for UPI Intent / String "success" for UPI Collect), `reference_id` / `txn_id` (connector_transaction_id), `intent_uri_data` (for UPI Intent redirect), `result.mihpayid` (for UPI Collect), `error` (error code), `message` (error message)
- **Status Mapping for Wallet/NB (redirect flows)**: Return `AttemptStatus::AuthenticationPending` with redirect URL in `redirection_data`
- **Mandatory Payment Methods**:
  - `Upi:UPI_PAY` (UpiIntent) — already implemented
  - `Upi:UPI_QR` (UpiQr) — already implemented
  - `Upi:UPI_COLLECT` (UpiCollect) — already implemented
  - `Wallet:REDIRECT_WALLET_DEBIT` — PENDING
  - `Netbanking:Netbanking` — PENDING
- **Optional Payment Methods**: `Card:Card` (OPTIONAL, not mandatory)
- **Hard Gate References**:
  - Wallet: https://github.com/juspay/hyperswitch-prism/tree/feat/per-wallet-indian-variants-v2
  - Netbanking: https://github.com/juspay/hyperswitch-prism/tree/feat/pmt-netbanking
- **Testing Notes**: For Wallet/NB redirect flows, response must be `REQUIRES_CUSTOMER_ACTION` status (AuthenticationPending). For UPI flows, already tested.

---

### 2. Capture

- **Status**: PLAN
- **Techspec Section**: "Flow 3: `captureTxn`" (lines 995-1012), "3.6 PayuCaptureRequest" (lines 246-254), "4.2 PayuCaptureOrVoidSyncResponse" (lines 544-557)
- **API Endpoint**: POST `/merchant/postservice.php?form=2`
- **HTTP Method**: POST
- **Content-Type**: `application/x-www-form-urlencoded`
- **gRPC Service**: `types.PaymentService/Capture`
- **Pattern Guide**: `grace/rulesbook/codegen/guides/patterns/pattern_capture.md`
- **Key Request Fields**:
  - `key` — Merchant key
  - `command` — `"capture_transaction"` (literal string)
  - `var1` — PayU payment ID (`mihpayid`) — this is the `connector_transaction_id` from Authorize response
  - `var2` — Amount to capture (decimal string)
  - `hash` — `SHA512(key|command|var1|salt)` (standard `makePayuHash` formula)
- **Key Response Fields**: `payuId`, `status`, `amount`, `message`, `requestId`, `error_code`, `error_description`
- **Status Mapping**:
  - Success + status indicates captured → `AttemptStatus::Charged`
  - Error (`Left err`) → `AttemptStatus::CaptureFailed` (error code: `CAPTURE_PROCESSING_FAILED`)
- **Mandatory Payment Methods**: N/A (Capture is PM-agnostic; only applicable to pre-authorized Card payments)
- **Optional Payment Methods**: N/A
- **Hard Gate References**: None
- **Testing Notes**: Testing Agent must run its OWN fresh Authorize with MANUAL capture (`CaptureMethod::Manual`), then use the `connector_transaction_id` (mihpayid) from that Authorize for Capture. The mihpayid is returned in the Authorize response `reference_id` field.

---

### 3. Void

- **Status**: PLAN
- **Techspec Section**: "Flow 4: `voidTxn`" (lines 1013-1029), "3.7 PayuVoidRequest" (lines 256-263)
- **API Endpoint**: POST `/merchant/postservice.php?form=2`
- **HTTP Method**: POST
- **Content-Type**: `application/x-www-form-urlencoded`
- **gRPC Service**: `types.PaymentService/Void`
- **Pattern Guide**: `grace/rulesbook/codegen/guides/patterns/pattern_void.md`
- **Key Request Fields**:
  - `key` — Merchant key
  - `command` — `"cancel_refund_transaction"` (same command as Refund, but WITHOUT `var2`/`var3`)
  - `var1` — PayU payment ID (`mihpayid`) — `connector_transaction_id` from Authorize
  - `hash` — `SHA512(key|command|var1|salt)`
- **Key Response Fields**: Same as Capture response structure (`PayuCaptureOrVoidSyncResponse`): `payuId`, `status`, `amount`, `message`, `requestId`, `error_code`, `error_description`
- **Status Mapping**:
  - Success → `AttemptStatus::Voided`
  - Error (`Left err`) → `AttemptStatus::VoidFailed` (error code: `VOID_PROCESSING_FAILED`)
- **IMPORTANT**: Void and Refund share the same `command` value (`"cancel_refund_transaction"`). Void uses `var1` only (no amount). Refund uses `var1` + `var2` (amount) + `var3` (txnId).
- **Mandatory Payment Methods**: N/A
- **Optional Payment Methods**: N/A
- **Hard Gate References**: None
- **Testing Notes**: Testing Agent must run its OWN fresh Authorize with MANUAL capture, then Void WITHOUT Capturing.

---

### 4. Refund

- **Status**: PLAN
- **Techspec Section**: "Flow 15: `initPayuRefundRequestApi` / `RefundResponseHandler`" (lines 1150-1169), "3.5 PayuRefundRequest" (lines 235-244), "Section 7.11 Refund Status Mapping" (lines 1778-1799)
- **API Endpoint**: POST `/merchant/postservice.php?form=2`
- **HTTP Method**: POST
- **Content-Type**: `application/x-www-form-urlencoded`
- **gRPC Service**: `types.PaymentService/Refund`
- **Pattern Guide**: `grace/rulesbook/codegen/guides/patterns/pattern_refund.md`
- **Key Request Fields**:
  - `key` — Merchant key
  - `command` — `"cancel_refund_transaction"` (same command as Void, but WITH `var2`/`var3`)
  - `var1` — PayU payment ID (`mihpayid`) — `connector_transaction_id` from Authorize
  - `var2` — Refund amount (decimal string)
  - `var3` — Transaction ID (`txnid`) — original transaction ID
  - `hash` — `SHA512(key|command|var1|salt)` (standard makePayuHash with var1)
- **Key Response Fields**: PayU returns `PayuRefundResp` ADT:
  - `SuccessRefundFetch PayuRefundStatusResponse` — contains `status` (int or string), `mihpayid`, `refundId`
  - `SplitRefundFetch PayuSplitRefundStatusResponse` — split refund response
  - `FailureRefundResponse` — contains error string
- **Status Mapping** (Section 7.11):
  - `StatusIntType 0` → `RefundStatus::Failure`
  - `StatusIntType n` (n ≠ 0) → `RefundStatus::Pending`
  - `StatusStringType "failure"` / `"failed"` → `RefundStatus::Failure`
  - `StatusStringType "success"` → `RefundStatus::Success`
  - `StatusStringType "od_hit"` → `RefundStatus::Pending`
  - Other string → `RefundStatus::Pending`
  - `FailureRefundResponse "No Refunds Found"` → error with code 404
  - `FailureRefundResponse "DB_EXCEPTION_ERROR_SLAVE"` / `"Requests limit reached"` → `RefundStatus::Pending`
  - Socket timeout → `RefundStatus::Pending`
- **Mandatory Payment Methods**: N/A (Refund is PM-agnostic)
- **Optional Payment Methods**: N/A
- **Hard Gate References**: None
- **Testing Notes**: Use `connector_transaction_id` (mihpayid) from the AUTOMATIC-capture Authorize for `var1`. The `var3` (txnid) is the original merchant transaction ID (`connector_request_reference_id`).

---

### 5. RSync

- **Status**: PLAN
- **Techspec Section**: "Flow 30: Refund ARN Sync" (lines 1315-1322), "3.16 PayuRefundArnSyncRequest" (lines 338-346), "Section 7.5 RefundSyncStatus" (lines 1624-1635), "Section 7.11 RefundSyncStatus Aggregation" (lines 1790-1799)
- **API Endpoint**: POST `/merchant/postservice.php?form=2`
- **HTTP Method**: POST
- **Content-Type**: `application/x-www-form-urlencoded`
- **gRPC Service**: `types.RefundService/Get`
- **Pattern Guide**: `grace/rulesbook/codegen/guides/patterns/pattern_rsync.md`
- **Key Request Fields**:
  - `key` — Merchant key
  - `command` — `"getAllRefundsFromTxnIds"`
  - `var1` — Transaction ID (txnId list — for single refund, this is the original `txnid`)
  - `hash` — `SHA512(key|command|var1|salt)`
- **Key Response Fields**:
  - `RefundSyncStatus` aggregation result (section 7.5): `REFUND_SUCCESS`, `REFUND_FAILURE`, `REFUND_PENDING`, `REFUND_NOT_FOUND`, `REFUND_MANUAL_REVIEW`
  - Aggregation rules (section 7.11): All success → `REFUND_SUCCESS`; All failure → `REFUND_FAILURE`; All pending → `REFUND_PENDING`; Mixed failure+success → `REFUND_MANUAL_REVIEW`; Not found → `REFUND_NOT_FOUND`
  - Error on ARN sync decode → `monitorRefundDecodeFailure "SYNC_REFUND_ARN_DECODE_FAILURE"`
- **Status Mapping**:
  - `REFUND_SUCCESS` → `RefundStatus::Success`
  - `REFUND_FAILURE` → `RefundStatus::Failure`
  - `REFUND_PENDING` → `RefundStatus::Pending`
  - `REFUND_NOT_FOUND` → `RefundStatus::Failure` with error code 404
  - `REFUND_MANUAL_REVIEW` → `RefundStatus::Pending` (manual review needed)
- **Mandatory Payment Methods**: N/A
- **Optional Payment Methods**: N/A
- **Hard Gate References**: None
- **Testing Notes**: Use `connector_refund_id` from Refund response + `connector_transaction_id` (original txnId). Note that `var1` for RSync uses the txnId (not mihpayid).

---

## Testing Strategy

- **Authorize (UPI — already implemented)**: Tested with UPI_PAY, UPI_QR, UPI_COLLECT. Expect `REQUIRES_CUSTOMER_ACTION` status.
- **Authorize (Wallet — pending)**: Test with REDIRECT_WALLET_DEBIT per-wallet variants. Expect `REQUIRES_CUSTOMER_ACTION` status.
- **Authorize (Netbanking — pending)**: Test with Netbanking + bank_code. Expect `REQUIRES_CUSTOMER_ACTION` status.
- **PSync**: Use `connector_transaction_id` from Authorize response. Already implemented and working.
- **Capture**: Testing Agent runs its OWN fresh Authorize with MANUAL capture, then Captures using `mihpayid` as `var1`.
- **Refund**: Use `connector_transaction_id` (mihpayid) from the AUTOMATIC-capture Authorize (CHARGED state). Use original txnid as `var3`.
- **RSync**: Use `connector_refund_id` from Refund response + original `txnid` as `var1`.
- **Void**: Testing Agent runs its OWN fresh Authorize with MANUAL capture, then Voids WITHOUT Capturing.

---

## Data Flow Map

```
Authorize (AUTOMATIC, UPI/Wallet/NB) → connector_transaction_id (mihpayid) + txnid → PSync, Refund
Authorize (MANUAL, by Capture testing) → connector_transaction_id (mihpayid) → Capture
Authorize (MANUAL, by Void testing) → connector_transaction_id (mihpayid) → Void
Refund → connector_refund_id → RSync (var1 = original txnid)
```

---

## Appendix: PayU Hash Formula Reference

| Flow | Formula |
|------|---------|
| Authorize (standard) | `SHA512(key\|txnid\|amount\|productinfo\|firstname\|email\|udf1\|udf2\|udf3\|udf4\|udf5\|udf6\|udf7\|udf8\|udf9\|udf10\|salt)` |
| PSync / Verify | `SHA512(key\|command\|var1\|salt)` where command=`"verify_payment"`, var1=txnId |
| Capture | `SHA512(key\|command\|var1\|salt)` where command=`"capture_transaction"`, var1=mihpayid |
| Void | `SHA512(key\|command\|var1\|salt)` where command=`"cancel_refund_transaction"`, var1=mihpayid |
| Refund | `SHA512(key\|command\|var1\|salt)` where command=`"cancel_refund_transaction"`, var1=mihpayid |
| RSync | `SHA512(key\|command\|var1\|salt)` where command=`"getAllRefundsFromTxnIds"`, var1=txnId list |

---

## Appendix: PayU Status → AttemptStatus Mapping

| PayU Response | PayU status | unmappedstatus | AttemptStatus |
|---|---|---|---|
| UPI Intent success | `1` (int) | — | `AuthenticationPending` |
| UPI Collect success + pending | `"success"` (string) | — | `AuthenticationPending` |
| UPI Collect success + other | `"success"` (string) | — | `Failure` |
| NB/Wallet redirect | redirect response | — | `AuthenticationPending` |
| Pre-auth | `"success"` | `"auth"` | `Authorized` |
| Captured | `"success"` | `"captured"` | `Charged` |
| Voided | `"success"` | `"cancelled"` | `Voided` |
| Failure | `"failure"` / `"error"` | — | `AuthenticationFailed` or `Failure` |
| Pending | `"pending"` | — | `Pending` |
