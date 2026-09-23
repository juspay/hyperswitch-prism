Source: https://docs.fiserv.dev/public/docs/payment-methods

Payment Methods

Jump to Content

ProductsDeveloperSupport

GuidesRecipesAPI ReferenceChangelogProductsDeveloperSupportLog In

Guides

Log In

GuidesRecipesAPI ReferenceChangelog

Payment Methods

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

# Payment Methods

Suggest Edits

This page details how we categorize the types of payment used, known as the 'payment instrument' or 'payment method'.

There are many different forms of 'payment instrument'; the item or mechanism a party uses to pay another party. The main categories for these within our APIs are:

- Card - a physical or digital card entity, these will sit under a 'card scheme' such as Visa and contain information like a card number and expiry date, the data required and stored in this case follows a specific set of rules

- Alternative - an alternative form of payment, such as digit wallets like Google Pay or PayPal, or tokenized card data, the data stored here varies depending on the service used 

## 

Card Brands

See the following table for some examples on the combined set of card brands / schemes referenced across RESTful payment API.

Value

(paymentMethod...
...paymentCard.brand)

OR

(paymentToken.brand)Description

AMEXClosed-loop payment scheme named American Express

CUPUnion Pay International (fomer knwn as China Union Pay)

DINERSCLUBCard brand owned by Discover

EFTPOSCard brand used in Australia

JAYWANDomestic payment network based in the United Arab Emirates

JCBInternational payment brand based in Japan, works with Discover and AMEX

MAESTROBrand of debit cards owned by Mastercard

MASTERCARDSecond most common card scheme supported almost everywhere

RUPAYGlobal payment network based in India

VISAMost common card scheme supported almost everywhere

## 

Alternative Payment Methods

Examples of alternative payment methods (sometimes abbreviated to APM's) are as follows:

Value

(paymentMethod.type)Description

ALIPAY_PLUSAlternative payment method supporting Alipay and sub brands

AANIAlternative payment method offered by Central Bank of UAE

BANCONTACT_QRMost common alternative payment method in Belgium

BIZUMMost common alternative payment method in Spain

BLIKMobile / instant payment system used in Poland

E-TRANSFERMobile payment in Poland called Token Banking

MAUCASQRAlternative paymnet method offered by ABSA

NATWEST_PAYITPayit (by Natwest)

PAYPALMost widely used digital wallet

POSTFINANCE_CARDAlternative payment method in Switzerland

POSTFINANCE_EFAlternative payment method in Switzerland

POSTFINANCE_PAYMost common alternative payment method in Switzerland

(will replace the other Postfinance methods in the future)

SAMSUNG_PAYEncrypted SamsungPay transaction and web SDK support

TWINTAlternative payment method in Switzerland

VISA_MOBILEVisa owned digital wallet (only available in Poland)

## 

China Domestic Payment Methods

## 📘
only available in APAC region

Value

(brand)Description

ALIPAY_DOMESTICMobile / digital wallet in China

CUP_DOMESTICMobile / digital wallet in China

WECHAT_DOMESTICMobile / digital wallet in China

## 

SEPA Payment

## 📘
only available in EMEA region

Value

(paymentMethod)Description

sepaobject with sub elementsSEPA direct debit payment

## 

Wallet Payment Methods

Value (walletPaymentMethod)Description

EncryptedGooglePayWalletPaymentMethodEncrypted GooglePay transaction

EncryptedApplePayWalletPaymentMethodEncrypted ApplePay transaction

DecryptedApplePayWalletPaymentMethodDecrypted ApplePay transaction

DecryptedGooglePayWalletPaymentMethodDecrypted GooglePay transaction

DecryptedSamsungPayWalletPaymentMethodDecrypted SamsungPay transaction

## 🚧
For more payment brands please check the latest version of yaml file.

Along with the brand you will receive paymentMethodType and paymentMethodBrandunder PaymentMethodDetails in the response.

Example:

paymentMethodType = PAYMENT_CARD

paymentMethodBrand = VISA

Updated 10 months ago 

Did this page help you?

Yes

No

- Table of Contents
- 

- Card Brands

- Alternative Payment Methods

- China Domestic Payment Methods

- SEPA Payment

- Wallet Payment Methods

  

  

    

      
        
      
    

    

      

        
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

  


