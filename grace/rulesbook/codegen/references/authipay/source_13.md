Source: https://docs.fiserv.dev/public/reference/submitprimarytransaction

Generate a primary transaction

Jump to Content

ProductsDeveloperSupport

GuidesRecipesAPI ReferenceChangelogProductsDeveloperSupportLog In

API Reference

Log In

GuidesRecipesAPI ReferenceChangelog

Generate a primary transaction

JUMP TO

## Catalog

- API Reference
- API Quick Start
- Postman Collection

## Payments API Reference

- Introduction
- Payment
- Generate a primary transactionpost
- Retrieve the state of a transaction.get
- Update a payment and continue processingpatch
- Perform a secondary transactionpost
- Perform update on an existing transactionpatch
- Order
- Perform return or postAuth secondary transactionspost
- Retrieve the state of an orderget
- Payment Schedules
- Create gateway payment schedulepost
- View a gateway payment scheduleget
- Cancel a gateway payment scheduledel
- Update a gateway payment schedulepatch
- Payment URL
- Create a payment URLpost
- Delete a payment URLdel
- Retrieve the state of payment URLget
- Payment Token
- Create a payment token from a payment cardpost
- Update one or more payment tokenspatch
- Get payment card details associated with tokenget
- Delete a payment tokendel
- Verification
- Verify a payment cardpost
- Verify a payment card or payment tokenpost
- Currency Conversion
- Generate dynamic currency conversion transactionspost
- Information Lookup
- Card Information Lookuppost
- Account Information Lookuppost
- Payment APM
- Perform an apm actionpost
- Perform action on existing APM transactionpatch
- Get Eligible Installment Plans
- Get Eligible Installment Planspost

## Checkout Solution

- Introduction
- /checkouts
- Create a new checkoutpost
- /checkouts/{checkoutId}
- Retrieve checkout detailsget

## Payment links

- /payment-links
- Create a payment linkpost
- /payment-links/{paymentLinkId}
- Get payment link detailsget

## Disputes

- Introduction
- Endpoints
- List disputesget
- Retrieve a single dispute by Idget
- Retrieve Dispute Document by IDget
- Execute an Actionpost
- Undo an Actionpost
- Add a Notepost
- Add a Documentpost

## Funding Rejects

- Introduction
- Funding Rejects
- Retrieve Funding Rejectsget
- Update Funding Reject Statuspatch

## SCA Exemptions

- Introduction
- Evaluate transaction for SCA exemptionpost
- Provide data for SCA exemption enginepost

## Statements

- Introduction
- List Statementsget
- Retrieve a single statementget

## Transactional Data

- Introduction
- List Authorisationsget
- Retrieve a single authorisationget
- List the daily authorisation summariesget
- List Transactionsget
- Retrieve a single transactionget
- List the daily transaction summariesget
- List Fundingsget
- Retrieve a single fundingget
- List funding detailsget
- List the daily funding summariesget

## OmniPay Financial Adjustments API

- Introduction
- Endpoints
- Create an Adjustmentpost
- Modify an Adjustmentput
- Cancel an Adjustmentdel
- Retrieve Adjustments - Stagedget
- Retrieve Reference Dataget

# Generate a primary transaction

post 
https://prod.emea.api.fiservapps.com/sandbox/ipp/payments-gateway/v2/payments

Use this to originate a financial transaction like a sale, preauthorization, or credit.

Recent Requests

Log in to see full request history

TimeStatusUser Agent 

Retrieving recent requests…

LoadingLoading…

Body Params

Accepted request types: PaymentCardCreditTransaction, PaymentCardForcedTicketTransaction, PaymentCardSaleTransaction, PaymentCardPreAuthTransaction, PaymentCardPayerAuthTransaction,  PaymentTerminalSaleTransaction, PaymentTerminalCreditTransaction, PaymentTerminalPreAuthTransaction, PaymentTokenCreditTransaction, PaymentTokenPreAuthTransaction, PaymentTokenSaleTransaction, SepaSaleTransaction, SepaCreditTransaction, WalletSaleTransaction, and WalletPreAuthTransaction.

PaymentCardCreditTransactionPaymentCardSaleTransactionPaymentCardPreAuthTransactionPaymentCardPayerAuthTransactionPaymentTerminalSaleTransactionPaymentTerminalCreditTransactionPaymentTerminalPreAuthTransactionPaymentTokenCreditTransactionPaymentTokenPreAuthTransactionPaymentTokenSaleTransactionSepaSaleTransactionSepaCreditTransactionWalletSaleTransactionWalletPreAuthTransactionApmSaleTransactionApmPreAuthTransactionApmCreditTransactionApmPayerAuthTransactionPaymentCardForcedTicketTransactionPaymentTokenPayerauthTransaction

Request to create credit transaction using payment card.

requestType
string

required

Object name of the primary transaction request.

transactionAmount
object

