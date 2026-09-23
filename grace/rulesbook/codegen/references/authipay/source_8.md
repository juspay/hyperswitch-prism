Source: https://docs.fiserv.dev/public/docs/s2s-notifications

Server-to-Server Notifications

Jump to Content

ProductsDeveloperSupport

GuidesRecipesAPI ReferenceChangelogProductsDeveloperSupportLog In

Guides

Log In

GuidesRecipesAPI ReferenceChangelog

Server-to-Server Notifications

## INTRODUCTION

- Overview
- Getting Started
- Generate a Message Signature / Hash
- References
- Response Codes
- Test Cards

## Payment REST API

- Introduction
- General Concepts
- Preauthorisation
- Voids and Returns
- Payment Methods
- Response Handling
- Features
- 3-D Secure
- AANI
- Account Owner (Name) Inquiry
- Apple Pay
- Bancontact QR
- Bancontact Wallet Initiated Payment (WIP)
- Bizum
- Blik
- Card Verification
- Click to Pay
- Currency Conversion
- Data Vault Tokenisation
- Google Pay
- Guest Checkout Tokenization
- iDEAL | Wero
- Industry Specific Data
- Mail Order / Telephone Order (MOTO)
- Managed Redirect
- Mastercard MoneySend Funding
- Mastercard MoneySend Payments
- Mastercard Transaction Link Identifier (TLID)
- MCC Mandates
- Merchant Initiated Transactions (MIT)
- Network Tokenisation
- Partial Authorisation
- Payment Facilitator
- Payment URL
- PayPal
- PayPal Recurring (Billing Agreement)
- Product Catalogue
- Recurring Payments
- Requests from POS device
- BLIK POS
- Encryption and MAC'ing
- girocard and Direct Debit
- Master Session Key Concept
- Perform strong customer authentication
- Transactions initiated from POS device
- RuPay
- SEPA Direct Debit
- Uniticket
- Visa Installments
- Visa Mobile
- Visa Account Funding Transactions (AFT)
- Visa Original Credit Transfers (OCT)

## SUBMISSION COMPONENTS

- Agentic Payment Link
- Checkout Solution
- Introduction
- Webhooks and status updates
- Supported languages & Payment Methods
- Features
- Data Vault Tokenisation
- iDEAL 2.0
- Payment Links
- PayPal
- SEPA Direct Debit
- Hosted Payment Page
- Introduction
- Getting Started
- Direct POST
- Payment Form Fields
- Mandatory & Optional Fields
- Additional Form Fields
- Custom Fields
- Transaction Response
- Features
- 3-D Secure
- Apple Pay
- Click to Pay
- Currency Conversion
- Data Vault Tokenisation
- Debit Disbursement
- Fraud Detect
- Guest Checkout Tokenization
- iDEAL | Wero
- Local Payments
- MCC Mandates
- Money Transfer
- Payment Facilitator
- PayPal
- PayPal Recurring (Billing Agreement)
- Recurring Scheduler
- References
- Hosted Payment Page Localisations
- Payment Methods
- nexo Acquirer API
- Introduction
- Getting started
- References and Identifiers
- General Message Layout
- Sale
- Preauthorisation
- Void and refund
- Reconciliation and Cutover
- Gratuity (tip) and cashback
- Other Transaction Types
- Balance and status inquiry
- Connectivity
- Encryption and MAC'ing
- Other useful information
- Features
- AANI
- Alipay+
- Bizum
- Blik
- Card Validation
- Dynamic Currency Conversion
- Gasóleo Bonificado
- Installment
- Mail Order / Telephone Order (MOTO)
- Money Transfer
- Payment Account Reference and Hosted data
- Payment Facilitator
- Plazox
- SEPA Direct debit and girocard
- UniVerde
- Uniticket
- Dual-Message Integration Guide
- SOAP API
- Introduction
- Setup and Configuration
- HTTP POST Request / Response
- TLS Connection
- Transaction Handling
- Generate a transaction
- API Request/Response Messages
- Transaction Result Analysis
- Troubleshooting
- Specific Transactions
- Generic Transaction Type for Voids and Returns
- Partial Authorisation
- Additional API Actions
- External Transaction Status
- Email Notifications
- Card Information Inquiry
- Basket Information & Product Catalogue
- Features
- 3-D Secure
- Bancontact QR code
- Custom Parameters
- China domestic processing
- Data Vault Tokenisation
- Global Choice™ and Dynamic Pricing
- Guest Checkout Tokenization
- Industry Specific Data
- Mastercard MoneySend Funding
- Mastercard Transaction Link Identifier (TLID)
- Merchant Initiated Transactions (MIT)
- Network Tokenisation
- Payment Facilitator
- Payment URL
- Purchasing cards
- Recurring Payments
- RuPay
- SEPA Direct Debit - Germany
- SEPA Direct Debit with Fiserv Local Payments
- Standing Instructions
- Visa Account Funding Transactions (AFT)
- Visa Installments
- SOAP API Integration Guide
- SOAP API EMV Appendix
- Shopping Cart Plugins
- WooCommerce
- Magento v2 (Adobe Commerce)
- PrestaShop
- Virtual Terminal (legacy)
- Virtual Terminal 2.0
- Introduction & Login
- Sale and Preauthorisation
- Completion
- Return (Refund)
- Void
- Credit
- Card-on-File (Gateway Tokenisation)
- Certificate management
- Checkout Builder
- Connect Builder
- Email notifications
- Market specific functionalities
- Order Details
- Order Management
- Payment Methods
- PayPal Configuration
- Password Management
- Recurring Payments (Scheduler)
- Reports
- Transaction Reporting
- Payment Link Report
- 3-D Secure Report
- Recurring Payments Report
- User Management
- Virtual Terminal FAQ
- Server-to-Server Notifications

