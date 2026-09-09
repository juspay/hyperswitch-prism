#!/usr/bin/env python3
"""Local mock of the Worldpay **Native RAFT API** v1.35.1 (credit + debit).

Purpose: we have no Worldpay credentials, so this mock is the only way to see
exactly what the UCS `worldpayraft` connector puts on the wire and to assert it
against the published OpenAPI specs. Every request is logged verbatim, then run
through a battery of field-level checks; each check emits an `[OK]` / `[FAIL]`
line and is accumulated into a report the test harness can pull over HTTP.

Ground truth: `Native Raft Credit API-1.35.1.yaml` / `Native Raft Debit
API-1.35.1.yaml` (`host: ws-cert.vantiv.com`,
`basePath: /merchant/servicing/apitransactions/NativeRaftApi/v1`). Every field
name emitted below was grepped out of those two documents.

Usage
-----
    python3 worldpayraft_mock_server.py [port] [license] [merchant_id]

Defaults: port 9098, license `MOCK_LICENSE`, merchant id `MOCK_MERCHANT_ID`.
Point the connector at `http://127.0.0.1:<port>/merchant/servicing/apitransactions/NativeRaftApi/v1`
(the base path is optional -- bare `/credit/authorization` is accepted too).

Endpoints and envelope keys (verbatim from the YAMLs)
----------------------------------------------------
    POST /credit/purchase        creditpurchase        -> creditpurchaseresponse
    POST /credit/authorization   creditauth            -> creditauthresponse
    POST /credit/completion      creditcompletion      -> creditcompletionresponse
    POST /credit/refund          creditrefund          -> creditrefundresponse
    POST /credit/balanceinquiry  creditbalanceinquiry  -> creditbalanceinquiryresponse
    POST /debit/purchase         debitpurchase         -> debitpurchaseresponse
    POST /debit/preauth          debitpreauth          -> debitpreauthresponse
    POST /debit/completion       debitcompletion       -> debitcompletionresponse
    POST /debit/refund           debitrefund           -> debitrefundresponse
    POST /tokenization/token     tokenize              -> tokenizeresponse

**There is no void/reversal/cancel endpoint for cards.** A Void is the *same*
message re-sent to the *same* path with `AuthorizationType: "RV"` (optionally
`ReversalAdviceReasonCd`), carrying the ORIGINAL `APITransactionID`. The mock
detects `AuthorizationType == "RV"` on any financial path and books it as a
reversal. `/credit/void`, `/credit/reversal`, `/credit/cancel`, `/credit/inquiry`,
`/credit/status`, `/debit/void` and `/debit/reversal` are registered only so that
a connector inventing them gets a loud `[FAIL]` plus the real 404 `NotFoundError`
body Worldpay would return.

HTTP status
-----------
Financial endpoints ALWAYS answer `200`. RAFT signals failure in the body only:
success is `ReturnCode == "0000"` AND `ResponseCode == "000"` (`"010"` is a
partial approval). Transport-level 401/404/500 shapes exist but are reserved for
bad auth / unknown routes, mirroring the specs.

Scripted failure modes
----------------------
Keyed off the **last four digits of `CardInfo.PAN`**. For a follow-up message
that carries no PAN (completion / refund / reversal) the PAN of the remembered
original transaction is used instead, so a capture of a declined auth declines
the same way. `UserDefinedData.UserData1 = "FORCE:<code>"` overrides everything.

    ....1111  approve                    ReturnCode 0000 / ResponseCode 000
    ....0010  partial approval           ResponseCode 010, PartiallyAuthorized=Y,
                                         OriginalAuthAmount = half the request
    ....0005  DO NOT HONOR               ResponseCode 005  + advice code 01
    ....0039  INSUFFICIENT FUNDS         ResponseCode 039  + advice code 02
    ....0051  NO MATCHING ORIGINAL       ResponseCode 051  + advice code 03
    ....0004  CARD EXPIRED               ResponseCode 004  + advice code 03
    ....0013  INVALID CARD SECURITY CODE ResponseCode 013, Cvv2Cvc2CIDResult=N
    ....0112  INVALID AVS INFORMATION    ResponseCode 112, AVSResult=N
    ....0550  DECLINED BY FRAUDSIGHT     ResponseCode 550  + advice code 03
    ....0666  edit error                 ReturnCode 0004 + ErrorInformation
                                         {FieldInError, ErrorText}, no ResponseCode
    ....0012  system issue               ReturnCode 0012, no ResponseCode

Note on the enums: `054` is VELOCITY: EXCEEDS COUNT and `051` is UNABLE TO LOCATE
A MATCHING ORIGINAL TRANSACTION in the real table (`raft_response_codes.json`);
CARD EXPIRED is `004` and INSUFFICIENT FUNDS is `039`. The mapping above uses the
real meanings rather than the approximate ones.

Report API
----------
    GET  /__report   accumulated findings + per-transaction memory, as JSON
    POST /__reset    clear findings and the transaction memory
"""

import json
import sys
from datetime import datetime, timezone
from http.server import BaseHTTPRequestHandler, HTTPServer

PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 9098
LICENSE = sys.argv[2] if len(sys.argv) > 2 else "MOCK_LICENSE"
MERCHANT_ID = sys.argv[3] if len(sys.argv) > 3 else "MOCK_MERCHANT_ID"

EXPECTED_AUTHORIZATION = 'VANTIV license="%s"' % LICENSE
BASE_PATH = "/merchant/servicing/apitransactions/NativeRaftApi/v1"