required

Amount of the transaction.

transactionAmount object

storeId
string

length ≤ 20

An optional outlet ID for clients that support multiple stores in the same app.

userId
string

length ≤ 128

This is the store's userID (not store-id) from where the product was purchased.

merchantTransactionId
string

length ≤ 40

The unique merchant transaction ID from the request header, if supplied.

transactionOrigin
string

enum

The source of the transaction. The possible values are ECOM (if the order was received via email or Internet), MOTO (mail order, telephone order), MAIL, PHONE and RETAIL (face to face).

ECOMECOMMOTOMAILPHONERETAIL

Allowed:
ECOMMOTOMAILPHONERETAIL

order
object

Use this model to provide order related details.

order object

ipgTransactionId
int64 | null

The IPG transactionId to reference a payerauth for example.

allowPartialApproval
boolean

Indicates if the particular transaction is a partial approval transaction, if supplied.

truetruefalse

parentUri
uri

To embed IPG hosted payment pages inside an iFrame this parameter is used (maximum length  2048 characters) to specify an URL of a page.

paymentMethod
object

required

Payment method containing payment card information. Only one payment card data is accepted for processing transaction either PaymantCard or PaymentCardEncrypted

paymentMethod object

currencyConversion
object

Currency conversion. Abstract class, do not use this class directly, use one of its children: Dcc, DynamicPricing.

currencyConversion object

senderReceiverInfo
object

Sender and Receiver Information

senderReceiverInfo object

terminalId
string

length between 6 and 32

Unique identifier for the payment terminal where the transaction takes place.

merchantId
string

length between 6 and 32

Unique identifier for the merchant conducting the transaction.

Headers

Client-Request-Id
string

required

A client-generated ID for request tracking and signature creation, unique per request.  This is also used for idempotency control. We recommend 128-bit UUID format.

Message-Authentication-Value
string

The Message Authentication Value (MAC) is optional header and it is only required for Card Present transactions or transactions originated from Terminals. The OpenAPI Header parameter format for the message authentication value of the complete payload follows the pattern ;;;[;][;<key version>].

- Derivation Algo: This refers to the algorithm used for key derivation. The options are 'DUKPT2009'  which represents the Derived Unique Key Per Transaction (DUKPT) algorithm, as defined by  ANSI X9.24-2009 Annex A, and 'AESDUKPT128ECB', which signifies the AES DUKPT ECB algorithm with a  key length of 128 bits, as defined in ANSI X9.24-3-2017 Annex.

- Mac Algo: This points to the algorithm used for Message Authentication Code (MAC). There are two  options: 'RetailSHA256MAC' which indicates the Retail-CBC-MAC using SHA-256 (Secure Hash standard)  and an ASN.1 Object Identifier: id-retail-cbc-mac-sha-256. The other option is 'SHA256CMACwithAES128',  which represents the CMAC (Cipher-based Message Authentication Code) as defined by NIST 800-38B -  May 2005. This option employs the Advanced Encryption Standard block cipher with a 128-bit  cryptographic key, as approved by FIPS 197 - November 6, 2001. The CMAC algorithm is computed on  the SHA-256 digest of the message.

The rest of the parameters include the 'key index', 'key name' (optional), and 'key version' (optional),  which are not specified here but contribute to the full formatting of the header parameter.

Api-Key
string

required

Key given to merchant after boarding associating their requests with the appropriate app in Apigee.

Timestamp
int64

required

Epoch timestamp in milliseconds in the request from a client system. Used for Message Signature generation and time limit (5 mins).

Message-Signature
string

required

Used to ensure the request has not been tampered with during transmission. The Message-Signature is the Base64 encoded HMAC hash (SHA256 algorithm with the API Secret as the key.) For more information, refer to the supporting documentation on the Developer Portal.

Responses

# 

 200

Success response.

# 

 400

The request cannot be validated.

# 

 401

The request cannot be authenticated or was submitted with the wrong credentials.

# 

 403

The request was unauthorized.

# 

 404

The requested resource doesn't exist.

# 

 409

The attempted action is not valid according to gateway rules. For example, the merchant is not set-up or the order already exists.

# 

 415

Format that is not supported by the server for the HTTP method.

# 

 422

The processor declined the transaction.

# 

 500

An unexpected internal server error occurred.

# 

 502

There was a problem communicating with the endpoint.

Updated 1 day ago 

Did this page help you?

Yes

No

Language

ShellNodeRubyPHPPython

Credentials

Header

Header

Log in to use your API keys

URL

Base URL
https://prod.emea.api.fiservapps.com/sandbox/ipp/payments-gateway/v2/payments

LoadingLoading…

Response 

Click Try It! to start a request and see the response here! Or choose an example:

application/json

200400401403404409415422500502

Updated 1 day ago 

Did this page help you?

Yes

No

  

  

    

      
        
      
    

    

      

        
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

  