## PAYMENT SCENARIOS

- Overview (GLOBAL)
- Overview (REGION-SPECIFIC)
- Wallets
- Processing of POS Transactions

## Disputes

- Quick start
- Disputes flow
- Search for list of disputes
- Search for a single dispute
- Respond to a dispute
- Add a note to a dispute
- Additional information
- Dispute Stages

## Funding REJECTS

- Quick start
- Search for list of funding rejects
- Update an existing funding reject status

## SCA Exemptions

- Quick start
- Exemption check scenario
- Learning data scenario
- Response codes
- Training the risk engine
- Key field details
- Transaction response details
- SCA result
- Authorization data
- FAQs

## Statements

- Quick start

## Transactional Data

- Quick start
- Reconciliation
- Payment Types

## Reference

- Sandbox Usage
- Policies
- Data Types
- Errors
- List Responses
- Message Signature
- Security
- Versioning & Deprecation
- Educational
- Merchants Category Codes
- Payment Lifecycle

## FINANCIAL ADJUSTMENT

- Quick start
- How to Use the API
- Overview
- Constructing the API call
- Guidance on Responses
- Features
- Create an adjustment
- Update an adjustment
- Cancel an adjustment
- List adjustments
- List reference data

# Server-to-Server Notifications

Suggest Edits

## 📘
Your account must eb enabled for this feature

IPG can send your application a server-to-server transaction notification after it processes a payment. The capability is not limited to IPG Connect: IPG sends these notifications from the applicable submission component when a transaction reaches a reportable result, including asynchronous result updates for supported payment methods.

The notification is an asynchronous HTTP request from IPG to your server. It is independent of a customer's browser redirect, so use it to update order state rather than treating a success or failure URL as the final payment confirmation.

## 

How it works

- Configure a transaction notification URL for the store or through the configuration mechanism provided by the submission component. Connect can also accept transactionNotificationURL in the payment request when the store is enabled to override configured URLs.

- IPG processes the transaction and sends the notification when a result is available. Some alternative payment methods can first report WAITING and later send an updated result.

- IPG sends an asynchronous POST to the configured URL with form URL-encoded fields (application/x-www-form-urlencoded).

- Your endpoint validates notification_hash, records the event idempotently, updates the order from the verified result, and returns a successful HTTP response promptly.