# ---------------------------------------------------------------------------
# Route table -- request wrapper / response wrapper / message kind.
# Taken verbatim from `<op>Request.required[0]` and `<op>Response.required[0]`
# in the credit and debit YAMLs.
# ---------------------------------------------------------------------------

ROUTES = {
    "/credit/purchase":       ("creditpurchase",       "creditpurchaseresponse",       "sale"),
    "/credit/authorization":  ("creditauth",           "creditauthresponse",           "auth"),
    "/credit/completion":     ("creditcompletion",     "creditcompletionresponse",     "completion"),
    "/credit/refund":         ("creditrefund",         "creditrefundresponse",         "refund"),
    "/credit/balanceinquiry": ("creditbalanceinquiry", "creditbalanceinquiryresponse", "inquiry"),
    "/debit/purchase":        ("debitpurchase",        "debitpurchaseresponse",        "sale"),
    "/debit/preauth":         ("debitpreauth",         "debitpreauthresponse",         "auth"),
    "/debit/completion":      ("debitcompletion",      "debitcompletionresponse",      "completion"),
    "/debit/refund":          ("debitrefund",          "debitrefundresponse",          "refund"),
    "/tokenization/token":    ("tokenize",             "tokenizeresponse",             "tokenize"),
}

# Paths a connector might invent. They do NOT exist in any Native RAFT spec.
PHANTOM_ROUTES = {
    "/credit/void":     'no such endpoint; a Void is the original message re-sent with AuthorizationType="RV"',
    "/credit/reversal": 'no such endpoint; a Void is the original message re-sent with AuthorizationType="RV"',
    "/credit/cancel":   "no such endpoint; only /apm/cancel exists, and that is for alternate payment methods",
    "/credit/inquiry":  "no such endpoint; /credit/balanceinquiry is a CARD BALANCE inquiry, not a transaction status lookup",
    "/credit/status":   "no such endpoint; Native RAFT has no transaction-status/PSync operation at all",
    "/credit/query":    "no such endpoint; Native RAFT has no transaction-status/PSync operation at all",
    "/debit/void":      'no such endpoint; use the original message with AuthorizationType="RV"',
    "/debit/reversal":  'no such endpoint; use the original message with AuthorizationType="RV"',
}

# Legal top-level members of a credit financial request object.
KNOWN_TOP_LEVEL = {
    "MiscAmountsBalances", "AccountCodesAndData", "CardInfo", "Multi-clearingData", "EMVData",
    "AddressVerificationData", "CardVerificationData", "EncryptionTokenData", "DeviceInformation",
    "PINProcessingData", "TerminalData", "E-commerceData", "BillPaymentData", "GatewayRoutingId",
    "ProcFlagsIndicators", "VisaSpecificData", "McrdSpecificData", "DiscSpecificData",
    "AmexSpecificData", "MarketSpecificData", "Duration", "STPData", "MerchantSpecificData",
    "ReferenceTraceNumbers", "WorldPayMerchantID", "UserDefinedData", "SoftDescriptorData",
    "WalletId", "OperatorEmployee", "BatchNumber", "CustomerInformation", "PrestigiousPropertyIND",
    "ReversalAdviceReasonCd", "AlternateMerchantID", "DynamicCurrConvInfo", "Level3Data",
    "PaymentSenderData", "PrivateLabelData", "AuthorizationType", "APITransactionID",
    "LocalDateTime", "LodgingData", "VehicleRentalData", "OnlineShipToAddress",
    "OnlineBillToAddress", "OnlineOrderCustomerData", "SynchronyData", "FisLoyaltyData",
    "PriorityRouting", "AdditionalFraudData", "BenefitCardServicesData", "EncryptedData",
    "TraceData", "MastercardDSRPCryptogram", "MastercardRemoteCommerceAcceptorIdentifier",
    "AdditionalPOSData", "AssuredPaymentsUserAccountData", "AssuredPaymentsPurchaseInformation",
    "AssuredPaymentsItemData", "AssuredPaymentsGeneralData", "AssuredPaymentsMembershipData",
    "AssuredSellerData", "AssuredSubscriptionData", "PassengerTransportData",
    "AirlineItineraryData", "AirlineAncillaryServiceData", "BasketData", "JWTProcessingData",
    "OnlineShipFromAddress", "OrderShipDate",
}

# Legal request-side members of the objects UCS actually populates.
KNOWN_SUBFIELDS = {
    "CardInfo": {"PAN", "TRACK_2", "TRACK_1", "ExpirationDate", "CardSequenceNumber"},
    "CardVerificationData": {"Cvv2Cvc2CIDValue", "Cvv2Cvc2CIDIndicator"},
    "AddressVerificationData": {"AVSZIPCode", "AVSAddress"},
    "ReferenceTraceNumbers": {
        "RetrievalREFNumber", "CorrelationID", "AuthorizationNumber", "RefInvoiceNumber",
        "DraftLocator", "TransactionLinkID", "TaxVATInvoiceNumber", "EconomicallyRelatedLinkID",
    },
    "TerminalData": {
        "EntryMode", "TerminalType", "TerminalNumber", "POSConditionCode", "POSEnvironment",
        "TerminalEntryCap", "AttendedDevice", "OperatingEnvironment",
    },
    "MiscAmountsBalances": {
        "TransactionAmount", "PreauthorizedAmount", "CashBackAmount", "SurchargeAmount",
        "ConvenienceFEE", "TIPAmount", "DispensedAmount", "SalesTAXAmount", "CumulativeAmount",
        "PaymentTrailingAmt", "OPTUMAmount", "GiftCardReloadableAmount",
        "GiftCardNonReloadableAmount", "InvoiceDiscountAmount", "InvoiceShippingAmount",
        "OriginalAuthAmount", "AvailableBALFromAcct",
    },
}

