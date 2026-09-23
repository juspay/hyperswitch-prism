Source: https://docs.fiserv.dev/public/docs/message-signature

Message Signature

Jump to Content

ProductsDeveloperSupport

GuidesRecipesAPI ReferenceChangelogProductsDeveloperSupportLog In

Guides

Log In

GuidesRecipesAPI ReferenceChangelog

Message Signature

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

# Message Signature

Suggest Edits

When calling our APIs you will often find you need to create a Message-Signature. This is a combination of your API Key, a ClientRequestId, the time and the body of your request.  We recommend you follow the linked recipe below to get a feel for how it is used. The rest of this page goes into more detail about it.

💳

Generate a message signature

Open Recipe

## 

Example

A standard API call to execute a Primary Transaction in our Payments API might look like this:

JSON

{
    method: "POST",
    url: "https://prod.emea.api.fiservapps.com/sandbox/ipp/payments-gateway/v2/payments/",
    headers: {
      "Content-Type": "application/json",
      "Client-Request-Id": "Client request ID goes here",
      "Api-Key": "API Key goes here",
      "Timestamp": "Date().getTime() goes here",
      "Message-Signature": "Message Signature goes here"
    },
    body: JSON.stringify({
      requestType: "PaymentCardSaleTransaction",
      transactionAmount: { total: "13", currency: "GBP" },
      paymentMethod: {
        paymentCard: {
          number: "4012000000000001",
          securityCode: "123",
          expiryDate: { month: "01", year: "29" }
        },
      },
      authenticationRequest: {
        authenticationType: "Secure3D21AuthenticationRequest",
        termURL: "http://localhost:3124/api/v1/payments/3ds",
        challengeIndicator: "04"
      },
    })
  }

## 

Variables

### 

API-Key

You can retrieve this key from the Developer Portal

### 

Client-Request-Id

This is generated by you the client of our API. It is a unique ID that is returned to you as part of the response. You can generate it like this:

JavaScript

var ClientRequestId = uuidv4();

### 

Time

The getTime() method returns the number of milliseconds since the Unix Epoch. The Unix epoch is the time 00:00:00 UTC on 1 January 1970.

JavaScript

var time = new Date().getTime();

### 

Request body

The request body must be stringified. 

JavaScript

var requestBody = JSON.stringify(body);

### 

Message-Signature

All of the variables above need to be available for the Message-Signature to be generated.

Generate the rawSignature first:

JavaScript

var rawSignature = apiKey + ClientRequestId + time + requestBody;

Create a HMAC using CryptoJS

JavaScript

var computedHash = CryptoJS.algo.HMAC.create(
    CryptoJS.algo.SHA256,
    secret.toString()
);

## 🚧
Secret Key

You can retrieve this secret key from the Developer Portal Apps screen. It must be the from the same application as your API Key.

Update your computedHash with your rawSignature

JavaScript

computedHash.update(rawSignature);
computedHash = computedHash.finalize();
var messageSignature = CryptoJS.enc.Base64.stringify(computedHash);

That's it, add the messageSignature into your header.

## 

Demo app

We have a demo app available here which has the above code.

## 

Example of code

JavaScript

//These libraries are for the running of this API.
const express = require("express");
const router = express.Router();
var request = require("request");
var CryptoJS = require("crypto-js");
const {v4: uuidv4} = require("uuid");

//These are the API keys and token for generating an encrypted message
const key = "API Key goes here";
const secret = "Secret goes here";
const url = "https://prod.emea.api.fiservapps.com/sandbox/ipp/payments-gateway/v2/payments/"

//When ever you communicate with IPG you need to encrypt the body of the message. This function modifies the API call to include the correct message signatures. 
function fiservEncode(method, url, body, callback) {
  var ClientRequestId = uuidv4();
  var time = new Date().getTime();
  var requestBody = JSON.stringify(body);
  if(method === 'GET') {
    requestBody = '';
  }  
  var rawSignature = key + ClientRequestId + time + requestBody;
  var computedHash = CryptoJS.algo.HMAC.create(
    CryptoJS.algo.SHA256,
    secret.toString()
  );
  computedHash.update(rawSignature);
  computedHash = computedHash.finalize();
  var computedHmac = CryptoJS.enc.Base64.stringify(computedHash);

  var options = {
    method: method,
    url,
    headers: {
      "Content-Type": "application/json",
      "Client-Request-Id": ClientRequestId,
      "Api-Key": key,
      Timestamp: time,
      "Message-Signature": computedHmac
    },
    body: JSON.stringify(body),
  };

    callback(options);

}

//Step 2: Create Primary Transaction (Only performs a standard payment that requests 3DSecure!)
router.post("/payments", async (req, res) => {
  //Start by encoding the message.
  fiservEncode(
    "POST",
    url,
    {
      requestType: "PaymentCardSaleTransaction",
      transactionAmount: { total: "13", currency: "GBP" },
      paymentMethod: {
        paymentCard: {
          number: req.body.cardNumber,
          securityCode: req.body.securityCode,
          expiryDate: { month: req.body.expiryMonth, year: req.body.expiryYear },
        },
      },
      authenticationRequest: {
        authenticationType: "Secure3D21AuthenticationRequest",
        termURL: "http://localhost:3124/api/v1/payments/3ds",
        challengeIndicator: "04", // This indicates what type of transaction we would like. 
      },
    },
    (options) => {
      //Submit the API call to Fiserv
      request(options, function (error, paymentResponse) {
        if (error) throw new Error(error);
        let paymentData = JSON.parse(paymentResponse.body);
        return res.status(200).json({
          requestName: "Payment POST - Creating the payment request",
          ...paymentData
        });
      });
    }
  );
});

Updated almost 3 years ago 

Did this page help you?

Yes

No

- Table of Contents
- 

- 
Example

- 
Variables

- API-Key

- Client-Request-Id

- Time

- Request body

- Message-Signature

- 
Demo app

- 
Example of code

  

  

    

      
        
      
    

    

      

        
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

  


