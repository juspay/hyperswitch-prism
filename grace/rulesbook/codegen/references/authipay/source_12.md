Source: https://docs.fiserv.dev/public/reference/orderinquiry

Retrieve the state of an order

Jump to Content

ProductsDeveloperSupport

GuidesRecipesAPI ReferenceChangelogProductsDeveloperSupportLog In

API Reference

Log In

GuidesRecipesAPI ReferenceChangelog

Retrieve the state of an order

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

# Retrieve the state of an order

get 
https://prod.emea.api.fiservapps.com/sandbox/ipp/payments-gateway/v2/orders/{order-id}

Use this query to get the current state of an existing order.

Recent Requests

Log in to see full request history

TimeStatusUser Agent 

Retrieving recent requests…

LoadingLoading…

Path Params

order-id
string

required

Gateway order identifier as returned in the parameter orderId.

Query Params

storeId
string

An optional outlet ID for clients that support multiple stores in the same developer app.

Headers

Client-Request-Id
string

required

A client-generated ID for request tracking and signature creation, unique per request.  This is also used for idempotency control. We recommend 128-bit UUID format.

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

 500

An unexpected internal server error occurred.

# 

 502

There was a problem communicating with the endpoint.

Updated almost 3 years ago 

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
https://prod.emea.api.fiservapps.com/sandbox/ipp/payments-gateway/v2/orders/{order-id}

LoadingLoading…

Response 

Click Try It! to start a request and see the response here! Or choose an example:

application/json

200400401403404500502

Updated almost 3 years ago 

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

  