# Fields the spec declares response-only. Sending them is a bug.
RESPONSE_ONLY_SUBFIELDS = {
    "ReferenceTraceNumbers": {
        "SystemTraceNumber", "NetworkTraceNumber", "NetworkRefNumber", "PaymentAcctREFNumber",
        "PanReferenceID",
    },
    "CardVerificationData": {"Cvv2Cvc2CIDResult"},
    "AddressVerificationData": {"AVSResult"},
}

# Wrong field names seen in the wild, mapped to the real spec name.
LEGACY_ALIASES = {
    "CardVerificationData": {
        "CVV2CVC2": "Cvv2Cvc2CIDValue",
        "CVV2": "Cvv2Cvc2CIDValue",
        "CVV": "Cvv2Cvc2CIDValue",
        "CardSecurityCode": "Cvv2Cvc2CIDValue",
        "CVV2CVC2Indicator": "Cvv2Cvc2CIDIndicator",
    },
}

ECI_CODES = {
    "01": "Single transaction - default for Bill Payments",
    "02": "Recurring Transaction",
    "03": "Installment Payment",
    "05": "VbV authenticated / MC SecureCode with AAV / Discover with CAVV",
    "06": "VbV attempts processing / MC SecureCode with or without AAV",
    "07": "eCommerce, but neither Verified by Visa nor MasterCard SecureCode",
    "08": "No security method",
    "09": "SET (non-US)",
    "10": "Recurring, first of a series",
    "20": "Token Initiated (AMEX only)",
}

# last-4 of PAN -> (ReturnCode, ResponseCode, ReturnText, MastercardMerchantAdviceCode)
MAGIC_PANS = {
    "0010": ("0000", "010", None, None),
    "0005": ("0000", "005", "DO NOT HONOR", "01"),
    "0039": ("0000", "039", "INSUFFICIENT FUNDS", "02"),
    "0051": ("0000", "051", "UNABLE TO LOCATE A MATCHING ORIGINAL TRANSACTION", "03"),
    "0004": ("0000", "004", "CARD EXPIRED", "03"),
    "0013": ("0000", "013", "INVALID CARD SECURITY CODE", "01"),
    "0112": ("0000", "112", "INVALID ADDRESS VERIFICATION INFORMATION", "01"),
    "0550": ("0000", "550", "TRANSACTION DECLINED BY FRAUDSIGHT", "03"),
    "0666": ("0004", None, "EDIT ERROR ON INPUT", None),
    "0012": ("0012", None, "SYSTEM ISSUE", None),
}

# ---------------------------------------------------------------------------
# Mutable state: validation findings + remembered transactions
# ---------------------------------------------------------------------------

FINDINGS = []      # list of dicts, newest last
TRANSACTIONS = {}  # APITransactionID -> record
BY_AUTH_NUMBER = {}  # AuthorizationNumber -> APITransactionID
COUNTERS = {"requests": 0, "OK": 0, "FAIL": 0, "WARN": 0, "INFO": 0}
SEQ = {"n": 0, "trace": 100000, "auth": 100000, "rrn": 0}


def record(level, check, message, ctx):
    """Append one finding and print it as an [OK]/[FAIL]/[WARN]/[INFO] line."""
    SEQ["n"] += 1
    COUNTERS[level] = COUNTERS.get(level, 0) + 1
    finding = {
        "seq": SEQ["n"],
        "at": datetime.now(timezone.utc).isoformat(),
        "level": level,
        "check": check,
        "message": message,
        "path": ctx.get("path"),
        "wrapper": ctx.get("wrapper"),
        "api_transaction_id": ctx.get("api_transaction_id"),
    }
    FINDINGS.append(finding)
    print("[%s] %-28s %s" % (level, check, message), flush=True)


def ok(check, message, ctx):
    record("OK", check, message, ctx)


def fail(check, message, ctx):
    record("FAIL", check, message, ctx)


def warn(check, message, ctx):
    record("WARN", check, message, ctx)


def info(check, message, ctx):
    record("INFO", check, message, ctx)


# ---------------------------------------------------------------------------
# Validation
# ---------------------------------------------------------------------------

def obj(body, name):
    value = body.get(name)
    return value if isinstance(value, dict) else {}


