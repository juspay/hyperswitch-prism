Source: https://docs.fiserv.dev/public/docs/payments-general-concepts

General Concepts

Jump to Content

ProductsDeveloperSupport

GuidesRecipesAPI ReferenceChangelogProductsDeveloperSupportLog In

Guides

Log In

GuidesRecipesAPI ReferenceChangelog

General Concepts

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

# General Concepts

Suggest Edits

To fully leverage the power of the payments API, the following concepts should be understood.

## 

Primary and Secondary Transactions

A primary transaction (performed via this endpoint or via our hosted solutions) is a new initiating payment transaction. A few examples of different primary transactions would be:

- A standard one-time transaction (sale) is made when someone purchases their shopping at the supermarket

- A 'pre-authorisation' transaction is made when a person uses the pay-at-pump gas station before any payment is taken

A secondary transaction (performed using either the transaction id or the order id) is one that serves to follow up a previous primary transaction. A few examples of different secondary transactions would be:

- A 'void' is performed on a primary transaction to cancel the payment on the same day before any money is actually moved

- A 'return' is performed on a primary transaction to refund the payment on a following day

- A 'completion' is made to a primary 'pre-authorisation' when the person completes their use of the pay-at-pump gas station

## 

PCI DSS

The Payment Card Industry Data Security Standard (PCI DSS) is an information security standard for organisations that handle payment information such as credit card details.

Please note that if you plan to collect your customer's card details within your environment and send it to our API, you must ensure that your system components are PCI DSS compliant. This is particularly important if you plan to store any of the information.

To avoid the need to worry about PCI DSS, we recommend taking a look at our Checkout product which is a hosted solution for processing payments without you needing to process or store payment information.

## 

Orders

Orders are a concept that create a through-line for the lifetime of a payment, grouping relating primary and secondary transactions. When using the payment API, it is useful to keep track of orders using the order inquiry endpoint to ensure that the full picture of a payment can be recorded.

In the pay-at-pump example above, retrieving the order would include reference to both the primary pre-auth transaction as well as the secondary pre-auth completion transaction

## 

Payment Methods

The payment method (also known as payment instrument) is the item used to make the payment. This might be a debit card, a digital wallet, or a SEPA bank transfer.

In the Payments API this is controlled by the requestType (detailed above) as well as the resulting information provided as part of the polymorphic model.

For more detail on the different types of instruments supported by our platform, see the Payment Methods 

## 

Request Type

The requestType field is used to define the high-level nature of the payment, such as the type of transaction (pre-auth, sale etc.) and the type of instrument used (card, wallet etc.). This field is used in most POST requests as part of the Payment API such as Submit a Primary Transaction.

The requestType field has an allowable list of values, which also defines which additional fields are required as part of the payment. See the table below for some detail on the different values for primary transaction requestType:

requestTypePayment Scenario

PaymentCardSaleTransactionExecute a normal customer payment transaction with a credit or debit card.

PaymentCardCreditTransactionExecute an original credit payment transaction to a customer’s credit or debit card. This means you’ll refund this amount to the customer’s card without a reference to any previous Sale transaction

PaymentCardPreAuthTransactionPre-authorize an amount against a card, for completion at a later point.

PaymentTerminalSaleTransactionInitiate a sale transaction from a physical terminal.

PaymentTerminalPreauthTransactionInitiate a pre-authorization from a physical terminal.

PaymentTerminalCreditTransactionInitiate a naked refund from a physical terminal.

PaymentTokenSaleTransactionExecute a normal customer payment transaction with a token generated previously.

PaymentTokenCreditTransactionExecute a return to a customer’s credit or debit card using a token generated previously.

PaymentTokenPreAuthTransactionPre-authorise an amount using a token, for completion at a later point.

SepaSaleTransactionTake payment from a customer via SEPA.

SepaCreditTransactionTake a naked refund from a customer via SEPA.

WalletSaleTransactionExecute a normal customer payment transaction with a wallet payment method.

WalletPreAuthTransactionExecute a pre-authorization using a Wallet Payment Method.

ApmSaleTransactionInitiate a sale transaction via an alternative payment method.

ApmPreauthTransactionInitiate a pre-authorization via an alternative payment method.

And the list of available secondary transaction requestType:

requestTypeDescription

VoidTransactionCancel a transaction you submitted earlier the same day

VoidPreAuthTransactionsCancel all transaction done under a pre-authorization (same order ID)

PostAuthTransactionComplete a pre-authorization Transaction against the same

ReturnTransactionComplete a refund against a transaction taken prior to the current day

PreAuthSecondaryTransactionCreate an incremental or decremental secondary transaction against an existing pre-authorization

Updated almost 3 years ago 

Did this page help you?

Yes

No

- Table of Contents
- 

- Primary and Secondary Transactions

- PCI DSS

- Orders

- Payment Methods

- Request Type

  

  

    

      
        
      
    

    

      

        
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

  