The customer's return to responseSuccessURL or responseFailureURL is a browser navigation and can be abandoned, replayed, or manipulated. Never ship goods or mark an order paid solely because the customer reached a return URL.

## 

Configure the notification URL

Configure the store-level Transaction Notification URL in your IPG administration or store-management integration. The equivalent property is transactionNotificationUrl. This store configuration is used by submission components that support the common transaction-notification flow.

For IPG Connect integrations that are enabled for per-request URL overrides, send this field with the payment request:

Text

transactionNotificationURL=https://merchant.example.com/payments/ipg/notifications

Use a stable, publicly reachable HTTPS endpoint. Do not use an endpoint that requires an interactive login, browser cookies, CSRF validation, or a customer session. If the endpoint changes, update the store configuration before directing live payments to the new endpoint.

Recurring payments have a separate store configuration, recurringTransactionNotificationUrl. Use it when recurring-payment notifications are enabled for the store.

## 

Notification request

IPG sends a form URL-encoded HTTP POST. A representative notification is:

HTTP

POST /payments/ipg/notifications HTTP/1.1
Host: merchant.example.com
Content-Type: application/x-www-form-urlencoded

ipgTransactionId=1234567890&oid=ORDER-100045&chargetotal=49.99¤cy=978&txndatetime=2026:09:10-15:30:45&storename=12345678901&approval_code=Y:123456:...&status=APPROVED&hash_algorithm=SHA256¬ification_hash=...

The exact field set varies by transaction type, payment method, and enabled services. Your receiver must accept additional fields and must not reject a valid notification merely because it contains fields it does not use.

FieldDescription

ipgTransactionIdIPG's transaction identifier. Store it and use it as a primary idempotency key.

oidYour order identifier, when supplied in the original Connect request.

chargetotalProcessed transaction amount, formatted for the integration.

currencyTransaction currency. For Connect notifications this is normally the ISO numeric code, for example 978 for EUR.

txndatetimeTransaction date/time value. In Connect it is normally formatted as yyyy:MM:dd-HH:mm:ss.

storenameIPG store ID used to select the correct shared secret and validate the notification hash.

approval_codeIPG approval/result code. Do not infer approval from its presence; evaluate the verified result fields.

statusTransaction result where available, for example APPROVED, DECLINED, FAILED, or WAITING.

processor_response_codeProcessor-specific response code, when available.

hash_algorithmAlgorithm used for the notification hash. If absent, use the algorithm configured for the transaction/store integration.

notification_hashIntegrity value that must be verified with your IPG shared secret.

orderIdOrder identifier included for some asynchronous transaction flows.

Depending on the payment method, IPG can also include processor, funding, masked-card, token, mandate, bank-account, or scheme-specific fields. Treat these as optional. Do not log sensitive values unnecessarily, and never expect a full PAN or CVV in a notification.

## 

Validate the notification

Before changing order state, validate notification_hash using the shared secret configured for the same store and transaction origin. Use a constant-time comparison. The common notification flow uses the following hash contract; consult the submission component's integration guide for any component-specific variation.

For current HMAC-capable algorithms, the signed values are, in this order:

Text

chargetotal | currency | txndatetime | storename | approval_code

storename is the IPG store ID. Generate the HMAC using the indicated hash_algorithm and your shared secret, then compare the result with notification_hash. The separator shown above is part of the current HMAC representation; do not add spaces or substitute the merchant order ID.

Some established integrations use legacy hash algorithms with a legacy concatenation format. Keep the validation implementation aligned with the algorithm selected for your Connect integration. When migrating an existing integration, validate test notifications with the configured algorithm before switching live traffic. Do not accept a notification when the hash is missing, cannot be calculated, or does not match.

## 

Receiver requirements

Implement the receiver as a small, durable ingestion endpoint:

- Accept POST form fields and preserve the raw parameter values needed to validate the hash.

- Look up the expected store and shared secret from trusted server-side configuration, not from a customer browser or request-supplied secret.

- Verify notification_hash before any business-side state change.

- De-duplicate by ipgTransactionId; an endpoint should tolerate the same notification more than once.

- Apply state transitions safely. In particular, do not replace a terminal approved result with an older or less final result.