def validate(path, headers, body, req_wrapper, kind, ctx):
    """Run every check against one decoded request. Returns the operation dict."""

    # --- 1. Authorization header --------------------------------------------
    got_auth = headers.get("Authorization")
    if got_auth is None:
        fail("auth.header.present", "no Authorization header sent; expected %r"
             % EXPECTED_AUTHORIZATION, ctx)
    elif got_auth == EXPECTED_AUTHORIZATION:
        ok("auth.header", "Authorization matches %r" % EXPECTED_AUTHORIZATION, ctx)
    else:
        fail("auth.header",
             "Authorization is %r, expected exactly %r (scheme must be the literal "
             "'VANTIV', the license quoted, no base64)" % (got_auth, EXPECTED_AUTHORIZATION), ctx)

    ctype = (headers.get("Content-Type") or "").split(";")[0].strip()
    if ctype == "application/json":
        ok("http.content_type", "Content-Type: application/json", ctx)
    else:
        fail("http.content_type", "Content-Type is %r, spec consumes application/json" % ctype, ctx)

    # --- 2. Envelope wrapper key --------------------------------------------
    if not isinstance(body, dict):
        fail("envelope.wrapper", "body is not a JSON object", ctx)
        return {}
    keys = list(body.keys())
    if keys == [req_wrapper]:
        ok("envelope.wrapper", "body wrapped in the single key %r" % req_wrapper, ctx)
    elif req_wrapper in body:
        fail("envelope.wrapper",
             "wrapper %r present but body also carries extra top-level keys %r; the spec "
             "declares exactly one root property" % (req_wrapper, [k for k in keys if k != req_wrapper]), ctx)
    else:
        fail("envelope.wrapper",
             "wrong wrapper key: got %r, %s expects %r (response comes back as %r)"
             % (keys, path, req_wrapper, ROUTES[path][1]), ctx)
        # Best effort: continue against whatever single object was sent.
        if len(keys) == 1 and isinstance(body[keys[0]], dict):
            return validate_operation(path, body[keys[0]], kind, ctx)
        return {}

    return validate_operation(path, body.get(req_wrapper) or {}, kind, ctx)


