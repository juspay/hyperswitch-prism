Source: https://docs.fiserv.dev/public/docs/payments-test-cards

Test Cards

Jump to Content

ProductsDeveloperSupport

GuidesRecipesAPI ReferenceChangelogProductsDeveloperSupportLog In

Guides

Log In

GuidesRecipesAPI ReferenceChangelog

Test Cards

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

# Test Cards

Suggest Edits

# 

3-D Secure / Authentication Test Data

## ❗️
Please note, that 3DS test cards are configured to support only below listed authentication scenarios and are not meant to be used for end-to-end testing including authorization processing.

Expiry date = any future date, e.g. 12/2028

CVV = any 3 digits number, e.g. 123

## 

Frictionless Flow

Scenario3DS Response Code3DS Transaction StatusTest card number

Frictionless - Fully Authenticated 1Y4147463011110083

5239290700000028

Frictionless - Not Authenticated3N4147463011110091

5239290700000036

Frictionless - Attempted Authentication4A4147463011110117

5239290700000044

Frictionless - Rejected Authentication3R4147463011110042

5239290700000051

Frictionless - Unable to authenticate6U4147463011110067

4147463011110125

5239290700000069

## 

Frictionless Flow with 3DSMethod

Scenario3DS Response Code3DS Transaction StatusTest card number

Frictionless - Fully Authenticated 1Y4012000000012011004

4761120010000492

4265880000000007

4265880000000049

4099000000001978

5204740000002711

5204247750001471

Frictionless - Not Authenticated3N4012000000012011012

4265880000000015

4099000000001986

5204740000002729

5426064000425117

5426064000425190

Frictionless - Attempted Authentication4A4149011500000519

4265880000000023

5426064000425208

Frictionless - Rejected Authentication3R4265880000000031

4012000000012011038

5204740000002778

Frictionless - Unable to authenticate6U4012000000012011020

4012001037167778

4265880000000056

4265880000000072

4265880000000080

5204740000002786

5426064000425216

## 

Challenge Flow

Scenario3DS Response Code3DS Transaction StatusTest card number

Challenge - Configurable responses 1 or 4 or 6 or 3Y or A or U or N/R4147463011110059

5239290700000002

Challenge - Fully Authenticated1Y4147463011110109

Challenge - Rejected Authentication3R4147463011110034

5239290700000010

## 

Challenge Flow with 3DSMethod

Scenario3DS Response Code3DS Transaction StatusTest card number

Challenge - Configurable responses 1 or 4 or 6 or 3Y or A or U or N/R4099000000001960

4149011500000527

4265880000000064

5204740000002745

5544330000000235

Challenge - Rejected Authentication3R4149011500000535

5204740000002760

# 

General Test Cards

## ❗️
Following test data can be used to test credit card processing on the Gateway, but they are not compatible with all authorization host simulators automatically.

Card NumberPINCVVExpiry MonthExpiry YearCard BrandCard TypeDescription

476173900101001012340021030VisaPrepaidSuccessful Transaction

400552000000012912340021030VisaCreditSuccessful Transaction

541333008901064043150021030MastercardCreditSuccessful Transaction

541333008960011943150021030MastercardCreditSuccessful Transaction

37424500172100912340021030AmexCreditSuccessful Transaction

4035874000424977-9771230VisaCreditSuccessful Transaction

5413330089010640-Any1230MastercardCreditSuccessful Transaction

Updated almost 2 years ago 

Did this page help you?

Yes

No

- Table of Contents
- 

- 
3-D Secure / Authentication Test Data

- Frictionless Flow

- Frictionless Flow with 3DSMethod

- Challenge Flow

- Challenge Flow with 3DSMethod

- 
General Test Cards

  

  

    

      
        
      
    

    

      

        
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

  