- Persist the verified event and its processing outcome before acknowledging it. Keep enough masked diagnostic data to investigate disputes.

- Return a 2xx response as soon as the notification is durably accepted. Queue slow fulfilment, email, and downstream work rather than delaying the HTTP response.

IPG delivers the notification asynchronously and can retry delivery after a transport failure or server error. Delivery has no exactly-once guarantee, so your receiver must be idempotent. Return a 5xx response only when retry is appropriate; a 4xx response is suitable for a notification that is permanently invalid.

## 

Result handling

Use the verified notification to drive order state:

Verified resultRecommended order action

APPROVEDMark the payment as successful and start the fulfilment workflow subject to your own business controls.

DECLINED or FAILEDRecord the unsuccessful payment; do not fulfil the order.

WAITINGKeep the order pending. Wait for a later notification or reconcile the transaction through the IPG reporting/query capability available to your integration.

Unknown or malformedDo not update the order. Record the event for investigation and return an error only if you want IPG to retry it.

The final business decision must use the verified status and applicable transaction type. For example, an authorization, capture, refund, or void must update the corresponding payment operation, not simply a generic order-paid flag.

## 

Example receiver pseudocode

Text

on POST /payments/ipg/notifications:
    fields = parseFormUrlEncodedBody(request)
    storeId = fields["storename"]
    secret = configuredSecretFor(storeId, trustedTransactionOrigin)

    expectedHash = createNotificationHash(
        secret,
        fields["hash_algorithm"],
        fields["chargetotal"],
        fields["currency"],
        fields["txndatetime"],
        storeId,
        fields["approval_code"]
    )

    if !constantTimeEquals(expectedHash, fields["notification_hash"]):
        recordRejectedNotification(fields)
        return 400

    eventId = fields["ipgTransactionId"]
    if alreadyProcessed(eventId):
        return 204

    persistAndApplyVerifiedPaymentUpdate(eventId, fields)
    enqueueAnySlowFollowUpWork(eventId)
    return 204

## 

Test checklist

- Configure a non-production notification endpoint for the test store.

- Test approved, failed, and, where applicable, waiting-to-final payment flows.

- Confirm that the receiver rejects an altered notification_hash and does not change the order.

- Re-submit a valid notification and confirm it does not create duplicate payments, fulfilments, or emails.

- Confirm the endpoint responds quickly while slow downstream work continues asynchronously.

- Verify that application logs and monitoring mask payment and personal data.

Updated 12 days ago 

Did this page help you?

Yes

No

- Table of Contents
- 

- How it works

- Configure the notification URL

- Notification request

- Validate the notification

- Receiver requirements

- Result handling

- Example receiver pseudocode

- Test checklist

  

  

    

      
        
      
    

    

      

        
## API Products

        
          
- 
            Payments
          
          
- 
            Boarding
          
          
- 
            Transactional Data
          
        
      

      

        
## Point of Sale

        
          
- 
            Soft Point-of-Sale
          
          
- 
            Terminals
          
          
- 
            ePOS
          
        
      

      

        
## Developer

        
          
- 
            Getting Started
          
          
- 
            API Documentation
          
          
- 
            Support
          
        
      

      

        
## Resources

        
          
- 
            Outside EMEA
          
          
- 
            CARAT
          
          
- 
            Blog
          
        
      

    

  

  

    

      Contact
      Cookies
      Privacy Notice	
      Legal
    

    

Copyright© 2024 Fiserv. All rights reserved.

    

      

        First Data Europe Limited, is a subsidiary of Fiserv, Inc. and is trading as Fiserv. First Data Europe Limited, is a private limited company incorporated in England (Company Number 02012925) with a registered address at Janus House, Endeavour Drive, Basildon, Essex, SS14 3WF. First Data Europe Limited is authorised and regulated by the UK Financial Conduct Authority (FCA Register No. 582703; CCA No. 739230). All trademarks, service marks, and trade names referenced in this material are the property of their respective owners.
      

      
v1.11

    

  

  
    
      
    
  
  
Want a quick overview?

  