def validate_operation(path, op, kind, ctx):
    if not isinstance(op, dict):
        fail("envelope.wrapper", "wrapper value is not a JSON object", ctx)
        return {}

    ctx["api_transaction_id"] = op.get("APITransactionID")

    # --- 3. WorldPayMerchantID ----------------------------------------------
    mid = op.get("WorldPayMerchantID")
    if mid is None:
        fail("field.WorldPayMerchantID", "missing; it is a required field on every operation", ctx)
    elif mid == MERCHANT_ID:
        ok("field.WorldPayMerchantID", "== %r" % MERCHANT_ID, ctx)
    else:
        fail("field.WorldPayMerchantID",
             "is %r, expected %r -- the connector is probably sending the license/api key "
             "instead of the merchant id" % (mid, MERCHANT_ID), ctx)

    # --- 4. APITransactionID ------------------------------------------------
    txn_id = op.get("APITransactionID")
    if not txn_id:
        fail("field.APITransactionID",
             "missing or empty; required on every operation and it is the matching + "
             "idempotency key", ctx)
    elif len(str(txn_id)) > 16:
        fail("field.APITransactionID",
             "%r is %d chars, maxLength is 16 (Worldpay left zero-pads to a fixed 16)"
             % (txn_id, len(str(txn_id))), ctx)
    else:
        ok("field.APITransactionID", "%r present, %d/16 chars" % (txn_id, len(str(txn_id))), ctx)

    # --- 5. LocalDateTime ---------------------------------------------------
    ldt = op.get("LocalDateTime")
    if not ldt:
        fail("field.LocalDateTime", "missing; required, format YYYY-MM-DDTHH:mm:ss", ctx)
    else:
        try:
            datetime.strptime(str(ldt)[:19], "%Y-%m-%dT%H:%M:%S")
            if len(str(ldt)) > 19:
                fail("field.LocalDateTime",
                     "%r is %d chars, maxLength is 19 -- merchant-local "
                     "'YYYY-MM-DDTHH:mm:ss' with no timezone suffix" % (ldt, len(str(ldt))), ctx)
            else:
                ok("field.LocalDateTime", "%r well formed" % ldt, ctx)
        except ValueError:
            fail("field.LocalDateTime", "%r is not YYYY-MM-DDTHH:mm:ss" % ldt, ctx)

    # --- 6. Unknown / misplaced top-level members ---------------------------
    unknown = [k for k in op if k not in KNOWN_TOP_LEVEL]
    if unknown:
        fail("schema.unknown_field",
             "top-level members not in the spec: %r" % unknown, ctx)
    else:
        ok("schema.unknown_field", "all top-level members exist in the spec", ctx)

    for parent, allowed in KNOWN_SUBFIELDS.items():
        block = obj(op, parent)
        # Response-only members exist in the spec; they get their own, sharper
        # finding below rather than being reported as unknown here.
        allowed = allowed | RESPONSE_ONLY_SUBFIELDS.get(parent, set())
        strays = [k for k in block if k not in allowed]
        if strays:
            aliases = LEGACY_ALIASES.get(parent, {})
            for stray in strays:
                if stray in aliases:
                    fail("schema.legacy_field_name",
                         "%s.%s is NOT a Native RAFT field -- the spec name is %s.%s "
                         "(literal '%s' appears 0 times in credit.yaml)"
                         % (parent, stray, parent, aliases[stray], stray), ctx)
                else:
                    fail("schema.unknown_field",
                         "%s.%s is not in the spec" % (parent, stray), ctx)

    # --- 7. CVV field name and presence indicator ---------------------------
    cvd = obj(op, "CardVerificationData")
    cvv_value = cvd.get("Cvv2Cvc2CIDValue")
    legacy_cvv = next((cvd[k] for k in ("CVV2CVC2", "CVV2", "CVV", "CardSecurityCode")
                       if k in cvd), None)
    indicator = cvd.get("Cvv2Cvc2CIDIndicator")

    if cvv_value:
        ok("cvv.field_name", "CVV carried in the correct field CardVerificationData.Cvv2Cvc2CIDValue", ctx)
    elif legacy_cvv:
        fail("cvv.field_name",
             "CVV carried in a NON-EXISTENT field; Worldpay will silently drop it and the "
             "transaction will run without CVV verification. Rename to "
             "CardVerificationData.Cvv2Cvc2CIDValue (maxLength 4)", ctx)
    elif cvd:
        warn("cvv.field_name", "CardVerificationData present but carries no CVV value", ctx)

    have_cvv = bool(cvv_value or legacy_cvv)
    if have_cvv:
        if indicator == "1":
            ok("cvv.indicator", 'Cvv2Cvc2CIDIndicator="1" (value is present) matches the CVV sent', ctx)
        elif indicator == "0":
            fail("cvv.indicator",
                 'Cvv2Cvc2CIDIndicator="0" means "the CVV2/CVC2/CID value was BYPASSED or not '
                 'given", but a CVV value IS being sent -- the indicator is inverted. It must '
                 'be "1" when a value is present', ctx)
        elif indicator is None:
            fail("cvv.indicator",
                 'a CVV value is sent but Cvv2Cvc2CIDIndicator is missing; it must be "1"', ctx)
        else:
            warn("cvv.indicator",
                 'Cvv2Cvc2CIDIndicator=%r with a CVV present; expected "1" '
                 '(2=illegible, 9=not on card)' % indicator, ctx)
    elif indicator not in (None, "0", "9"):
        warn("cvv.indicator",
             'Cvv2Cvc2CIDIndicator=%r but no CVV value is being sent' % indicator, ctx)

    # --- 8. SystemTraceNumber must not be sent ------------------------------
    for parent, banned in RESPONSE_ONLY_SUBFIELDS.items():
        block = obj(op, parent)
        for name in banned:
            if name in block:
                fail("field.response_only",
                     "%s.%s is sent on the REQUEST; the spec declares it response-only "
                     "(%r, value %r) -- Worldpay generates it and returns it, the acquirer "
                     "must never populate it"
                     % (parent, name, "generated by Worldpay", block[name]), ctx)
    rtn = obj(op, "ReferenceTraceNumbers")
    if "SystemTraceNumber" not in rtn:
        ok("field.SystemTraceNumber", "not sent on the request (correct: response-only)", ctx)

    # --- 9. Amounts ---------------------------------------------------------
    amounts = obj(op, "MiscAmountsBalances")
    txn_amount = amounts.get("TransactionAmount")
    if kind in ("auth", "sale", "completion", "refund"):
        if txn_amount in (None, ""):
            fail("field.TransactionAmount",
                 "MiscAmountsBalances.TransactionAmount missing; required on every financial "
                 "operation", ctx)
        else:
            ok("field.TransactionAmount", "= %r" % txn_amount, ctx)

    if kind == "completion":
        preauth = amounts.get("PreauthorizedAmount")
        if preauth in (None, ""):
            fail("field.PreauthorizedAmount",
                 "MiscAmountsBalances.PreauthorizedAmount is MISSING on %s -- the spec marks "
                 "it REQUIRED on completion ('the acquirer places the amount that the "
                 "transaction was originally authorized for in this field'). Without it the "
                 "capture cannot be matched to the preauth" % path, ctx)
        else:
            ok("field.PreauthorizedAmount", "= %r on %s" % (preauth, path), ctx)
            if txn_amount not in (None, "") and str(txn_amount) != str(preauth):
                info("capture.partial",
                     "TransactionAmount %r != PreauthorizedAmount %r -> partial capture"
                     % (txn_amount, preauth), ctx)

    # --- 10. E-commerceIndicator -------------------------------------------
    ecom = obj(op, "E-commerceData")
    eci = ecom.get("E-commerceIndicator")
    if eci is None:
        warn("field.E-commerceIndicator",
             "not sent; all e-commerce transactions must include it", ctx)
    elif eci in ECI_CODES:
        info("field.E-commerceIndicator",
             "received %r (%s)%s" % (eci, ECI_CODES[eci],
                                     " -- note: hardcoded 07 loses 3DS/recurring signalling"
                                     if eci == "07" else ""), ctx)
    else:
        fail("field.E-commerceIndicator",
             "%r is not a documented ECI value %s" % (eci, sorted(ECI_CODES)), ctx)

    if ecom.get("3dSecureData") and eci not in ("05", "06"):
        fail("field.E-commerceIndicator",
             "3dSecureData is present but E-commerceIndicator is %r; an authenticated 3DS "
             "transaction must be 05 (or 06 for attempts)" % eci, ctx)

    # --- 11. Reversal / follow-up linkage -----------------------------------
    auth_type = op.get("AuthorizationType")
    is_reversal = auth_type == "RV"
    if auth_type is not None and auth_type not in ("FP", "RV"):
        fail("field.AuthorizationType",
             "%r is not a documented value; only FP (Force Post) and RV (Reversal) exist"
             % auth_type, ctx)
    if is_reversal:
        ok("void.mechanism",
             'AuthorizationType="RV" on %s -- correct: there is no void/reversal endpoint, a '
             "void is the original message re-sent with RV" % path, ctx)
        rr = op.get("ReversalAdviceReasonCd")
        if rr is None:
            info("field.ReversalAdviceReasonCd",
                 "not sent; Worldpay will default it (000 Normal Reversal)", ctx)
        elif rr not in ("000", "002", "003", "005", "006", "010"):
            fail("field.ReversalAdviceReasonCd",
                 "%r is not a documented value (000/002/003/005/006/010)" % rr, ctx)

    follow_up = kind in ("completion", "refund") or is_reversal
    if follow_up:
        check_followup_linkage(op, txn_id, rtn, kind, is_reversal, ctx)
    elif txn_id:
        remember(op, txn_id, kind, path, ctx)

    return op


def check_followup_linkage(op, txn_id, rtn, kind, is_reversal, ctx):
    """Did the follow-up reuse the ORIGINAL APITransactionID, as the spec requires?"""
    label = "reversal" if is_reversal else kind
    auth_number = rtn.get("AuthorizationNumber")

    if txn_id and txn_id in TRANSACTIONS:
        original = TRANSACTIONS[txn_id]
        ok("followup.api_transaction_id",
           "%s reuses APITransactionID %r from the original %s -- correct, that is how "
           "Worldpay matches follow-up messages" % (label, txn_id, original["kind"]), ctx)
        return

    linked = BY_AUTH_NUMBER.get(str(auth_number)) if auth_number else None
    if linked:
        fail("followup.api_transaction_id",
             "%s sent a REGENERATED APITransactionID %r. The original transaction was %r "
             "(matched via ReferenceTraceNumbers.AuthorizationNumber %r). The spec: 'If you "
             "are initiating a subsequent message that ties back to an original request, "
             "provide the APITransactionID of the original transaction.' With a fresh id "
             "Worldpay cannot match the %s back to the auth"
             % (label, txn_id, linked, auth_number, label), ctx)
        return

    if auth_number:
        warn("followup.api_transaction_id",
             "%s carries APITransactionID %r which this mock has never seen, and "
             "AuthorizationNumber %r does not match a remembered authorization either "
             "(nothing to link it to)" % (label, txn_id, auth_number), ctx)
    else:
        fail("followup.api_transaction_id",
             "%s carries neither a known APITransactionID (%r) nor a "
             "ReferenceTraceNumbers.AuthorizationNumber -- there is no way for Worldpay to "
             "match it back to the original transaction" % (label, txn_id), ctx)

    if auth_number and len(str(auth_number)) > 6:
        fail("field.AuthorizationNumber",
             "ReferenceTraceNumbers.AuthorizationNumber %r is %d chars, maxLength is 6 "
             "(it is the 6-char issuer approval code, not a connector transaction id)"
             % (auth_number, len(str(auth_number))), ctx)

    rrn = rtn.get("RetrievalREFNumber")
    if rrn == "":
        fail("field.RetrievalREFNumber",
             "sent as an empty string; omit the field entirely rather than sending '' "
             "(Worldpay generates one when the network requires it)", ctx)


def remember(op, txn_id, kind, path, ctx):
    pan = obj(op, "CardInfo").get("PAN") or ""
    record_ = {
        "api_transaction_id": txn_id,
        "kind": kind,
        "path": path,
        "pan_last4": str(pan)[-4:],
        "amount": obj(op, "MiscAmountsBalances").get("TransactionAmount"),
        "local_date_time": op.get("LocalDateTime"),
    }
    TRANSACTIONS[str(txn_id)] = record_


# ---------------------------------------------------------------------------
# Response shaping
# ---------------------------------------------------------------------------

def brand_of(pan):
    pan = str(pan or "")
    if pan.startswith("4"):
        return "visa"
    if pan[:2] in ("34", "37"):
        return "amex"
    if pan[:4] in ("6011",) or pan[:2] == "65":
        return "discover"
    if pan[:2] in ("51", "52", "53", "54", "55"):
        return "mastercard"
    if pan[:4].isdigit() and 2221 <= int(pan[:4] or 0) <= 2720:
        return "mastercard"
    return "unknown"


def next_id(bucket, width):
    SEQ[bucket] += 1
    return str(SEQ[bucket])[-width:].rjust(width, "0")


def outcome_for(op, txn_id):
    """Resolve the scripted outcome for this message."""
    forced = obj(op, "UserDefinedData").get("UserData1") or ""
    if str(forced).startswith("FORCE:"):
        code = str(forced).split(":", 1)[1].strip()
        if code in MAGIC_PANS:
            return MAGIC_PANS[code]
        return ("0000", code, None, None)

    pan = obj(op, "CardInfo").get("PAN") or ""
    last4 = str(pan)[-4:]
    if not last4 and txn_id and str(txn_id) in TRANSACTIONS:
        last4 = TRANSACTIONS[str(txn_id)].get("pan_last4") or ""
    if last4 in MAGIC_PANS:
        return MAGIC_PANS[last4]
    return ("0000", "000", None, None)


def build_response(path, resp_wrapper, kind, op, ctx):
    txn_id = op.get("APITransactionID") or ""
    return_code, response_code, return_text, advice = outcome_for(op, txn_id)

    body = {
        "ReturnCode": return_code,
        "ReasonCode": "0000",
        "APITransactionID": str(txn_id)[:16].rjust(16, "0"),
    }
    if return_text and return_code != "0000":
        body["ReturnText"] = return_text[:60]

    if return_code == "0004":
        body["ReasonCode"] = "0101"
        body["ErrorInformation"] = {
            "FieldInError": "CardVerificationData.Cvv2Cvc2CIDValue",
            "ErrorText": "NON-NUMERIC",
        }
        return {resp_wrapper: body}
    if return_code == "0012":
        body["ReasonCode"] = "9999"
        return {resp_wrapper: body}

    body["ResponseCode"] = response_code

    if kind == "tokenize":
        body["EncryptionTokenData"] = {
            "TokenizedPAN": "9%s" % str(obj(op, "CardInfo").get("PAN") or "")[-15:].rjust(15, "0"),
            "TokenID": "0001",
        }
        return {resp_wrapper: body}

    pan = str(obj(op, "CardInfo").get("PAN") or "")
    amounts = obj(op, "MiscAmountsBalances")
    amount = amounts.get("TransactionAmount")
    approved = response_code in ("000", "010")

    misc = {}
    if response_code == "010":
        try:
            misc["OriginalAuthAmount"] = "%.2f" % (float(amount) / 2.0)
        except (TypeError, ValueError):
            misc["OriginalAuthAmount"] = amount
        body["ProcFlagsIndicators"] = {"PartiallyAuthorized": "Y"}
    elif amount is not None:
        misc["OriginalAuthAmount"] = amount
        body["ProcFlagsIndicators"] = {"PartiallyAuthorized": "N"}
    if misc:
        body["MiscAmountsBalances"] = misc

    if pan:
        body["CardInfo"] = {"PAN": ("*" * max(len(pan) - 4, 0)) + pan[-4:], "CardProductCode": "CRD"}

    avs_result = "N" if response_code == "112" else "Y"
    cvv_result = "N" if response_code == "013" else ("M" if approved else "P")
    avd = obj(op, "AddressVerificationData")
    # "Worldpay returns this field in the response message if the authorization
    # request included AVSZipCode or AVSAddress."
    if avd.get("AVSZIPCode") or avd.get("AVSAddress") or response_code == "112":
        body["AddressVerificationData"] = {"AVSResult": avs_result}
    cvd = obj(op, "CardVerificationData")
    if cvd:
        body["CardVerificationData"] = {"Cvv2Cvc2CIDResult": cvv_result}

    eci = obj(op, "E-commerceData").get("E-commerceIndicator")
    if eci is not None:
        body["E-commerceData"] = {
            "ReturnE-commerceIndicator": eci,
            "3dSecureResult": "2" if eci in ("05", "06") else "5",
        }

    auth_number = next_id("auth", 6) if approved else "      "
    trace_number = next_id("trace", 6)
    SEQ["rrn"] += 1
    retrieval = "%s%08d" % (datetime.now(timezone.utc).strftime("%y%j")[-4:], SEQ["rrn"])
    body["ReferenceTraceNumbers"] = {
        "AuthorizationNumber": auth_number,
        "RetrievalREFNumber": obj(op, "ReferenceTraceNumbers").get("RetrievalREFNumber") or retrieval,
        "SystemTraceNumber": trace_number,
    }
    correlation = obj(op, "ReferenceTraceNumbers").get("CorrelationID")
    if correlation:
        body["ReferenceTraceNumbers"]["CorrelationID"] = correlation

    brand = brand_of(pan) if pan else (
        TRANSACTIONS.get(str(txn_id), {}).get("brand") or "unknown")
    if brand == "mastercard":
        mcrd = {
            "McrdBanknetREFNUM": trace_number.rjust(9, "0")[-9:],
            "McrdBanknetSettleDate": datetime.now(timezone.utc).strftime("%m%d"),
        }
        if advice and obj(op, "ProcFlagsIndicators").get("MastercardAdviceCodeIndicator") == "Y":
            mcrd["MastercardMerchantAdviceCode"] = advice
        elif advice:
            # Still return it so the error path can be exercised, but say so.
            mcrd["MastercardMerchantAdviceCode"] = advice
            info("response.advice_code",
                 'returning MastercardMerchantAdviceCode=%r even though the request did not set '
                 'ProcFlagsIndicators.MastercardAdviceCodeIndicator="Y"; the real host would '
                 "omit it" % advice, ctx)
        body["McrdSpecificData"] = mcrd
    elif brand == "visa":
        body["VisaSpecificData"] = {
            "VisaTransactionId": trace_number.rjust(15, "0"),
            "VisaValidationCode": "A1B2",
            "VisaCardLevelResults": "A ",
        }
    elif brand == "amex":
        body["AmexSpecificData"] = {"AmexTransactionId": trace_number.rjust(15, "0")}
    elif brand == "discover":
        body["DiscSpecificData"] = {"DiscTransactionId": trace_number.rjust(15, "0")}

    body["AuthorizationSource"] = "5" if approved else "C"

    if approved and kind in ("auth", "sale") and txn_id:
        TRANSACTIONS.setdefault(str(txn_id), {})
        TRANSACTIONS[str(txn_id)].update({
            "api_transaction_id": txn_id,
            "kind": kind,
            "path": path,
            "pan_last4": pan[-4:],
            "brand": brand,
            "amount": amount,
            "authorization_number": auth_number,
            "retrieval_ref_number": body["ReferenceTraceNumbers"]["RetrievalREFNumber"],
            "system_trace_number": trace_number,
        })
        BY_AUTH_NUMBER[auth_number] = str(txn_id)
        # A connector that keeps its own "connector transaction id" will echo it
        # back on the capture; remember that too so the linkage check can fire.
        for candidate in (obj(op, "ReferenceTraceNumbers").get("CorrelationID"),
                          obj(op, "ReferenceTraceNumbers").get("RefInvoiceNumber")):
            if candidate:
                BY_AUTH_NUMBER.setdefault(str(candidate), str(txn_id))
        BY_AUTH_NUMBER.setdefault(str(txn_id), str(txn_id))

    return {resp_wrapper: body}


def summary():
    return {
        "requests": COUNTERS["requests"],
        "ok": COUNTERS["OK"],
        "fail": COUNTERS["FAIL"],
        "warn": COUNTERS["WARN"],
        "info": COUNTERS["INFO"],
        "failed_checks": sorted({f["check"] for f in FINDINGS if f["level"] == "FAIL"}),
    }


def print_summary():
    s = summary()
    print("-" * 78, flush=True)
    print("SUMMARY  requests=%(requests)d  OK=%(ok)d  FAIL=%(fail)d  WARN=%(warn)d  "
          "INFO=%(info)d" % s, flush=True)
    for check in s["failed_checks"]:
        print("  FAILED: %s" % check, flush=True)
    print("-" * 78, flush=True)


# ---------------------------------------------------------------------------
# HTTP plumbing
# ---------------------------------------------------------------------------

class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *_args):
        pass  # superseded by the explicit dump below

    # -- helpers ------------------------------------------------------------
    def _normalised_path(self):
        path = self.path.split("?")[0]
        if path.startswith(BASE_PATH):
            path = path[len(BASE_PATH):] or "/"
        return path.rstrip("/") or "/"

    def _send(self, status, payload):
        encoded = json.dumps(payload).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(encoded)))
        self.end_headers()
        self.wfile.write(encoded)

    def _dump_request(self, path, raw, body):
        print("=" * 78, flush=True)
        print(">>> INBOUND REQUEST  %s %s HTTP/%s" % (self.command, self.path,
                                                      self.request_version), flush=True)
        print("--- headers ---", flush=True)
        for name, value in self.headers.items():
            print("%s: %s" % (name, value), flush=True)
        print("--- body (verbatim) ---", flush=True)
        print(raw.decode("utf-8", "replace") if raw else "<empty>", flush=True)
        if body is not None:
            print("--- body (pretty) ---", flush=True)
            print(json.dumps(body, indent=2, sort_keys=True), flush=True)
        print("--- validation (route %s) ---" % path, flush=True)

    # -- verbs --------------------------------------------------------------
    def do_GET(self):
        path = self._normalised_path()
        if path == "/__report":
            self._send(200, {
                "license_expected": EXPECTED_AUTHORIZATION,
                "merchant_id_expected": MERCHANT_ID,
                "summary": summary(),
                "findings": FINDINGS,
                "transactions": TRANSACTIONS,
            })
            return
        if path == "/__health":
            self._send(200, {"status": "ok", "port": PORT})
            return
        self._send(404, {"fault": {"faultType": "Server Error Processing Message",
                                   "faultDescription": "Requested Service does not exist"}})

    def do_POST(self):
        path = self._normalised_path()

        if path == "/__reset":
            del FINDINGS[:]
            TRANSACTIONS.clear()
            BY_AUTH_NUMBER.clear()
            for key in COUNTERS:
                COUNTERS[key] = 0
            print("=" * 78, flush=True)
            print(">>> /__reset -- findings and transaction memory cleared", flush=True)
            self._send(200, {"status": "reset"})
            return

        length = int(self.headers.get("Content-Length", 0) or 0)
        raw = self.rfile.read(length) if length else b""
        try:
            body = json.loads(raw.decode() or "{}")
        except ValueError:
            body = None

        COUNTERS["requests"] += 1
        self._dump_request(path, raw, body)
        ctx = {"path": path, "wrapper": None, "api_transaction_id": None}

        if path in PHANTOM_ROUTES:
            fail("route.nonexistent",
                 "POST %s -- %s. Worldpay answers 404 NotFoundError here"
                 % (path, PHANTOM_ROUTES[path]), ctx)
            payload = {"fault": {"faultType": "Server Error Processing Message",
                                 "faultDescription": "Requested Service does not exist"}}
            self._finish(404, payload)
            return

        if path not in ROUTES:
            fail("route.unknown",
                 "POST %s is not a Native RAFT operation; known routes: %s"
                 % (path, ", ".join(sorted(ROUTES))), ctx)
            payload = {"fault": {"faultType": "Server Error Processing Message",
                                 "faultDescription": "Requested Service does not exist"}}
            self._finish(404, payload)
            return

        req_wrapper, resp_wrapper, kind = ROUTES[path]
        ctx["wrapper"] = req_wrapper
        ok("route", "POST %s -> %s / %s" % (path, req_wrapper, resp_wrapper), ctx)

        if body is None:
            fail("http.body", "request body is not valid JSON", ctx)
            self._finish(200, {resp_wrapper: {
                "ReturnCode": "0004", "ReasonCode": "0001",
                "ReturnText": "MALFORMED MESSAGE RECEIVED",
                "ErrorInformation": {"FieldInError": "body", "ErrorText": "NOT JSON"},
            }})
            return

        op = validate(path, self.headers, body, req_wrapper, kind, ctx)
        payload = build_response(path, resp_wrapper, kind, op, ctx)
        self._finish(200, payload)

    def _finish(self, status, payload):
        print("--- response ---", flush=True)
        print("HTTP %d" % status, flush=True)
        print(json.dumps(payload, indent=2), flush=True)
        print_summary()
        self._send(status, payload)


if __name__ == "__main__":
    print("Worldpay Native RAFT mock listening on "
          "http://127.0.0.1:%d%s" % (PORT, BASE_PATH), flush=True)
    print('  expecting Authorization: %s' % EXPECTED_AUTHORIZATION, flush=True)
    print("  expecting WorldPayMerchantID: %s" % MERCHANT_ID, flush=True)
    print("  routes: %s" % ", ".join(sorted(ROUTES)), flush=True)
    print("  report: GET /__report   reset: POST /__reset", flush=True)
    try:
        HTTPServer(("127.0.0.1", PORT), Handler).serve_forever()
    except KeyboardInterrupt:
        print_summary()
