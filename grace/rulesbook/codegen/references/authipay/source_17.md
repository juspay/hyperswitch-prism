Source: https://www.aibms.com/wp-content/uploads/2025/07/AIBMS_Authipay_API-Integration-Guide.pdf

Web Service API
Integration Guide
Version 2021-3 (IPG)
Contents
1. Introduction                                                 5

2. Artefacts You Need                                           6

3. How the API works                                            9


4 Sending transactions to the Gateway                          11


5      Building Transactions in XML                            11
       5.1.1 Credit/Debit Card transactions                    12
       5.1.2 Sale 		                                           13
       5.1.3 Pre-Authorisation                                 14
       5.1.4 Post-Authorisation                                14
       5.1.5 ForceTicket                                       15
       5.1.6 Return 		                                         15
       5.1.7 Credit 		                                         16
       5.1.8 Void 		                                           16
       5.1.9 Recurring Sale (Merchant-triggered)               17
       5.1.10 Standing Instructions                            19
       5.1.11 MasterPass™                                      19
       5.1.12 SEPA Direct Debit - Germany                      20
       5.1.13 Sale 		                                          20
       5.1.14 Void 		                                          21
       5.1.15 Credit 		                                        21
       5.1.16 Return                                           22
       5.1.17 SEPA Direct Debit with Authipay Local Payments   22
       5.1.18 PayPal                                           24
       5.1.19 Post-Authorisation Payment Transaction           24
       5.1.20 Recurring Payment Transaction                    25
       5.1.21 Return                                           25
       5.1.22 Void 		                                          26
       5.1.23 Credit                                           26
       5.1.24 SOFORT Überweisung                               27
       5.1.25 Return                                           27
       5.1.26 iDEAL                                            27
       5.1.27 Return                                           27
       5.1.28 Generic Transaction Type for Voids and Returns   28


6      Additional Web Service actions                          11
       6.1.1 Initiate Clearing                                 29
       6.1.2 Inquiry Order                                     30
       6.1.3 Inquiry Transaction                               33
       6.1.4 Get Last Orders                                   34
       6.1.5 Latest orders of a Store                          34


Web Service API Integration Guide
Contents (continued)
       6.1.6 Latest orders of a Store within a given date range                  34
       6.1.7 All orders of a Store after a given Order ID                        35
       6.1.8 Response                                                            35
       6.1.9 Get Last Transactions                                               39
       6.1.10 Latest transactions of a Store                                     39
       6.1.11 All transactions of a Store after a given Transaction ID          40
       6.1.12 Response                                                          40
       6.1.13 Recurring Payments (Scheduler)                                     42
       6.1.14 Install 42
       6.1.15 Modify                                                             43
       6.1.16 Cancel                                                            44
       6.1.17 Test Recurring Payments in test environment                       44
       6.1.18 Response                                                          44
       6.1.19 External transaction status                                       44
       6.1.20 Trigger email notifications                                        45
       6.1.21 Card Information Inquiry                                           45
       6.1.22 Basket Information and Product Catalogue                           45
       6.1.23 Basket information in transaction messages                         45
       6.1.24 Setting up a Product Catalogue                                    46
       6.1.25 Manage Product Stock                                               47
       6.1.26 Sale transactions using product stock                             48


7      Data Vault                                                                11
       7.1.1 Token Type Options                                                 49
       7.1.2 Store or update payment information when performing a transaction 52
       7.1.3 Store payment information from an approved transaction              52
       7.1.4 Initiate payment transactions using stored data                     53
       7.1.5 S
              tore payment information without performing a transaction
             at the same time                                                    53
       7.1.6 Avoid duplicate cardholder data for multiple records                56
       7.1.7 Display stored records                                              56
       7.1.8 Delete existing records                                             56


8      Global Choice™ and Dynamic Pricing                                        11
       8.1.1 Exchange rate requests for Global Choice™                           58
       8.1.2 Exchange rate requests for Dynamic Pricing                          59
       8.1.3 Exchange rate responses                                            60
       8.1.4 Conversion offering                                                60
       8.1.5 Declined rate request                                               61
       8.1.6 Failed rate request                                                 61
       8.1.7 Global Choice™ transactions                                         62
		             Step 1: Rate request                                              62
		             Step 2: Using the conversion rate for the payment transaction   6348




Web Service API Integration Guide
Contents (continued)
9      Payment URL                                                            11
       9.1.1 Payment URL creation                                             65
       9.1.2 Payment URL                                                      66
       9.1.3 Payment URL custom text                                          66


10 Solvency Information from Bürgel                                           11


11 3-D Secure Authentication                                                  11
       11.1 3-D Secure authentication (3DS 1.0)                               68
       11.2 EMV 3-D Secure authentication (3DS 2.0)                           72
       11.2.1 Non-Payment Authentication (NPA)                                72


12 Purchasing cards                                                           11


13 XML-Tag overview                                                           11
       13.1.1 Overview by transaction type                                    76
       13.1.2 Description of the XML-Tags                                     85
       13.1.3 CreditCardTxType                                                85
       13.1.4 CreditCardData                                                  85
       13.1.5 recurringType                                                   86
       13.1.6 UnscheduledCredentialOnFileType                                 86
       13.1.7 Wallet 		                                                       86
       13.1.8 cardFunction                                                    86
       13.1.9 CreditCard3DSecure                                              87
       13.1.10 India Mobile / IVR Extension Verification Request              88
       13.1.11 India Mobile / IVR Extension Authentication Request            89
       13.1.12 3DSecure 1.0 Authentication / Verification Redirect Response   89
       13.1.13 3DSecure 1.0 Authentication / ACS Response                     90
       13.1.14 UnionPay Secure Plus                                           90
       13.1.15 UnionPay SecurePlusRequest                                     90
       13.1.16 DE_DirectDebitTxType                                           91
       13.1.17 DE_DirectDebitData                                             91
       13.1.18 PayPalTxType                                                   92
       13.1.19 Payment                                                        92
       13.1.20 TransactionDetails                                             93
       13.1.21 Purchasing Cards                                               94
       13.1.22 Purchasing Cards / Line Item Data                              95
       13.1.23 InquiryRateReference                                           95
       13.1.24 Billing                                                        96
       13.1.25 Shipping                                                       96
       13.1.26 ClientLocale                                                   97
       13.1.27 RequestCardRateForDCC                                          97
       13.1.28 RequestMerchantRateForDynamicPricing                           97
       13.1.29 CardRateForDCC and MerchantRateForDynamicPricing               98

Web Service API Integration Guide
Contents (continued)
       13.1.30 MCC 6012 Visa and Mastercard Mandate     98
       13.1.31 Market Segment Addendum                  99
       13.1.32 SCA Exemptions                           99
       13.2.31 China Domestic                           99
       13.2.32 EMI with ICICI Debit Card               100
       13.2.33 Boleto                                  101

       13.2.33 StandIn Details                         102


14 Custom Parameters                                    11
       14.1.1 Additional parameters for Fraud Detect


15 Building a SOAP Request Message                      11


16 Reading the SOAP Response Message                    11
       16.1.1 SOAP Response Message                    105
       16.1.2 SOAP Fault Message                       106
       16.1.3 SOAP-ENV:Server                          106
       16.1.4 SOAP-ENV:Client                          107


17 Analysing the Transaction Result                     11
       17.1.1 Transaction Approval                     109
       17.1.2 Transaction Failure                      111


18 Building an HTTPS POST Request                       11
       18.1.1 PHP 		                                   113
		             Using the cURL PHP Extension            114
		             Using the cURL Command Line Tool        114
       18.1.2 ASP 		                                   115


19 Establishing a TLS connection                        11
       19.1.1 PHP 		                                   113
		             Using the PHP cURL Extension            114
		             Using the cURL Command Line Tool        114
       19.1.2 ASP 		                                   115


20 Sending the HTTPS POST Request
   and Receiving the Response                           11
       20.1.1 PHP 		                                   119
		             Using the PHP cURL Extension            119
		             Using the cURL Command Line Tool        119
       20.1.2 ASP 		                                   120




Web Service API Integration Guide
Contents (continued)
21 Using a Java Client to connect
   to the web service                                                      11
       21.1.1 Instance an IPGApiClient                                    120
       21.1.2 How to construct a transaction and handle the response      121
       21.1.3 How to construct an action                                  121
       21.1.4 How to connect behind a proxy                               122


22 Appendix                                                                11
       XML 			                                                            122
       XML Schemata                                                       122
       Union Pay SecurePlus                                               123
       Bancontact QR code transactions                                    127
       China domestic processing                                          129
       Troubleshooting – Merchant Exceptions                              130
       Troubleshooting –Processing Exceptions                             135
       Troubleshooting –Login error messages when using cURL              139
       Troubleshooting –Login error messages when using the Java Client   141




Web Service API Integration Guide
Getting Support
There are different manuals available for Authipay’s eCommerce solutions.
This Integration Guide will be the most helpful for integrating the Web Service API
for usage with our distribution channels in Europe, Asia, Australia, Latin America
and Africa.

For information about settings, customisation, reports and how to process
transactions manually (by keying in the information) please refer to the User Guide
Virtual Terminal.

If you have read the documentation and cannot find the answer to your question,
please contact your local support team.

Information for merchants with existing Web Service API integration using the
Java client to connect to the web service:

•	The implementation of the IPGApiClient and some signatures of methods of
   this class have been changed due to a change from appache http client 3.x
   to appache http client 4.x

• 	Transaction classes and transaction factory have not been changed

• 	If the previous IPGApiClient works in your environment, you can continue to
    use it.




Authipay Connect Integration Guide
1. Introduction
The Web Service API is an Application Programming Interface which allows you to connect your application with
the Authipay Gateway. In this way, your application is able to submit payment transactions without any user
interference.
Please note that if you store or process cardholder data within your own application, you must ensure that your
system components are compliant with the Data Security Standard of the Payment Card Industry (PCI DSS).
Depending on your transaction volume, an assessment by a Qualified Security Assessor may be mandatory to
declare your compliance status.
From a technical point of view, this API is a Web Service offering one remote operation for performing transactions.
The three core advantages of this design can be summarized as follows:
• Platform independence: Communicating with the Web Service API means that your application must only be
  capable of sending and receiving SOAP messages. There are no requirements tied to a specific platform, since
  the Web Service technology builds on a set of open standards. In short, you are free to choose any technology
  you want (e.g. J2EE, . PHP, ASP, etc.) for making your application capable of communicating with the Web
  Service API.
• Easy integration: Communicating with a Web Service is simple – your application has to build a SOAP request
  message encoding your transaction, send it via HTTPS to the Web Service and wait for a SOAP response
  message which contains your transaction’s status report. Since SOAP and HTTP are designed to be lightweight
  protocols, building requests and responses becomes a straightforward task. Furthermore, you rarely have to
  do this manually, since there are plenty of libraries available in almost every technology. In general, building a
  SOAP request and handling the response is reduced to a few lines of code.
• Security: All communication between your application and the Web Service API is TLS-encrypted. This is
  established by your application holding a client certificate which identifies it uniquely at the Web Service. In
  the same way, the Authipay Gateway holds a server certificate which your application may check for making
  sure that it speaks to our Web Service API. Finally, your application has to do a basic authentication (user
  name / password) before being allowed to communicate with the Web Service. In this way, the users who
  are authorised to communicate with the Authipay Gateway are identified. These two security mechanisms
  guarantee that the transaction data sent to Authipay both stays private and is identified as transaction data that
  your application has committed and belongs to no one else.
While this represents just a short summary of the Web Service API’s features, the focus of this guide lies on
integrating the Authipay Gateway functionality into your application. A detailed description, explaining how this is
done step by step, is presented in this guide.




Web Service API Integration Guide                                                                      1. Introduction 8
2. Artefacts You Need
Supporting a high degree of security requires several artefacts you need for communicating securely with the Web
Service API. Since these artefacts are referenced throughout the remainder of this guide, the following checklist
shall provide an overview enabling you to make sure that you have received the whole set when registering your
application for the Authipay Gateway:
• Store ID: Your store ID (e.g. 10012345678) which is required for the basic authentication.
• User ID: The user ID denoting the user who is allowed to access the Web Service API, e.g. 1.
  Again, this is required for the basic authentication.
• Password: The password required for the basic authentication.
• Client Certificate p12 File: The client certificate and private key stored in a p12 file having the naming scheme
  WSstoreID._.userID.p12, e.g. in case of the above store ID / user ID examples, this would be WS101._.007.p12.
  This file is used for authenticating the client at the Gateway. For connecting with Java you need a ks-File, e.g.:
  WS10012345678._.1.ks.
• Client Certificate Installation Password: The password which is required to access the p12 file (containing the
  client certificate and private key file).
• Client Certificate Private Key: The private key of the client certificate stored in a key file having the naming
  scheme WSstoreID._.userID.key, e.g. in case of the above store ID / user ID examples, this would be
  WS10012345678._.1.key. Some tools which support you in setting up your application for using the Web Service
  API require the private key in this format when doing the client authentication at the Gateway.
• Client Certificate Private Key Password: This password protects the private key of the client certificate. This
  password is needed to access the private key file (“Client Certificate Private Key”) It follows the naming scheme
  ckp_creationTimestamp. For instance, this might be ckp_1193927132.
• Client Certificate PEM File: The list of client certificates stored in a PEM file having the naming
  scheme WSstoreID._.userID.pem, e.g. in case of the above store ID / user ID examples, this would be
  WS10012345678._.1.pem. Some tools which support you in setting up your application for using the Gateway
  require this file instead of the p12 file described above.
• Trust Anchor as concatenated PEM File (tlstrust.pem): The file contains a list of client certificates you should trust
  to establish a trusted connection to the running the Web Service API. A Concatenated list of PEM-formatted
  certificates allow easy installation for Apache Webservers or PHP. Trust Anchor as Java Keystore File (truststore.
  jsk): The file contains a list of client certificates you should trust to establish a trusted connection to the server
  running the Web Service API. This format is can easily support Java-based integrations
• Trust Anchor as PKCS#7 File (tlstrust.p7b): This file contains a list of CA certificates you should trust to establish
  a trusted connection to the server running the Web Service API. PKCS#7 Files allow the easy installation of
  multiple certificate for example within Microsoft Windows.
If you should be planning to handle multiple Store IDs through your integration, we can issue a special API User
and Client Certificate for you that you can use across all your Stores. When you submit transactions from that API
user, you do not need to vary the API User Name as the API User is the same for all your Stores. You will need to
include the Store ID in each transaction request in that case.




Web Service API Integration Guide                                                                    2. Artefacts You Need 9
3. How the API works
The following section describes the API by means of a credit card transaction. The process for other payment types
is similar.
In most cases, a customer starts the overall communication process by buying goods or services with her credit
card in your online store. Following this, your store sends a credit card transaction (mostly in order to capture the
customer’s funds) via the Web Service API. Having received the transaction, the Authipay Gateway forwards it to
the credit card processor for authorisation. Based on the result, an approval or error is returned to your online store.
This means that all communication and processing details are covered by the Authipay Gateway and you only have
to know how to communicate with this Web Service.




The Web Service Standard defines such an interface by using the Web Service Definition Language (WSDL).
A WSDL file defining the Web Service API for the Authipay Gateway can be found at:
https://test.ipg-online.com/ipgapi/services/order.wsdl
Note that you will have to supply your client certificate and your credentials, when viewing or requesting the file
e.g. in a Web browser. For instance, in case you want to view the WSDL file in Microsoft’s Internet Explorer running
on Microsoft Windows XP, you first have to install your client certificate, and then call the above URL. This is done
by executing the following steps:
1. Open the folder in which you have saved your client certificate p12 file.
2. Double-click the client certificate p12 file.
3. 	Click Next. Check the file name (which should be already set to the path of your client certificate p12 file) and
     click Next.
4. Provide the Client Certificate Installation Password and click Next.
5. 	Choose the option Automatically select the certificate store based on the type of certificate and click Next.
     This will place the certificate in your personal certificate store (more precisely in the local Windows user’s
     personal certificate store).
6. Check the displayed settings and click Finish. Your client certificate is now installed.
7. Now, open a Microsoft Internet Explorer window and provide the above URL in the address field.
8. 	After requesting the URL, the server will ask your browser to supply the client certificate to making sure that it is
     talking to your application correctly. Since you have installed the certificate in the previous steps, it is transferred
     to the server without prompting you for any input (i.e. you will not notice this process). Then, the Authipay
     Gateway sends its server certificate (identifying it uniquely) to you. This certificate is verified against pre-installed
     certificates of your browser. Again, this is done automatically without prompting you for any input. Now, a secure
     connection is established and all data transferred between your application and the Web Service API is TLS-
     encrypted. Please note, that only TLS secured communication over standard HTTPS TCP port 443 is accepted.
9. 	Next, you will be prompted to supply your credentials for authorisation. As user name you have to provide your
     store ID and user ID encoded in the format WSstoreID._.userID (unless you manage multiple Stores through
     your integration). For instance, assuming your store ID is 101, your user ID 007, and your password myPW, you
     have to supply WS101._.007 in the user name field and myPW in the password field. Note that your credentials
     are encrypted before being passed to the server due to the TLS connection established in the steps above.
     Then, click OK.
10. The Web Service API WSDL file is displayed.
Web Service API Integration Guide                                                                       3. How the API works 10
In short, the WSDL file defines the operations offered by the Web Service, their input and return parameters,
and how these operations can be invoked. In case of the Authipay Gateway Web Service API, it defines only one
operation (IPGApiOrder) callable by sending a SOAP HTTP request to the following URL:
https://test.ipg-online.com/ipgapi/services
This operation takes an XML-encoded transaction as input and returns an XML-encoded response. Note that it
is not necessary to understand how the WSDL file is composed for using the Authipay Gateway. The following
chapters will guide you in setting up your store for building and performing custom credit card transactions.
However, in case you are using third-party tools supporting you in setting up your store for accessing the Web
Service API, you might have to supply the URL where the WSDL file can be found. In a similar way as described
above, you have to tell your Web Service tool, that the communication is TLS-enabled, requiring you to provide
your client certificate and accept the server certificate as a trusted one. Furthermore, you have to supply your
credentials. How all is done heavily depends on your Web Service tool. Hence, check the tool’s documentation
for details.




Web Service API Integration Guide                                                               3. How the API works 11
4. Sending transactions to the Gateway
The purpose of this chapter is to give you a basic understanding of the steps to be taken when committing
transactions to the Authipay Gateway. It describes what happens if a customer pays with her credit card in an
online store using the Web Service API for committing transactions.
• The customer clicks on the Pay button in the online store.
• The online store displays a form asking the customer to provide her credit card number and the expiry month
  and year.
• The customer types in these three fields and submits the data to the online store (i. e. purchases the goods).
• The online store receives the data and builds an XML document encoding a Sale transaction which includes the
  data provided by the customer and the total amount to be paid by the customer.
• After building the XML Sale transaction, the online store wraps it in a SOAP message which describes the Web
  Service operation to be called with the transaction XML being passed as a parameter.
• Having built the SOAP message, the online store prepares it for being transferred over the Internet by packing
  its content into an HTTPS POST request. Furthermore, the store sets the HTTP headers, especially its credentials
  (note that the credentials are the same as the ones you have to provide for viewing the WSDL file).
• Now, the store establishes an TLS connection by providing the client and server certificate. Please note, that
  only TLS secured communication over standard HTTPS TCP port 443 is accepted.
• Then, the online store sends the HTTPS request to the Web Service API and waits for an HTTP response.
• The Web Service API receives the HTTPS request and parses out the authorization information provided by the
  store in the HTTP headers.
• Having authorized the store to use the Authipay Gateway, the SOAP message contained in the HTTP request
  body is parsed out. This triggers the Web Service operation handling the transaction processing to run.
• The Gateway then performs the transaction processing, builds an XML response document, wraps it in a SOAP
  message, and sends this SOAP message back to the client in the body of an HTTPS response.
• Receiving this HTTPS response wakes up the store which reads out the SOAP message and response XML
  document being part of it.
• Depending on the data contained in the XML response document an approval page is sent back to the
  customer in case of a successful transaction, otherwise an error page is returned.
• The approval or error page is displayed.
While this example describes the case of a Sale transaction, other transactions basically follow the same process.
Summarising the scenario, your application has to perform the following steps in order to commit credit card
transactions and analyze the result:
• Build an XML document encoding your transactions
• Wrap that XML document in a SOAP request message
• Build an HTTPS POST request with the information identifying your store provided in the HTTP header and the
  SOAP request message in the body
• Establish an TLS connection between your application and the Web Service API
• Send the HTTPS POST request to the Authipay Gateway and receive the response
• Read the SOAP response message out of the HTTPS response body
• Analyse the XML response document contained in the SOAP response message
These seven steps are described in the following chapters. They guide you through the process of setting up your
application for performing custom credit card transactions.




Web Service API Integration Guide                                                 4. Sending transactions to the Gateway 12
5. B
    uilding Transactions in XML
This chapter describes how the different transaction types can be built in XML. As the above example scenario has
outlined, a transaction is first encoded in an XML document which is then wrapped as payload in a SOAP message.
That means the XML-encoded transaction represents the parameter passed to the Web Service API operation.
Note that there exists a variety of Web Service tools supporting you in the generation of client stubs which might
free you of the necessity to deal with raw XML. However, a basic understanding of the XML format is crucial in
order to build correct transactions regardless of the available tool support. Hence, it is recommended to become
familiar with the XML format used by the Web Service API for encoding transactions.


5.1.1 Credit/Debit Card transactions
Regardless of the transaction type, the basic XML document structure of a credit/debit card transaction is as
follows:
<ipgapi:IPGApiOrderRequest
			 xmlns:v1=”http://ipg-online.com/ipgapi/schemas/v1”
			 xmlns:ipgapi=”http://ipg-online.com/ipgapi/schemas/ipgapi”>
		<v1:Transaction>
			 <v1:CreditCardTxType>...</v1:CreditCardTxType>
			 <v1:CreditCardData>...</v1:CreditCardData>
			 <v1:Payment>...</v1:Payment>
			 <v1:TransactionDetails>...</v1:TransactionDetails>
			 <v1:Billing>...</v1:Billing>
			 <v1:Shipping>...</v1:Shipping>
		</v1:Transaction>
  </ipgapi:IPGApiOrderRequest>
The element CreditCardDataTXType is mandatory for all credit card transactions. The other elements depend on
the transaction type. The transaction content is type-specific. The elements in XML structure must be kept in the
same order as shown in examples, otherwise the OrderRequest will fail.
For XML-tags related to Card Present transactions with a chip reader and PIN entry device please refer to the xsd’s
in the Appendix of this document.


5.1.2 Sale
The following XML document represents an example of a Sale transaction using the minimum set of elements:
<ipgapi:IPGApiOrderRequest
			 xmlns:v1=”http://ipg-online.com/ipgapi/schemas/v1”
			 xmlns:ipgapi=”http://ipg-online.com/ipgapi/schemas/ipgapi”>
		<v1:Transaction>
			 <v1:CreditCardTxType>
				          <v1:Type>sale</v1:Type>
			 </v1:CreditCardTxType>
			 <v1:CreditCardData>
				          <v1:CardNumber>4111********1111</v1:CardNumber>
				          <v1:ExpMonth>12</v1:ExpMonth>
				          <v1:ExpYear>07</v1:ExpYear>
			 </v1:CreditCardData>
			 <v1:Payment>
				          <v1:ChargeTotal>19.95</v1:ChargeTotal>
				          <v1:Currency>978</v1:Currency>
			 </v1:Payment>
  </v1:Transaction>
</ipgapi:IPGApiOrderRequest>


Web Service API Integration Guide                                                       5. Building Transactions in XML 13
See chapter XML-Tag overview for a detailed description of all elements used in the above example as well as
further optional elements.
The following XML document represents an example of a Sale transaction for API users handling multiple Store IDs:
<ipgapi:IPGApiOrderRequest
		xmlns:ipgapi=’http://ipg-online.com/ipgapi/schemas/ipgapi’
		xmlns:v1=’http://ipg-online.com/ipgapi/schemas/v1’>
  <v1:Transaction>
		<v1:CreditCardTxType>
			 <v1:StoreId>1234567890</v1:StoreId>
			 <v1:Type>sale</v1:Type>
		</v1:CreditCardTxType>
		<v1:CreditCardData>
			 <v1:CardNumber>4111******1111</v1:CardNumber>
			 <v1:ExpMonth>12</v1:ExpMonth>
			 <v1:ExpYear>20</v1:ExpYear>
			 <v1:CardCodeValue>XXX</v1:CardCodeValue>
		</v1:CreditCardData>
		<v1:Payment>
			 <v1:ChargeTotal>15.00</v1:ChargeTotal>
			 <v1:Currency>978</v1:Currency>
		</v1:Payment>
			 <v1:TransactionDetails>
			 <v1:OrderId>12-34-56</v1:OrderId>
			 <v1:MerchantTransactionId>AB500500</v1:MerchantTransactionId>
			 <v1:TransactionOrigin>ECI</v1:TransactionOrigin>
			 <v1:DynamicMerchantName>MyWebsite</v1:DynamicMerchantName>
		</v1:TransactionDetails>
		<v1:Billing>
			 <v1:Zip>0001</v1:Zip>
		</v1:Billing>
		</v1:Transaction>
</ipgapi:IPGApiOrderRequest>


5.1.3 Pre-Authorisation
The following XML document represents an example of a PreAuth transaction using the minimum set of elements:
<ipgapi:IPGApiOrderRequest
			 xmlns:v1=”http://ipg-online.com/ipgapi/schemas/v1”
			 xmlns:ipgapi=”http://ipg-online.com/ipgapi/schemas/ipgapi”>
		<v1:Transaction>
			 <v1:CreditCardTxType>
				          <v1:Type>preAuth</v1:Type>
			 </v1:CreditCardTxType>
				          <v1:CreditCardData>
				          <v1:CardNumber>4111********1111</v1:CardNumber>
				          <v1:ExpMonth>12</v1:ExpMonth>
				          <v1:ExpYear>07</v1:ExpYear>
			 </v1:CreditCardData>
			 <v1:Payment>
				          <v1:ChargeTotal>100.00</v1:ChargeTotal>
				          <v1:Currency>978</v1:Currency>
			 </v1:Payment>
  </v1:Transaction>
</ipgapi:IPGApiOrderRequest>
See chapter XML-Tag overview for a detailed description of all elements used in the above example as well as
further optional elements.

Web Service API Integration Guide                                                    5. Building Transactions in XML 14
5.1.4 Post-Authorisation
The following XML document represents an example of a PostAuth transaction using the minimum set of elements:
<ipgapi:IPGApiOrderRequest
			 xmlns:v1=”http://ipg-online.com/ipgapi/schemas/v1”
			 xmlns:ipgapi=”http://ipg-online.com/ipgapi/schemas/ipgapi”>
		<v1:Transaction>
			 <v1:CreditCardTxType>
				          <v1:Type>postAuth</v1:Type>
			 </v1:CreditCardTxType>
			 <v1:Payment>
				          <v1:ChargeTotal>59.00</v1:ChargeTotal>
				          <v1:Currency>978</v1:Currency>
			 </v1:Payment>
			 <v1:TransactionDetails>
				          <v1:OrderId>
					 703d2723-99b6-4559-8c6d-797488e8977
				          </v1:OrderId>
			 </v1:TransactionDetails>
		</v1:Transaction>
</ipgapi:IPGApiOrderRequest>
In case your system is not aware of the payment method that has been used for the original Pre-Authorisation
transaction, the Post-Authorisation can be performed using any TxType which supports Post-Authorisations. The
gateway will then select the correct payment method based on the referenced Order ID.
See chapter XML-Tag overview for a detailed description of all elements used in the above example as well as
further optional elements.


5.1.5 ForceTicket
The following XML document represents an example of a ForceTicket transaction using the minimum set of
elements:
<ipgapi:IPGApiOrderRequest
			 xmlns:v1=”http://ipg-online.com/ipgapi/schemas/v1”
			 xmlns:ipgapi=”http://ipg-online.com/ipgapi/schemas/ipgapi”>
		<v1:Transaction>
			 <v1:CreditCardTxType>
			 <v1:Type>forceTicket</v1:Type>
		</v1:CreditCardTxType>
		<v1:CreditCardData>
			 <v1:CardNumber>4111********1111</v1:CardNumber>
			 <v1:ExpMonth>12</v1:ExpMonth>
			 <v1:ExpYear>07</v1:ExpYear>
		</v1:CreditCardData>
  <v1:Payment>
			 <v1:ChargeTotal>59.00</v1:ChargeTotal>
			 <v1:Currency>978</v1:Currency>
		</v1:Payment>
		<v1:TransactionDetails>
			 <v1:ReferenceNumber>123456</v1:ReferenceNumber>
		</v1:TransactionDetails>
  </v1:Transaction>
</ipgapi:IPGApiOrderRequest>
See chapter XML-Tag overview for a detailed description of all elements used in the above example as well as
further optional elements.



Web Service API Integration Guide                                                    5. Building Transactions in XML 15
5.1.6 Return
The following XML document represents an example of a Return transaction using the minimum set of elements:
<ipgapi:IPGApiOrderRequest
			 xmlns:v1=”http://ipg-online.com/ipgapi/schemas/v1”
			 xmlns:ipgapi=”http://ipg-online.com/ipgapi/schemas/ipgapi”>
		<v1:Transaction>
			 <v1:CreditCardTxType>
				          <v1:Type>return</v1:Type>
			 </v1:CreditCardTxType>
			 <v1:Payment>
				          <v1:ChargeTotal>19.00</v1:ChargeTotal>
				          <v1:Currency>978</v1:Currency>
			 </v1:Payment>
			 <v1:TransactionDetails>
				          <v1:OrderId>
					 62e3b5df-2911-4e89-8356-1e49302b1807
				          </v1:OrderId>
			 </v1:TransactionDetails>
		</v1:Transaction>
</ipgapi:IPGApiOrderRequest>
In case your system is not aware of the payment method that has been used for the original transaction, the Return
can be performed using any TxType which supports Returns. The gateway will then select the correct payment
method based on the referenced Order ID.
See chapter XML-Tag overview for a detailed description of all elements used in the above example as well as
further optional elements.


5.1.7 Credit
Please note that Credit is a transaction type that requires special user permissions.
The following XML document represents an example of a Credit transaction using the minimum set of elements:
<ipgapi:IPGApiOrderRequest
			 xmlns:v1=”http://ipg-online.com/ipgapi/schemas/v1”
			 xmlns:ipgapi=”http://ipg-online.com/ipgapi/schemas/ipgapi”>
		<v1:Transaction>
			 <v1:CreditCardTxType>
				          <v1:Type>credit</v1:Type>
			 </v1:CreditCardTxType>
			 <v1:CreditCardData>
				          <v1:CardNumber>4111********1111</v1:CardNumber>
				          <v1:ExpMonth>12</v1:ExpMonth>
				          <v1:ExpYear>07</v1:ExpYear>
			 </v1:CreditCardData>
			 <v1:Payment>
				          <v1:ChargeTotal>50.00</v1:ChargeTotal>
				          <v1:Currency>978</v1:Currency>
			 </v1:Payment>
		</v1:Transaction>
</ipgapi:IPGApiOrderRequest>
See chapter XML-Tag overview for a detailed description of all elements used in the above example as well as
further optional elements.




Web Service API Integration Guide                                                     5. Building Transactions in XML 16
5.1.8 Void
The following XML document represents an example of a Void transaction using the minimum set of elements:
<ipgapi:IPGApiOrderRequest
			 xmlns:v1=”http://ipg-online.com/ipgapi/schemas/v1”
			 xmlns:ipgapi=”http://ipg-online.com/ipgapi/schemas/ipgapi”>
		<v1:Transaction>
			 <v1:CreditCardTxType>
				          <v1:Type>void</v1:Type>
			 </v1:CreditCardTxType>
			 <v1:TransactionDetails>
				          <v1:IpgTransactionId>1234567890</v1:IpgTransactionId>
			 </v1:TransactionDetails>
		</v1:Transaction>
</ipgapi:IPGApiOrderRequest>
For referencing to the transaction that shall be voided, this example uses the parameter IpgTransactionId. If you
have assigned a transaction ID (MerchantTransactionId) in the original transaction, you can alternatively submit this
ID as ReferencedMerchantTransactionId instead.
In case your system is not aware of the payment method that has been used for the original transaction, the
Void can be performed using any TxType which supports Voids. The gateway will then select the correct payment
method based on the referenced Order ID and TDate.
See chapter XML-Tag overview for a detailed description of all elements used in the above example as well as
further optional elements.


5.1.9 Recurring Sale (Merchant-triggered)
The following XML document represents an example of a first Sale transaction of a series of recurring payments:
<?xml version=”1.0” encoding=”UTF-8”?>
<SOAP-ENV:Envelope
  xmlns:SOAP-ENV=”http://schemas.xmlsoap.org/soap/envelope/”>
  <SOAP-ENV:Header/>
  <SOAP-ENV:Body>
		<ns4:IPGApiOrderRequest
			 xmlns:ns4=”http://ipg-online.com/ipgapi/schemas/ipgapi”
			 xmlns:ns2=”http://ipg-online.com/ipgapi/schemas/v1”
			 xmlns:ns3=”http://ipg-online.com/ipgapi/schemas/a1”>
			 <ns2:Transaction>
				          <ns2:CreditCardTxType>
					 <ns2:StoreId>1109950006</ns2:StoreId>
					 <ns2:Type>sale</ns2:Type>
				          </ns2:CreditCardTxType>
				          <ns2:CreditCardData>
					 <ns2:CardNumber>52392*****0002</ns2:CardNumber>
					 <ns2:ExpMonth>12</ns2:ExpMonth>
					 <ns2:ExpYear>22</ns2:ExpYear>
					 <ns2:CardCodeValue>XXX</ns2:CardCodeValue>
				          </ns2:CreditCardData>
					 <ns2:recurringType>FIRST</ns2:recurringType>
				          <ns2:Payment>
					 <ns2:ChargeTotal>13.99</ns2:ChargeTotal>
					 <ns2:Currency>978</ns2:Currency>
				          </ns2:Payment>
			 </ns2:Transaction>
		</ns4:IPGApiOrderRequest>
  </SOAP-ENV:Body>
</SOAP-ENV:Envelope>
Web Service API Integration Guide                                                       5. Building Transactions in XML 17
In case you have received SchemeTransactionId in the response from the Gateway you should use its value in the
subsequent RecurringType=REPEAT request:
<SOAP-ENV:Envelope xmlns:SOAPENV=”
http://schemas.xmlsoap.org/soap/envelope/”>
  <SOAP-ENV:Header/>
  <SOAP-ENV:Body>
		<ipgapi:IPGApiOrderResponse xmlns:a1=”http://ipg-
  online.com/ipgapi/schemas/a1” xmlns:ipgapi=”http://ipg-
  online.com/ipgapi/schemas/ipgapi” xmlns:v1=”http://ipg-
  online.com/ipgapi/schemas/v1”>
<ipgapi:ApprovalCode>Y:403939:4566959508:YYYM:441809</ipgapi:ApprovalCode>
		<ipgapi:AVSResponse>YYY</ipgapi:AVSResponse>
		<ipgapi:Brand>MASTERCARD</ipgapi:Brand>
<ipgapi:CommercialServiceProvider>BOSMS</ipgapi:CommercialServiceProvider>
		<ipgapi:OrderId>A-b64adf8c-8fe8-44cb-97defd721033da2e</ipgapi:OrderId>
		<ipgapi:IpgTransactionId>84566959508</ipgapi:IpgTransactionId>
		<ipgapi:PaymentType>CREDITCARD</ipgapi:PaymentType>
<ipgapi:ProcessorApprovalCode>403939</ipgapi:ProcessorApprovalCode>
		<ipgapi:ProcessorCCVResponse>M</ipgapi:ProcessorCCVResponse>
<ipgapi:ProcessorReferenceNumber>119509441809</ipgapi:ProcessorReferenceNumber>
		<ipgapi:ProcessorResponseCode>00</ipgapi:ProcessorResponseCode>
  <ipgapi:ProcessorResponseMessage>Function performed errorfree</
  ipgapi:ProcessorResponseMessage>
<ipgapi:SchemeTransactionId>0714MCC417474</ipgapi:SchemeTransactionId>
		<ipgapi:TDate>1626255235</ipgapi:TDate>
		<ipgapi:TDateFormatted>2021.07.14 11:33:55
(CEST)</ipgapi:TDateFormatted>
		<ipgapi:TerminalID>80000860</ipgapi:TerminalID>
		<ipgapi:TransactionResult>APPROVED</ipgapi:TransactionResult>
		<ipgapi:TransactionTime>1626255235</ipgapi:TransactionTime>
		</ipgapi:IPGApiOrderResponse>
  </SOAP-ENV:Body>
</SOAP-ENV:Envelope>
Subsequent transactions in a series need to be flagged like this and submitted with the SchemeTransactionId you
have received in the previous step :
<?xml version=”1.0” encoding=”UTF-8”?>
  <SOAP-ENV:Envelope
		xmlns:SOAP-ENV=”http://schemas.xmlsoap.org/soap/envelope/”>
  <SOAP-ENV:Header/>
  <SOAP-ENV:Body>
		<ns4:IPGApiOrderRequest
			 xmlns:ns4=”http://ipg-online.com/ipgapi/schemas/ipgapi”
			 xmlns:ns2=”http://ipg-online.com/ipgapi/schemas/v1”
			 xmlns:ns3=”http://ipg-online.com/ipgapi/schemas/a1”>
			 <ns2:Transaction>
				          <ns2:CreditCardTxType>
					 <ns2:StoreId>1109950006</ns2:StoreId>
					 <ns2:Type>sale</ns2:Type>
				          </ns2:CreditCardTxType>
				          <ns2:CreditCardData>
					 <ns2:CardNumber>5239*******002</ns2:CardNumber>
					 <ns2:ExpMonth>12</ns2:ExpMonth>
					 <ns2:ExpYear>22</ns2:ExpYear>
					 <ns2:CardCodeValue>XXX</ns2:CardCodeValue>
				          </ns2:CreditCardData>
			 <ns2:recurringType>REPEAT</ns2:recurringType>
			 <ns2:Payment>

Web Service API Integration Guide                                                    5. Building Transactions in XML 18
			 <ns2:ChargeTotal>13.99</ns2:ChargeTotal>
			 <ns2:Currency>978</ns2:Currency>
		</ns2:Payment>
		<ns2:TransactionDetails>
<ns2:ReferencedSchemeTransactionId>0714MCC417474</ns2:ReferencedSchemeTransactionId>
			 </ns2:TransactionDetails>
		</ns2:Transaction>
		</ns4:IPGApiOrderRequest>
  </SOAP-ENV:Body>
</SOAP-ENV:Envelope>
Please see chapter Recurring Payments (Scheduler) for the alternative option to let the Gateway automaticall
trigger recurring transactions.


5.1.10 Standing Instructions
Standing Instructions are instructions a consumer (payer) gives to a bank to pay a set amount at regular intervals
to another’s (payee) account. They are typically used to pay rent, mortgage or any other fixed regular payments.
 If your Store is enabled to process this type of transactions, Standing Instuctions can be submitted in the
following format:
<ipgapi:IPGApiOrderRequest
			 xmlns:v1=”http://ipg-online.com/ipgapi/schemas/v1”
			 xmlns:ipgapi=”http://ipg-online.com/ipgapi/schemas/ipgapi”>
		<v1:Transaction>
			 <v1:CreditCardTxType>
				          <v1:Type>sale</v1:Type>
			 </v1:CreditCardTxType>
			 <v1:CreditCardData>
				          <v1:CardNumber>4111********1111</v1:CardNumber>
				          <v1:ExpMonth>12</v1:ExpMonth>
				          <v1:ExpYear>07</v1:ExpYear>
			 </v1:CreditCardData>
		<ns2:recurringType>STANDIN</ns2:recurringType>
		<v1:Payment>
			 <v1:ChargeTotal>19.95</v1:ChargeTotal>
			 <v1:Currency>978</v1:Currency>
		</v1:Payment>
  </v1:Transaction>
</ipgapi:IPGApiOrderRequest>


5.1.11 MasterPass™
MasterPass is MasterCard’s digital wallet solution that allows consumers to store payment, billing and shipping
details for a fast, convenient, and secure checkout experience at a merchant’s website.
The easiest way to make use of MasterPass is to use our Connect solution. However in case you prefer to integrate
directly to MasterPass and manage the authentication and wallet process yourself, you can submit the Wallet ID
and Wallet Type within your request for a credit card transaction.
Please see details for a direct integration with MasterPass here:
https://developer.mastercard.com/portal/display/api/MasterPass+-+Merchant+Checkout+Services+-
+Documentation
The following XML document represents an example of a Sale transaction with MasterPass using the minimum set
of elements:
<ipgapi:IPGApiOrderRequest
			 xmlns:v1=”http://ipg-online.com/ipgapi/schemas/v1”
			 xmlns:ipgapi=”http://ipg-online.com/ipgapi/schemas/ipgapi”>
		<v1:Transaction>

Web Service API Integration Guide                                                       5. Building Transactions in XML 19
			 <v1:CreditCardTxType>
				          <v1:Type>sale</v1:Type>
			 </v1:CreditCardTxType>
			 <v1:CreditCardData>
				          <v1:CardNumber>4111********1111</v1:CardNumber>
				          <v1:ExpMonth>12</v1:ExpMonth>
				          <v1:ExpYear>07</v1:ExpYear>
			 </v1:CreditCardData>
		<v1:Wallet>
			 <v1:WalletType>MASTERPASS</ns2:WalletType>
			 <v1:WalletID>101</ns2:WalletID>
		</v1:Wallet>
		<v1:Payment>
			 <v1:ChargeTotal>19.95</v1:ChargeTotal>
			 <v1:Currency>978</v1:Currency>
		</v1:Payment>
  </v1:Transaction>
</ipgapi:IPGApiOrderRequest>


5.1.12 SEPA Direct Debit – Germany
Regardless of the transaction type, the basic XML document structure of a German Direct Debit transaction is as
follows:
<ipgapi:IPGApiOrderRequest
			 xmlns:v1=”http://ipg-online.com/ipgapi/schemas/v1”
			 xmlns:ipgapi=”http://ipg-online.com/ipgapi/schemas/ipgapi”>
		<v1:Transaction>
			 <v1:DE_DirectDebitTxType>...</v1:DE_DirectDebitTxType>
			 <v1:DE_DirectDebitData>...</v1:DE_DirectDebitData>
			 <v1:Payment>...</v1:Payment>
			 <v1:TransactionDetails>...</v1:TransactionDetails>
			 <v1:Billing>...</v1:Billing>
			 <v1:Shipping>...</v1:Shipping>
  </v1:Transaction>
</ipgapi:IPGApiOrderRequest>
The element DE_DirectDebitTXType is mandatory for all debit transactions. The other elements depend on the
transaction type. The transaction content is type-specific.


5.1.13 Sale
The following XML document represents an example of a Sale transaction using the minimum set of elements:
<ipgapi:IPGApiOrderRequest
			 xmlns:v1=”http://ipg-online.com/ipgapi/schemas/v1”
			 xmlns:ipgapi=”http://ipg-online.com/ipgapi/schemas/ipgapi”>
		<v1:Transaction>
			 <v1:DE_DirectDebitTxType>
				          <v1:Type>sale</v1:Type>
			 </v1:DE_DirectDebitTxType>
			 <v1:DE_DirectDebitData>
				          <v1:IBAN>DE345001XXXX32121604</v1:IBAN>
				          <v1:MandateReference>0/8/15</v1:MandateReference>
			 </v1:DE_DirectDebitData>
			 <v1:Billing>
				          <v1:Name>Markus Mustermann</v1:Name>
			 </v1:Billing>
			 <v1:Payment>
			 <v1:ChargeTotal>19.00</v1:ChargeTotal>

Web Service API Integration Guide                                                     5. Building Transactions in XML 20
			 <v1:Currency>978</v1:Currency>
		</v1:Payment>
  </v1:Transaction>
</ipgapi:IPGApiOrderRequest>
See chapter XML-Tag overview for a detailed description of all elements used in the above example as well as
further optional elements.


5.1.14 Void
The following XML document represents an example of a Void transaction using the minimum set of elements:
<ipgapi:IPGApiOrderRequest
			 xmlns:v1=”http://ipg-online.com/ipgapi/schemas/v1”
			 xmlns:ipgapi=”http://ipg-online.com/ipgapi/schemas/ipgapi”>
		<v1:Transaction>
			 <v1:DE_DirectDebitTxType>
				          <v1:Type>void</v1:Type>
			 </v1:DE_DirectDebitTxType>
			 <v1:TransactionDetails>
				          <v1:IpgTransactionId>1234567890</v1:IpgTransactionId>
			 </v1:TransactionDetails>
  </v1:Transaction>
</ipgapi:IPGApiOrderRequest>
For referencing to the transaction that shall be voided, this example uses the parameter IpgTransactionId. If you
have assigned a transaction ID (MerchantTransactionId) in the original transaction, you can alternatively submit this
ID as ReferencedMerchantTransactionId instead.
In case your system is not aware of the payment method that has been used for the original transaction, the
Void can be performed using any TxType which supports Voids. The gateway will then select the correct payment
method based on the referenced Transaction ID.
See chapter XML-Tag overview for a detailed description of all elements used in the above example as well as
further optional elements.


5.1.15 Credit
Please note that Credit is a transaction type that requires special user permissions.
The following XML document represents an example of a Credit transaction using the minimum set of elements:
<ipgapi:IPGApiOrderRequest
			 xmlns:v1=”http://ipg-online.com/ipgapi/schemas/v1”
			 xmlns:ipgapi=”http://ipg-online.com/ipgapi/schemas/ipgapi”>
		<v1:Transaction>
			 <v1:DE_DirectDebitTxType>
				          <v1:Type>credit</v1:Type>
			 </v1:DE_DirectDebitTxType>
			 <v1:DE_DirectDebitData>
<v1:IBAN>DE34500****0032121604</v1:IBAN>
			 </v1:DE_DirectDebitData>
			 <v1:Billing>
				          <v1:Name>Markus Mustermann</v1:Name>
			 </v1:Billing>
			 <v1:Payment>
				          <v1:ChargeTotal>19.00</v1:ChargeTotal>
				          <v1:Currency>978</v1:Currency>
			 </v1:Payment>
  </v1:Transaction>
</ipgapi:IPGApiOrderRequest>
See chapter XML-Tag overview for a detailed description of all elements used in the above example as well as
further optional elements.
Web Service API Integration Guide                                                       5. Building Transactions in XML 21
5.1.16 Return
Please note that Return is a transaction type that requires special user permissions.
The following XML document represents an example of a Return transaction using the minimum set of elements:
<ipgapi:IPGApiOrderRequest
			 xmlns:v1=”http://ipg-online.com/ipgapi/schemas/v1”
			 xmlns:ipgapi=”http://ipg-online.com/ipgapi/schemas/ipgapi”>
		<v1:Transaction>
			 <v1:DE_DirectDebitTxType>
				               <v1:Type>return</v1:Type>
			 </v1:DE_DirectDebitTxType>
			 <v1:Payment>
				               <v1:ChargeTotal>1.00</v1:ChargeTotal>
				               <v1:Currency>978</v1:Currency>
			 </v1:Payment>
			 <v1:TransactionDetails>
				               <v1:OrderId>
					 62e3b5df-2911-4e89-8356-1e49302b1807
				               </v1:OrderId>
			 </v1:TransactionDetails>
		</v1:Transaction>
</ipgapi:IPGApiOrderRequest>
In case your system is not aware of the payment method that has been used for the original transaction, the Return
can be performed using any TxType which supports Returns. The gateway will then select the correct payment
method based on the referenced Order ID.
See chapter XML-Tag overview for a detailed description of all elements used in the above example as well as
further optional elements.


5.1.17 SEPA Direct Debit with Authipay Local Payments
The Authipay® Local Payments solution offers a unique combination of global coverage, a single contracting and
integration experience, and a broad and expanding portfolio of local payment methods.
If your Store has been activated for this product option, you can use this Web Service API to initiate SEPA Direct
Debit payments where you manage the mandates on your side.
This is especially useful in cases where you have a large number of mandates on file from previously used solutions
and want to continue to use these mandates when migrating to Authipay.

Recurring payment
 Billing/Email                 O    Consumer’s email address
 SepaData/IBAN                 M    Consumer’s IBAN – International Bank Account Number (22 digits)
 Mandate/Type                  M    Sequence type of Direct Debit, defaults to ‘single’ Values:
                                    SINGLE – Direct Debit is executed once
                                    FIRST_COLLECTION – First Direct Debit in a series of recurring
                                    RECURRING_COLLECTION – Follow-up Direct Debit in a series of recurring
                                    FINAL_COLLECTION – Last Direct Debit in a series of recurring
 Mandate/Reference             M    To be populated with the mandate reference
 Mandate/Date                  M    To be populated with the initial mandate signature date
 Mandate/Url                   M    To be populated with the valid URL of the SEPA mandate
The following represents an example of a transaction request, which includes reference to the mandate and the
URL, where the mandate could be verified:
<ipgapi:IPGApiOrderRequest
		xmlns:v1=”http://ipg-online.com/ipgapi/schemas/v1”

Web Service API Integration Guide                                                             5. Building Transactions in XML 22
		xmlns:ipgapi=”http://ipg-online.com/ipgapi/schemas/ipgapi”>
			 <v1:Transaction>
				        <v1:SepaTxType>
					 <v1:StoreId>990004</v1:StoreId>
					 <v1:Type>sale</v1:Type>
				        </v1:SepaTxType>
				        <v1:SepaData>
					 <v1:IBAN>DE345001006***32121604</v1:IBAN>
					 <v1:Mandate>
						                <v1:Reference>4HJUZ67T</v1:Reference>
						                <v1:Type>FIRST_COLLECTION</v1:Type>
						                <v1:Date>20150715</v1:Date>
						                <v1:Url>https://www.firstdata.com</v1:Url>
					 <v1:Mandate>
				        </v1:SepaTxType>
				        <v1:Payment>
					 <v1:ChargeTotal>1</v1:ChargeTotal>
					 <v1:Currency>EUR</v1:Currency>
				        </v1:Payment>
				          <v1:Billing>
					 <v1:Name>Testname</v1:Firstname>
					 <v1:Country>DE</v1:Country>
				          <v1:Email>youremail@email.com</v1:Email>
				          </v1:Billing>
			 </v1:Transaction>
</ipgapi:IPGApiOrderRequest>
When you do not want to manage the SEPA Direct Debit mandates on your side, you can instead use the outof-
box solution offered by Authipay. Upon receiving the mandate reference and the mandate date as part of the
Connect response, you can process the subsequent payments under this mandate via this Web Service API.

Follow-up payment in recurring series:
 Field Name                         M/O   Description
 Billing/Email                      M     Consumer’s email address
 SepaData/IBAN                      M     Consumer’s IBAN – International Bank Account Number (22 digits)
 Mandate/Type                       M     Sequence type of Direct Debit Values:
                                          RECURRING_COLLECTION – Follow-up Direct Debit in a series of
                                          recurring
                                          FINAL_COLLECTION – Last Direct Debit in a series of recurring
 Mandate/Reference                  M     To be populated with the mandate reference from the response
 Mandate/Date                       M     To be populated with the initial mandate signature date from the response
<ipgapi:IPGApiOrderRequest
			 xmlns:v1=”http://ipg-online.com/ipgapi/schemas/v1”
			 xmlns:ipgapi=”http://ipg-online.com/ipgapi/schemas/ipgapi”>
				          <v1:Transaction>
					 <v1:SepaTxType>
					 <v1:StoreId>990004</v1:StoreId>
					 <v1:Type>sale</v1:Type>
				          </v1:SepaTxType>
				          <v1:SepaData>
					 <v1:IBAN>DE345001006***32121604</v1:IBAN>
					 <v1:Mandate>
						                  <v1:Reference>25IGX0N</v1:Reference>
						                  <v1:Type>RECURRING_COLLECTION</v1:Type>
						                  <v1:Date>20180124</v1:Date>
					 <v1:Mandate>

Web Service API Integration Guide                                                        5. Building Transactions in XML 23
				          </v1:SepaTxType>
				          <v1:Payment>
					 <v1:ChargeTotal>1</v1:ChargeTotal>
					 <v1:Currency>EUR</v1:Currency>
				          </v1:Payment>
				          <v1:Billing>
					 <v1:Name>Testname</v1:Firstname>
					 <v1:Country>DE</v1:Country>
					 <v1:Email>youremail@email.com</v1:Email>
				          </v1:Billing>
			 </v1:Transaction>
</ipgapi:IPGApiOrderRequest>


5.1.18 PayPal

5.1.19 Post-Authorisation Payment Transaction
After a payment authorisation for PayPal has been submitted via the Gateway’s Connect interface, the Web Service
API can be used to perform post-authorisation payments.
The following XML document represents an example of a PostAuth transaction using the minimum set of elements:
<ns5:IPGApiOrderRequest
			 xmlns:ns5=”http://ipg-online.com/ipgapi/schemas/ipgapi”
			 xmlns:ns3=”http://ipg-online.com/ipgapi/schemas/a1”
			 xmlns:ns4=”http://ipg-online.com/ipgapi/schemas/v1”>
		<ns4:Transaction>
			 <ns4:PayPalTxType>
				          <ns4:Type>postAuth</ns4:Type>
			 </ns4:PayPalTxType>
			 <ns4:Payment>
				          <ns4:ChargeTotal>1</ns4:ChargeTotal>
				          <ns4:Currency>EUR</ns4:Currency>
			 </ns4:Payment>
			 <ns4:TransactionDetails>
				          <ns4:OrderId>
					 C-32121f4d-852f-4f48-8095-8585b917c079
				          </ns4:OrderId>
			 </ns4:TransactionDetails>
		</ns4:Transaction>
</ns5:IPGApiOrderRequest>
See chapter XML-Tag overview for a detailed description of all elements used in the above example as well as
further optional elements.


5.1.20 Recurring Payment Transaction
The recurring payments for PayPal can be executed via the Connect solution. You have to submit a SALE
transaction request with the corresponding parameters to install the recurring payments. The first transaction is
always conducted immediately along with the request.
The subsequent transactions are executed by the Gateway’s scheduler, via the API Web Service, as defined during
the initial SALE transaction with the instalation.




Web Service API Integration Guide                                                       5. Building Transactions in XML 24
5.1.21 Return
The following XML document represents an example of a Return transaction using the minimum set of elements:
<ns5:IPGApiOrderRequest
			 xmlns:ns5=”http://ipg-online.com/ipgapi/schemas/ipgapi”
			 xmlns:ns3=”http://ipg-online.com/ipgapi/schemas/a1”
			 xmlns:ns4=”http://ipg-online.com/ipgapi/schemas/v1”>
		<ns4:Transaction>
			 <ns4:PayPalTxType>
				          <ns4:Type>return</ns4:Type>
			 </ns4:PayPalTxType>
			 <ns4:Payment>
				          <ns4:ChargeTotal>0.4</ns4:ChargeTotal>
				          <ns4:Currency>EUR</ns4:Currency>
			 </ns4:Payment>
			 <ns4:TransactionDetails>
				          <ns4:OrderId>
					 C-32121f4d-852f-4f48-8095-8585b917c079
				          </ns4:OrderId>
			 </ns4:TransactionDetails>
		</ns4:Transaction>
</ns5:IPGApiOrderRequest>
In case your system is not aware of the payment method that has been used for the original transaction, the Return
can be performed using any TxType which supports Returns. The gateway will then select the correct payment
method based on the referenced Order ID.


5.1.22 Void
The following XML document represents an example of a Void transaction using the minimum set of elements:
<ns5:IPGApiOrderRequest
			 xmlns:ns5=”http://ipg-online.com/ipgapi/schemas/ipgapi”
			 xmlns:ns3=”http://ipg-online.com/ipgapi/schemas/a1”
			 xmlns:ns4=”http://ipg-online.com/ipgapi/schemas/v1”>
		<ns4:Transaction>
			 <ns4:PayPalTxType>
				          <ns4:Type>void</ns4:Type>
			 </ns4:PayPalTxType>
			 <ns4:TransactionDetails>
				          <v1:IpgTransactionId>1234567890</v1:IpgTransactionId>
			 </ns4:TransactionDetails>
		</ns4:Transaction>
</ns5:IPGApiOrderRequest>
For referencing to the transaction that shall be voided, this example uses the parameter IpgTransactionId. If you
have assigned a transaction ID (MerchantTransactionId) in the original transaction, you can alternatively submit this
ID as ReferencedMerchantTransactionId instead of sending a TDate.
In case your system is not aware of the payment method that has been used for the original transaction, the
Void can be performed using any TxType which supports Voids. The gateway will then select the correct payment
method based on the referenced Transaction ID.




Web Service API Integration Guide                                                       5. Building Transactions in XML 25
5.1.23 Credit
Please note that Credit is a transaction type that requires special user permissions.
The following XML document represents an example of a Credit transaction using the minimum set of elements:
<ns5:IPGApiOrderRequest
			 xmlns:ns5=”http://ipg-online.com/ipgapi/schemas/ipgapi”
			 xmlns:ns2=”http://ipg-online.com/ipgapi/schemas/a1”
			 xmlns:ns4=”http://ipg-online.com/ipgapi/schemas/v1”>
		<ns4:Transaction>
			 <ns4:PayPalTxType>
				          <ns4:Type>credit</ns4:Type>
			 </ns4:PayPalTxType>
			 <ns4:Payment>
				          <ns4:ChargeTotal>1</ns4:ChargeTotal>
				          <ns4:Currency>EUR</ns4:Currency>
			 </ns4:Payment>
			 <ns4:Billing>
				          <ns4:Email>x@y.zz</ns4:Email>
			 </ns4:Billing>
		</ns4:Transaction>
</ns5:IPGApiOrderRequest>
Unlike with other payment methods, PayPal transactions contain no payment data like a card number. Therefore
this transaction requires the resgistered email address of the recipient of the payment. This email address must be
submitted in the field ns4:Billing/ns4:Email.


5.1.24 SOFORT Überweisung
5.1.25 Return
The following XML document represents an example of a Return transaction using the minimum set of elements:
<ns5:IPGApiOrderRequest
			 xmlns:ns5=”http://ipg-online.com/ipgapi/schemas/ipgapi
			 xmlns:ns3=”http://ipg-online.com/ipgapi/schemas/a1”
			 xmlns:ns4=”http://ipg-online.com/ipgapi/schemas/v1”>
		<ns4:Transaction>
			 <ns4:SofortTxType>
				          <ns4:Type>return</ns4:Type>
			 </ns4:SofortTxType>
			 <ns4:Payment>
				          <ns4:ChargeTotal>1.00</ns4:ChargeTotal>
				          <ns4:Currency>EUR</ns4:Currency>
			 </ns4:Payment>
			 <ns4:TransactionDetails>
				          <ns4:OrderId>
					 C-32121f4d-852f-4f48-8095-8585b917c079
				          </ns4:OrderId>
			 </ns4:TransactionDetails>
		</ns4:Transaction>
</ns5:IPGApiOrderRequest>
When your are triggering a return for SOFORT Banking transition it will only be marked as being prepared
for the refund on SOFORT’s side but not yet executed. In order for your customer to receive the money,
you need to execute the return transaction via SOFORT’s merchant portal or via its API. For details please see
https://www.sofort.com/integrationCenter-eng-DE/content/view/full/3363.
In case your system is not aware of the payment method that has been used for the original transaction, the Return
can be performed using any TxType which supports Returns. The gateway will then select the correct payment
method based on the referenced Order ID.
Web Service API Integration Guide                                                       5. Building Transactions in XML 26
5.1.26 iDEAL

5.1.27 Return
Please note that this feature is not available through all distribution channels.
The following XML document represents an example of a Return transaction using the minimum set of elements:
<ns5:IPGApiOrderRequest
			 xmlns:ns5=http://ipg-online.com/ipgapi/schemas/ipgapi
			 xmlns:ns3=”http://ipg-online.com/ipgapi/schemas/a1”
			 mlns:ns4=”http://ipg-online.com/ipgapi/schemas/v1”>
		<ns4:Transaction>
			 <ns4:IdealTxType>
				          <ns4:Type>return</ns4:Type>
			 </ns4:IdealTxType>
			 <ns4:Payment>
				          <ns4:ChargeTotal>1.00</ns4:ChargeTotal>
				          <ns4:Currency>EUR</ns4:Currency>
			 </ns4:Payment>
			 <ns4:TransactionDetails>
				          <ns4:OrderId>
					 C-32121f4d-852f-4f48-8095-8585b917c079
				          </ns4:OrderId>
		</ns4:TransactionDetails>
  </ns4:Transaction>
</ns5:IPGApiOrderRequest>
In case your system is not aware of the payment method that has been used for the original transaction, the Return
can be performed using any TxType which supports Returns. The gateway will then select the correct payment
method based on the referenced Order ID.


5.1.28 Generic Transaction Type for Voids and Returns
The Tag SubsequentTransaction allows you to submit Voids and Refunds independently from which payment
method had been used for the original payment transaction.
You can initiate such transactions by referencing to a previous transaction using one of the following options:
• IPG Transaction ID
• Merchant Transaction ID
The following XML document represents an example of a Void transaction using the minimum set of elements for
IPG Transaction ID:
<ns5:IPGApiOrderRequest
xmlns:ns5=”http://ipg-online.com/ipgapi/schemas/ipgapi”
xmlns:ns2=”http://ipg-online.com/ipgapi/schemas/v1”
xmlns:ns3=”http://ipg-online.com/ipgapi/schemas/a1”>
  <ns2:SubsequentTransaction>
		<ns2:IpgTransactionId>1234567890</ns2:IpgTransactionId>
			 <ns2:Options>
				          <ns2:StoreId>120995000</ns2:StoreId>
			 </ns2:Options>
			 <ns2:TransactionType>VOID</ns2:TransactionType>
		</ns2:SubsequentTransaction>
</ns5:IPGApiOrderRequest>
The following XML document represents an example of a Void transaction using the minimum set of elements for
Merchant Transaction ID:



Web Service API Integration Guide                                                        5. Building Transactions in XML 27
<ns5:IPGApiOrderRequest
xmlns:ns5=”http://ipg-online.com/ipgapi/schemas/ipgapi”
xmlns:ns2=”http://ipg-online.com/ipgapi/schemas/v1”
xmlns:ns3=”http://ipg-online.com/ipgapi/schemas/a1”>
  <ns2:SubsequentTransaction>
		<ns2:ReferencedMerchantTransactionId>ITID-
000380</ns2:ReferencedMerchantTransactionId>
		<ns2:Options>
			 <ns2:StoreId>44036000750</ns2:StoreId>
		</ns2:Options>
		<ns2:TransactionType>VOID</ns2:TransactionType>
  </ns2:SubsequentTransaction>
</ns5:IPGApiOrderRequest>
The following XML document represents an example of a Return transaction using the minimum set of elements
for IPG Transaction ID:
<ns5:IPGApiOrderRequest
xmlns:ns5=”http://ipg-online.com/ipgapi/schemas/ipgapi”
xmlns:ns2=”http://ipg-online.com/ipgapi/schemas/v1”
xmlns:ns3=”http://ipg-online.com/ipgapi/schemas/a1”>
  <ns2:SubsequentTransaction>
		<ns2:IpgTransactionId>123456789</ns2:IpgTransactionId>
		<ns2:Options>
			 <ns2:StoreId>120995000</ns2:StoreId>
		</ns2:Options>
		<ns2:TransactionType>RETURN</ns2:TransactionType>
		<ns2:Payment>
			 <ns2:ChargeTotal>1.00</ns2:ChargeTotal>
			 <ns2:Currency>978</ns2:Currency>
		</ns2:Payment>
  </ns2:SubsequentTransaction>
</ns5:IPGApiOrderRequest>
The following XML document represents an example of a Return transaction using the minimum set of elements
for Merchant Transaction ID:
<ns5:IPGApiOrderRequest
xmlns:ns5=”http://ipg-online.com/ipgapi/schemas/ipgapi”
xmlns:ns2=”http://ipg-online.com/ipgapi/schemas/v1”
xmlns:ns3=”http://ipg-online.com/ipgapi/schemas/a1”>
  <ns2:SubsequentTransaction>
		<ns2:ReferencedMerchantTransactionId>ITID-
000380</ns2:ReferencedMerchantTransactionId>
		<ns2:Options>
			 <ns2:StoreId>44036000750</ns2:StoreId>
		</ns2:Options>
		<ns2:TransactionType>RETURN</ns2:TransactionType>
		<ns2:Payment>
			 <ns2:ChargeTotal>1.00</ns2:ChargeTotal>
			 <ns2:Currency>978</ns2:Currency>
		</ns2:Payment>
  </ns2:SubsequentTransaction>
</ns5:IPGApiOrderRequest>




Web Service API Integration Guide                                                 5. Building Transactions in XML 28
6. Additional Web Service actions
6.1.1 Initiate Clearing
Clearing for transactions can be initiated via the Web Service similar to a payment transaction:
<ipgapi:IPGApiActionRequest
			 xmlns:a1=”http://ipg-online.com/ipgapi/schemas/a1”
			 xmlns:ipgapi=”http://ipg-online.com/ipgapi/schemas/ipgapi”>
  <a1:Action>
			 </a1:InitiateClearing>
  </a1:Action>
</ipgapi:IPGApiActionRequest>
Clearing will will be executed directly. If clearing was not successful for at least one terminal, the gateway will send
“false” in the response.
<ipgapi:IPGApiActionResponse
			 xmlns:ipgapi=”http://ipg-online.com/ipgapi/schemas/ipgapi”
			 xmlns:a1=”http://ipg-online.com/ipgapi/schemas/a1”
			 xmlns:v1=”http://ipg-online.com/ipgapi/schemas/v1”>
  <ipgapi:successfully>false</ipgapi:successfully>
</ipgapi:IPGApiActionResponse>


6.1.2 Inquiry Order
The action InquiryOrder allows you to get details about previously processed transactions of a specific order.
You therefore need to submit the corresponding Order ID:
<ns4:IPGApiActionRequest
			 xmlns:ns4=”http://ipg-online.com/ipgapi/schemas/ipgapi”
			 xmlns:ns2=”http://ipg-online.com/ipgapi/schemas/a1”
			 xmlns:ns3=”http://ipg-online.com/ipgapi/schemas/v1”>
  <ns2:Action>
		<ns2:InquiryOrder>
			 <ns2:OrderId>
				          b5b7fb49-3310-4212-9103-5da8bd026600
			 </ns2:OrderId>
		</ns2:InquiryOrder>
  </ns2:Action>
</ns4:IPGApiActionRequest>
The result contains information about all transactions belonging to the corresponding Order ID:
<?xml version=”1.0” encoding=”UTF-8”?><ipgapi:IPGApiActionResponse
xmlns:ipgapi=”http://ipg-online.com/ipgapi/schemas/ipgapi”
xmlns:a1=”http://ipg-online.com/ipgapi/schemas/a1” xmlns:v1=”http://ipgonline.
com/ipgapi/schemas/v1”>
  <ipgapi:successfully>true</ipgapi:successfully>
  <ipgapi:OrderId>b5b7fb49-3310-4212-9103-5da8bd026600</ipgapi:OrderId>
  <v1:Billing/>
  <v1:Shipping/>
  <a1:TransactionValues>
		<v1:CreditCardTxType>
			 <v1:Type>sale</v1:Type>
		</v1:CreditCardTxType>
		<v1:CreditCardData>
			 <v1:CardNumber>4501*****8992</v1:CardNumber>
			 <v1:ExpMonth>11</v1:ExpMonth>

Web Service API Integration Guide                                                        6. Additional Web Service actions 29
			 <v1:ExpYear>17</v1:ExpYear>
			 <v1:Brand>VISA</v1:Brand>
		</v1:CreditCardData>
		<v1:Payment>
			 <v1:ChargeTotal>350.05</v1:ChargeTotal>
			 <v1:Currency>826</v1:Currency>
		</v1:Payment>
		<v1:TransactionDetails>
			 <v1:Comments>AS400</v1:Comments>
			 <v1:InvoiceNumber>551294633441</v1:InvoiceNumber>
			 <v1:OrderId>b5b7fb49-3310-4212-9103-5da8bd026600</v1:OrderId>
			 <v1:Ip>194.127.72.6</v1:Ip>
			 <v1:TDate>1450091856</v1:TDate>
		<v1:TransactionOrigin>MOTO</v1:TransactionOrigin>
  </v1:TransactionDetails>
<ipgapi:IPGApiOrderResponse>
<ipgapi:ApprovalCode>Y:015722:0795783078:PPXM:2062</ipgapi:ApprovalCode>
			 <ipgapi:AVSResponse>PPX</ipgapi:AVSResponse>
			 <ipgapi:Brand>VISA</ipgapi:Brand>
			 <ipgapi:Country>GBR</ipgapi:Country>
			      <ipgapi:OrderId> b5b7fb49-3310-4212-9103- 5da8bd026600</ipgapi:OrderId>
			 <ipgapi:PaymentType>CREDITCARD</ipgapi:PaymentType>
<ipgapi:ProcessorApprovalCode>015722</ipgapi:ProcessorApprovalCode>
			 <ipgapi:ProcessorCCVResponse>M</ipgapi:ProcessorCCVResponse>
			 <ipgapi:ReferencedTDate>1450091856</ipgapi:ReferencedTDate>
			 <ipgapi:TDate>1450091856</ipgapi:TDate>
			      <ipgapi:TDateFormatted>2015.12.14 12:17:36 (CET)</ipgapi:TDateFormatted>
			 <ipgapi:TerminalID>80250837</ipgapi:TerminalID>
		</ipgapi:IPGApiOrderResponse>
  <a1:TraceNumber>2062</a1:TraceNumber>
  <a1:TransactionState>CAPTURED</a1:TransactionState>
  <a1:SubmissionComponent>CONNECT</a1:SubmissionComponent>
  </a1:TransactionValues>
</ipgapi:IPGApiActionResponse>
If your Store is activated for the Fraud Detect product, you will find the score value in the element FraudScore as
shows an example below:
<SOAP-ENV:Envelope xmlns:SOAPENV=”
http://schemas.xmlsoap.org/soap/envelope/”>
		<SOAP-ENV:Header/>
		<SOAP-ENV:Body>
			 <ipgapi:IPGApiActionResponse xmlns:ipgapi=”http://ipgonline.com/ipgapi/
schemas/ipgapi” xmlns:a1=”http://ipgonline.com/ipgapi/schemas/a1” xmlns:v1=”http://
ipgonline.com/ipgapi/schemas/v1”>
		<ipgapi:successfully>true</ipgapi:successfully>
		<ipgapi:OrderId>VT-6d3c8793-f5cb-44c4-99a9-1209f8681693</ipgapi:OrderId>
		<v1:Billing/>
		<v1:Shipping/>
		<a1:TransactionValues>
		<v1:CreditCardTxType>
			 <v1:Type>preauth</v1:Type>
		</v1:CreditCardTxType>
		<v1:CreditCardData>
			 <v1:CardNumber>4035*****4977</v1:CardNumber>
			 <v1:ExpMonth>12</v1:ExpMonth>
			 <v1:ExpYear>18</v1:ExpYear>
			 <v1:Brand>VISA</v1:Brand>
		</v1:CreditCardData>

Web Service API Integration Guide                                                      6. Additional Web Service actions 30
		<v1:Payment>
			 <v1:ChargeTotal>10</v1:ChargeTotal>
			 <v1:Currency>840</v1:Currency>
		</v1:Payment>
		<v1:TransactionDetails>
			 <v1:OrderId>VT-6d3c8793-f5cb-44c4-99a9- 1209f8681693</v1:OrderId>
			 <v1:Ip>127.0.0.1</v1:Ip>
			 <v1:TDate>1498202166</v1:TDate>
			 <v1:TransactionOrigin>ECI</v1:TransactionOrigin>
		</v1:TransactionDetails>
  <ipgapi:IPGApiOrderResponse>
  <ipgapi:ApprovalCode>Y:471142:0096409818:PPX0:000056</ipgapi :ApprovalCode>
  <ipgapi:AVSResponse>PPX</ipgapi:AVSResponse>
  <ipgapi:Brand>VISA</ipgapi:Brand>
  <ipgapi:FraudScore>102</ipgapi:FraudScore>
  <ipgapi:OrderId>VT-6d3c8793-f5cb-44c4-99a9-1209f8681693</ipgapi:OrderId>
  <ipgapi:IpgTransactionId>8383410710</ipgapi:IpgTransactionId>
  <ipgapi:PaymentType>CREDITCARD</ipgapi:PaymentType>
  <ipgapi:ProcessorApprovalCode>471142</ipgapi:ProcessorApprovalCode>
  <ipgapi:ProcessorCCVResponse>0</ipgapi:ProcessorCCVResponse>
  <ipgapi:ReferencedTDate>1498202166</ipgapi:ReferencedTDate>
  <ipgapi:TDate>1498202166</ipgapi:TDate>
  <ipgapi:TDateFormatted>2017.06.23 09:16:06
(CEST)</ipgapi:TDateFormatted>
  <ipgapi:TerminalID>1287451</ipgapi:TerminalID>
  </ipgapi:IPGApiOrderResponse>
  <a1:TraceNumber>000204</a1:TraceNumber>
  <a1:Brand>VISA</a1:Brand>
  <a1:TransactionType>PREAUTH</a1:TransactionType>
  <a1:TransactionState>DECLINED</a1:TransactionState>
  <a1:UserID>54001110</a1:UserID>
  <a1:SubmissionComponent>VT</a1:SubmissionComponent>
  </a1:TransactionValues>
  <a1:TransactionValues>
		<v1:CreditCardTxType>
		<v1:Type>postauth</v1:Type>
		</v1:CreditCardTxType>
  <v1:CreditCardData>
		<v1:Brand>VISA</v1:Brand>
  </v1:CreditCardData>
  <v1:Payment>
		<v1:ChargeTotal>10</v1:ChargeTotal>
		<v1:Currency>978</v1:Currency>
		</v1:Payment>
  <v1:TransactionDetails>
		<v1:OrderId>VT-6d3c8793-f5cb-44c4-99a9-1209f8681693</v1:OrderId>
		<v1:Ip>127.0.0.1</v1:Ip>
		<v1:TDate>1498203371</v1:TDate>
		<v1:TransactionOrigin>ECI</v1:TransactionOrigin>
  </v1:TransactionDetails>
  <ipgapi:IPGApiOrderResponse>
		 <ipgapi:ApprovalCode>N:-50653:Sent invalid currency or no currencies were setup
for this store.</ipgapi:ApprovalCode>
		<ipgapi:Brand>VISA</ipgapi:Brand>
		<ipgapi:OrderId>VT-6d3c8793-f5cb-44c4-99a9- 1209f8681693</ipgapi:OrderId>
		<ipgapi:IpgTransactionId>8383410730</ipgapi:IpgTransactionId >
		<ipgapi:PaymentType>CREDITCARD</ipgapi:PaymentType>
		<ipgapi:ReferencedTDate>1498203371</ipgapi:ReferencedTDate>

Web Service API Integration Guide                            6. Additional Web Service actions 31
		<ipgapi:TDate>1498203371</ipgapi:TDate>
		<ipgapi:TDateFormatted>2017.06.23 09:36:11
(CEST)</ipgapi:TDateFormatted>
		</ipgapi:IPGApiOrderResponse>
		<a1:Brand>VISA</a1:Brand>
		<a1:TransactionType>POSTAUTH</a1:TransactionType>
		<a1:TransactionState>DECLINED</a1:TransactionState>
		<a1:SubmissionComponent>CONNECT</a1:SubmissionComponent>
		</a1:TransactionValues>
		<a1:TransactionValues>
			 <v1:CreditCardTxType>
			 <v1:Type>postauth</v1:Type>
			 </v1:CreditCardTxType>
			 <v1:CreditCardData/>
		<v1:Payment>
			 <v1:ChargeTotal>10</v1:ChargeTotal>
			 <v1:Currency>840</v1:Currency>
		</v1:Payment>
		<v1:TransactionDetails>
			 <v1:OrderId>VT-6d3c8793-f5cb-44c4-99a9- 1209f8681693</v1:OrderId>
			 <v1:Ip>127.0.0.1</v1:Ip>
			 <v1:TDate>1498203386</v1:TDate>
			 <v1:TransactionOrigin>ECI</v1:TransactionOrigin>
		</v1:TransactionDetails>
			 <ipgapi:IPGApiOrderResponse>
			 <ipgapi:ApprovalCode>Y:576275:0096397936:PPX
:0718432585</ipgapi:ApprovalCode>
			 <ipgapi:AVSResponse>PPX</ipgapi:AVSResponse>
			 <ipgapi:OrderId>VT-6d3c8793-f5cb-44c4-99a9-
1209f8681693</ipgapi:OrderId>
			 <ipgapi:IpgTransactionId>8383410731</ipgapi:IpgTransactionId >
			 <ipgapi:PaymentType>CREDITCARD</ipgapi:PaymentType>
			 <ipgapi:ProcessorApprovalCode>576275</ipgapi:ProcessorApprovalCode>
			 <ipgapi:ProcessorReceiptNumber>2585</ipgapi:ProcessorReceipt Number>
			 <ipgapi:ProcessorCCVResponse></ipgapi:ProcessorCCVResponse>
			 <ipgapi:ProcessorTraceNumber>071843</ipgapi:ProcessorTraceNumber>
			 <ipgapi:ReferencedTDate>1498203386</ipgapi:ReferencedTDate>
			 <ipgapi:TDate>1498203386</ipgapi:TDate>
			 <ipgapi:TDateFormatted>2017.06.23 09:36:26
(CEST)</ipgapi:TDateFormatted>
			 </ipgapi:IPGApiOrderResponse>
			 <a1:TransactionType>POSTAUTH</a1:TransactionType>
			 <a1:TransactionState>DECLINED</a1:TransactionState>
			 <a1:SubmissionComponent>CONNECT</a1:SubmissionComponent>
		</a1:TransactionValues>
  </ipgapi:IPGApiActionResponse>
  </SOAP-ENV:Body>
</SOAP-ENV:Envelope>


6.1.3 Inquiry Transaction
The action InquiryTransaction allows you to get details about a previously processed transaction. You therefore
need to either submit the merchantTransactionId if you have assigned one or alternatively the ipgTransactionId:
<soapenv:Envelope xmlns:soapenv=”http://schemas.xmlsoap.org/soap/envelope/”
xmlns:ipg=”http://ipg-online.com/ipgapi/schemas/ipgapi”
xmlns:a1=”http://ipg-online.com/ipgapi/schemas/a1” xmlns:v1=”http://ipgonline.
com/ipgapi/schemas/v1”>

Web Service API Integration Guide                                                    6. Additional Web Service actions 32
		<soapenv:Header/>
		<soapenv:Body>
			 <ipg:IPGApiActionRequest>
				          <a1:Action>
					 <a1:InquiryTransaction>
						                  <!--Optional:-->
					 <a1:StoreId>12072591</a1:StoreId>
					             <!--You have a CHOICE of the next 3 items at this level-->
					 <a1:OrderId>C-38fd1bcd-1d67-4248-b9d5-d30376d92163</a1:OrderId>
					 <a1:TDate>1453814407</a1:TDate>
					 </a1:InquiryTransaction>
				          </a1:Action>
			 </ipg:IPGApiActionRequest>
		</soapenv:Body>
</soapenv:Envelope>
The response contains the same elements as in the Inquiry Order example above.


6.1.4 Get Last Orders
This action provides a query interface for information on the latest orders that have been submitted in order to
support in-app reporting.
This functionality is not enabled by default, as it requires additional configuration. Please contact your local support
team for more information. Please do not use this functionality to regularly request the result of transactions you
have processed but store the API transaction response instead.


6.1.5 Latest orders of a Store
This query returns “the last n orders of the given store”.
<?xml version=”1.0” encoding=”UTF-8”?>
<SOAP-ENV:Envelope xmlns:SOAPENV=”http://schemas.xmlsoap.org/soap/envelope/”>
		<SOAP-ENV:Header/>
		<SOAP-ENV:Body>
			 <ns5:IPGApiActionRequest
			 xmlns:ns5=”http://ipgonline.com/ipgapi/schemas/ipgapi”
			 xmlns:ns2=”http://ipgonline.com/ipgapi/schemas/v1”
			 xmlns:ns3=”http://ipgonline.com/ipgapi/schemas/a1”>
			 <ns3:Action>
				          <ns3:GetLastOrders>
					 <ns3:Count>5</ns3:Count>
				          </ns3:GetLastOrders>
			 </ns3:Action>
		</ns5:IPGApiActionRequest>
  </SOAP-ENV:Body>
</SOAP-ENV:Envelope>


6.1.6 Latest orders of a Store within a given date range
This query returns “the last n orders of the given store within the given date-range”. It could also be used for
pagination.
Both dates DateFrom and DateTo are to be specified, in the form of xs:dateTime
<?xml version=”1.0” encoding=”UTF-8”?>
<SOAP-ENV:Envelope xmlns:SOAPENV=”
http://schemas.xmlsoap.org/soap/envelope/”>
<SOAP-ENV:Header/>
<SOAP-ENV:Body>

Web Service API Integration Guide                                                        6. Additional Web Service actions 33
		<ns5:IPGApiActionRequest
		xmlns:ns5=”http://ipgonline.com/ipgapi/schemas/ipgapi”
		xmlns:ns2=”http://ipgonline.com/ipgapi/schemas/v1”
		xmlns:ns3=”http://ipgonline.com/ipgapi/schemas/a1”>
			 <ns3:Action>
				          <ns3:GetLastOrders>
				          <ns3:Count>5</ns3:Count>
				          <ns3:DateFrom>2014-04-05T10:23:37.143+02:00</ns3:DateFrom>
				          <ns3:DateTo>2014-05-05T10:23:37.143+02:00</ns3:DateTo>
				          </ns3:GetLastOrders>
			 </ns3:Action>
		</ns5:IPGApiActionRequest>
  </SOAP-ENV:Body>
</SOAP-ENV:Envelope>


6.1.7 All orders of a Store after a given Order ID
This interface is intended to support pagination of large result-sets. It returns “The last n orders of the given store
after a given order (by orderId)”
<?xml version=”1.0” encoding=”UTF-8”?>
<SOAP-ENV:Envelope xmlns:SOAPENV=”
http://schemas.xmlsoap.org/soap/envelope/”>
		<SOAP-ENV:Header/>
		<SOAP-ENV:Body>
			 <ns5:IPGApiActionRequest
				          xmlns:ns5=”http://ipgonline.com/ipgapi/schemas/ipgapi”
				          xmlns:ns2=”http://ipgonline. com/ipgapi/schemas/v1”
				          xmlns:ns3=”http://ipgonline. com/ipgapi/schemas/a1”>
			 <ns3:Action>
			 <ns3:GetLastOrders>
				          <ns3:Count>2</ns3:Count>
			 <ns3:OrderID>Test SGSDAO.ConversionDate
1382020873203</ns3:OrderID>
				          </ns3:GetLastOrders>
			 </ns3:Action>
		</ns5:IPGApiActionRequest>
  </SOAP-ENV:Body>
</SOAP-ENV:Envelope


6.1.8 Response
All query methods return the same structure as a result.
• The success-status is returned by <ipgapi:successfully>true</ipgapi:successfully>
• <ipgapi:ResultInfo>/ <a1:MoreResultsAvailable>true</a1:MoreResultsAvailable> tells
  if there are more results available.
  o The service is stateless, therefore subsequent queries for pagination have to use either…
  o	GetLastOrders(storeID, count, dateFrom, dateTo) w/ dateTo set to the last order’s order_date of the previous
     resultset OR
  o	GetLastOrders(storeID, count, orderId) w/ orderId set to the last order of the previous resultset
• List of orders <ipgapi:OrderValues>, consisting of
  o OrderId – the orders’ unique id
  o <a1:TransactionValues> transactions
  o <v1:Basket> the basket
		■ with basket-items <v1:Item>
		■ and each item with item-options <v1:Option>

Web Service API Integration Guide                                                         6. Additional Web Service actions 34
<?xml version=”1.0” encoding=”UTF-8”?><ipgapi:IPGApiActionResponse
xmlns:ipgapi=”http://ipg-online.com/ipgapi/schemas/ipgapi”
xmlns:a1=”http://ipg-online.com/ipgapi/schemas/a1” xmlns:v1=”http://ipgonline.com/
ipgapi/schemas/v1”>
  <ipgapi:successfully>true</ipgapi:successfully>
  <ipgapi:ResultInfo>
		<a1:MoreResultsAvailable>true</a1:MoreResultsAvailable>
  </ipgapi:ResultInfo>
  <ipgapi:OrderValues>
		<a1:OrderId>A-00ddff18-b210-428b-804f-150b2567dbc9</a1:OrderId>
		<a1:OrderDate>2015-09-30T13:43:44.000+02:00</a1:OrderDate>
		<v1:Basket>
			 <v1:Item>
				          <v1:ID>d160c63e-7e9e-4a4a-bd5e-ae50a9133bf7</v1:ID>
				          <v1:Description>katharistiko</v1:Description>
				          <v1:ChargeTotal>25</v1:ChargeTotal>
				          <v1:Quantity>1</v1:Quantity>
			 </v1:Item>
		</v1:Basket>
		<v1:Billing/>
		<v1:Shipping/>
		<a1:TransactionValues>
			 <v1:CreditCardTxType>
				          <v1:Type>sale</v1:Type>
			 </v1:CreditCardTxType>
			 <v1:CreditCardData>
				          <v1:CardNumber>5185****9001</v1:CardNumber>
				          <v1:ExpMonth>04</v1:ExpMonth>
				          <v1:ExpYear>17</v1:ExpYear>
				          <v1:Brand>MASTERCARD</v1:Brand>
			 </v1:CreditCardData>
			 <v1:Payment>
				          <v1:ChargeTotal>25</v1:ChargeTotal>
				          <v1:Currency>978</v1:Currency>
			 </v1:Payment>
			 <v1:TransactionDetails>
				          <v1:OrderId>A-00ddff18-b210-428b-804f-150b2567dbc9</v1:OrderId>
				          <v1:TDate>1443620624</v1:TDate>
				          <v1:TransactionOrigin>RETAIL</v1:TransactionOrigin>
			 </v1:TransactionDetails>
			 <ipgapi:IPGApiOrderResponse>
  <ipgapi:ApprovalCode>Y:024309:0782287817:PPXX:796023</ipgapi:ApprovalCode>
		<ipgapi:AVSResponse>PPX</ipgapi:AVSResponse>
		<ipgapi:Brand>MASTERCARD</ipgapi:Brand>
		<ipgapi:Country>GRC</ipgapi:Country>
		<ipgapi:OrderId>A-00ddff18-b210-428b-804f-150b2567dbc9</ipgapi:OrderId>
		<ipgapi:PayerSecurityLevel>N</ipgapi:PayerSecurityLevel>
		<ipgapi:PaymentType>CREDITCARD</ipgapi:PaymentType>
  <ipgapi:ProcessorApprovalCode>024309</ipgapi:ProcessorApprovalCode>
  <ipgapi:ProcessorCCVResponse>X</ipgapi:ProcessorCCVResponse>
		<ipgapi:TDate>1443620624</ipgapi:TDate>
		<ipgapi:TDateFormatted>2015.09.30 15:43:44
(CEST)</ipgapi:TDateFormatted>
  <ipgapi:TerminalID>90000001</ipgapi:TerminalID>
  </ipgapi:IPGApiOrderResponse>
  <a1:TraceNumber>796023</a1:TraceNumber>
  <a1:TransactionState>SETTLED</a1:TransactionState>
  <a1:UserID>1</a1:UserID>

Web Service API Integration Guide                              6. Additional Web Service actions 35
			 <a1:SubmissionComponent>API</a1:SubmissionComponent>
		</a1:TransactionValues>
  </ipgapi:OrderValues>
  <ipgapi:OrderValues>
		<a1:OrderId>A-85a682a4-8481-48a3-b94c-a612fdc3a528</a1:OrderId>
		<a1:OrderDate>2015-09-29T15:45:46.000+02:00</a1:OrderDate>
		<v1:Basket>
			 <v1:Item>
				          <v1:ID>5105971d-b5fd-482b-be35-cb8a6569f7c7</v1:ID>
				          <v1:Description>efimerida</v1:Description>
				          <v1:ChargeTotal>12.15</v1:ChargeTotal>
			 <v1:Quantity>1</v1:Quantity>
			 </v1:Item>
		</v1:Basket>
		<v1:Billing/>
		<v1:Shipping/>
		<a1:TransactionValues>
			 <v1:CreditCardTxType>
				          <v1:Type>sale</v1:Type>
			 </v1:CreditCardTxType>
			 <v1:CreditCardData>
				          <v1:CardNumber>4060.....8009</v1:CardNumber>
				          <v1:ExpMonth>02</v1:ExpMonth>
				          <v1:ExpYear>17</v1:ExpYear>
				          <v1:Brand>VISA</v1:Brand>
			 </v1:CreditCardData>
			 <v1:Payment>
				          <v1:ChargeTotal>12.15</v1:ChargeTotal>
				          <v1:Currency>978</v1:Currency>
			 </v1:Payment>
			 <v1:TransactionDetails>
				          <v1:OrderId>A-85a682a4-8481-48a3-b94ca612fdc3a528</v1:OrderId>
				          <v1:TDate>1443541546</v1:TDate>
				          <v1:TransactionOrigin>RETAIL</v1:TransactionOrigin>
			 </v1:TransactionDetails>
			 <ipgapi:IPGApiOrderResponse>
		<ipgapi:ApprovalCode>Y:201846:0782126690:PPXX:796005</ipgapi:ApprovalCode>
			 <ipgapi:AVSResponse>PPX</ipgapi:AVSResponse>
			 <ipgapi:Brand>VISA</ipgapi:Brand>
			 <ipgapi:Country>GRC</ipgapi:Country>
			 <ipgapi:OrderId>A-85a682a4-8481-48a3-b94ca612fdc3a528</ipgapi:OrderId>
			 <ipgapi:PayerSecurityLevel>V</ipgapi:PayerSecurityLevel>
			 <ipgapi:PaymentType>CREDITCARD</ipgapi:PaymentType>
		<ipgapi:ProcessorApprovalCode>201846</ipgapi:ProcessorApprovalCode>
		<ipgapi:ProcessorCCVResponse>X</ipgapi:ProcessorCCVResponse>
			 <ipgapi:TDate>1443541546</ipgapi:TDate>
			 <ipgapi:TDateFormatted>2015.09.29 17:45:46
(CEST)</ipgapi:TDateFormatted>
			 <ipgapi:TerminalID>90000001</ipgapi:TerminalID>
			 </ipgapi:IPGApiOrderResponse>
			 <a1:TraceNumber>796005</a1:TraceNumber>
			 <a1:TransactionState>SETTLED</a1:TransactionState>
			 <a1:UserID>1</a1:UserID>
			 <a1:SubmissionComponent>API</a1:SubmissionComponent>
		</a1:TransactionValues>
  </ipgapi:OrderValues>
  <ipgapi:OrderValues>
  <a1:OrderId>A-787829af-2baa-408e-881e-3f43f584496e</a1:OrderId>

Web Service API Integration Guide                           6. Additional Web Service actions 36
  <a1:OrderDate>2015-09-29T13:58:15.000+02:00</a1:OrderDate>
  <v1:Basket>
		<v1:Item>
			 <v1:ID>bd5c1138-e734-4379-89a7-075c1ac31bd0</v1:ID>
			 <v1:Description>taigara</v1:Description>
			 <v1:ChargeTotal>3.5</v1:ChargeTotal>
			 <v1:Quantity>1</v1:Quantity>
		</v1:Item>
  </v1:Basket>
  <v1:Billing/>
  <v1:Shipping/>
  <a1:TransactionValues>
		<v1:CreditCardTxType>
		<v1:Type>sale</v1:Type>
  </v1:CreditCardTxType>
  <v1:CreditCardData>
		<v1:CardNumber>5167.....7382</v1:CardNumber>
		<v1:ExpMonth>07</v1:ExpMonth>
		<v1:ExpYear>18</v1:ExpYear>
		<v1:Brand>MASTERCARD</v1:Brand>
  </v1:CreditCardData>
  <v1:Payment>
		<v1:ChargeTotal>3.5</v1:ChargeTotal>
		<v1:Currency>978</v1:Currency>
  </v1:Payment>
  <v1:TransactionDetails>
		<v1:OrderId>A-787829af-2baa-408e-881e-3f43f584496e</v1:OrderId>
		<v1:TDate>1443535095</v1:TDate>
		<v1:TransactionOrigin>RETAIL</v1:TransactionOrigin>
  </v1:TransactionDetails>
  <ipgapi:IPGApiOrderResponse>
  <ipgapi:ApprovalCode>Y:328188:0782108096:PPXX:795995</ipgapi:ApprovalCode>
		<ipgapi:AVSResponse>PPX</ipgapi:AVSResponse>
		<ipgapi:Brand>MASTERCARD</ipgapi:Brand>
		<ipgapi:Country>GRC</ipgapi:Country>
		<ipgapi:OrderId>A-787829af-2baa-408e-881e-3f43f584496e</ipgapi:OrderId>
		<ipgapi:PayerSecurityLevel>N</ipgapi:PayerSecurityLevel>
		<ipgapi:PaymentType>CREDITCARD</ipgapi:PaymentType>
  <ipgapi:ProcessorApprovalCode>328188</ipgapi:ProcessorApprovalCode>
  <ipgapi:ProcessorCCVResponse>X</ipgapi:ProcessorCCVResponse>
		<ipgapi:TDate>1443535095</ipgapi:TDate>
		<ipgapi:TDateFormatted>2015.09.29 15:58:15
(CEST)</ipgapi:TDateFormatted>
			 <ipgapi:TerminalID>90000001</ipgapi:TerminalID>		
		</ipgapi:IPGApiOrderResponse>
		<a1:TraceNumber>795995</a1:TraceNumber>
		<a1:TransactionState>SETTLED</a1:TransactionState>
		<a1:UserID>1</a1:UserID>
		<a1:SubmissionComponent>API</a1:SubmissionComponent>
  </a1:TransactionValues>
  </ipgapi:OrderValues>
  <ipgapi:OrderValues>
		<a1:OrderId>A-0606cb2c-d947-4557-855e-98722fc100f8</a1:OrderId>
		<a1:OrderDate>2015-09-28T21:34:01.000+02:00</a1:OrderDate>
		<v1:Basket>
		<v1:Item>
			 <v1:ID>a3686a1e-e2dd-4f2b-aab0-2131af33c141</v1:ID>
			 <v1:Description>kpxol</v1:Description>

Web Service API Integration Guide                             6. Additional Web Service actions 37
				        <v1:ChargeTotal>12.8</v1:ChargeTotal>
				        <v1:Quantity>1</v1:Quantity>
			 </v1:Item>
		</v1:Basket>
		<v1:Billing/>
		<v1:Shipping/>
		<a1:TransactionValues>
			 <v1:CreditCardTxType>
				        <v1:Type>sale</v1:Type>
			 </v1:CreditCardTxType>
			 <v1:CreditCardData>
				        <v1:CardNumber>5185.....9001</v1:CardNumber>
				        <v1:ExpMonth>04</v1:ExpMonth>
				        <v1:ExpYear>17</v1:ExpYear>
				        <v1:Brand>MASTERCARD</v1:Brand>
			 </v1:CreditCardData>
			 <v1:Payment>
				        <v1:ChargeTotal>12.8</v1:ChargeTotal>
				        <v1:Currency>978</v1:Currency>
			 </v1:Payment>
			 <v1:TransactionDetails>
				        <v1:OrderId>A-0606cb2c-d947-4557-855e-98722fc100f8</v1:OrderId>
				        <v1:TDate>1443476041</v1:TDate>
				        <v1:TransactionOrigin>RETAIL</v1:TransactionOrigin>
			 </v1:TransactionDetails>
			 <ipgapi:IPGApiOrderResponse>
		<ipgapi:ApprovalCode>Y:021474:0782015139:PPXX:795975</ipgapi:ApprovalCode>
			 <ipgapi:AVSResponse>PPX</ipgapi:AVSResponse>
			 <ipgapi:Brand>MASTERCARD</ipgapi:Brand>
			 <ipgapi:Country>GRC</ipgapi:Country>
			 <ipgapi:OrderId>A-0606cb2c-d947-4557-855e-98722fc100f8</ipgapi:OrderId>
			 <ipgapi:PayerSecurityLevel>N</ipgapi:PayerSecurityLevel>


6.1.9 Get Last Transactions
This action provides a query interface for information on the latest transactions that have been submitted in order
to support in-app reporting.
This functionality is not enabled by default, as it requires additional configuration. Please contact your local support
team for more information.

Please do not use this functionality to regularly request the result of transactions you have
processed but store the API transaction response instead.

6.1.10 Latest transactions of a Store
This query returns “the last n transactions of the given store”.
<?xml version=”1.0” encoding=”UTF-8”?>
<SOAP-ENV:Envelope xmlns:SOAPENV=”
http://schemas.xmlsoap.org/soap/envelope/”>
<SOAP-ENV:Header/>
<SOAP-ENV:Body>
<ns5:IPGApiActionRequest
xmlns:ns5=”http://ipg-online.com/ipgapi/schemas/ipgapi”
xmlns:ns2=”http://ipg-online.com/ipgapi/schemas/a1”
xmlns:ns3=”http://ipg-online.com/ipgapi/schemas/v1”>
		<ns2:Action>
			 <ns2:GetLastTransactions>
Web Service API Integration Guide                                                        6. Additional Web Service actions 38
				          <ns2:count>2</ns2:count>
			 </ns2:GetLastTransactions>
		</ns2:Action>
		</ns5:IPGApiActionRequest>
  </SOAP-ENV:Body>
</SOAP-ENV:Envelope>


6.1.11 All transactions of a Store after a given Transaction ID
This interface is intended to support pagination of large result-sets. It returns “The last n transactions of the given
store before a given transaction (by transactionId {orderId, TDate})”
A transactionID consists of the tuple
• OrderId the ID of the transactions’ order
• TDate the date of the transaction
<?xml version=”1.0” encoding=”UTF-8”?>
<SOAP-ENV:Envelope xmlns:SOAPENV=”http://schemas.xmlsoap.org/soap/envelope/”>
  <SOAP-ENV:Header/>
  <SOAP-ENV:Body>
		<ns5:IPGApiActionRequest
		xmlns:ns5=”http://ipgonline.com/ipgapi/schemas/ipgapi”
		xmlns:ns2=”http://ipgonline.com/ipgapi/schemas/a1”
		xmlns:ns3=”http://ipgonline.com/ipgapi/schemas/v1”>
			 <ns2:Action>
				          <ns2:GetLastTransactions>
					 <ns2:count>2</ns2:count>
					 <ns2:OrderId>A-eb65437a-c538-4cdd-82b3-d316ae160c22</ns2:OrderId>
					 <ns2:TDate>1407373211</ns2:TDate>
				          </ns2:GetLastTransactions>
			 </ns2:Action>
		</ns5:IPGApiActionRequest>
  </SOAP-ENV:Body>
</SOAP-ENV:Envelope>


6.1.12 Response
All query methods return the same structure as a result.
• The success-status is returned by <ipgapi:successfully>true</ipgapi:successfully>
• List of transactions <a1:TransactionValues>
<?xml version=”1.0” encoding=”UTF-8”?>
<SOAP-ENV:Envelope xmlns:SOAPENV=”http://schemas.xmlsoap.org/soap/envelope/”>
		<SOAP-ENV:Header/>
		<SOAP-ENV:Body>
			 <ipgapi:IPGApiActionResponse xmlns:ipgapi=
			 ”http://ipgonline.com/ipgapi/schemas/ipgapi”
			 xmlns:a1=”http://ipgonline.com/ipgapi/schemas/a1”
			 xmlns:v1=”http://ipgonline.com/ipgapi/schemas/v1”>
		<ipgapi:successfully>true</ipgapi:successfully>
		<a1:TransactionValues>
			 <v1:CreditCardTxType>
		<v1:Type>periodic</v1:Type>
  </v1:CreditCardTxType>
  <v1:CreditCardData>
		<v1:CardNumber>4035*****4977</v1:CardNumber>
		<v1:ExpMonth>12</v1:ExpMonth>
		<v1:ExpYear>14</v1:ExpYear>

Web Service API Integration Guide                                                         6. Additional Web Service actions 39
		<v1:Brand>VISA</v1:Brand>
 </v1:CreditCardData>
 <v1:Payment>
		<v1:ChargeTotal>1</v1:ChargeTotal>
		<v1:Currency>978</v1:Currency>
 </v1:Payment>
 <v1:TransactionDetails>
		<v1:OrderId>A-bcbb36ad-90ad-4ff7-ad96-b5d73dd9c5e9</v1:OrderId>
		<v1:TDate>1407373210</v1:TDate>
 </v1:TransactionDetails>
 <ipgapi:IPGApiOrderResponse>
		<ipgapi:ApprovalCode>Y:272450:0014750514:PPXM:0433836659</ipgapi:ApprovalCode>
			 <ipgapi:AVSResponse>PPX</ipgapi:AVSResponse>
			 <ipgapi:Brand>VISA</ipgapi:Brand>
			 <ipgapi:OrderId>A-bcbb36ad-90ad-4ff7-ad96-b5d73dd9c5e9</ipgapi:OrderId>
			 <ipgapi:PaymentType>CREDITCARD</ipgapi:PaymentType>
			 <ipgapi:ProcessorApprovalCode>272450</ipgapi:ProcessorApprovalCode>
			 <ipgapi:ProcessorReceiptNumber>6659</ipgapi:ProcessorReceiptNumber>
			 <ipgapi:ProcessorCCVResponse>M</ipgapi:ProcessorCCVResponse>
			 <ipgapi:ProcessorTraceNumber>043383</ipgapi:ProcessorTraceNumber>
			 <ipgapi:ReferencedTDate>1407373210</ipgapi:ReferencedTDate>
			 <ipgapi:TDate>1407373210</ipgapi:TDate>
			 <ipgapi:TDateFormatted>2014.08.07 03:00:10
			 (CEST)</ipgapi:TDateFormatted>
			 <ipgapi:TerminalID>54000667</ipgapi:TerminalID>
			 </ipgapi:IPGApiOrderResponse>
				         <a1:TransactionState>CAPTURED</a1:TransactionState>
				         <a1:UserID>1</a1:UserID>
				         <a1:SubmissionComponent>BUS</a1:SubmissionComponent>
			 </a1:TransactionValues>
			 <a1:TransactionValues>
				         <v1:CreditCardTxType>
					 <v1:Type>periodic</v1:Type>
				         </v1:CreditCardTxType>
				         <v1:CreditCardData>
					 <v1:CardNumber>4035*****4977</v1:CardNumber>
					 <v1:ExpMonth>12</v1:ExpMonth>
					 <v1:ExpYear>14</v1:ExpYear>
					 <v1:Brand>VISA</v1:Brand>
				         </v1:CreditCardData>
				         <v1:Payment>
					 <v1:ChargeTotal>1</v1:ChargeTotal>
					 <v1:Currency>978</v1:Currency>
				         </v1:Payment>
				         <v1:TransactionDetails>
					 <v1:OrderId>A-52421c39-69c4-4b2d-959d-9fdcd3a9420a</v1:OrderId>
					 <v1:TDate>1407373209</v1:TDate>
				         </v1:TransactionDetails>
				         <ipgapi:IPGApiOrderResponse>
		<ipgapi:ApprovalCode>Y:416502:0014750513:PPXM:4625106408</ipgapi:ApprovalCode>
			 <ipgapi:AVSResponse>PPX</ipgapi:AVSResponse>
			 <ipgapi:Brand>VISA</ipgapi:Brand>
			 <ipgapi:OrderId>A-52421c39-69c4-4b2d-959d-9fdcd3a9420a</ipgapi:OrderId>
			 <ipgapi:PaymentType>CREDITCARD</ipgapi:PaymentType>
		<ipgapi:ProcessorApprovalCode>416502</ipgapi:ProcessorApprovalCode>
		<ipgapi:ProcessorReceiptNumber>6408</ipgapi:ProcessorReceiptNumber>
		<ipgapi:ProcessorCCVResponse>M</ipgapi:ProcessorCCVResponse>
		<ipgapi:ProcessorTraceNumber>462510</ipgapi:ProcessorTraceNumber>

Web Service API Integration Guide                           6. Additional Web Service actions 40
		<ipgapi:ReferencedTDate>1407373209</ipgapi:ReferencedTDate>
		<ipgapi:TDate>1407373209</ipgapi:TDate>
		<ipgapi:TDateFormatted>2014.08.07 03:00:09
		(CEST)</ipgapi:TDateFormatted>
		<ipgapi:TerminalID>54000666</ipgapi:TerminalID>
			 </ipgapi:IPGApiOrderResponse>
			 <a1:TransactionState>CAPTURED</a1:TransactionState>
			 <a1:UserID>1</a1:UserID>
			 <a1:SubmissionComponent>BUS</a1:SubmissionComponent>
		</a1:TransactionValues>
  </ipgapi:IPGApiActionResponse>
  </SOAP-ENV:Body>
</SOAP-ENV:Envelope>


6.1.13 Recurring Payments (Scheduler)
The action RecurringPayment allows you to install, modify or cancel periodic payments in a way that subsequent
transactions will automatically be triggered by the gateway.
For every recurring transaction, the gateway can send a server-to-server transaction notification to a defined
Notification URL. Please contact your local support team to get your URL registered for these notifications.


6.1.14 Install
The following example shows how to install a monthly credit card payment with 12 executions (InstallmentCount)
in 2011 starting on 15 January 2011.
Please note that the RecurringStartDate will be interpreted based on the timezone Europe/Berlin.
<ns4:IPGApiActionRequest
		xmlns:ns4=”http://ipg-online.com/ipgapi/schemas/ipgapi”
		xmlns:ns2=”http://ipg-online.com/ipgapi/schemas/a1”
		xmlns:ns3=”http://ipg-online.com/ipgapi/schemas/v1”>
   <ns2:Action>
		<ns2:RecurringPayment>
			 <ns2:Function>install</ns2:Function>
			 <ns2:RecurringPaymentInformation>
				                <ns2:RecurringStartDate>
					 20110115
				                </ns2:RecurringStartDate>
				                <ns2:InstallmentCount>12</ns2:InstallmentCount>
				                <ns2:InstallmentFrequency>
					 1
				                </ns2:InstallmentFrequency>
				                <ns2:InstallmentPeriod>
					 month
				                </ns2:InstallmentPeriod>
			 </ns2:RecurringPaymentInformation>
			 <ns2:CreditCardData>
				                <ns3:CardNumber>4035……4977</ns3:CardNumber>
				                <ns3:ExpMonth>12</ns3:ExpMonth>
				                <ns3:ExpYear>12</ns3:ExpYear>
				                <ns3:CardCodeValue>XXX</ns3:CardCodeValue>
			 </ns2:CreditCardData>
			 <ns3:Payment>
				                <ns3:ChargeTotal>1</ns3:ChargeTotal>
				                <ns3:Currency>978</ns3:Currency>
			 </ns3:Payment>
		</ns2:RecurringPayment>
   </ns2:Action>
</ns4:IPGApiActionRequest>
Web Service API Integration Guide                                                     6. Additional Web Service actions 41
If you set the RecurringStartDate to the actual date, the first payment will immediately be initiated. In this case, the
payment data will only be stored for future payments if this first payment was succesful/approved. A start date in
the past is not allowed.
The default value for TransactionOrigin is ‘ECI’. If you want to change this value, you can submit a different
TransactionOrigin tag in the RecurringPayment tag.


6.1.15 Modify
Modifications of an existing Recurring Payment can be initiated using the Order ID:
<ns4:IPGApiActionRequest
			 xmlns:ns4=”http://ipg-online.com/ipgapi/schemas/ipgapi”
			 xmlns:ns2=”http://ipg-online.com/ipgapi/schemas/a1”
			 xmlns:ns3=”http://ipg-online.com/ipgapi/schemas/v1”>
		<ns2:Action>
			 <ns2:RecurringPayment>
				          <ns2:Function>modify</ns2:Function>
				          <ns2:OrderId>
					 e368a525-173f-4f56-9ae2-beb4023a6993
				          </ns2:OrderId>
				          <ns2:RecurringPaymentInformation>
					 <ns2:InstallmentCount>999</ns2:InstallmentCount>
				          </ns2:RecurringPaymentInformation>
			 </ns2:RecurringPayment>
		</ns2:Action>
</ns4:IPGApiActionRequest>
You only need to include the elements that need to be changed. If you change the credit card number, it is also
required to include the expiry date, otherwise you can change the expiry date without specifying the credit card
number. If you want to change the amount, you also need to include the currency.
It is possible to change the payment method, e. g. from Credit Card to German Direct Debit.


6.1.16 Cancel
To cancel a Recurring Payment, you also use the Order ID:
<ns4:IPGApiActionRequest
			 xmlns:ns4=”http://ipg-online.com/ipgapi/schemas/ipgapi”
			 xmlns:ns2=”http://ipg-online.com/ipgapi/schemas/a1”
			 xmlns:ns3=”http://ipg-online.com/ipgapi/schemas/v1”>
		<ns2:Action>
			 <ns2:RecurringPayment>
				          <ns2:Function>cancel</ns2:Function>
				          <ns2:OrderId>
					 e368a525-173f-4f56-9ae2-beb4023a6993
				          </ns2:OrderId>
			 </ns2:RecurringPayment>
		</ns2:Action>
</ns4:IPGApiActionRequest>


6.1.17 Test Recurring Payments in test environment
The test system allows you to manually initiate a scheduled payment to test this functionality. This function will not
work in live mode.
<ns4:IPGApiActionRequest
			 xmlns:ns4=”http://ipg-online.com/ipgapi/schemas/ipgapi”
			 xmlns:ns2=”http://ipg-online.com/ipgapi/schemas/a1”
			 xmlns:ns3=”http://ipg-online.com/ipgapi/schemas/v1”>
Web Service API Integration Guide                                                        6. Additional Web Service actions 42
		<ns2:Action>
			 <ns2:RecurringPayment>
				          <ns2:Function>
					 perform only in test environment
				          </ns2:Function>
				          <ns2:OrderId>
					 A-eab002b9-5889-4082-9cc9-5bc06b8eaa61
				          </ns2:OrderId>
			 </ns2:RecurringPayment>
		</ns2:Action>
</ns4:IPGApiActionRequest>


6.1.18 Response
The response for a successful instalment, modification or cancellation contains the value true for the parameter
<ns4:successfully>:
<ns4:IPGApiActionResponse
			 xmlns:ns4=”http://ipg-online.com/ipgapi/schemas/ipgapi”
			 xmlns:ns2=”http://ipg-online.com/ipgapi/schemas/a1”
			 xmlns:ns3=”http://ipg-online.com/ipgapi/schemas/v1”>
		<ns4:successfully>true</ns4:successfully>
		<ns4:OrderId>e368a525-173f-4f56-9ae2-beb4023a6993</ns4:OrderId>
</ns4:IPGApiActionResponse>


6.1.19 External transaction status
Some payment endpoints do not send the final result of a payment transaction within their response.
In such cases the Gateway returns an approval code that starts with a question mark (?:…).The action
GetExternalTransactionState allows you to request updates on the state of such transactions. You can use OrderID
+ TDate, MerchantTransactionId or IpgTransactionId to reference to a transaction.


6.1.20 Trigger email notifications
The action SendEMailNotification triggers an email notification for a given transaction. The email will be created
with the email template that has been configured for your Store.
See the User Guide Virtual Terminal & Online Portal for more information on transaction notifications by email.
<ns5:IPGApiActionRequest
			 xmlns:ns5=http://ipg-online.com/ipgapi/schemas/ipgapi
			 xmlns:ns2=”http://ipg-online.com/ipgapi/schemas/a1”
			 xmlns:ns3=”http://ipg-online.com/ipgapi/schemas/v1”>
		<ns2:Action>
			 <ns2:SendEMailNotification>
				          <ns2:OrderId>0/8/15</ns2:OrderId>
				          <ns2:TDate>1250599046</ns2:TDate>
			 </ns2:SendEMailNotification>
		</ns2:Action>
</ns5:IPGApiActionRequest>
If the optional parameter Email is not set, the email address of the customer stored with the transaction will be used.


6.1.21 Card Information Inquiry
The function InquiryCardInformation allows you to check the brand and function of a card by submitting the
card number.
Request:
…<a1:InquiryCardInformation>
		<ns2:StoreId>123456789</ns2:StoreId>
Web Service API Integration Guide                                                       6. Additional Web Service actions 43
		<ns2:CardNumber>5413…0002</ns2:CardNumber>
</a1:InquiryCardInformation>…
Response:
…<ipgapi:CardInformation>
		<ns2:Brand>MASTERCARD</ns2:Brand>
		<ns2:CardFunction>credit</ns2:CardFunction>
		<ns2:Country>USA</ns2:Country>
		<ns2:Corporate>CORPORATE</ns2:Corporate>
</ipgapi:CardInformation>
</ipgapi:IPGApiActionResponse>


6.1.22 Basket Information and Product Catalogue

6.1.23 Basket information in transaction messages
The following example shows how you can use the basket parameters to document in the transaction what has
been sold.
<ns5:IPGApiOrderRequest
			 xmlns:ns5=”http://ipg-online.com/ipgapi/schemas/ipgapi”
			 xmlns:ns2=”http://ipg-online.com/ipgapi/schemas/v1”
			 xmlns:ns3=”http://ipg-online.com/ipgapi/schemas/a1”>
		<ns2:Transaction>
			 <ns2:CreditCardTxType>
				          <ns2:Type>sale</ns2:Type>
			 </ns2:CreditCardTxType>
			 <ns2:CreditCardData>
				          <ns2:CardNumber>4035..…4977</ns2:CardNumber>
				          <ns2:ExpMonth>12</ns2:ExpMonth>
				          <ns2:ExpYear>14</ns2:ExpYear>
			 </ns2:CreditCardData>
			 <ns2:Payment>
				          <ns2:ChargeTotal>1</ns2:ChargeTotal>
				          <ns2:Currency>EUR</ns2:Currency>
			 </ns2:Payment>
			 <ns2:TransactionDetails>
				          <ns2:OrderId>68d4a595-fd58-4859-83cd-1ae13962a3ac</ns2:OrderId>
			 </ns2:TransactionDetails>
			 <ns2:Basket>
				          <ns2:Item>
					 <ns2:ID>product ID xyz</ns2:ID>
					             <ns2:Description>description of abc</ns2:Description>
					 <ns2:ChargeTotal>11</ns2:ChargeTotal>
					 <ns2:Currency>EUR</ns2:Currency>
					 <ns2:Quantity>5</ns2:Quantity>
					 <ns2:Option>
						                  <ns2:Name>colour</ns2:Option>
						                  <ns2:Choice>blue</ns2:Choice>
					 </ns2:Option>
					 <ns2:Option>
						                  <ns2:Name>size</ns2:Option>
						                  <ns2:Choice>large</ns2:Choice>
					 </ns2:Option>
				          </ns2:Item>
			 </ns2:Basket>
  </ns2:Transaction>
</ns5:IPGApiOrderRequest>
Web Service API Integration Guide                                               6. Additional Web Service actions 44
6.1.24 Setting up a Product Catalogue
You can store basic information about the products you sell in the following way:
<ns5:IPGApiActionRequest
			 xmlns:ns5=”http://ipg-online.com/ipgapi/schemas/ipgapi”
			 xmlns:ns2=”http://ipg-online.com/ipgapi/schemas/v1”
			 xmlns:ns3=”http://ipg-online.com/ipgapi/schemas/a1”>
  <ns3:Action>
		<ns3:ManageProducts>
			 <ns3:Function>store</ns3:Function>
			 <ns3:Product>
				          <ns3:ProductID>product ID xyz</ns3:ProductID>
				          <ns2:ChargeTotal>2</ns2:ChargeTotal>
				          <ns2:Currency>EUR</ns2:Currency>
				          <ns3:OfferStarts>
					 2014-12-27T13:29:41.000+01:00
				          </ns3:OfferStarts>
				          <ns3:OfferEnds>
					 2015-09-19T14:29:41.000+02:00
				          </ns3:OfferEnds>
				          <ns2:Option>
					 <ns2:Name>colour</ns2:Option>
					 <ns2:Choice>blue</ns2:Choice>
				          </ns2:Option>
				          <ns2:Option>
					 <ns2:Name>size</ns2:Option>
					 <ns2:Choice>large</ns2:Choice>
				          </ns2:Option>
			 </ns3:Product>
		</ns3:ManageProducts>
  </ns3:Action>
</ns5:IPGApiActionRequest>
OfferStarts and OfferEnds are optional and can be used to restrict the visibility of the related products in custom
applications but they will not restrict the possibility of a sale. There are further optional fields Description,
OptionName and Name. Please take a look at the a1.xsd in the appendix of this document.
The function display shows the requested product with every characteristics.
<ns5:IPGApiActionRequest
			 xmlns:ns5=”http://ipg-online.com/ipgapi/schemas/ipgapi”
			 xmlns:ns2=”http://ipg-online.com/ipgapi/schemas/v1”
			 xmlns:ns3=”http://ipg-online.com/ipgapi/schemas/a1”>
		<ns3:Action>
			 <ns3:ManageProducts>
				          <ns3:Function>display</ns3:Function>
				          <ns3:Product>
					 <ns3:ProductID>product ID xyz</ns3:ProductID>
				          </ns3:Product>
			 </ns3:ManageProducts>
		</ns3:Action>
</ns5:IPGApiActionRequest>
The function delete can be used to set the available stock of a product to zero.
<ns5:IPGApiActionRequest
			 xmlns:ns5=”http://ipg-online.com/ipgapi/schemas/ipgapi”
			 xmlns:ns2=”http://ipg-online.com/ipgapi/schemas/v1”
			 xmlns:ns3=”http://ipg-online.com/ipgapi/schemas/a1”>
		<ns3:Action>
			 <ns3:ManageProducts>
Web Service API Integration Guide                                                      6. Additional Web Service actions 45
				          <ns3:Function>delete</ns3:Function>
				          <ns3:Product>
					 <ns3:ProductID>product ID xyz</ns3:ProductID>
				          </ns3:Product>
			 </ns3:ManageProducts>
		</ns3:Action>
</ns5:IPGApiActionRequest>


6.1.25 Manage Product Stock
For every product stock function, the product ID and given options need to exist in your Product Catalogue.
After you have installed a product, you can fill the product stock with the function add.
<ns5:IPGApiActionRequest
			 xmlns:ns5=http://ipg-online.com/ipgapi/schemas/ipgapi
			 xmlns:ns2=”http://ipg-online.com/ipgapi/schemas/v1”
			 xmlns:ns3=”http://ipg-online.com/ipgapi/schemas/a1”>
  <ns3:Action>
		<ns3:ManageProductStock>
			 <ns3:Function>add</ns3:Function>
			 <ns3:ProductStock>
				          <ns3:ProductID>product ID xyz</ns3:ProductID>
				          <ns2:Option>
					 <ns2:Name>colour</ns2:Option>
					 <ns2:Choice>blue</ns2:Choice>
				          </ns2:Option>
				          <ns2:Option>
					 <ns2:Name>size</ns2:Option>
					 <ns2:Choice>large</ns2:Choice>
				          </ns2:Option>
				          <ns3:Quantity>13</ns3:Quantity>
			 </ns3:ProductStock>
		</ns3:ManageProductStock>
  </ns3:Action>
</ns5:IPGApiActionRequest>
The function substract works in the same way, but will only change the quantity, if the difference will not be
negative. If you want to set the quantity to zero you can use the function delete described above.


6.1.26 Sale transactions using product stock
After you have set up the product stock, you can use it to verify if there are enough items on stock for a transaction.
A succesful transaction will then substract the quantity. If the product stock contains less than the requested
quantity, the transaction will be rejected without any changes to the product stock.
To use this function, add <ns2:ProductStock>check</ns2:ProductStock> to Basket.
<ns5:IPGApiOrderRequest
			 xmlns:ns5=”http://ipg-online.com/ipgapi/schemas/ipgapi”
			 xmlns:ns2=”http://ipg-online.com/ipgapi/schemas/v1”
			 xmlns:ns3=”http://ipg-online.com/ipgapi/schemas/a1”>
		<ns2:Transaction>
			 <ns2:CreditCardTxType>
				          <ns2:Type>sale</ns2:Type>
			 </ns2:CreditCardTxType>
			 <ns2:CreditCardData>
				          <ns2:CardNumber>4035....4977</ns2:CardNumber>
				          <ns2:ExpMonth>12</ns2:ExpMonth>
				          <ns2:ExpYear>14</ns2:ExpYear>

Web Service API Integration Guide                                                       6. Additional Web Service actions 46
				          </ns2:CreditCardData>
				          <ns2:Payment>
					 <ns2:ChargeTotal>1</ns2:ChargeTotal>
					 <ns2:Currency>EUR</ns2:Currency>
				          </ns2:Payment>
				          <ns2:TransactionDetails>
					 <ns2:OrderId>68d4a595-fd58-4859-83cd-1ae13962a3ac</ns2:OrderId>
				          </ns2:TransactionDetails>
				          <ns2:Basket>
				          <ns2:ProductStock>check</ns2:ProductStock>
					 <ns2:Item>
						                  <ns2:ID>product ID xyz</ns2:ID>
						                  <ns2:Description>description of abc</ns2:Description>
						                  <ns2:ChargeTotal>11</ns2:ChargeTotal>
					 <ns2:Currency>EUR</ns2:Currency>
					 <ns2:Quantity>5</ns2:Quantity>
				          <ns2:Option>
					 <ns2:Name>colour</ns2:Option>
					 <ns2:Choice>blue</ns2:Choice>
				          </ns2:Option>
				          <ns2:Option>
					 <ns2:Name>size</ns2:Option>
					 <ns2:Choice>large</ns2:Choice>
				          </ns2:Option>
			 </ns2:Item>
		</ns2:Basket>
  </ns2:Transaction>
</ns5:IPGApiOrderRequest>




Web Service API Integration Guide                          6. Additional Web Service actions 47
7. Data Vault
With the Data Vault product option you can store sensitive cardholder data in an encrypted database in Authipay’s
data centre to use it for subsequent transactions without the need to store this data within your own systems.
If you have ordered this product option, the Web Service API offers you the following functions.
See further possibilities with the Data Vault product in the Integration Guide for the Connect solution.


7.1.1 Token Type Options
The type of token can be defined with the optional element TokenType, which can have 2 possible values :
“ONETIME” or “MULTIPAY”.
The default value (when no token type gets submitted) is MULTIPAY.
One time token (that are only valid for a specific time span) is an option for merchants, which work with tokens for
every transaction, no matter if the consumer registers or prefers to check out as a “guest”.
The following XML document represents an example of a request with included element
TokenType = MULTIPAY:
<?xml version=”1.0” encoding=”UTF-8”?>
<ns4:IPGApiOrderRequest
xmlns:ns4=”http://ipg-online.com/ipgapi/schemas/ipgapi”
xmlns:ns2=”http://ipg-online.com/ipgapi/schemas/v1”
xmlns:ns3=”http://ipg-online.com/ipgapi/schemas/a1”>
  <ns2:Transaction>
		<ns2:CreditCardTxType>
			 <ns2:StoreId>330995001</ns2:StoreId>
			 <ns2:Type>sale</ns2:Type>
		</ns2:CreditCardTxType>
		<ns2:CreditCardData>
			 <ns2:CardNumber>4035*****4977</ns2:CardNumber>
			 <ns2:ExpMonth>12</ns2:ExpMonth>
			 <ns2:ExpYear>28</ns2:ExpYear>
			 <ns2:CardCodeValue>XXX</ns2:CardCodeValue>
		</ns2:CreditCardData>
		<ns2:Payment>
			 <ns2:ChargeTotal>27.2</ns2:ChargeTotal>
			 <ns2:Currency>INR</ns2:Currency>
			 <ns2:TokenType>MULTIPAY</ns2:TokenType>
		</ns2:Payment>
		<ns2:TransactionDetails>
			 <ns2:TransactionOrigin>ECI</ns2:TransactionOrigin>
		</ns2:TransactionDetails>
  </ns2:Transaction>
</ns4:IPGApiOrderRequest>
The following XML document represents an example of a request with included element TokenType = ONETIME:
<?xml version=”1.0” encoding=”UTF-8”?>
<ns4:IPGApiOrderRequest
xmlns:ns4=”http://ipg-online.com/ipgapi/schemas/ipgapi”
xmlns:ns2=”http://ipg-online.com/ipgapi/schemas/v1”
xmlns:ns3=”http://ipg-online.com/ipgapi/schemas/a1”>
  <ns2:Transaction>
		<ns2:CreditCardTxType>
			 <ns2:StoreId>330995001</ns2:StoreId>
			 <ns2:Type>sale</ns2:Type>
		</ns2:CreditCardTxType>

Web Service API Integration Guide                                                                          7. Data Vault 48
		<ns2:CreditCardData>
			 <ns2:CardNumber>4035*****4977</ns2:CardNumber>
			 <ns2:ExpMonth>12</ns2:ExpMonth>
			 <ns2:ExpYear>28</ns2:ExpYear>
			 <ns2:CardCodeValue>XXX</ns2:CardCodeValue>
		</ns2:CreditCardData>
		<ns2:Payment>
			 <ns2:ChargeTotal>27.2</ns2:ChargeTotal>
			 <ns2:Currency>INR</ns2:Currency>
		<ns2:TokenType>ONETIME</ns2:TokenType>
		</ns2:Payment>
		<ns2:TransactionDetails>
			 <ns2:TransactionOrigin>ECI</ns2:TransactionOrigin>
		</ns2:TransactionDetails>
  </ns2:Transaction>
</ns4:IPGApiOrderRequest>
For merchants, which do not wish to define the token themselves, but want it to be generated and returned, the
element AssignToken should be set to ‘true’ and no HostedDataId needs to be sent in that case.
The following XML document represents an example of a request for getting the token generated by Gateway,
with the element AssignToken = true:
<soap:Envelope xmlns:soap=”http://schemas.xmlsoap.org/soap/envelope/”>
  <soap:Header/>
  <soap:Body>
		<ns5:IPGApiOrderRequest
xmlns:ns5=”http://ipg-online.com/ipgapi/schemas/ipgapi”
xmlns:ns2=”http://ipg-online.com/ipgapi/schemas/v1”
xmlns:ns3=”http://ipg-online.com/ipgapi/schemas/a1”>
  <ns2:Transaction>
		<ns2:CreditCardTxType>
			 <ns2:StoreId>2209905999</ns2:StoreId>
			 <ns2:Type>sale</ns2:Type>
		</ns2:CreditCardTxType>
		ns2:CreditCardData>
			 <ns2:CardNumber>4257******0111</ns2:CardNumber>
			 <ns2:ExpMonth>12</ns2:ExpMonth>
			 <ns2:ExpYear>17</ns2:ExpYear>
			 <ns2:CardCodeValue>XXX</ns2:CardCodeValue>
		</ns2:CreditCardData>
		<ns2:Payment>
			 <ns2:ChargeTotal>700.00</ns2:ChargeTotal>
			 <ns2:Currency>GBP</ns2:Currency>
			 <ns2:AssignToken>true</ns2:AssignToken>
		</ns2:Payment>
		<ns2:TransactionDetails>
			 <ns2:TransactionOrigin>MOTO</ns2:TransactionOrigin>
		</ns2:TransactionDetails>
		<ns2:Billing>
			      <ns2:Address1>Flat 412a 123 London Rd</ns2:Address1>
			 <ns2:City>London</ns2:City>
			 <ns2:Zip>CH488AQ</ns2:Zip>
			 <ns2:Country>GB</ns2:Country>
		</ns2:Billing>
  </ns2:Transaction>
</ns5:IPGApiOrderRequest>
  </soap:Body>
</soap:Envelope>


Web Service API Integration Guide                                                                   7. Data Vault 49
The following XML document represents an example of a response with the token generated in the element
HostedDataID:
<SOAP-ENV:Envelope xmlns:SOAPENV=”
http://schemas.xmlsoap.org/soap/envelope/”>
  <SOAP-ENV:Header/>
  <SOAP-ENV:Body>
			 <ipgapi:IPGApiOrderResponse
xmlns:ipgapi=”http://ipg-online.com/ipgapi/schemas/ipgapi”
xmlns:a1=”http://ipg-online.com/ipgapi/schemas/a1”
xmlns:v1=”http://ipg-online.com/ipgapi/schemas/v1”>
  <ipgapi:ApprovalCode>Y:609287:8383366115:PPXP:006295</ipgapi:ApprovalCode>
  <ipgapi:AVSResponse>PPX</ipgapi:AVSResponse>
  <ipgapi:Brand>VISA</ipgapi:Brand>
  <ipgapi:Country>ESP</ipgapi:Country>
  <ipgapi:CommercialServiceProvider>CARDNET</ipgapi:CommercialServiceProvider>
  <ipgapi:OrderId>A-0dd18b32-bc19-40ea-8173-
80537093b18f</ipgapi:OrderId>
  <ipgapi:IpgTransactionId>8383366115</ipgapi:IpgTransactionId>
  <ipgapi:PaymentType>CREDITCARD</ipgapi:PaymentType>
  <ipgapi:ProcessorApprovalCode>609287</ipgapi:ProcessorApprovalCode>
  <ipgapi:ProcessorCCVResponse>P</ipgapi:ProcessorCCVResponse>
  <ipgapi:ProcessorReferenceNumber>702514006295</ipgapi:ProcessorReferenceNumber>
  <ipgapi:ProcessorResponseCode>00</ipgapi:ProcessorResponseCode>
  <ipgapi:ProcessorResponseMessage>Function performed errorfree</
ipgapi:ProcessorResponseMessage>
  <ipgapi:TDate>1485354544</ipgapi:TDate>
  <ipgapi:TDateFormatted>2017.01.25 15:29:04 (MEZ)</ipgapi:TDateFormatted>
  <ipgapi:TerminalID>IPGCNP00</ipgapi:TerminalID>
  <ipgapi:TransactionResult>APPROVED</ipgapi:TransactionResult>
  <ipgapi:TransactionTime>1485354544</ipgapi:TransactionTime>
  <ipgapi:HostedData>
		<ipgapi:HostedDataID>7F98D913-85CF-4B88-B994-B59CB0D4AEB2</ipgapi:HostedDataID>
  </ipgapi:HostedData>
  </ipgapi:IPGApiOrderResponse>
</SOAP-ENV:Body>
</SOAP-ENV:Envelope>


7.1.2 Store or update payment information when performing
       a transaction
Additionally send the parameter HostedDataID together with the transaction data as a unique identification for the
payment information in this transaction. Depending on the payment type, credit card number and expiry date or
account number and bank code will be stored under this ID. In cases where the submitted ‚HostedDataID’ already
exists for your store, the stored payment information will be updated.
<ipgapi:IPGApiOrderRequest
		xmlns:v1=”http://ipg-online.com/ipgapi/schemas/v1”
		xmlns:ipgapi=”http://ipg-online.com/ipgapi/schemas/ipgapi”>
		<v1:Transaction>
			 <v1:CreditCardTxType>
				          <v1:Type>sale</v1:Type>
			 </v1:CreditCardTxType>
			 <v1:CreditCardData>
				          <v1:CardNumber>4111********1111</v1:CardNumber>
				          <v1:ExpMonth>12</v1:ExpMonth>
				          <v1:ExpYear>07</v1:ExpYear>
			 </v1:CreditCardData>

Web Service API Integration Guide                                                                    7. Data Vault 50
			 <v1:Payment>
				          <v1:HostedDataID>
					             HDID customer 1234567
				          </v1:HostedDataID>
					 <v1:ChargeTotal>19.00</v1:ChargeTotal>
					 <v1:Currency>978</v1:Currency>
			 </v1:Payment>
		</v1:Transaction>
</ipgapi:IPGApiOrderRequest>
The record is only being stored if the authorisation of the payment transaction is successful and your Store has
been setup for this service.
If you want to assign multiple IDs to the same payment information (e.g. because your customer has several
contracts or accounts with you where they want to used the same card for payment), you can include the
parameter HostedDataID multiple times with different values.


7.1.3 Store payment information from an approved transaction
Payment information can also be stored referring to a previously approved transaction
<ns4:IPGApiActionRequest
			 xmlns:ns4=”http://ipg-online.com/ipgapi/schemas/ipgapi”
			 xmlns:ns2=”http://ipg-online.com/ipgapi/schemas/a1”
			 xmlns:ns3=”http://ipg-online.com/ipgapi/schemas/v1”>
		<ns2:Action>
			 <ns2:StoreHostedData>
				          <ns2:DataStorageItem>
					 <ns2:OrderId>1234567890</ns2:OrderId>
				          <ns2:HostedDataID>
					 4e72021b-d155-4062-872a-30228c0fe023
				          </ns2:HostedDataID>
			 </ns2:DataStorageItem>
		</ns2:StoreHostedData>
  </ns2:Action>
</ns4:IPGApiActionRequest>
This action stores the payment information of the transaction with the order id 1234567890. The transaction must
be an approved transaction, otherwise this action fails.


7.1.4 Initiate payment transactions using stored data
If you stored cardholder information using the Data Vault product, you can perform transactions using the
‚HostedDataID’ without the need to pass the credit card or bank account data again.
Please note that it is not allowed to store the card code (in most cases on the back of the card) so that for credit
card transactions, the cardholder still needs to enter this value. For the checkout process in your web shop, we
recommend that you also store the last four digits of the credit card number on your side and display it when it
comes to payment. In that way the cardholder can see which of his maybe several cards has been registered in
your shop and will be used for this payment transaction.
<ipgapi:IPGApiOrderRequest
			 xmlns:v1=”http://ipg-online.com/ipgapi/schemas/v1”
			 xmlns:ipgapi=”http://ipg-online.com/ipgapi/schemas/ipgapi”>
			 <v1:Transaction>
				          <v1:CreditCardTxType>
					 <v1:Type>sale</v1:Type>
				          </v1:CreditCardTxType>
				          <v1:Payment>
					 <v1:HostedDataID>
						                  HDID customer 1234567

Web Service API Integration Guide                                                                         7. Data Vault 51
				          </v1:HostedDataID>
				          <v1:ChargeTotal>19.00</v1:ChargeTotal>
				          <v1:Currency>978</v1:Currency>
			 </v1:Payment>
		</v1:Transaction>
</ipgapi:IPGApiOrderRequest>


7.1.5 Store payment information without performing
       a transaction at the same time
Besides the possibility to store new records when performing a payment transaction, you can store payment
information using an Action Request. In that way it is also possible to upload multiple records at once. The
following example shows the upload for a record with credit card data as well as the direct debit data. Please note
that also in this case, existing records will be updated if the HostedDataID is the same.
<soapenv:Envelope xmlns:soapenv=”http://schemas.xmlsoap.org/soap/envelope/”
xmlns:a1=”http://ipg-online.com/ipgapi/schemas/a1”
xmlns:ipg=”http://ipg-online.com/ipgapi/schemas/ipgapi”
xmlns:v1=”http://ipg-online.com/ipgapi/schemas/v1”>
  <soapenv:Header/>
  <soapenv:Body>
		<ipg:IPGApiActionRequest>
			 <a1:Action>
			 <a1:StoreHostedData>
				          <a1:StoreId>120992233</a1:StoreId>
				          <a1:DataStorageItem>
				          <a1:DE_DirectDebitData>
			 <v1:IBAN>DE34*************1604</v1:IBAN>
			 <v1:MandateReference>12/12/19</v1:MandateReference>
				          </a1:DE_DirectDebitData>
				          <a1:AssignToken>true</a1:AssignToken>
				          <a1:BillingName>Test User</a1:BillingName>
			 </a1:DataStorageItem>
			 <a1:DataStorageItem>
				          <a1:CreditCardData>
					 <v1:CardNumber>5426******4979</v1:CardNumber>
					 <v1:ExpMonth>12</v1:ExpMonth>
				          <v1:ExpYear>22</v1:ExpYear>
			 <v1:CardCodeValue>XXX</v1:CardCodeValue>
			 </a1:CreditCardData>
			 <a1:AssignToken>true</a1:AssignToken>
		</a1:DataStorageItem>
  </a1:StoreHostedData>
</a1:Action>
</ipg:IPGApiActionRequest>
</soapenv:Body>
  </soapenv:Envelope>
The result for a successful storage contains the HostedDataID:
		<ipgapi:IPGApiActionResponse
xmlns:a1=”http://ipg-online.com/ipgapi/schemas/a1”
xmlns:ipgapi=”http://ipg-online.com/ipgapi/schemas/ipgapi”
xmlns:v1=”http://ipg-online.com/ipgapi/schemas/v1”>
			 <ipgapi:successfully>true</ipgapi:successfully>
			 <ipgapi:DataStorageItem>
				          <a1:DE_DirectDebitData>
					 <v1:BIC>PBNKDEFFXXX</v1:BIC>
					 <v1:IBAN>DE34***************1604</v1:IBAN>

Web Service API Integration Guide                                                                     7. Data Vault 52
					 <v1:BankCode>50010060</v1:BankCode>
					 <v1:AccountNumber>32121604</v1:AccountNumber>
				          </a1:DE_DirectDebitData>
				          <a1:HostedDataID>0EFC3495-4F4F-4A1B-BCA5-
C81F80D834C0</a1:HostedDataID>
			 </ipgapi:DataStorageItem>
			 <ipgapi:DataStorageItem>
				          <a1:CreditCardData>
					 <v1:CardNumber>5426******4979</v1:CardNumber>
					 <v1:ExpMonth>12</v1:ExpMonth>
				          <v1:ExpYear>22</v1:ExpYear>
				          <v1:Brand>MASTERCARD</v1:Brand>
				          </a1:CreditCardData>
			 <a1:HostedDataID>136DFF32-5BE3-4FCE-A537-6F491DA31039</a1:HostedDataID>
				          </ipgapi:DataStorageItem>
			 </ipgapi:IPGApiActionResponse>
Example of Request to store the data under given hostedDataID:
<SOAP-ENV:Envelope xmlns:SOAPENV=”
http://schemas.xmlsoap.org/soap/envelope/”>
  <SOAP-ENV:Header/>
  <SOAP-ENV:Body>
		<ns4:IPGApiActionRequest
xmlns:ns4=”http://ipg-online.com/ipgapi/schemas/ipgapi”
xmlns:ns2=”http://ipg-online.com/ipgapi/schemas/a1”
xmlns:ns3=”http://ipg-online.com/ipgapi/schemas/v1”>
		<ns2:Action>
			 <ns2:StoreHostedData>
				          <ns2:DataStorageItem>
					 <ns2:CreditCardData>
						                  <ns3:CardNumber>4035******4977</ns3:CardNumber>
						                  <ns3:ExpMonth>12</ns3:ExpMonth>
						                  <ns3:ExpYear>22</ns3:ExpYear>
					 </ns2:CreditCardData>
<ns2:HostedDataID>2a356872-54c7-4d09-800c-0be221e72edb</ns2:HostedDataID>
		</ns2:DataStorageItem>
		<ns2:DataStorageItem>
			 <ns2:DE_DirectDebitData>
				          <ns3:BankCode>50010060</ns3:BankCode>
				          <ns3:AccountNumber>32121604</ns3:AccountNumber>
				          </ns2:DE_DirectDebitData>
				          <ns2:HostedDataID>6f6de992-e484-4a68-a520-
5f3a32e46fad</ns2:HostedDataID>
				          <ns2:BillingName>Dummy Owner</ns2:BillingName>
			 </ns2:DataStorageItem>
  </ns2:StoreHostedData>
  </ns2:Action>
</ns4:IPGApiActionRequest>
</SOAP-ENV:Body>
</SOAP-ENV:Envelope>
The result for a successful storage contains the value true for the parameter <ns4:successfully>:
<ns4:IPGApiActionResponse
			 xmlns:ns4=”http://ipg-online.com/ipgapi/schemas/ipgapi”
			 xmlns:ns2=”http://ipg-online.com/ipgapi/schemas/a1”
			 xmlns:ns3=”http://ipg-online.com/ipgapi/schemas/v1”>
  <ns4:successfully>true</ns4:successfully>
</ns4:IPGApiActionResponse>

Web Service API Integration Guide                                                                   7. Data Vault 53
In cases where one or more records have not been stored successfully, the corresponding Hosted Data IDs are
marked in the result:
<ns4:IPGApiActionResponse
			 xmlns:ns4=”http://ipg-online.com/ipgapi/schemas/ipgapi”
			 xmlns:ns2=”http://ipg-online.com/ipgapi/schemas/a1”
			 xmlns:ns3=”http://ipg-online.com/ipgapi/schemas/v1”>
<ns4:successfully>true</ns4:successfully>
<ns2:Error Code=”SGSDAS-020300”>
			 <ns2:ErrorMessage>
				          Could not store the hosted data id:
				          691c7cb3-a752-4d6d-abde-83cad63de258.
				          Reason: An internal error has occured while
				          processing your request
			 </ns2:ErrorMessage>
		</ns2:Error>
</ns4:IPGApiActionResponse>
Example of response in case of missing mandatory parameter:
<ipgapi:IPGApiActionResponse
xmlns:ipgapi=”http://ipg-online.com/ipgapi/schemas/ipgapi”
xmlns:a1=”http://ipg-online.com/ipgapi/schemas/a1”
xmlns:v1=”http://ipg-online.com/ipgapi/schemas/v1”>
		<ipgapi:successfully>true</ipgapi:successfully>
		<a1:Error>
			      <a1:ErrorMessage>Billing Name is mandatory while creating
hosted data for SEPA direct debit.
		</a1:ErrorMessage>
  </a1:Error>
<ipgapi:IPGApiActionResponse>
Example of response in case where hosted data has not been stored successfully:
<ipgapi:IPGApiActionResponse xmlns:ipgapi=”http://ipgonline.
com/ipgapi/schemas/ipgapi” xmlns:a1=”http://ipgonline.
com/ipgapi/schemas/a1” xmlns:v1=”http://ipgonline.
com/ipgapi/schemas/v1”>
		<ipgapi:successfully>true</ipgapi:successfully>
		<a1:Error>
			      <a1:ErrorMessage>hosted data id:
E99F19BB9D4F4503B8908D9F86C183F8. Invalid expiration date: CreditCard
[cardNumber=492181...2311, expirationMonth=3, expirationYear=2020,
trackData=(masked), trackOneData=(masked), trackTwoData=(masked),
cardCodeValue=(len:null, isChipCard=null,
enrichedCreditCard=EnrichedCreditCard [typeString=null, issuername=null,
country=null, binCreditCardTypes=[], creditCardInformationList=[],
creditCardType=null, cardFunction=null, commercialCardType=null]]
</a1:ErrorMessage>
			 </a1:Error>
		</ipgapi:IPGApiActionResponse>


7.1.6 Avoid duplicate cardholder data for multiple records
To avoid customers using the same cardholder data for multiple user accounts, the additional tag
DeclineHostedDataDuplicates can be sent along with the request. The valid values for this tag are ‘true’/’false’. If
the value for this tag is set to ‘true’ and the cardholder data in the request is already found to be associated with
another ‘hosteddataid’, the transaction will be declined.




Web Service API Integration Guide                                                                         7. Data Vault 54
7.1.7 Display stored records
Existing records can be displayed using the action Display:
<ns4:IPGApiActionRequest
			 xmlns:ns4=”http://ipg-online.com/ipgapi/schemas/ipgapi”
			 xmlns:ns2=”http://ipg-online.com/ipgapi/schemas/v1”
			 xmlns:ns3=”http://ipg-online.com/ipgapi/schemas/a1”>
		<ns3:Action>
			 <ns3:StoreHostedData>
				          <ns3:DataStorageItem>
					 <ns3:Function>display</ns3:Function>
					 <ns3:HostedDataID>
						                   d56feaaf-2d96-4159-8fd6-887e07fc9052
					 </ns3:HostedDataID>
				          </ns3:DataStorageItem>
			 </ns3:StoreHostedData>
		</ns3:Action>
</ns4:IPGApiActionRequest>
The response contains the stored information. For security reasons, only the first 6 and last 4 digits of credit card
numbers are being sent back.
<ns4:IPGApiActionResponse
			 xmlns:ns4=”http://ipg-online.com/ipgapi/schemas/ipgapi”
			 xmlns:ns2=”http://ipg-online.com/ipgapi/schemas/a1”
			 xmlns:ns3=”http://ipg-online.com/ipgapi/schemas/v1”>
		<ns4:successfully>true</ns4:successfully>
		<ns4:DataStorageItem>
			 <ns2:CreditCardData>
				          <ns3:CardNumber>4035*****4977</ns3:CardNumber>
				          <ns3:ExpMonth>12</ns3:ExpMonth>
				          <ns3:ExpYear>12</ns3:ExpYear>
			 </ns2:CreditCardData>
			 <ns2:HostedDataID>
				          d56feaaf-2d96-4159-8fd6-887e07fc9052
			 </ns2:HostedDataID>
		</ns4:DataStorageItem>
</ns4:IPGApiActionResponse>
If the Hosted Data ID does not exist, the API response indicates an error:
<ns4:IPGApiActionResponse
			 xmlns:ns4=”http://ipg-online.com/ipgapi/schemas/ipgapi”
			 xmlns:ns2=”http://ipg-online.com/ipgapi/schemas/a1”
			 xmlns:ns3=”http://ipg-online.com/ipgapi/schemas/v1”>
		<ns4:successfully>true</ns4:successfully>
		<ns2:Error Code=”SGSDAS-020301”>
			 <ns2:ErrorMessage>
				          Hosted data id:
				          6c814261-a843-49fb-bacd-1411d3780286 not found.
			 </ns2:ErrorMessage>
		</ns2:Error>
</ns4:IPGApiActionResponse>
The value successfully contains false, only if the data vault can’t determined because the request finished in
an error.




Web Service API Integration Guide                                                                         7. Data Vault 55
7.1.8 Delete existing records
The action “Delete” allows you to remove data records that are no longer needed:
<ns4:IPGApiActionRequest
			 xmlns:ns4=”http://ipg-online.com/ipgapi/schemas/ipgapi”
			 xmlns:ns2=”http://ipg-online.com/ipgapi/schemas/v1”
			 xmlns:ns3=”http://ipg-online.com/ipgapi/schemas/a1”>
		<ns3:Action>
			 <ns3:StoreHostedData>
			 <ns3:DataStorageItem>
				          <ns3:Function>delete</ns3:Function>
				          <ns3:HostedDataID>
					 9605c2d1-428c-4de2-940e-4bec4737ab5d
				          </ns3:HostedDataID>
			 </ns3:DataStorageItem>
		</ns3:StoreHostedData>
  </ns3:Action>
</ns4:IPGApiActionRequest>
A successful deletion will be confirmed with the following response:
<ns4:IPGApiActionResponse
			 xmlns:ns4=”http://ipg-online.com/ipgapi/schemas/ipgapi”
			 xmlns:ns2=”http://ipg-online.com/ipgapi/schemas/a1”
			 xmlns:ns3=”http://ipg-online.com/ipgapi/schemas/v1”>
  <ns4:successfully>true</ns4:successfully>
</ns4:IPGApiActionResponse>




Web Service API Integration Guide                                                  7. Data Vault 56
8. Global Choice™ and Dynamic Pricing
With Authipay’s Global Choice™, foreign customers have the choice to pay for goods and services purchased
online in their home currency when using their Visa or MasterCard credit card for the payment. The currency
conversion is quick and eliminates the need for customers to mentally calculate the estimated cost of the purchase
in their home currency.
International Visa and MasterCard eCommerce customers can make informed decisions about their online purchases
and eradicate any unexpected pricing or foreign exchange conversions on receipt of their monthly statements.
Another option for your foreign customers is to display all pricing within your online store in their home currency
using our Dynamic Pricing solution. This solution removes the need for your company to set pricing in any other
currency other than your home currency.
If your Store has been activated for one of these product options, you can use this Web Service API to request the
currency exchange rates for such transactions.


8.1.1 Exchange rate requests for Global Choice™
The following example shows a request to the Web Service API to request a card-related exchange rate.
<?xml version=”1.0” encoding=”UTF-8”?>
<SOAP-ENV:Envelope xmlns:SOAPENV=”
http://schemas.xmlsoap.org/soap/envelope/”>
		<SOAP-ENV:Header/>
		<SOAP-ENV:Body>
			 <ns5:IPGApiActionRequest
			 xmlns:ns5=”http://ipgonline.com/ipgapi/schemas/ipgapi”
			 xmlns:ns2=”http://ipgonline.com/ipgapi/schemas/v1”
			 xmlns:ns3=”http://ipgonline.com/ipgapi/schemas/a1”>
			 <ns3:Action>
				          <ns3:RequestCardRateForDCC>
					 <ns3:StoreId>110994125</ns3:StoreId>
					 <ns3:BIN>402939</ns3:BIN>
					 <ns3:BaseAmount>100.5</ns3:BaseAmount>
				          </ns3:RequestCardRateForDCC>
			 </ns3:Action>
		</ns5:IPGApiActionRequest>
  </SOAP-ENV:Body>
</SOAP-ENV:Envelope>
A successful respose is shown in the following answer:
• The status is given by <ipgapi:successfully>true</ipgapi:successfully>
• The response is wrapped within <ipgapi:CardRateForDCC>
<?xml version=”1.0” encoding=”UTF-8”?>
<SOAP-ENV:Envelope xmlns:SOAPENV=”
http://schemas.xmlsoap.org/soap/envelope/”>
		<SOAP-ENV:Header/>
		<SOAP-ENV:Body>
			 <ipgapi:IPGApiActionResponse
			 xmlns:ipgapi=”http://ipgonline.com/ipgapi/schemas/ipgapi”
			 xmlns:a1=”http://ipgonline.com/ipgapi/schemas/a1”
			 xmlns:v1=”http://ipgonline.com/ipgapi/schemas/v1”>
				          <ipgapi:successfully>true</ipgapi:successfully>
				          <ipgapi:CardRateForDCC>
						                  <v1:InquiryRateId>49150</v1:InquiryRateId>
						                  <a1:ForeignCurrencyCode>978</a1:ForeignCurrencyCode>
						                  <a1:ForeignAmount>130.33</a1:ForeignAmount>

Web Service API Integration Guide                                                 8. Global Choice™ and Dynamic Pricing 57
						                  <a1:ExchangeRate>1.2968</a1:ExchangeRate>
						                  <a1:DccOffered>true</a1:DccOffered>
						                  <a1:ExpirationTimestamp>2015-06-23T13:46:00.000+02:00
						                  </a1:ExpirationTimestamp>
						                  <a1:MarginRatePercentage>3.0000</a1:MarginRatePercentage>
						                  <a1:ExchangeRateSourceName>REUTERS WHOLESALE INTERBANK</
a1:ExchangeRateSourceName>
						                  <a1:ExchangeRateSourceTimestamp>2014-07-
14T12:46:00.000+02:00</a1:ExchangeRateSourceTimestamp>
				          </ipgapi:CardRateForDCC>
			 </ipgapi:IPGApiActionResponse>
		</SOAP-ENV:Body>
  </SOAP-ENV:Envelope>


8.1.2 Exchange rate requests for Dynamic Pricing
<?xml version=”1.0” encoding=”UTF-8”?>
<SOAP-ENV:Envelope xmlns:SOAPENV=”
http://schemas.xmlsoap.org/soap/envelope/”>
		<SOAP-ENV:Header/>
		<SOAP-ENV:Body>
			 <ns5:IPGApiActionRequest
			 xmlns:ns5=”http://ipgonline.com/ipgapi/schemas/ipgapi”
			 xmlns:ns2=”http://ipgonline.com/ipgapi/schemas/v1”
			 xmlns:ns3=”http://ipgonline.com/ipgapi/schemas/a1”>
				          <ns3:Action>
					 <ns3:RequestMerchantRateForDynamicPricing>
						                  <ns3:StoreId>110994125</ns3:StoreId>
						                  <ns3:ForeignCurrency>826</ns3:ForeignCurrency>
						                  <ns3:BaseAmount>100.5</ns3:BaseAmount>
					 </ns3:RequestMerchantRateForDynamicPricing>
				          </ns3:Action>
			 </ns5:IPGApiActionRequest>
		</SOAP-ENV:Body>
</SOAP-ENV:Envelope>
A successful respose is shown in the following answer from IPG:
• The status is given by <ipgapi:successfully>true</ipgapi:successfully>
• The response is wrapped within <ipgapi:MerchantRateForDynamicPricing>
<?xml version=”1.0” encoding=”UTF-8”?>
<SOAP-ENV:Envelope xmlns:SOAPENV=”
http://schemas.xmlsoap.org/soap/envelope/”>
		<SOAP-ENV:Header/>
		<SOAP-ENV:Body>
			 <ipgapi:IPGApiActionResponse
			 xmlns:ipgapi=”http://ipgonline. com/ipgapi/schemas/ipgapi”
			 xmlns:a1=”http://ipgonline. com/ipgapi/schemas/a1”
			 xmlns:v1=”http://ipgonline. com/ipgapi/schemas/v1”>
				          <ipgapi:successfully>true</ipgapi:successfully>
				          <ipgapi:MerchantRateForDynamicPricing>
					 <v1:InquiryRateId>49150</v1:InquiryRateId>
					 <a1:ForeignCurrencyCode>978</a1:ForeignCurrencyCode>
					 <a1:ForeignAmount>130.33</a1:ForeignAmount>
					 <a1:ExchangeRate>1.2968</a1:ExchangeRate>
					 <a1:DccOffered>true</a1:DccOffered>
					 <a1:ExpirationTimestamp>2015-06-23T13:46:00.000+02:00</
a1:ExpirationTimestamp>
					 <a1:MarginRatePercentage>3.0000</a1:MarginRatePercentage>
Web Service API Integration Guide                                 8. Global Choice™ and Dynamic Pricing 58
					 <a1:ExchangeRateSourceName>REUTERS WHOLESALE INTERBANK</
a1:ExchangeRateSourceName>
					 <a1:ExchangeRateSourceTimestamp>2014-07-14T12:46:00.000+02:00</
a1:ExchangeRateSourceTimestamp>
				          </ipgapi:MerchantRateForDynamicPricing>
			 </ipgapi:IPGApiActionResponse>
		</SOAP-ENV:Body>
</SOAP-ENV:Envelope>


8.1.3 Exchange rate responses
All rate responses share the same XML data-type, they are just wrapped in different parent-tags.
Common fields for all requests are the following:
• Success-status: given with <ipgapi:successfully>true</ipgapi:successfully>
• The ID of the request, given with <v1:InquiryRateId>49150</v1:InquiryRateId>
The latter InquiryRateId is later to be used to reference the rate request, when performing a transaction with a
converted transaction amount.


8.1.4 Conversion offering
A rate request with an offering returned is shown with the following example.
• The offering is denoted with <a1:DccOffered>true</a1:DccOffered>
• Each offering has associated timestamps, given as xml:date-time.
   o 	Source time <a1:ExchangeRateSourceTimestamp>2014-07-14T12:46:00.000+02:00</
       a1:ExchangeRateSourceTimestamp>
   o 	Expiration time <a1:ExpirationTimestamp>2015-06-23T13:46:00.000+02:00</
       a1:ExpirationTimestamp>
• T
   he source of the curreny-conversion is shown by
  <a1:ExchangeRateSourceName>REUTERS WHOLESALE INTERBANK</a1:ExchangeRateSourceName>
• Finally, the currency conversion results are given by the following fields
   o Foreign currency: <a1:ForeignCurrencyCode>978</a1:ForeignCurrencyCode>
   o Foreign amount: <a1:ForeignAmount>130.33</a1:ForeignAmount>
   o Exchange rate: <a1:ExchangeRate>1.2968</a1:ExchangeRate>
<?xml version=”1.0” encoding=”UTF-8”?>
<SOAP-ENV:Envelope xmlns:SOAPENV=”http://schemas.xmlsoap.org/soap/envelope/”>
		<SOAP-ENV:Header/>
		<SOAP-ENV:Body>
			 <ipgapi:IPGApiActionResponse
			 xmlns:ipgapi=”http://ipgonline.com/ipgapi/schemas/ipgapi”
			 xmlns:a1=”http://ipgonline.com/ipgapi/schemas/a1”
			 xmlns:v1=”http://ipgonline.com/ipgapi/schemas/v1”>
			 <ipgapi:successfully>true</ipgapi:successfully>
			 <ipgapi:CardRateForDCC>
				          <v1:InquiryRateId>49150</v1:InquiryRateId>
				          <a1:ForeignCurrencyCode>978</a1:ForeignCurrencyCode>
				          <a1:ForeignAmount>130.33</a1:ForeignAmount>
				          <a1:ExchangeRate>1.2968</a1:ExchangeRate>
				          <a1:DccOffered>true</a1:DccOffered>
				          <a1:ExpirationTimestamp>2015-06-23T13:46:00.000+02:00</
a1:ExpirationTimestamp>
				          <a1:MarginRatePercentage>3.0000</a1:MarginRatePercentage>
				          <a1:ExchangeRateSourceName>REUTERS WHOLESALE INTERBANK</
a1:ExchangeRateSourceName>

Web Service API Integration Guide                                                8. Global Choice™ and Dynamic Pricing 59
					 <a1:ExchangeRateSourceTimestamp>2014-07-14T12:46:00.000+02:00</
a1:ExchangeRateSourceTimestamp>
				          </ipgapi:CardRateForDCC>
			 </ipgapi:IPGApiActionResponse>
		</SOAP-ENV:Body>
</SOAP-ENV:Envelope>


8.1.5 Declined rate request
A rate request with a declined offering is shown with the following example.
• The declined offering is denoted with <a1:DccOffered>false</a1:DccOffered>
• Also for declined offerings an ID is returned: <v1:InquiryRateId>4051</v1:InquiryRateId>
<?xml version=”1.0” encoding=”UTF-8”?>
<SOAP-ENV:Envelope xmlns:SOAPENV=”
http://schemas.xmlsoap.org/soap/envelope/”>
		<SOAP-ENV:Header/>
		<SOAP-ENV:Body>
			 <ipgapi:IPGApiActionResponse
			 xmlns:ipgapi=”http://ipgonline.com/ipgapi/schemas/ipgapi”
			 xmlns:a1=”http://ipgonline.com/ipgapi/schemas/a1”
			 xmlns:v1=”http://ipgonline.com/ipgapi/schemas/v1”>
				          <ipgapi:successfully>true</ipgapi:successfully>
				          <ipgapi:MerchantRateForDynamicPricing>
					 <v1:InquiryRateId>4051</v1:InquiryRateId>
					 <a1:DccOffered>false</a1:DccOffered>
				          </ipgapi:MerchantRateForDynamicPricing>
			 </ipgapi:IPGApiActionResponse>
		</SOAP-ENV:Body>
</SOAP-ENV:Envelope>


8.1.6 Failed rate request
A rate request which couldn’t be processed successfully is shown by the following example:
• Failure-status: given with <ipgapi:successfully>false</ipgapi:successfully>
• The error-element:
   o The error-code by the Code attribute: <a1:Error Code=”SGS-27440”>
   o The human readable message: <a1:ErrorMessage>no amount given</a1:ErrorMessage>
<?xml version=”1.0” encoding=”UTF-8”?>
<SOAP-ENV:Envelope xmlns:SOAPENV=”
http://schemas.xmlsoap.org/soap/envelope/”>
		<SOAP-ENV:Header/>
		<SOAP-ENV:Body>
			 <ipgapi:IPGApiActionResponse
			 xmlns:ipgapi=”http://ipgonline.com/ipgapi/schemas/ipgapi”
			 xmlns:a1=”http://ipgonline.com/ipgapi/schemas/a1”
			 xmlns:v1=”http://ipgonline.com/ipgapi/schemas/v1”>
				          <ipgapi:successfully>false</ipgapi:successfully>
				          <a1:Error Code=”SGS-27440”>
					 <a1:ErrorMessage>no amount given</a1:ErrorMessage>
				          </a1:Error>
			 </ipgapi:IPGApiActionResponse>
		</SOAP-ENV:Body>
</SOAP-ENV:Envelope>



Web Service API Integration Guide                                               8. Global Choice™ and Dynamic Pricing 60
8.1.7 Global Choice™ transactions
For compliance reasons Authipay’s Global Choice can only be offered on transactions that take place in full at that
time (e.g. Sale, Refund) and not on any delayed settlement (e.g. pre/post auth, recurring) due to the fluctuation of
the rate of exchange.
Performing transactions with a converted amount involves the following steps
1. Perform a rate request as described in the sections above.
2.	Use the returned InquiryRateId to reference the conversion in the payment transaction message. Use the field
    DccApplied to denote whether the user has chosen to use the proposed conversion or not.

   Please note that an InquiryRateId may be used only once. After each transaction request, whether successful
   or not, regardless of the dccApplied setting used, a new rate has to be requested.
   Re-using a conversion-rate will result in an error message CORE-DCC-10, since the rate-inquiry is already
   associated with another transaction.


Step 1: Rate request
The Global Choice™ card-rate-request is shown here to give a complete example:
<?xml version=”1.0” encoding=”UTF-8”?>
<SOAP-ENV:Envelope xmlns:SOAPENV=”
http://schemas.xmlsoap.org/soap/envelope/”>
		<SOAP-ENV:Header/>
		<SOAP-ENV:Body>
			 <ns5:IPGApiActionRequest
			 xmlns:ns5=”http://ipgonline.com/ipgapi/schemas/ipgapi”
			 xmlns:ns2=”http://ipgonline.com/ipgapi/schemas/v1”
			 xmlns:ns3=”http://ipgonline.com/ipgapi/schemas/a1”>
				          <ns3:Action>
					 <ns3:RequestCardRateForDCC>
						                  <ns3:StoreId>110994125</ns3:StoreId>
						                  <ns3:BIN>419681</ns3:BIN>
						                  <ns3:BaseAmount>202.02</ns3:BaseAmount>
					 </ns3:RequestCardRateForDCC>
				          </ns3:Action>
			 </ns5:IPGApiActionRequest>
		</SOAP-ENV:Body>
</SOAP-ENV:Envelope>
The order to be used in a later transaction, the request has to be
• Successful: <ipgapi:successfully>true</ipgapi:successfully>
• With a returned conversion offering <a1:DccOffered>true</a1:DccOffered>
• Not expired <a1:ExpirationTimestamp>2015-06-23T12:46:00.000+02:00</
  a1:ExpirationTimestamp>
<?xml version=”1.0” encoding=”UTF-8”?>
<SOAP-ENV:Envelope xmlns:SOAPENV=”
http://schemas.xmlsoap.org/soap/envelope/”>
		<SOAP-ENV:Header/>
		<SOAP-ENV:Body>
			 <ipgapi:IPGApiActionResponse
				          xmlns:ipgapi=”http://ipgonline.com/ipgapi/schemas/ipgapi”
				          xmlns:a1=”http://ipgonline.com/ipgapi/schemas/a1”
				          xmlns:v1=”http://ipgonline.com/ipgapi/schemas/v1”>
					 <ipgapi:successfully>true</ipgapi:successfully>
					 <ipgapi:CardRateForDCC>
						                  <v1:InquiryRateId>8391</v1:InquiryRateId>
						                  <a1:ForeignCurrencyCode>978</a1:ForeignCurrencyCode>

Web Service API Integration Guide                                                 8. Global Choice™ and Dynamic Pricing 61
						                  <a1:ForeignAmount>261.98</a1:ForeignAmount>
						                  <a1:ExchangeRate>1.2968</a1:ExchangeRate>
						                  <a1:DccOffered>true</a1:DccOffered>
						                  <a1:ExpirationTimestamp>2015-06-23T12:46:00.000+02:00</
a1:ExpirationTimestamp>
						                  <a1:MarginRatePercentage>3.0000</a1:MarginRatePercentage>
						                  <a1:ExchangeRateSourceName>REUTERS WHOLESALE INTERBANK</
a1:ExchangeRateSourceName>
						                  <a1:ExchangeRateSourceTimestamp>2014-07-
14T12:46:00.000+02:00</a1:ExchangeRateSourceTimestamp>
					 </ipgapi:CardRateForDCC>
				          </ipgapi:IPGApiActionResponse>
			 </SOAP-ENV:Body>
</SOAP-ENV:Envelope>

Step 2: Using the conversion rate for the payment transaction
The Global Choice™ feature is selected by the element <ns2:InquiryRateReference>.
o The rate-id is used to reference the conversion rate: <ns2:InquiryRateId>8391</ns2:InquiryRateId>
o 	The users choice whether to apply the proposed rate is specified by:
    <ns2:DccApplied>true</ns2:DccApplied>
<SOAP-ENV:Envelope xmlns:SOAPENV=”
http://schemas.xmlsoap.org/soap/envelope/”>
		<SOAP-ENV:Header/>
		<SOAP-ENV:Body>
			 <ns5:IPGApiOrderRequest
			 xmlns:ns5=”http://ipgonline.com/ipgapi/schemas/ipgapi”
			 xmlns:ns2=”http://ipgonline.com/ipgapi/schemas/v1”
			 xmlns:ns3=”http://ipgonline.com/ipgapi/schemas/a1”>
				          <ns2:Transaction>
					 <ns2:CreditCardTxType>
						                  <ns2:StoreId>110994125</ns2:StoreId>
						                  <ns2:Type>return</ns2:Type>
					 </ns2:CreditCardTxType>
					 <ns2:Payment>
						                  <ns2:ChargeTotal>202.02</ns2:ChargeTotal>
						                  <ns2:Currency>826</ns2:Currency>
					 </ns2:Payment>
					 <ns2:TransactionDetails>
						                  <ns2:OrderId>API-Test 7dcb3590-2fa7-4702-afabadfd34390620
DCCTest::testSaleReturnDCC(110)</ns2:OrderId>
					 <ns2:InquiryRateReference>
						                  <ns2:InquiryRateId>8391</ns2:InquiryRateId>
						                  <ns2:DccApplied>true</ns2:DccApplied>
					 </ns2:InquiryRateReference>
					 </ns2:TransactionDetails>
				          </ns2:Transaction>
			 </ns5:IPGApiOrderRequest>
		</SOAP-ENV:Body>
</SOAP-ENV:Envelope>
For completeness the successful response is also shown here.
<?xml version=”1.0” encoding=”UTF-8”?>
<SOAP-ENV:Envelope xmlns:SOAPENV=”
http://schemas.xmlsoap.org/soap/envelope/”>
		<SOAP-ENV:Header/>
		<SOAP-ENV:Body>
			 <ipgapi:IPGApiOrderResponse
Web Service API Integration Guide                                          8. Global Choice™ and Dynamic Pricing 62
			 xmlns:ipgapi=”http://ipgonline.com/ipgapi/schemas/ipgapi”
			 xmlns:a1=”http://ipgonline.com/ipgapi/schemas/a1”
			 xmlns:v1=”http://ipgonline.com/ipgapi/schemas/v1”>
		<ipgapi:ApprovalCode>Y:000000:0014746213:PPXM:0000</ipgapi:ApprovalCode>
			 <ipgapi:AVSResponse>PPX</ipgapi:AVSResponse>
			 <ipgapi:Brand>VISA</ipgapi:Brand>
			 <ipgapi:Country>MLT</ipgapi:Country>
<ipgapi:CommercialServiceProvider>BOSMS</ipgapi:CommercialServiceProvider>
		<ipgapi:OrderId>API-Test 7dcb3590-2fa7-4702-afab-adfd34390620
DCCTest::testSaleReturnDCC(110)</ipgapi:OrderId>
		<ipgapi:PaymentType>CREDITCARD</ipgapi:PaymentType>
<ipgapi:ProcessorApprovalCode>000000</ipgapi:ProcessorApprovalCode>
		<ipgapi:ProcessorCCVResponse>M</ipgapi:ProcessorCCVResponse>
		<ipgapi:ProcessorResponseCode>00</ipgapi:ProcessorResponseCode>
<ipgapi:ProcessorResponseMessage>Authorised</ipgapi:ProcessorResponseMessage>
		<ipgapi:ReferencedTDate>1407154820</ipgapi:ReferencedTDate>
		<ipgapi:TDate>1407154821</ipgapi:TDate>
		<ipgapi:TDateFormatted>2014.08.04 14:20:21 (CEST)</ipgapi:TDateFormatted>
		<ipgapi:TerminalID>80000012</ipgapi:TerminalID>
		<ipgapi:TransactionResult>APPROVED</ipgapi:TransactionResult>
		<ipgapi:TransactionTime>1407154821</ipgapi:TransactionTime>
  </ipgapi:IPGApiOrderResponse>
</SOAP-ENV:Body>
</SOAP-ENV:Envelope>




Web Service API Integration Guide                          8. Global Choice™ and Dynamic Pricing 63
9. Payment URL
Payment URL is a functionality that allows you to provide a link to your customers (e.g. in an email invoice,
WhatsApp message, SMS, QR code, etc.) which then takes the customer to a Authipay-hosted page to securely
make the payment with their preferred payment method, whenever convenient for them.
This is especially useful in scenarios where goods get paid after delivery, where no goods get shipped at all
(e.g. final payment for trips that have been booked months ago) or for the payment of monthly bills.
You can also implement this functionality for unsuccessful purchases where the original payment transaction has
been declined so that you can proactively give your customer a second chance to make their purchase.
The Authipay Gateway provides
• The capability to request a Payment URL (link) for a specific amount through this Web Service API
• A hosted payment page where the customer can select the preferred payment method (based on the payment
  methods that are activated for your account) and make the payment
• A hosted result page that tells the customer if the payment was successful or not, including a Retry button
  where the customer can chose a different payment method in case the transaction was not successful
• Support for the specific fields that are required for Visa transactions with MCC 6012 in the UK


9.1.1 Payment URL creation
The request for a Payment URL includes transaction type, amount and currency as well as the language that shall
be used on the payment page that will be shown to the customer after accessing the URL.
The URL request stays valid for 182 days (182 * 24 * 3600 seconds) + 1 day (on which the URL was generated).
A merchant can override these settings by setting ‘Expiration element’ to desired value, which is an expiration
date in unix timestamp (in seconds, while IPG calculates it in milliseconds), this value shall be calculated by a
merchant himself.
<?xml version=”1.0” encoding=”UTF-8”?><SOAP-ENV:Envelope xmlns:SOAPENV=”
http://schemas.xmlsoap.org/soap/envelope/”>
		<SOAP-ENV:Header/>
		<SOAP-ENV:Body>
			 <ns5:IPGApiActionRequest
xmlns:ns5=”http://ipg-online.com/ipgapi/schemas/ipgapi”
xmlns:ns2=”http://ipg-online.com/ipgapi/schemas/a1”
xmlns:ns3=”http://ipg-online.com/ipgapi/schemas/v1”>
			 <ns2:Action>
				          <ns2:CreatePaymentURL>
					 <ns2:Transaction>
						                  <ns3:PaymentUrlTxType>
							                       <ns3:StoreId>120995000</ns3:StoreId>
							                       <ns3:Type>sale</ns3:Type>
						                  </ns3:PaymentUrlTxType>
						                  <ns3:Payment>
							                       <ns3:ChargeTotal>13.99</ns3:ChargeTotal>
							                       <ns3:Currency>EUR</ns3:Currency>
						                  </ns3:Payment>
						                  <ns3:TransactionDetails/>
						                  <ns3:ClientLocale>
							                       <ns3:Language>en</ns3:Language>
							                       <ns3:Country>GB</ns3:Country>
						                  </ns3:ClientLocale>
				          </ns2:Transaction>
			 </ns2:CreatePaymentURL>
		</ns2:Action>
  </ns5:IPGApiActionRequest>

Web Service API Integration Guide                                                                     9. Payment URL 64
</SOAP-ENV:Body>
</SOAP-ENV:Envelope>
The Response contains the Payment URL:
<?xml version=”1.0” encoding=”UTF-8”?><SOAP-ENV:Envelope xmlns:SOAPENV=”
http://schemas.xmlsoap.org/soap/envelope/”>
		<SOAP-ENV:Header/>
		<SOAP-ENV:Body>
			 <ipgapi:IPGApiActionResponse xmlns:ipgapi=
			 ”http://ipgonline.com/ipgapi/schemas/ipgapi”
			 xmlns:a1=”http://ipgonline.com/ipgapi/schemas/a1”
			 xmlns:v1=”http://ipgonline.com/ipgapi/schemas/v1”>
			 <ipgapi:successfully>true</ipgapi:successfully>
			 <ipgapi:OrderId>A-a22cd17f-0e50-4541-9404-159aa62815f0</ipgapi:OrderId>
			 <ipgapi:TransactionId>88963651</ipgapi:TransactionId>
			 <ipgapi:paymentUrl> https://test.ipgonline.com/connect/gateway/processing?s
torename=120995000&amp;oid=A-6d6f02ee-1020-4935-a8fd-e8d34e0ace03&amp;paymentUrlId=ef
c0d59b-7128-4d36-ba7d-0f7b642fd9ea</ipgapi:paymentUrl>
			 </ipgapi:IPGApiActionResponse>
		</SOAP-ENV:Body>
</SOAP-ENV:Envelope>
When the customer completed the payment transaction, the gateway can send a server-to-server transaction
notification to a defined Notification URL. Please contact your local support team to get your URL registered for
these notifications.


9.1.2 Payment URL deletion
For cases, when you need to prevent your customers to make a payment twice, you can use “DeletePaymentURL”
feature.
<?xml version=”1.0” encoding=”UTF-8”?><SOAP-ENV:Envelope xmlns:SOAPENV=”
http://schemas.xmlsoap.org/soap/envelope/”>
		<SOAP-ENV:Header/>
		<SOAP-ENV:Body>
			 <ns5:IPGApiActionRequest
xmlns:ns5=”http://ipg-online.com/ipgapi/schemas/ipgapi”
xmlns:ns2=”http://ipg-online.com/ipgapi/schemas/a1”
xmlns:ns3=”http://ipg-online.com/ipgapi/schemas/v1”>
<ns2:Action>
		<ns2:DeletePaymentURL>
			 <ns2:StoreId>120995000</ns2:StoreId>
			 <ns2:PaymentUrlID>e2fd0144-7644-4a5e-9e72-71cfa14c37ff</ns2:PaymentUrlID>
			 </ns2:DeletePaymentURL>
		</ns2:Action>
  </ns5:IPGApiActionRequest>
</SOAP-ENV:Body>
</SOAP-ENV:Envelope>
When processing a Payment URL an additional check ensures the Payment URL has not been voided, and if it has,
the URL will lead the customer to a screen that explains that the URL is no longer valid.


9.1.3 Payment URL custom text
For cases where you would like to add a free text to be shown above the payment options on the page
that the consumer will see when going to the URL for making the payment, you can submit an element
hostedPaymentPageText in your request to our Gateway:
<?xml version=”1.0” encoding=”UTF-8”?><SOAP-ENV:Envelope xmlns:SOAPENV=”
http://schemas.xmlsoap.org/soap/envelope/”>

Web Service API Integration Guide                                                                   9. Payment URL 65
		<SOAP-ENV:Header/>
		<SOAP-ENV:Body>
			 <ns5:IPGApiActionRequest
xmlns:ns5=”http://ipg-online.com/ipgapi/schemas/ipgapi”
xmlns:ns2=”http://ipg-online.com/ipgapi/schemas/a1”
xmlns:ns3=”http://ipg-online.com/ipgapi/schemas/v1”>
<ns2:Action>
  <ns2:CreatePaymentURL>
		<ns2:Transaction>
			 <ns3:PaymentUrlTxType>
				          <ns3:StoreId>120995000</ns3:StoreId>
				          <ns3:Type>sale</ns3:Type>
			 </ns3:PaymentUrlTxType>
		<ns3:Payment>
				          <ns3:Currency>EUR</ns3:Currency>
			 </ns3:Payment>
			 <ns3:TransactionDetails/>
				          <ns3:ClientLocale>
					 <ns3:Language>en</ns3:Language>
					 <ns3:Country>GB</ns3:Country>
				          </ns3:ClientLocale>
			 </ns2:Transaction>
			 <ns2:hostedPaymentPageText>This is a sample text
			 </ns2:hostedPaymentPageText>
		</ns2:CreatePaymentURL>
</ns2:Action>




Web Service API Integration Guide                         9. Payment URL 66
10. Solvency Information from Bürgel
The Authipay gateway is integrated with Bürgel Wirtschaftsinformationen, a leading company in the field of
business information.
If you have a contract with Bürgel and have ordered this product option, the action
GetExternalConsumerInformation allows you to request information on the non-payment risk of a customer:
<ns5:IPGApiActionRequest
			 xmlns:ns5=http://ipg-online.com/ipgapi/schemas/ipgapi
			 xmlns:ns2=”http://ipg-online.com/ipgapi/schemas/a1”
			 xmlns:ns3=”http://ipg-online.com/ipgapi/schemas/v1”>
			 >
<ns2:Action>
		<ns2:GetExternalConsumerInformation>
			 <ns2:DataProvider>Bürgel</ns2:DataProvider>
			 <ns2:FirstName>Karl</ns2:FirstName>
			 <ns2:Surname>Hafti</ns2:Surname>
			 <ns2:Birthday>19650423</ns2:Birthday>
			 <ns2:Street>Friedensallee</ns2:Street>
			 <ns2:HouseNumber>4</ns2:HouseNumber>
			 <ns2:PostCode>22765</ns2:PostCode>
			 <ns2:City>Hamburg</ns2:City>
			 <ns2:Country>276</ns2:Country>
			 </ns2:GetExternalConsumerInformation>
		</ns2:Action>
</ns5:IPGApiActionRequest>
If you want to allocate the Bürgel request to a specific Order ID, you can submit an Order ID in the request and use
this same Order ID for a subsequent transaction. In that way the request to Bürgel and the payment transaction will
be grouped under the same Order ID in the Gateway’s online reports.
The response contains the entire request and response to and from Bürgel. The Gateway’s processor response
code shows Bürgel’s non-payment risk score. Please refer to Bürgel’s product documentation for more information
about their service.




Web Service API Integration Guide                                                   10. Solvency Information from Bürg 67
11. 3-D Secure Authentication
11.1 3-D Secure authentication (3DS 1.0)
3-D Secure is an authentication mechanism designed to reduce fraud and chargebacks in relation to
Card-Not-Present transactions.
With our Connect solution (see separate Integration Guide Connect), we can manage the required flows for the
authentication process for you. If you should however prefer to handle this process and the required redirections
yourself, the Web Service API allows you to make single API calls for the required steps:
1. You make an API call to verify if the cardholder is enrolled to participate in a 3D Secure program
2. 	For the cases where the cardholder is enrolled, you redirect your customer to the card issuer’s Access Control
     Server (ACS) using the URL that you received in the respose to your verification request
3. 	You receive the payer authentication response from the card issuer which includes encoded confirmation of the
     authentication status and send this information in a second API call so that we can verify the signature, decode
     it and provide you with the result of the authentication
4. 	You finally trigger the financial transaction (Sale or Pre-Authorisation), referencing to the obtained
     authentication with a Transaction ID

API call for Step 1
To verify if the card has been enrolled you need to submit a verification request with an
AuthenticateTransaction parameter set to “true” and TxType = payerAuth.
The following represents an example of a Verification Request (VEReq) with TxType=payerAuth:
<?xml version=”1.0” encoding=”UTF-8”?><SOAP-ENV:Envelope xmlns:SOAPENV=”
http://schemas.xmlsoap.org/soap/envelope/”>
		<SOAP-ENV:Header/>
		<SOAP-ENV:Body>
			 <ns4:IPGApiOrderRequest
xmlns:ns4=”http://ipg-online.com/ipgapi/schemas/ipgapi”
xmlns:ns2=”http://ipg-online.com/ipgapi/schemas/v1”
xmlns:ns3=”http://ipg-online.com/ipgapi/schemas/a1”>
			 <ns2:Transaction>
				          <ns2:CreditCardTxType>
					 <ns2:StoreId>120995000</ns2:StoreId>
					 <ns2:Type>payerAuth</ns2:Type>
				          </ns2:CreditCardTxType>
				          <ns2:CreditCardData>
					 <ns2:CardNumber>5426*****4979</ns2:CardNumber>
					 <ns2:ExpMonth>12</ns2:ExpMonth>
					 <ns2:ExpYear>24</ns2:ExpYear>
				          </ns2:CreditCardData>
				          <ns2:CreditCard3DSecure>
			 <ns2:AuthenticateTransaction>true</ns2:AuthenticateTransaction>
				          </ns2:CreditCard3DSecure>
				          <ns2:Payment>
					 <ns2:ChargeTotal>13.99</ns2:ChargeTotal>
					 <ns2:Currency>978</ns2:Currency>
				          </ns2:Payment>
			 </ns2:Transaction>
		</ns4:IPGApiOrderRequest>
  </SOAP-ENV:Body>
</SOAP-ENV:Envelope>
Our integrated Merchant Plug-in (MPI) then checks the card participation from the 3-D Secure directory and
returns the redirection URL of the card issuer’s Access Control Server (ACS).

Web Service API Integration Guide                                                             11. 3-D Secure Authentication 68
If the card is enrolled in 3D Secure, the response to the verification request should contain the following key values:
• PaReq: The Payer Authentication Request, required to initiate the authentication
• ACS URL: The target of 3D Secure redirection
• Term URL: The URL, that the ACS should send the outcome to in your application
• MD: Merchant Data which have to be sent to ACS URL
The following represents an example of a VEReq response:
<?xml version=”1.0” encoding=”UTF-8”?><SOAP-ENV:Envelope xmlns:SOAPENV=”
http://schemas.xmlsoap.org/soap/envelope/”>
		<SOAP-ENV:Header/>
		<SOAP-ENV:Body>
			 <ipgapi:IPGApiOrderResponse
xmlns:ipgapi=”http://ipg-online.com/ipgapi/schemas/ipgapi”
xmlns:a1=”http://ipg-online.com/ipgapi/schemas/a1”
xmlns:v1=”http://ipg-online.com/ipgapi/schemas/v1”>
		<ipgapi:ApprovalCode>?:waiting 3dsecure</ipgapi:ApprovalCode>
		<ipgapi:Brand>MASTERCARD</ipgapi:Brand>
		<ipgapi:CommercialServiceProvider>TELECASH</ipgapi:CommercialServiceProvider>
		<ipgapi:OrderId>A-4b9804e6410b84475809e59e1b26</ipgapi:OrderId>
		<ipgapi:IpgTransactionId>8383394827</ipgapi:IpgTransactionId>
		<ipgapi:PaymentType>CREDITCARD</ipgapi:PaymentType>
		<ipgapi:TDate>1493130774</ipgapi:TDate>
		<ipgapi:TDateFormatted>2017.04.25 16:32:54(CEST)</ipgapi:TDateFormatted>
		<ipgapi:TransactionTime>1493130774</ipgapi:TransactionTime>
		<ipgapi:Secure3DResponse>
			 <v1:Secure3DVerificationResponse>
			 <v1:VerificationRedirectResponse>
				          <v1:AcsURL>https://3dsacs.test.modirum.com/mdpayacs/pareq</v1:AcsURL>
					 <v1:PaReq> c7fb83b8ag...73t4a827t4af8738a</v1:PaReq>
					 <v1:TermUrl>https://www.mywebshop.com/process3dSecure/</v1:TermUrl>
					 <v1:MD>MD1234....sdfk</v1:MD>
				          </v1:VerificationRedirectResponse>
			 </v1:Secure3DVerificationResponse>
		</ipgapi:Secure3DResponse>
  </ipgapi:IPGApiOrderResponse>
</SOAP-ENV:Body>
</SOAP-ENV:Envelope>

API call for Step 3
After you have redirected the cardholder for authentication and have received the payer authentication response
from the card issuer, you submit the PARes and MD in your second call to our API. The transaction type must have
the same value as in API call for Step 1 message example.
In case you have not obtained “MD” element in a response from the ACS, it does not have to be included in the
AcsResponse
<?xml version=”1.0” encoding=”UTF-8”?><SOAP-ENV:Envelope xmlns:SOAPENV=”
http://schemas.xmlsoap.org/soap/envelope/”>
		<SOAP-ENV:Header/>
		<SOAP-ENV:Body>
			 <ns4:IPGApiOrderRequest
xmlns:ns4=”http://ipg-online.com/ipgapi/schemas/ipgapi”
xmlns:ns2=”http://ipg-online.com/ipgapi/schemas/v1”
xmlns:ns3=”http://ipg-online.com/ipgapi/schemas/a1”>
		<ns2:Transaction>
			 <ns2:CreditCardTxType>
				          <ns2:StoreId>120995000</ns2:StoreId>
				          <ns2:Type>payerAuth</ns2:Type>

Web Service API Integration Guide                                                          11. 3-D Secure Authentication 69
				          </ns2:CreditCardTxType>
				          <ns2:CreditCard3DSecure>
						                  <ns2:Secure3DRequest>
						                  <ns2:Secure3DAuthenticationRequest>
							                       <ns2:AcsResponse>
							                       <ns2:MD>MDasdadA5809e59e1b263b4aa9</ns2:MD>
							                       <ns2:PaRes>eJzVWNeyq8iS…83IBmfhg</ns2:PaRes>
						                  </ns2:AcsResponse>
					 </ns2:Secure3DAuthenticationRequest>
				          </ns2:Secure3DRequest>
				          </ns2:CreditCard3DSecure>
			 <ns2:Payment/>
			 <ns2:TransactionDetails>
			 <ns2:IpgTransactionId>8383394827</ns2:IpgTransactionId>
		</ns2:TransactionDetails>
  </ns2:Transaction>
</ns4:IPGApiOrderRequest>
</SOAP-ENV:Body>
</SOAP-ENV:Envelope>
Our Gateway verifies the response and provides the result back to you, including the data required as the part of
authorization request.
The following represents the example of an Authentication Response:
<?xml version=”1.0” encoding=”UTF-8”?><SOAP-ENV:Envelope xmlns:SOAPENV=”
http://schemas.xmlsoap.org/soap/envelope/”>
		<SOAP-ENV:Header/>
		<SOAP-ENV:Body>
			 <ipgapi:IPGApiOrderResponse
xmlns:ipgapi=”http://ipg-online.com/ipgapi/schemas/ipgapi”
xmlns:a1=”http://ipg-online.com/ipgapi/schemas/a1”
xmlns:v1=”http://ipg-online.com/ipgapi/schemas/v1”>
			 <ipgapi:ApprovalCode>Y:ECI2/5:Authenticated</ipgapi:ApprovalCode>
			 <ipgapi:Brand>MASTERCARD</ipgapi:Brand>
			 <ipgapi:CommercialServiceProvider>TELECASH</
ipgapi:CommercialServiceProvider>
			 <ipgapi:OrderId>A-123456789</ipgapi:OrderId>
			 <ipgapi:IpgTransactionId>8383394827</ipgapi:IpgTransactionId>
			 <ipgapi:PaymentType>CREDITCARD</ipgapi:PaymentType>
			 <ipgapi:TDate>1493137253</ipgapi:TDate>
			 <ipgapi:TDateFormatted>2017.04.25 18:20:53
(CEST)</ipgapi:TDateFormatted>
			 <ipgapi:TransactionResult>APPROVED</ipgapi:TransactionResult>
			 <ipgapi:TransactionTime>1493137253</ipgapi:TransactionTime>
			 <ipgapi:Secure3DResponse>
				          <v1:ResponseCode3dSecure>1</v1:ResponseCode3dSecure>
			 </ipgapi:Secure3DResponse>
		</ipgapi:IPGApiOrderResponse>
  </SOAP-ENV:Body>
</SOAP-ENV:Envelope>

API call for Step 4
The following represents an example of a Sale transaction initiated after previously authenticated payerauth request:
<?xml version=”1.0” encoding=”UTF-8”?>
<SOAP-ENV:Envelope
  xmlns:SOAP-ENV=”http://schemas.xmlsoap.org/soap/envelope/”>
  <SOAP-ENV:Header/>
  <SOAP-ENV:Body>
		<ns4:IPGApiOrderRequest
Web Service API Integration Guide                                                         11. 3-D Secure Authentication 70
			 xmlns:ns4=”http://ipg-online.com/ipgapi/schemas/ipgapi”
			 xmlns:ns2=”http://ipg-online.com/ipgapi/schemas/v1”
			 xmlns:ns3=”http://ipg-online.com/ipgapi/schemas/a1”>
		<ns2:Transaction>
			 <ns2:CreditCardTxType>
				          <ns2:StoreId>1109950006</ns2:StoreId>
				          <ns2:Type>sale</ns2:Type>
				          </ns2:CreditCardTxType>
					 <ns2:CreditCardData>
					 <ns2:CardCodeValue>XXX</ns2:CardCodeValue>
				          </ns2:CreditCardData>
				          <ns2:TransactionDetails>
				          <ns2:IpgTransactionId>84548553950</ns2:IpgTransactionId>
				          </ns2:TransactionDetails>
			 </ns2:Transaction>
		</ns4:IPGApiOrderRequest>
  </SOAP-ENV:Body>
</SOAP-ENV:Envelope>
If you should have your own Merchant Plug-in (MPI) for 3-D Secure or use a 3rd party provider for this, you can
alternatively submit the result of the authentication process in your Sale or Pre-Authorisation transaction message
to the Web Service API. See CreditCard3DSecure elements in the XML-Tag overview chapter of this document.
In principle, it may occur that 3-D Secure authentications cannot be processed successfully for technical reasons. If
one of the systems involved in the authentication process is temporarily not responding, the payment transaction
will be processed as a “regular” eCommerce transaction (ECI 7). A liability shift to the card issuer for possible
chargebacks is not warranted in this case. If you prefer that such transactions shall not be processed at all, our
technical support team can block them for your Store on request.


11.2 EMV 3-D Secure authentication (3DS 2.0)
The new EMV 3-D Secure protocol (also known as 3DS 2.0) specification has been developed for the benefit of the
entire industry to collaboratively develop the next generation of 3-D Secure protocol. The new version promotes
frictionless consumer authentication and enables consumers to authenticate themselves with their card issuer
when making card-not-present e-commerce purchases.
Due to continuous development and changes demanded by payment schemes and issuers the integration guide
for EMV 3DS protocol has been maintained separately.
Detailed description and examples of the flows can be found on Gateway’s online portal:
https://docs.firstdata.com/org/gateway/node/476


11.2.1 Non-Payment Authentication (NPA)
For cases, where you prefer to register your customers’ credit cards on file without charging them in the same
session, you can submit a payerAuth request to our Gateway with a value ‘02’ in “threeDSEmvCoMessageCategory”
element.
As it is mandatory to use Strong Customer Authentication (SCA) for all new cards added to Card-On-
File, NPA transaction request must include “ThreeDSRequestorChallengeIndicator” value ’04’ and
“ThreeDSRequestorAuthenticationIndicator” value ‘04=Add card’.
The following represents an example of a ‘payerAuth’ request with basic set of elements:
<?xml version=”1.0” encoding=”UTF-8”?><SOAP-ENV:Envelope xmlns:SOAPENV=”
http://schemas.xmlsoap.org/soap/envelope/”>
		<SOAP-ENV:Header/>
		<SOAP-ENV:Body>
			 <ns4:IPGApiOrderRequest
xmlns:ns4=”http://ipg-online.com/ipgapi/schemas/ipgapi”
xmlns:ns2=”http://ipg-online.com/ipgapi/schemas/v1”
xmlns:ns3=”http://ipg-online.com/ipgapi/schemas/a1”>
Web Service API Integration Guide                                                          11. 3-D Secure Authentication 71
		<ns2:Transaction>
			 <ns2:CreditCardTxType>
				          <ns2:StoreId>1109950006</ns2:StoreId>
				          <ns2:Type>payerAuth</ns2:Type>
			 </ns2:CreditCardTxType>
			 <ns2:CreditCardData>
				          <ns2:CardNumber>40169*******0014</ns2:CardNumber>
				          <ns2:ExpMonth>12</ns2:ExpMonth>
				          <ns2:ExpYear>22</ns2:ExpYear>
				          <ns2:CardCodeValue>XXX</ns2:CardCodeValue>
			 </ns2:CreditCardData>
			 <ns2:CreditCard3DSecure>
<ns2:AuthenticateTransaction>true</ns2:AuthenticateTransaction>
<ns2:ThreeDSRequestorChallengeIndicator>04</ns2:ThreeDSRequestorChallengeIndicator>
<ns2:ThreeDSEmvCoMessageCategory>02</ns2:ThreeDSEmvCoMessageCategory>
<ns2:TermUrl>https://mywebshop.com</ns2:TermUrl>
<ns2:ThreeDSMethodNotificationURL>https://mywebshop.com/notification</
ns2:ThreeDSMethodNotificationURL>
<ns2:ThreeDSRequestorChallengeWindowSize>01</ns2:ThreeDSRequestorChallengeWindowSize>
<ns2:ThreeDSRequestorAuthenticationIndicator>04</
ns2:ThreeDSRequestorAuthenticationIndicator>
			 </ns2:CreditCard3DSecure>
			 <ns2:Payment>
				          <ns2:ChargeTotal>0.00</ns2:ChargeTotal>
				          <ns2:Currency>978</ns2:Currency>
			 </ns2:Payment>
		</ns2:Transaction>
  </ns4:IPGApiOrderRequest>
</SOAP-ENV:Body>
</SOAP-ENV:Envelope>
After this request a standard EMV 3-D Secure authentication flow as described here:
https://docs.firstdata.com/org/gateway/node/476 follows and is completed with a final ‘payerAuth’ response:
<SOAP-ENV:Envelope xmlns:SOAPENV=”
http://schemas.xmlsoap.org/soap/envelope/”>
		<SOAP-ENV:Header/>
		<SOAP-ENV:Body>
			 <ipgapi:IPGApiOrderResponse xmlns:a1=”http://ipgonline.com/
ipgapi/schemas/a1” xmlns:ipgapi=”http://ipgonline.com/ipgapi/schemas/ipgapi”
xmlns:v1=”http://ipgonline.com/ipgapi/schemas/v1”>
			 <ipgapi:ApprovalCode>Y:ECI2/5:Authenticated</ipgapi:ApprovalCode>
			 <ipgapi:Brand>VISA</ipgapi:Brand>
			 <ipgapi:Country>USA</ipgapi:Country>
			 <ipgapi:CommercialServiceProvider>BOSMS</ipgapi:CommercialServiceProvider>
			 <ipgapi:OrderId>A-f2adf245-7a38-4729-b9e0-1fb7f1296abd</ipgapi:OrderId>
			 <ipgapi:IpgTransactionId>84572410148</ipgapi:IpgTransactionId>
			 <ipgapi:PaymentType>CREDITCARD</ipgapi:PaymentType>
			 <ipgapi:TDate>1630925618</ipgapi:TDate>
			      <ipgapi:TDateFormatted>2021.09.06 12:53:38 (CEST)</ipgapi:TDateFormatted>
			 <ipgapi:TransactionResult>APPROVED</ipgapi:TransactionResult>
			 <ipgapi:TransactionTime>1630925618</ipgapi:TransactionTime>
			 <ipgapi:Secure3DResponse>
				          <v1:ResponseCode3dSecure>1</v1:ResponseCode3dSecure>
			 </ipgapi:Secure3DResponse>
		</ipgapi:IPGApiOrderResponse>
</SOAP-ENV:Body>
</SOAP-ENV:Envelope>
Please note, that fully authenticated NPA cannot be used as a proof of authentication for any subsequent
transactions and serves only to verify identity of your clients while adding their card on file.
Web Service API Integration Guide                                                      11. 3-D Secure Authentication 72
12. Purchasing cards
Purchasing Cards offer businesses the ability to allow their employees to purchase items with a credit card while
providing additional information on sales tax, customer code etc. When providing specific details on the payment
being made with a Purchasing card favourable addendum interchange rates are applied.
There are three levels of details required for Purchasing Cards:
• Level I — The first level is the standard transaction data; no enhanced data is required at this level.
• Level II — The second level requires that data such as tax amount and customer code be supplied in addition to
  the standard transaction date. (Visa only have a level II option)
• Level III — The third level allows a merchant to pass a detailed accounting of goods and services purchased
  to the buyer. All the data for Level I and Level II must also be passed to participate in Level III. (Visa and
  Mastercard).
PurchaseCard element can contain contain 0-100 LineItemData elements.
Detailed description of all PurchaseCard elements can be found in the XML-Tag overview chapter of this document.
The following represents an example of a purchasing card L3 transaction including a single LineItemData element
and mandatory fields:
<?xml version=”1.0” encoding=”UTF-8”?><SOAP-ENV:Envelope xmlns:SOAPENV=”
http://schemas.xmlsoap.org/soap/envelope/”>
		<SOAP-ENV:Header/>
		<SOAP-ENV:Body>
			 <ns4:IPGApiOrderRequest
xmlns:ns4=”http://ipgonline.com/ipgapi/schemas/ipgapi”
xmlns:ns2=”http://ipgonline.com/ipgapi/schemas/a1”
xmlns:ns3=”http://ipgonline.com/ipgapi/schemas/v1”>
			 <ns3:Transaction>
				          <ns3:CreditCardTxType>
					 <ns3:StoreId>110995100</ns3:StoreId>
					 <ns3:Type>sale</ns3:Type>
				          </ns3:CreditCardTxType>
				          <ns3:CreditCardData>
					 <ns3:CardNumber>4035*****4977</ns3:CardNumber>
					 <ns3:ExpMonth>12</ns3:ExpMonth>
					 <ns3:ExpYear>18</ns3:ExpYear>
					 <ns3:CardCodeValue>XXX</ns3:CardCodeValue>
				          </ns3:CreditCardData>
				          <ns3:Payment>
					 <ns3:ChargeTotal>23</ns3:ChargeTotal>
					 <ns3:Currency>GBP</ns3:Currency>
				          </ns3:Payment>
				          <ns3:TransactionDetails>
				          <ns3:PurchaseCard>
					 <ns3:CustomerReferenceID>9632587410</ns3:CustomerReferenceID>
					 <ns3:SupplierInvoiceNumber>321456987</ns3:SupplierInvoiceNumber>
					 <ns3:SupplierVATRegistrationNumber>GB18150620</
					 ns3:SupplierVATRegistrationNumber>
				          <ns3:LineItemData>
					 <ns3:CommodityCode>0</ns3:CommodityCode>
					             <ns3:Description>DIRECT MARKETING PURCH</ns3:Description>
					 <ns3:Quantity>200000</ns3:Quantity>
					 <ns3:UnitOfMeasure>TPR</ns3:UnitOfMeasure>
					 <ns3:UnitPrice>1200</ns3:UnitPrice>
					 <ns3:LineItemTotal>1200</ns3:LineItemTotal>
				          </ns3:LineItemData>
			 </ns3:PurchaseCard>

Web Service API Integration Guide                                                                 12. Purchasing cards 73
				          </ns3:TransactionDetails>
			 </ns3:Transaction>
		</ns4:IPGApiOrderRequest>
  </SOAP-ENV:Body>
</SOAP-ENV:Envelope>
If LineItemData element were removed, the example above would represent a purchasing card level II transaction.
The following represents an example of a purchasing card Level III transaction including multipule LineItemData
elements with all possible fields populated:
<?xml version=”1.0” encoding=”UTF-8”?><SOAP-ENV:Envelope
xmlns:SOAP-ENV=”http://schemas.xmlsoap.org/soap/envelope/”>
		<SOAP-ENV:Header/>
		<SOAP-ENV:Body>
			 <ns3:IPGApiOrderRequest
xmlns:ns3=”http://ipg-online.com/ipgapi/schemas/ipgapi”
xmlns:ns2=”http://ipg-online.com/ipgapi/schemas/v1”
xmlns:ns4=”http://ipg-online.com/ipgapi/schemas/a1”>
			 <ns2:Transaction>
				          <ns2:CreditCardTxType>
					 <ns2:StoreId>110995100</ns2:StoreId>
					 <ns2:Type>preAuth</ns2:Type>
				          </ns2:CreditCardTxType>
				          <ns2:CreditCardData>
					 <ns2:CardNumber>4035*****4977</ns2:CardNumber>
					 <ns2:ExpMonth>12</ns2:ExpMonth>
					 <ns2:ExpYear>18</ns2:ExpYear>
					 <ns2:CardCodeValue>XXX</ns2:CardCodeValue>
				          </ns2:CreditCardData>
				          <ns2:Payment>
					 <ns2:SubTotal>15</ns2:SubTotal>
					 <ns2:ValueAddedTax>4</ns2:ValueAddedTax>
				          <ns2:DeliveryAmount>5</ns2:DeliveryAmount>
				          <ns2:ChargeTotal>24</ns2:ChargeTotal>
				          <ns2:Currency>GBP</ns2:Currency>
			 </ns2:Payment>
			 <ns2:TransactionDetails>
				          <ns2:PurchaseCard>
				          <ns2:CustomerReferenceID>9632587410</ns2:CustomerReferenceID>
				          <ns2:SupplierInvoiceNumber>321456987</ns2:SupplierInvoiceNumber>
				          <ns2:SupplierVATRegistrationNumber>GB18150620</
				          ns2:SupplierVATRegistrationNumber>
				          <ns2:TotalDiscountAmountAndRate>
				          <ns2:Amount>55</ns2:Amount>
				          <ns2:Rate>99.99</ns2:Rate>
				          </ns2:TotalDiscountAmountAndRate>
				          <ns2:VATShippingAmountAndRate>
				          <ns2:Amount>35</ns2:Amount>
				          <ns2:Rate>0.10</ns2:Rate>
				          </ns2:VATShippingAmountAndRate>
			 <ns2:LineItemData>
				          <ns2:CommodityCode>1112</ns2:CommodityCode>
				          <ns2:ProductCode>22369852147</ns2:ProductCode>
				          <ns2:Description>DIRECTMARKETINGPURCH</ns2:Description>
				          <ns2:Quantity>200000</ns2:Quantity>
				          <ns2:UnitOfMeasure>TPR</ns2:UnitOfMeasure>
				          <ns2:UnitPrice>1200</ns2:UnitPrice>
				          <ns2:VATAmountAndRate>
				          <ns2:Amount>9999</ns2:Amount>

Web Service API Integration Guide                                                             12. Purchasing cards 74
			 <ns2:Rate>0.1</ns2:Rate>
			 </ns2:VATAmountAndRate>
			 <ns2:DiscountAmountAndRate>
				          <ns2:Amount>13</ns2:Amount>
				          <ns2:Rate>99.99</ns2:Rate>
			 </ns2:DiscountAmountAndRate>
			 <ns2:LineItemTotal>1200</ns2:LineItemTotal>
		</ns2:LineItemData>
		<ns2:LineItemData>
			 <ns2:CommodityCode>5647</ns2:CommodityCode>
			 <ns2:ProductCode>22369852148</ns2:ProductCode>
			 <ns2:Description>2-DIRECTMARKETINGPURCH</ns2:Description>
			 <ns2:Quantity>200001</ns2:Quantity>
			 <ns2:UnitOfMeasure>DAY</ns2:UnitOfMeasure>
			 <ns2:UnitPrice>1201</ns2:UnitPrice>
			 <ns2:VATAmountAndRate>
				          <ns2:Amount>9999</ns2:Amount>
				          <ns2:Rate>0.2</ns2:Rate>
			 </ns2:VATAmountAndRate>
			 <ns2:DiscountAmountAndRate>
				          <ns2:Amount>14</ns2:Amount>
				          <ns2:Rate>99.99</ns2:Rate>
			 </ns2:DiscountAmountAndRate>
			 <ns2:LineItemTotal>1202</ns2:LineItemTotal>
		</ns2:LineItemData>
		<ns2:LineItemData>
			 <ns2:CommodityCode>575</ns2:CommodityCode>
			 <ns2:ProductCode>22369852149</ns2:ProductCode>
			 <ns2:Description>3-DIRECTMARKETINGPURCH</ns2:Description>
			 <ns2:Quantity>200002</ns2:Quantity>
			 <ns2:UnitOfMeasure>ACR</ns2:UnitOfMeasure>
			 <ns2:UnitPrice>1203</ns2:UnitPrice>
			 <ns2:VATAmountAndRate>
				          <ns2:Amount>9999</ns2:Amount>
				          <ns2:Rate>0.3</ns2:Rate>
			 </ns2:VATAmountAndRate>
			 <ns2:DiscountAmountAndRate>
				          <ns2:Amount>15</ns2:Amount>
				          <ns2:Rate>99.99</ns2:Rate>
			 </ns2:DiscountAmountAndRate>
			 <ns2:LineItemTotal>1204</ns2:LineItemTotal>
		</ns2:LineItemData>
  </ns2:PurchaseCard>
</ns2:TransactionDetails>
</ns2:Transaction>
</ns3:IPGApiOrderRequest>
</SOAP-ENV:Body>
</SOAP-ENV:Envelope>




Web Service API Integration Guide                               12. Purchasing cards 75
13. XML-Tag overview
13.1.1 Overview by transaction type
The following shows which XML-tags need to be submitted for each transaction type as well as which ones can
optionally be used. Please only use the fields stated below and also note the order.
For XML-tags related to Card Present transactions with a chip reader and PIN entry device please refer to the xsd’s
in the Appendix of this document.

 Abbreviations:
 m:             mandatory
 o:             optional
 d:             optional with default value
 a and b:       maximum one of the two values (In case you will find a value=”a” in the column for a transaction
                type, it means either all those elements marked with “a” need to be present in the message, or the
                one marked with “b”.)
 1:             if a or b is provided optional, mandatory if a and b have not been provided
 3:             mandatory for 3D Secure transactions
 s:             see details in 3D Secure chapter
 f:             mandatory for Visa transactions of UK-based Financial Institutions with Merchant Category Code
                6012
 r:             mandatory for recurring SEPA Direct Debit
 p:             mandatory for split shipment
 q:             see details in Purchasing cards chapter
 u:             mandatory for Union Pay Secure Plus transactions



Path Name                                                 Credit Card                                      Debit Card
all paths relative to
                                           Force
ipgapi:IPGApiOrderRequest/          Sale            PreAuth   PostAuth   Return   Credit   Void    Sale   Return   Credit    Void
                                           Ticket
v1:Transaction
v1:CreditCardTxType/
                                    m         m       m         m         m        m       m
v1:Type
v1:CreditCardData/
                                     a        a       a                             a
v1:CardNumber
v1:CreditCardData/ v1:ExpMonth       a        a       a                             a
v1:CreditCardData/
                                     a        a       a                             a
v1:ExpYear
v1:CreditCardData/
                                     o        o       o                             o
v1:CardCodeValue
v1:CreditCardData/ v1:TrackData      b        b       b                             b
v1:CreditCardData/
                                     o        o       o                             o
v1:Brand
v1:CreditCard3DSecure/
                                     3        3       3                             3
v1:VerificationResponse
v1:CreditCard3DSecure/
                                     s        s       s                             s
v1:PayerAuthenticationResponse
v1:CreditCard3DSecure/
                                     s        s       s                             s
v1:DRSPECI
v1:CreditCard3DSecure/
                                     s        s       s                             s
v1:AuthenticationValue
v1:CreditCard3DSecure/
                                     s        s       s                             s
v1:XID


Web Service API Integration Guide                                                          15. Building a SOAP Request Message 76
v1:CreditCard3DSecure/
                                    s   s   s           s
v1:AuthenticateTransaction
v1:CreditCard3DSecure/
v1:Secure3DRequest/
v1: Secure3DAuthentication          s   s   s           s
Request/
v1:IVRAuthenticationRequest
v1:CreditCard3DSecure/
v1:Secure3DRequest/
v1:Secure3DAuthentication           s   s   s           s
Request/
v1:AcsResponse
v1:CreditCard3DSecure/
v1:Secure3DRequest/
                                    s   s   s           s
v1:Secure3DVerificationRequest
v1:IVRVerificationRequest
v1:CreditCard3DSecure/
v1:Secure3Dverification
                                    s   s   s           s
Response/
v1:IVRVerificationResponse
v1:CreditCard3DSecure/
v1:Secure3DVerification
                                    s   s   s           s
Response/ v1:Verification
RedirectResponse
v1:CreditCardData/
                                    u   u   u           u
v1:Upop
v1:cardFunction/
                                    o   o   o           o
v1:Type
v1:DE_DirectDebitTxType/
                                                            m      m       m        m
v1:Type
v1:DE_DirectDebitData/
                                                            o               1
v1:BIC
v1:DE_DirectDebitData/
                                                            a               a
v1:IBAN
v1:DE_DirectDebitData/
                                                            b               b
v1:TrackData
v1:DE_DirectDebitData/
                                                            m
v1:MandateReference
v1:DE_DirectDebitData/
                                                            d,r
v1:MandateType
v1:DE_DirectDebitData/
                                                             r
v1:DateOfMandate
v1:Payment/
                                    1   1   1           1   1               1
v1:HostedDataID
v1:Payment/
                                    1   1   1           1   1               1
v1:HostedDataStoreID
v1:Payment/
                                    1   1   1           1   1               1
v1:DeclineHostedDataDuplicates
v1:Payment/
                                    o
v1:numberOfInstallments
v1:Payment/
                                    d
v1:installmentsInterest
v1:Payment/
                                    o
v1:installmentDelayMonths
v1:Payment/
                                    o   o   o   o   o   o   o      o        o
v1:SubTotal
v1:Payment/
                                    o   o   o   o   o   o   o      o        o
v1:ValueAddedTax
v1:Payment/
                                    o   o   o   o   o   o   o      o        o
v1:localTax

Web Service API Integration Guide                                 13. XML – Tag overv 77
v1:Payment/
                                    o   o   o   o   o   o       o    o        o
v1:DeliveryAmount
v1:Payment/
                                    m   m   m   m   m   m       m    m        m
v1:ChargeTotal
v1:Payment/
                                    m   m   m   m   m   m       m    m        m
v1:Currency
v1:recurringType                    o
v1:WalletType                       o
v1:WalletID                         o
v1:TransactionDetails/
                                    o   o   o   m   m   o   a   o    m        o        a
v1:OrderId
v1:TransactionDetails/
                                    o   o   o   o   o   o   o   o    o        o        o
v1:MerchantTransactionId
v1:TransactionDetails/
                                    o       o           o       o             o
v1:Ip
v1:TransactionDetails/
                                        m
v1:ReferenceNumber
v1:TransactionDetails/
                                                            a                          a
v1:Tdate
v1:TransactionDetails/
v1:ReferencedMerchant                                       b                          b
TransactionId
v1:TransactionDetails/
                                    d       d           d
v1:TransactionOrigin
v1:TransactionDetails/
                                    o   o   o   o       o       o             o
v1:InvoiceNumber
v1:TransactionDetails/
                                    o   o   o           o       o             o
v1:PONumber
v1:TransactionDetails/
                                    o   o   o           o       o             o
v1:DynamicMerchantName
v1:TransactionDetails/
                                    o   o   o   o   o   o   o   o    o        o        o
v1:Comments
v1:TransactionDetails/
                                    q   q   q   q   q   q   q   q    q        q        q
v1:PurchaseCard
v1:TransactionDetails/
                                    o   o   o           o       o             o
v1:Terminal/ v1:TerminalID
v1:TransactionDetails/
                                    o   o   o
v1:InquiryRateReference
v1:TransactionDetails/
v1:SplitShipment/                           o   o
v1:SequenceCount
v1:TransactionDetails/
v1:SplitShipment/                               p
v1:FinalShipment
v1:Billing/
                                    o   o   o           o       o             o
v1:CustomerID
v1:Billing/
                                    o   o   o           o       m             m
v1:Name
v1:Billing/
                                    o   o   o           o       o             o
v1:Company
v1:Billing/
                                    o   o   o           o       o             o
v1:Address1
v1:Billing/
                                    o   o   o           o       o             o
v1:Address2
v1:Billing/
                                    o   o   o           o       o             o
v1:City
v1:Billing/
                                    o   o   o           o       o             o
v1:State

Web Service API Integration Guide                                   13. XML – Tag overv 78
v1:Billing/
                                    o   o   o   o   o             o
v1:Zip
v1:Billing/
                                    o   o   o   o   o             o
v1:Country
v1:Billing/
                                    o   o   o   o   o             o
v1:Phone
v1:Billing/
                                    o   o   o   o   o             o
v1:Fax
v1:Billing/
                                    o   o   o   o   o             o
v1:Email
v1:Shipping/
                                    o   o   o   o   o             o
v1:Type
v1:Shipping/
                                    o   o   o   o   o             o
v1:Name
v1:Shipping/
                                    o   o   o   o   o             o
v1:Address1
v1:Shipping/
                                    o   o   o   o   o             o
v1:Address2
v1:Shipping/
                                    o   o   o   o   o             o
v1:City
v1:Shipping/
                                    o   o   o   o   o             o
v1:State
v1:Shipping/
                                    o   o   o   o   o             o
v1:Zip
v1:Shipping/
                                    o   o   o   o   o             o
v1:Country
v1:Basket/
v1:Item/                            o   o   o   o   o             o
v1:ID
v1:Basket/
v1:Item/                            o   o   o   o   o             o
v1:Description
v1:Basket/
v1:Item/
v1:SubTotal
v1:Basket/
v1:Item/
v1:ValueAddedTax
v1:Basket/
v1:Item/
v1:DeliveryAmount
v1:Basket/
v1:Item/                            o   o   o   o   o             o
v1:ChargeTotal
v1:Basket/
v1:Item/
v1:Currency
v1:Basket/
v1:Item/                            o   o   o   o   o             o
v1:Quantity
v1:Basket/
v1:Item/
                                    o   o   o   o   o             o
v1:Option/
v1:Name
v1:Basket/
v1:Item/                            o   o   o   o   o             o
v1:Choice
v1:TopUpTxType/
v1:MPCharge/
v1:MNSP

Web Service API Integration Guide                       13. XML – Tag overv 79
v1:TopUpTxType/
v1:MPCharge/
v1:MSISDN
v1:TopUpTxType/
v1:MPCharge/
v1:PaymentType
v1:ClientLocale/
                                        d    d        d       d         d          d   d          d       d       d        d
v1:Language
v1:ClientLocale/
                                        d    d        d       d         d          d   d          d       d       d        d
v1:Country
v1:MCC6012Details/
                                        f     f       f
v1:BirthDate
v1:MCC6012Details/
                                       f,a   f,a     f,a
v1:AccountFirst6
v1:MCC6012Details/
                                       f,a   f,a     f,a
v1:AccountLast4
v1:MCC6012Details/
                                       f,b   f,b     f,b
v1:AccountNumber
v1:MCC6012Details/
                                        f     f       f
v1:PostCode
v1:MCC6012Details/
                                        f     f       f
v1:Surname

                                                                                                      Mobile
                Path Name                                             PayPal
                                                                                                      Top Up
                all paths relative to
                ipgapi:IPGApiOrderRequest/         PostAuth   Return        Credit         Void       MP Charge
                v1:Transaction
                v1:CreditCardTxType/
                                                     m            m            m           m             m
                v1:Type
                v1:CreditCardData/
                v1:CardNumber
                v1:CreditCardData/
                v1:ExpMonth
                v1:CreditCardData/
                v1:ExpYear
                v1:CreditCardData/
                v1:CardCodeValue
                v1:CreditCardData/
                v1:TrackData
                v1:CreditCard3DSecure/
                v1:Verification Response
                v1:CreditCard3DSecure/
                v1:Payer Authentication
                Response
                v1:CreditCard3DSecure/
                v1:Authentication Value
                v1:CreditCard3DSecure/
                v1:XID
                v1:DE_DirectDebitTxType/
                v1:Type
                v1:DE_DirectDebitData/
                v1:BIC
                v1:DE_DirectDebitData/
                v1:IBAN
                v1:DE_DirectDebitData/
                v1:MandateReference
                v1:DE_DirectDebitData/
                v1:MandateType

Web Service API Integration Guide                                                                       13. XML – Tag overv 80
                v1:DE_DirectDebitData/
                v1:TrackData
                v1:PayPalTxType/
                                                 m   m   m   m
                v1:Type
                v1:Payment/
                v1:HostedDataID
                v1:Payment/
                v1:HostedData StoreID
                v1:Payment/
                v1:DeclineHostedDataDuplicates
                v1:Payment/
                                                 o   o   o       o
                v1:SubTotal
                v1:Payment/
                                                 o   o   o       o
                ValueAddedTax
                v1:Payment/
                                                 o   o   o       o
                v1:DeliveryAmount
                v1:Payment/
                                                 m   m   m       m
                v1:ChargeTotal
                v1:Payment/
                                                 m   m   m       m
                v1:Currency
                v1:TransactionDetails/
                                                 m   m   o   m   o
                v1:OrderId
                v1:TransactionDetails/
                                                 o   o   o   o   o
                v1:Merchant TransactionId
                v1:TransactionDetails/
                                                         o       o
                v1:Ip
                v1:TransactionDetails/
                v1:ReferenceNumber
                v1:TransactionDetails/
                                                             a
                v1:Tdate
                v1:TransactionDetails/
                v1:Referenced Merchant                       b
                TransactionId
                v1:TransactionDetails/
                                                         d
                v1:Transaction Origin
                v1:TransactionDetails/
                                                         o       o
                v1:InvoiceNumber
                v1:TransactionDetails/
                                                         o       o
                v1:PONumber
                v1:TransactionDetails/
                                                         o       o
                v1:Dynamic MerchantName
                v1:TransactionDetails/
                                                 o   o   o   o   o
                v1:Comments
                v1:Billing/
                                                         o       o
                v1:CustomerID
                v1:Billing/
                                                         o       o
                v1:Name
                v1:Billing/
                                                         o       o
                v1:Company
                v1:Billing/
                                                         o       o
                v1:Address1
                v1:Billing/
                                                         o       o
                v1:Address2
                v1:Billing/
                                                         o       o
                v1:City
                v1:Billing/
                                                         o       o
                v1:State



Web Service API Integration Guide                                13. XML – Tag overv 81
                v1:Billing/
                                     o   o
                v1:Zip
                v1:Billing/
                                     o   o
                v1:Country
                v1:Billing/
                                     o   o
                v1:Phone
                v1:Billing/
                                     o   o
                v1:Fax
                v1:Billing/
                                     m   o
                v1:Email
                v1:Shipping/
                                     o
                v1:Type
                v1:Shipping/
                                     o
                v1:Name
                v1:Shipping/
                                     o
                v1:Address1
                v1:Shipping/
                                     o
                v1:Address2
                v1:Shipping/
                                     o
                v1:City
                v1:Shipping/
                                     o
                v1:State
                v1:Shipping/
                                     o
                v1:Zip
                v1:Shipping/
                                     o
                v1:Country
                v1:Basket/
                v1:Item/             o
                v1:ID
                v1:Basket/
                v1:Item/             o
                v1:Description
                v1:Basket/
                v1:Item/
                v1:SubTotal
                v1:Basket/
                v1:Item/
                v1:ValueAddedTax
                v1:Basket/
                v1:Item/
                v1:Delivery Amount
                v1:Basket/
                v1:Item/             o
                v1:ChargeTotal
                v1:Basket/
                v1:Item/
                v1:Currency
                v1:Basket/
                v1:Item/             o
                v1:Quantity
                v1:Basket/
                v1:Item/
                                     o
                v1:Option/
                v1:Name
                v1:Basket/
                v1:Item/             o
                v1:Choice
                v1:TopUpTxType/
                v1:MPCharge/             m
                v1:MNSP

Web Service API Integration Guide        13. XML – Tag overv 82
                v1:TopUpTxType/
                v1:MPCharge/                                                                              m
                v1:MSISDN
                v1:TopUpTxType/
                v1:MPCharge/                                                                              m
                v1:PaymentType
                v1:ClientLocale/
                                                     d            d             d            d            d
                v1:Language
                v1:ClientLocale/
                                                     d            d             d            d            d
                v1:Country



13.1.2 Description of the XML-Tags

13.1.3 CreditCardTxType
 Path/Name                           XML        Description
                                    Schema
                                     type
 v1:CreditCardTxType/               xs:string   Stores the transaction type. Possible values are sale, forceTicket,
 v1:Type                                        preAuth, postAuth, return, credit and void.


13.1.4             CreditCardData
 Path/Name                           XML        Description
                                    Schema
                                     type
 v1:CreditCardTxType/               xs:string   Stores the customer’s credit card number. Make sure that the string
 v1:CardNumber                                  contains only digits, i.e. passing the number e.g. in the format xxxx-
                                                xxxx-xxxx-xxxx will result in an error returned by the Web Service API.
 v1:CreditCardData/                 xs:string   Stores the expiration month of the customer’s credit card. Make sure
 v1:ExpMonth                                    that the content of this element always contains two digits, i.e. a card
                                                expiring in July will have this element with value 07.
                                                For authorisations on the Nashville front-end only: for cases, where
                                                you do not know the credit card expiry date, please send the value 12.
 v1:CreditCardData/                 xs:string   Stores the expiration year of the customer’s credit card. The same
 v1:ExpYear                                     formatting restrictions as for the v1:ExpMonth element apply here.
                                                For authorisations on the Nashville front-end only: for cases, where
                                                you do not know the credit card expiry date, please send the value 99.
 v1:CreditCardData/                 xs:string   Stores the three or four digit card security code (CSC) – sometimes
 v1:CardCodeValue                               also referred to as card verification value (CVV) or code (CVC) – which
                                                is typically printed on the back of the credit card. For information
                                                about the benefits of CSC contact support.
 v1:CreditCardData/                 xs:string   Stores the track data of a card when using a card reader instead of
 v1:TrackData                                   keying in card data (can optionally be used instead of transmitting
                                                CardNumber, ExpMonth and ExpYear). This field needs to contain
                                                at least the concatenated track 1 and 2 data. Track data 3 is optional.
                                                The track data must include the track and field separators as they are
                                                stored on the card. Example for the track data separator from track
                                                data 1 and 2 without the data: %…?;…?
 v1:CreditCardData/                 xs:string   Optional field for the brand of the credit card. If this field is set, the
 v1:TrackData                                   transaction will only be processed if the card number matches the
                                                brand.
For XML-tags related to Card Present transactions with a chip reader and PIN entry device please refer to the xsd’s
in the Appendix of this document.
Web Service API Integration Guide                                                                        13. XML – Tag overv 83
13.1.5 recurringType
 Path/Name                           XML        Description
                                    Schema
                                     type
 v1:recurringType                   xs:string   This field allows you to flag transactions as recurring. It can be set
                                                to FIRST for the first transaction of a series and to REPEAT for the
                                                subsequent transactions in a series.


13.1.6 UnscheduledCredentialOnFileType
 Path/Name                           XML        Description
                                    Schema
                                     type
 v1: unscheduled                    xs:string   This field allows you to flag transactions as Unscheduled Credential
 CredentialOnFileType                           On File Type. Currently the valid values are FIRST, CARDHOLDER_
                                                INITIATED or MERCHANT_INITIATED to advise the scenario if the
                                                credential is stored on your side.


13.1.7 Wallet
 Path/Name                           XML        Description
                                    Schema
                                     type
 v1:Wallet/                         xs:string   This field allows you to submit the wallet type for transactions that
 v1:WalletType                                  have been initiated through a digital wallet. Currently the valid values
                                                are MASTERPASS, APPLE_PAY,
                                                SAMSUNG_PAY, ANDROID_PAY
 v1:Wallet/                         xs:string   This field allows you to submit the wallet ID for transactions that have
 v1:WalletID                                    been initiated through a digital wallet.


13.1.8 cardFunction
 Path/Name                           XML        Description
                                    Schema
                                     type
 v1:cardFunction/                   xs:string   This field allows you to indicate the card function in case of combo
 v1:Type                                        cards which provide credit and debit functionality on the same card. It
                                                can be set to credit or debit.


13.1.9 CreditCard3DSecure
 Path/Name                           XML        Description
                                    Schema
                                     type
 v1:CreditCard 3DSecure/            xs:string   Stores the VerificationResponse (VERes) of your Merchant Plug-in,
 v1:VerificationResponse                        relevant for 3DS protocol 1.0 only
 v1:CreditCard 3DSecure/            xs:string   Stores the PayerAuthenticationResponse (PARes) of your Merchant
 v1:PayerAuthentication                         Plug-in, relevant for 3DS protocol 1.0 only
 Response




Web Service API Integration Guide                                                                      13. XML – Tag overv 84
 v1:CreditCard 3DSecure/             xs:string   To set ECI value for Digital Secure Remote Payments.
 v1:DSRPECI                                      If you submit this parameter, any values for parameters
                                                 VerificationResponse and PayerAuthenticationResponse will
                                                 be ignored.
 v1:CreditCard 3DSecure/             xs:string   Stores the AuthenticationValue (MasterCard: AAV or VISA: CAAV) of
 v1:AuthenticationValue                          your Merchant Plug-in.
 v1:CreditCard 3DSecure/             xs:string   Stores the XID of your Merchant Plug-in, relevant for 3DS protocol
 v1:XID                                          1.0 only.
 v1:CreditCard 3DSecure/            xs:boolean Indicates, if transaction is going to be authenticated as 3DSecure
 v1: AuthenticateTransaction                   transaction.
 v1:CreditCard 3DSecure/            xs:boolean Set true, if for this transaction you would like to enforce 3-D Secure
 v1: Override3dsCountry                        authentication, despite this country possibly being exempted from
 Exclusion                                     authentication due to the merchant configured list of countries where
                                               3-D Secure is not required.
 v1:CreditCard 3DSecure/            xs:boolean Set to true, if for this transaction you would enforce 3-D Secure
 v1: SkipTRA                                   authentication, despite of the result of Transaction Risk Analysis
                                               performed by RiskShield
 v1:CreditCard 3DSecure/             xs:string   The URL where the issuer(ACS) shall return the result of the
 v1: TermUrl                         500 max     authentication after cardholder’s challenge.
 v1:CreditCard 3DSecure/             xs:string   The URL where the the notification of 3DSMethod completion from
 v1: ThreeDSMethod                   500 max     the ACS shall be sent. Applicable for 3DS 2.x protocol only.
 NotificationURL
 v1:CreditCard 3DSecure/             xs:string   Optional parameter to be used for 3DS 2.1 protocol in order to
 v1: ThreeDSRequestor                            indicate the preferred type of authentication, default value submitted
 ChallengeIndicator                              by the Gateway is “01”.
                                                 Currently supported values:
                                                 01 = NO PREFERENCE
                                                 02 = NO CHALLENGE REQUESTED
                                                 03 = CHALLENGE REQUESTED 3DS REQUESTOR PREFERENCE
                                                 04 = CHALLENGE REQUESTED MANDATE
 v1:CreditCard 3DSecure/             xs:string   Represents the type of purchased item, mandatory for Visa and
 v1:ThreeDSTransType                             Brazilian market, otherwise optional. If no specific value is present in
                                                 the transaction request, default value “01” is used.
                                                 01 = Goods/ Service Purchase
                                                 03 = Check Acceptance
                                                 10 = Account Funding
                                                 11 = Quasi-Cash Transaction
                                                 28 = Prepaid Activation and Load
 v1:CreditCard 3DSecure/             xs:string   Represents the size of the challenge window displayed to your
 v1: hreeDSRequestor                             customers during the authentication process, you can submit this
 ChallengeWindowSize                             element with one of the values:
                                                 01 = 250 x 400
                                                 02 = 390 x 400
                                                 03 = 500 x 600
                                                 04 = 600 x 400
                                                 04 = Full screen
                                                 Note: Based on the payment schemes’ observation it is highly
                                                 recommended to use the value “05 - Full screen” only for browser-
                                                 based flows. Using full screen mode in app-based flows where the
                                                 authentication of the cardholder happens on a smartphone or tablet
                                                 might cause time-outs and trigger an error on issuer/ACS side.




Web Service API Integration Guide                                                                      13. XML – Tag overv 85
 v1:CreditCard 3DSecure/            xs:string   Represents EMVCo definition of the authentication category, if no
 v1: hreeDSEmvCo                                specific value is present in the transaction request, default value “01”
 MessageCategory                                is used.
                                                01 = Payment Authentication
                                                02 = Non-Payment Authentication
                                                80 = Mastercard Data Only (available for Brazilian merchants only)
 v1:CreditCard 3DSecure/            xs:string   Indicates the type of Authentication request as in EMVCo
 v1:ThreeDSRequestor                            specification:
 AuthenticationIndicator                        01 = Payment transaction
                                                02 = Recurring transaction
                                                03 = Installment transaction
                                                04 = Add card
                                                05 = Maintain card
                                                06 = Card holder verification as part of EMV token ID and Value
Please note, that some of these values you either receive from your own MPI/3DSServer or from your 3-D Secure
provider. The integrated 3-D Secure functionality of the Hosted Payment Pages/Direct POST feature can not be
used for transactions via the API for technical reasons.

13.1.10 India Mobile / IVR Extension Verification Request
 Path/Name                           XML        Description
                                    Schema
                                     type
 v1:IVRVerificationRequest/         xs:string   Cardholder Phone or Mobile Device ID Format , possible values are “I”
 v1:IVRDeviceIdFormat                           for International format (with country code), “D” for domestic format
 v1:IVRVerificationRequest/         xs:string   Cardholder’s phone number (with no “+” or leading zeros)
 v1:IVRDeviceId
 v1:IVRVerificationRequest/         xs:string   Indicates how the transaction is being initiated: IVR, CLIENT (J2EE or
 v1:IVRShoppingChannel                          STK app), TTP (via trusted 3rd party), SMS, WAP, native-app


 v1:IVRVerificationRequest/         xs:string   Indicates if the data entered by the customer was encrypted using
 v1:IVRAuthentication                           the key provided in the VERes (true/false). The ACS reads this tag and
 Channel                                        decrypts the value provided by the customer before processing.


13.1.11 India Mobile / IVR Extension Authentication Request
 Path/Name                           XML        Description
                                    Schema
                                     type
 v1:IVRAuthentication               xs:string   Specifies the data being requested by the ACS (value from VERes):
 Request/                                       SP (static password), OTP1 (issued to cardholder prior to transaction),
 v1:IVRUserDataName                             OTP2 (issued to cardholder during transaction), TTP (authentication
                                                performed by a Trusted Third Party), ICB (Issuer Call-Back), other (e.g.
                                                Netbanking PIN)
 v1:IVRAuthentication               xs:string   Value entered by the customer
 Request/
 v1:IVRUserDataValue
 v1:IVRAuthentication               xs:string   Provides a status of the user interaction:
 Request                                        “Y” User entered
 v1:IVRUserDataStatus                           “N” Value not received
                                                “T” Transaction timed out
                                                “U” Undefined failure
 v1:IVRAuthentication               xs:string   Indicates if the data entered by the customer was encrypted using
 Request/                                       the key provided in the VERes (true/false). The ACS reads this tag and
 v1:IVRUserDataEncrypted                        decrypts the value provided by the customer before processing.

Web Service API Integration Guide                                                                     13. XML – Tag overv 86
13.1.12 3
         DSecure 1.0 Authentication /
        Verification Redirect Response
 Path/Name                           XML        Description
                                    Schema
                                     type
 v1:VerificationRedirect            xs:string   Represents the target of the 3D Secure redirection
 Response/
 v1:AcsURL
 v1:VerificationRedirect            xs:string   Represents the PAReq data which has to be sent in the “PAReq”
 Response/                                      attribute to the ACS URL.
 v1:PaReq
 v1:VerificationRedirect            xs:string   Represents the default TermURL, which should be used in order to
 Response/                                      process the response from the 3-D Secure process.
 v1: TermUrl
                                                In case that a merchant would like to parse the response by himself,
                                                he has to specify the “TermUrl” parameter in the form with his custom
                                                URL, in which he will process the response and call the API with the
                                                response PARes and Merchant Data.
 v1:VerificationRedirect            xs:string   Represents the merchant data which has to be sent in the “MD”
 Response/                                      attribute to the ACS URL.
 v1: MD


13.1.13 3DSecure 1.0 Authentication / ACS Response
 Path/Name                           XML        Description
                                    Schema
                                     type
 v1:AcsResponse                     xs:string   Merchant Data from ACS redirection POST attribute (“MD”attribute).
 v1:MD                                          Please note, that this element might not be sent back by the issuer
                                                (ACS) in case of EMV 3DS protocol (3DS 2.0)
 v1:AcsResponse                     xs:string   Represents PARes data from ACS redirection POST attribute (“PARes”
 v1:PaRes                                       attribute).


13.1.14 UnionPay Secure Plus
 Path/Name                           XML        Description
                                    Schema
                                     type
 v1:Upop                            xs:string   Indicates, if transaction is going to be authenticated as SecurePlus
 v1:AuthenticateTransaction                     transaction. Set the element to “true”, if you would like to authenticate
                                                the transaction via Secure Plus.
 v1:Upop                            xs:string   Represents the response code from SMS verification authentication
 v1:SendSmsResponseCode                         response, possible values are “0-9”, max. 2 digits.
 v1:Upop                            xs:string   Represents the sms code from Secure Plus on Verify-Enrollment
 v1:VCode                                       request, minLength value=”1” ,maxLength value=”200”.
 v1:Upop                            xs:string   Represents a response from Secure Plus on Verify-Enrollment request,
 v1:ActivateStatus                              the element could be populated only with the following values: “A”,
                                                “Y”, “F”, “N” or “L”.
 v1:Upop                            xs:string   The element needs to be included in the Secure Plus request to verify
 v1:SecurePlusRequest                           the validity of sent sms code.




Web Service API Integration Guide                                                                     13. XML – Tag overv 87
13.1.15 UnionPay SecurePlusRequest
 Path/Name                           XML        Description
                                    Schema
                                     type
 v1:SecurePlusRequest               xs:string   The element needs to be sent in the Secure Plus request to verify the
 v1: SecurePlusVerify                           validity of sent sms code.
 SmsCodeRequest
 v1:SecurePlusRequest               xs:string   Represents the SMS code received on the cardholder’s mobile phone.
 v1: SecurePlusVerify               (32 max)
 SmsCodeRequest
 v1:smsCode


13.1.16 DE_DirectDebitTxType
 Path/Name                           XML        Description
                                    Schema
                                     type
 v1:DE_DirectDebitTxType/           xs:string   Stores the transaction type. Possible values are sale or void.
 v1:Type


13.1.17 DE_DirectDebitData
 Path/Name                           XML        Description
                                    Schema
                                     type
 v1:DE_DirectDebitData/             xs:string   Stores the bank code (Business Identifier Code) of the customer.
 v1:BIC                                         Please make sure that the value contains no spaces.
 v1:DE_DirectDebitData/             xs:string   Stores the IBAN (International Bank Account Number) of the customer.
 v1:IBAN                                        Please make sure that the value contains no spaces.
 v1:DE_DirectDebitData/             xs:string   Stores the SEPA mandate reference.
 v1:MandateReference
 v1:DE_DirectDebitData/             xs:string   Stores the type of SEPA mandate. Possible values are SINGLE for
 v1:MandateType                                 one-off debit collections, FIRST_COLLECTION when submitting the
                                                initial transaction related to a mandate for recurring Direct Debit
                                                collections or RECURRING_COLLECTION for subsequent recurring
                                                transactions. As a default, transactions where this parameter is not
                                                submitted by the merchant will be flagged as a single debit collection.
                                                Please note that it is mandatory to submit a MandateReference in
                                                case of recurring collections.
 v1:DE_DirectDebitData/             xs:string   Stores the reference to the date of the original mandate when
 v1:DateOfMandate                               performing recurring Direct Debit transactions.
                                                The date needs to be submitted in format YYYYMMDD.
                                                Please note that this is a mandatory field for recurring Direct Debit
                                                transactions.
 v1:DE_DirectDebitData/             xs:string   Stores the track data of a card when using a card reader instead of
 v1:TrackData                                   keying in card data (can optionally be used instead of transmitting
                                                BankCode and AccountNumber). The field needs to contain the
                                                concatenated track 2 and 3 data. The track data must include the track
                                                and field separators as they are stored on the card.
                                                Example for the track data separator from track data 1 and 2 without
                                                the data: %…?;…?3s

Web Service API Integration Guide                                                                    13. XML – Tag overv 88
13.1.18 PayPalTxType
 Path/Name                            XML        Description
                                     Schema
                                      type
 v1:PayPalTxType/                    xs:string   Stores the transaction type. Possible values are postAuth, return,
 v1:Type                                         credit and void.


13.1.19 Payment
 Path/Name                            XML        Description
                                     Schema
                                      type
 v1:Payment/                         xs:string   Stores the Hosted Data ID for the Data Vault product
 v1:HostedDataID
 v1:Payment/                         xs:string   Stores the Hosted Data ID for the Data Vault product in this store
 v1:HostedDataStoreID                            (only as technical user)
 v1:Payment/                         xs:string   Declines duplicate credit card or German direct debit accounts
 v1:DeclineHosted
 DataDuplicates
 v1:Payment/                         xs:string   Stores the number of instalments for a Sale transaction if the
 v1:numberOfInstallments                         customer pays the amount in several parts
 v1:Payment/                         xs:string   Indicates, if the installment interest has been applied; possible values
 v1:installmentsInterest                         “yes” or “no”
 v1:Payment/                         xs:string   Represents the number of months the first payment will be delayed;
 v1:installmentDelayMonths                       possible values in the range
                                                 <1; 99>
 v1:Payment/                         xs:string   If setup with the value = true indicates the cardholder applied for
 v1:revolvingPayment                             Revolving payment (available only for our distribution channel in
                                                 Japan).
 v1:Payment/                        xs:decimal   Stores the Sub Total of an order. If this member is set, then also
 v1:SubTotal                                     ChargeTotal has to be set.
 v1:Payment/                        xs:decimal   Stores the VAT of an order. If this member is set, then also SubTotal
 v1:ValueAddedTax                                has to be set.
 v1:Payment/                        xs:decimal   Stores the delivery amount of an order. If this member is set, then also
 v1:DeliveryAmount                               SubTotal has to be set.
 v1:Payment/                        xs:double    Stores the transaction amount. Make sure that the number of
 v1:ChargeTotal                                  positions after the decimal point does not exceed 2, e.g. 3.123 would
                                                 be invalid – however, 3.12, 3.1, and 3 are correct.
 v1:Payment/                         xs:string   Stores the currency as a three-digit ISO 4217 value (e. g. 978 for Euro)
 v1:Currency


13.1.20 TransactionDetails
 Path/Name                            XML        Description
                                     Schema
                                      type
 v1:TransactionDetails/              xs:string   Stores the order ID. This must be unique per Store ID. If no Order ID is
 v1:OrderId                                      transmitted, the Gateway will generate one automatically.
                                                 Note: For cases where you plan to use EMV 3DS Authentication prior
                                                 to the authoriazation, please use only the following characters in
                                                 OrderId: A-Z, a-z, 0-9, ‘-‘

Web Service API Integration Guide                                                                     13. XML – Tag overv 89
 v1:TransactionDetails/              xs:string   Allows you to assign a unique ID for the transaction. This ID
 v1:MerchantTransactionId                        can be used to reference to this transactions in a Void request
                                                 (ReferencedMerchantTransactionId) or to retrieve transaction details
                                                 with the API action InquiryTransaction. Uniqueness needs to be
                                                 enforced by the merchant.
 v1:TransactionDetails/              xs:string   Stores the customer’s IP address which can be used by the Web
 v1:Ip                                           Service API for fraud detection by IP address. Make sure that you
                                                 supply the IP in the format xxx.xxx.xxx.xxx, e.g. 128.0.10.2 would be a
                                                 valid IP.
 v1:TransactionDetails/              xs:string   Stores the six digit reference number you have received as the result
 v1:ReferenceNumber                              of a successful external authorization (e.g. by phone). The Gateway
                                                 needs this number for uniquely mapping a ForceTicket transaction to
                                                 a previously performed external authorization.
 v1:TransactionDetails/              xs:string   Stores the purchasing card Level II and Level III transaction data.
 v1:PurchaseCard
 v1:TransactionDetails/              xs:string   Stores the TDate of the Sale, PostAuth, ForceTicket, Return, or Credit
 v1:TDate                                        transaction this Void transaction refers to. A TDate value is returned
                                                 within the response to a successful transaction of one of these
                                                 five types. When performing a Void transaction, you have to pass
                                                 the TDate in addition to the order ID for uniquely identifying the
                                                 transaction to be voided. The scenario presented below gives an
                                                 example.
 v1:TransactionDetails/              xs:string   Stores the MerchantTransactionId of the Sale, PostAuth, ForceTicket,
 v1:ReferencedMerchant                           Return, or Credit transaction this Void transaction refers to.
 TransactionId                                   This can be used as an alternative to TDate if you assigne a
                                                 MerchantTransactionId in the original transaction request
 v1:TransactionDetails/              xs:string   The source of the transaction. The possible values are ECI (if the order
 v1:TransactionOrigin                            was received via email or Internet), MOTO (mail order / telephone
                                                 order), MAIL (mail order), PHONE (telephone order) and RETAIL
                                                 (face to face).
 v1:TransactionDetails/               xs:int     Stores the total number of shipments in case of split shipment.
 v1:SplitShipment/                               Can either be included in the PreAuth or the first PostAuth. A different
 v1:SequenceCount                                value in the first PostAuth overwrites the value from the PreAuth.
 v1:TransactionDetails/             xs:boolean Needs to be set to “true” in the final PostAuth of a series of split
 v1:SplitShipment/                             shipments.
 v1:FinalShipment
 v1:TransactionDetails/              xs:string   Stores the invoice number.
 v1:InvoiceNumber
 v1:TransactionDetails/              xs:string   Stores the purchase order number.
 v1:PONumber
 v1:TransactionDetails/              xs:string   Stores a dynamic merchant name for the cardholder’s statement
 v1:DynamicMerchantName
 v1:TransactionDetails/              xs:string   Stores the comments.
 v1:Comments
 v1:TransactionDetails/             xs:boolean For merchants with recurring payments to receive a Merchant Advice
 v1:MerchantAdvice                             Code from the issuer that provides detailed reasons and advice for
 CodeSupported                                 declined transactions. Available only for merchants that authorize on
                                               Nashville.
 v1:TransactionDetails/              xs:string   Indicates the reason to skip Strong Customer Authentication (SCA),
 v1:SCAExemptionIndicators                       e.g. 3-D Secure with submitting directly an authorization request. For
                                                 available values and more details see the chapter 13.2.30
 v1:TransactionDetails/             xs:boolean Needs to be set to ‘true’, for transactions handling a cryptocurrency
 v1:HighRisk                                   and initiated from a MCC 6051(Quasi Cash—Merchant) store; or for
 PurchaseIndicator                             transactions handling high risk securities initiated from the store with
                                               MCC 6211 (Securities—Brokers/ Dealers).
Web Service API Integration Guide                                                                     13. XML – Tag overv 90
 v1:TransactionDetails/              xs:string   8 characters Visa Merchant Identifier assigned by Visa, required for
 v1:vmid                                         Trusted Merchant and Delegated Authentication. Can be used only if
                                                 you are enrolled with Visa’s Delegated Authentication program.
 v1:TransactionDetails/             xs:boolean Set it to “true” if the transaction is set for deferred authorization.
 v1:IpgDeferredAuth


13.1.21 Purchasing Cards
 Path/Name                            XML        Description
                                     Schema
                                      type
 v1:PurchaseCard                     xs:string   A reference to a Customer Code/Customer Reference ID
 v1:CustomerReferenceID              (20max)
 v1:PurchaseCard                     xs:string   A reference to a Purchase Identifier/Merchant related data.
 v1:SupplierInvoiceNumber            (30max)
 v1:PurchaseCard                     xs:string   Represents a Merchant VAT registration/Single Business Reference
 v1:SupplierVAT                      (30max)     Number/Merchant Tax ID or Corporation VAT Number
 RegistrationNumber
 v1:PurchaseCard                     xs:string   Represents the total discount amount applied to a transaction (i.e.
 v1:TotalDiscount                                total transaction percentage discounts, fixed transaction amount
 AmountAndRate                                   reductions or summarization of line item discounts).
 v1:PurchaseCard                     xs:string   Represents the total freight/shipping amount applied to a transaction.
 v1:VATShipping
 AmountAndRate
 v1:PurchaseCard                     xs:string   Represents mandatory data for Level III transactions.
 v1:LineItemData


13.1.22 Purchasing Cards / Line Item Data
 Path/Name                            XML        Description
                                     Schema
                                      type
 v1:LineItemData                    xs:numeric A reference to a commodity code used to classify purchased item
 v1:CommodityCode                    (positive,
                                       4max)
 v1:LineItemData                     xs:string   A reference to a merchant product identifier, the Universal Product
                                     (20max)     Code (UPC) of purchased item
 v1:LineItemData                     xs:string   Represents a description of purchased item
                                     (30max)
 v1:LineItemData                    xs:numeric Represents a quantity of purchased items.
                                       (min
                                     Inclusive
                                    value=”1”)
 v1:LineItemData                     xs:string   Represents a unit of measure of purchased items
                                     (3 max)
 v1:LineItemData                    xs:decimal   Represents mandatory data for Level III transactions.
 v1:LineItemData                    xs:decimal   Represents a rate of the VAT amount, e.g. 0.09 (means 9%)
 v1:LineItemData                    xs:decimal   Represents a rate of the discount amount, e.g. 0.09 (means 9%)
 v1:LineItemData                    xs:decimal   This field is a calculation of the unit cost multiplied by the quantity
                                                 and less the discount per line item. The calculation is reflected as:
                                                 [Unit Cost * Quantity] - Discount per Line Item = Line Item Total.



Web Service API Integration Guide                                                                       13. XML – Tag overv 91
13.1.23 InquiryRateReference
 Path/Name                            XML        Description
                                     Schema
                                      type
 v1:InquiryRateReference/            xs:long     A reference to a rate-inquiry for transactions with Global Choice™ or
 v1:InquiryRateId                                Dynamic Pricing.

 v1:InquiryRateReference/           xs:boolean Specifies whether a cardholder has choosen to accept the proposed
 v1:DccApplied                                 currency conversion offering when using Global Choice™.


13.1.24 Billing
 Path/Name                            XML        Description
                                     Schema
                                      type
 v1:Billing/                         xs:string   Stores your ID for your customer
 v1:CustomerID
 v1:Billing/                         xs:string   Stores the customer’s name. If provided, it will appear on your
 v1:Name                                         transaction reports.
                                                 Please note that this is a mandatory field for SEPA Credit Transfers
 v1:Billing/                         xs:string   Stores the customer’s company. If provided, it will appear on your
 v1:Company                                      transaction reports.
 v1:Billing/                         xs:string   Stores the first line of the customer’s address. If provided, it will
 v1:Address1                                     appear on your transaction reports.
 v1:Billing/                         xs:string   Stores the second line of the customer’s address. If provided, it will
 v1:Address2                                     appear on your transaction reports.
 v1:Billing/                         xs:string   Stores the customer’s city. If provided, it will appear on your
 v1:City                                         transaction reports.
 v1:Billing/                         xs:string   Stores the customer’s state. If provided, it will appear on your
 v1:State                                        transaction reports.
 v1:Billing/                         xs:string   Stores the customer’s zip code. If provided, it will appear on your
 v1:Zip                                          transaction reports.
 v1:Billing/                         xs:string   Stores the customer’s country. If provided, it will appear on your
 v1:Country                                      transaction reports.
 v1:Billing/                         xs:string   Stores the customer’s phone number. If provided, it will appear on
 v1:Phone                                        your transaction reports.
 v1:Billing/                         xs:string   Stores the customer’s fax number. If provided, it will appear on your
 v1:Fax                                          transaction reports.
 v1:Billing/                         xs:string   Stores the customer’s Email address. If provided, it will appear on your
 v1:Email                                        transaction reports. If you are using the email transaction notification
                                                 feature, this email address will be used for notifications to your
                                                 customer.


13.1.25 Shipping
 Path/Name                            XML        Description
                                     Schema
                                      type
 v1:Shipping/                        xs:string   Stores the name of the recipient. If provided, it will appear on your
 v1:Name                                         transaction reports.


Web Service API Integration Guide                                                                        13. XML – Tag overv 92
 v1:Shipping/                        xs:string   Stores the first line of the shipping address. If provided, it will appear
 v1:Address1                                     on your transaction reports.
 v1:Shipping/                        xs:string   Stores the second line of the shipping address. If provided, it will
 v1:Address2                                     appear on your transaction reports.
 v1:Shipping/                        xs:string   Stores the recipient’s city. If provided, it will appear on your
 v1:City                                         transaction reports.
 v1:Shipping/                        xs:string   Stores the recipient’s state. If provided, it will appear on your
 v1:State                                        transaction reports.
 v1:Shipping/                        xs:string   Stores the recipient’s zip code. If provided, it will appear on your
 v1:Zip                                          transaction reports.
 v1:Shipping/                        xs:string   Stores the recipient’s country. If provided, it will appear on your
 v1:Country                                      transaction reports.


13.1.26 ClientLocale
 Path/Name                            XML        Description
                                     Schema
                                      type
 v1:ClientLocale/                    xs:string   If you are using the email transaction notification feature, this
 v1:Language                                     language will be used for notifications to your customer. Possible
                                                 values are: de, en, it.
 v1:ClientLocale/                    xs:string   Specifies the variant of the language. This member can only be set
 v1:Country                                      if the language is set. Possible values are: DE, GB, IT. If you do not
                                                 define a country, a matching country will be chosen.
If you do not submit language information in the transaction, the language settings of your store will be used for
the email notifications.


13.1.27 RequestCardRateForDCC
 Path/Name                            XML        Description
                                     Schema
                                      type
 v1:RequestCardRate                 xs:string    Your Store ID. The base currency is derived from the Store settings.
 ForDCC/                            (max 20)
 v1:StoreId
 v1:RequestCardRate                   xs:int     The credit cards’ Bank Identifier Number (first 6 digits of credit card
 ForDCC/                                         number)
 v1:BIN
 v1:RequestCardRate                 xs:decimal   The amount to be converted (optional).
 ForDCC/
                                                 When no amount is given in the request, no amount will be returned,
 v1:BaseAmount
                                                 only the conversion rate.


13.1.28 RequestMerchantRateForDynamicPricing
 Path/Name                            XML        Description
                                     Schema
                                      type
 v1:RequestCardRate                 xs:string    Your Store ID. The base currency is derived from the Store settings
 ForDCC/                            (max 20)
 v1:StoreId



Web Service API Integration Guide                                                                       13. XML – Tag overv 93
 v1:RequestCardRate                   xs:string    The currency to be converted to. (ISO_4217 format)
 ForDCC/
 v1:ForeignCurrency
 v1:RequestCardRate                 xs:decimal     The amount to be converted (optional).
 ForDCC/
                                                   When no amount is given in the request, no amount will be returned,
 v1:BaseAmount
                                                   only the conversion rate.


13.1.29 CardRateForDCC and MerchantRateForDynamicPricing
 Path/Name                             XML         Description
                                      Schema
                                       type
 <InquiryRateType>/                   xs:long      The Store ID. The base currency is derived from the Store’s settings.
 v1: InquiryRateId
 <InquiryRateType>/                   xs:string    The currency that the amount has been converted to (ISO_4217
 v1:ForeignCurrencyCode                            format)
 <InquiryRateType>/                 xs:decimal     The converted amount.
 v1:ForeignAmount
 <InquiryRateType>/                 xs:decimal     The exchange rate of the currency conversion
 v1:ExchangeRate
 <InquiryRateType>/                 xs:boolean Whether the user accepted the DCC offering or not.
 v1:DccApplied
 <InquiryRateType>/                 xs:boolean Whether an offering for dynamic currency conversion was extended
 v1:DccOffered
 <InquiryRateType>/                   xs:date      Timestamp after which this DCC offering expires
 v1:ExpirationTimestamp                Time
 <InquiryRateType>/                 xs:decimal     Optional margin information.
 v1:MarginRatePercentage
 <InquiryRateType>/                   xs:string    The source of the currency conversion.
 v1:ExchangeRate
 SourceName
 <InquiryRateType>/                   xs:date      The timestamp when the source has done the currency conversion
 v1:ExchangeRate                       Time
 SourceSourceTimestamp
Note:Instead of <InquiryRateType> substitute either CardRateForDCC or MerchantRateForDynamicPricing


13.1.30 MCC 6012 Visa and Mastercard Mandate
For UK-based Financial Institutions with Merchant Category Code 6012, Visa and Mastercard have mandated
additional information of the primary recipient of the loan to be included in the authorization message.
If you are a UK 6012 merchant use the following parameters for your transaction request:

 Path/Name                             XML         Description
                                      Schema
                                       type
 v1:MCC6012Details/                   xs:string    Date of birth in format YYYYMMDD
 v1:BirthDate
 v1:MCC6012Details/                   xs:string    First 6 digits of recipient PAN (where the primary recipient account is
 v1:AccountFirst6                                  a card)
 v1:MCC6012Details/                   xs:string    Last 4 digits of recipient PAN (where the primary recipient account is
 v1:AccountLast4                                   a card)



Web Service API Integration Guide                                                                       13. XML – Tag overv 94
 v1:MCC6012Details/                       xs:string    Recipient account number (where the primary recipient account is not
 v1:AccountNumber                         (max 50)     a card)
 v1:MCC6012Details/                       xs:string    Post Code
 v1:PostCode                              (max 50)
 v1:MCC6012Details/                        xs:string   Surname
 v1:Surname                               (max 100)
If you are a UK merchant with Merchant Category Code 6051 and 7299, you can optionally use the same MCC6012
parameters in your request for debt repayment transactions.


13.1.31 Market Segment Addendum
Card transactions in specific market segments can obtain incentive rates when they include addendum data.
The Web Service API allows you to submit addendum data for the following industries:

 Airlines                                              v1:AirlineDetails, v1: TravelRoute
 (MCC 3000-3299 or 4511
 Car Rental                                            v1:CarRental
 (MCC 3351-3500, 7512, 7513 or 7519
 Hotel Lodgings                                        v1:HotelLodgings
 (MCC 3501-3999 or 7011)
Please see v1.xsd for details (link in Appendix).


13.1.32 SCA Exemptions
Following PSD2 mandate requirements you are able to request an exemption from Strong Customer Authentication
(SCA) with including one of the available SCAExemptionIndicators in your transaction request to the Gateway.

 v1:SCAExemptionIndicator/                             Used for transaction amounts below 30 EUR or respective value in
 Low Value Exemption                                   other European currencies.
 v1:SCAExemptionIndicator/                             Used for cases where transaction risk analysis has been already
 TRA Exemption                                         perfomed.
 v1:SCAExemptionIndicator/                             Used for cases where merchant has been flagged as trusted by their
 Trusted Merchant Exemption                            customers.
 v1:SCAExemptionIndicator/                             Used for secure corporate payments transactions.
 SCP Exemption
 v1:SCAExemptionIndicator/                             Authentication failure must persist for at least five minutes, leading all
 Authentication Outage Exception                       authentications to fail (i.e. no attempt responses provided) before the
                                                       Authentication Outage Exception is used.
 v1:SCAExemptionIndicator/                             Used for cases where the issuer delegated SCA to the merchant.
 Delegated Authentication
Note: PSD2 mandate is only applicable for European distribution channels.


13.2.31 China Domestic
 Path/Name                                  XML        Description
                                           Schema
                                            type
 v1:Transaction/                           xs:string   Transaction types enabled for WeChat payment option, available
 v1:WeChatTxType                                       values :sale, return
 v1:Transaction/                           xs:string   Transaction types enabled for Alipay payment option, available values:
 v1:AlipayTxType                                       sale, return
 v1:Transaction/                           xs:string   Transaction types enabled for Union Pay e-banking and QuickPay
 v1:CUPDomesticTxType                                  payment options, available values: sale, return


Web Service API Integration Guide                                                                            13. XML – Tag overv 95
 v1:Transaction/                     xs:string   Mandatory element for transactions with China Domestic payment
 v1:ChinaDomestic                                methods
 Information
 v1:Transaction/                    xs:boolean Allows only debit cards to be processed
 v1:ChinaDomestic
 Information
 v1:LimitCard
 FunctionToDebit
 v1:Transaction/                     xs:string   Identification of the customer, the field is optional, but recommended
 v1:ChinaDomestic                     32max      in case the consumer has this information
 Information
 v1:CustomerId
 v1:Transaction/                     xs:string   Product code of purchased item, all available values could be found
 v1:ChinaDomestic                     32max      here: https://docs.firstdata.com/org/gateway/node/401
 Information
 v1:ProductCode
 v1:Transaction/                      xs:int     Quantity of purchased products
 v1:ChinaDomestic
 Information
 v1:ProductQuantity
 v1:Transaction/                    xs:decimal   Price of purchased product
 v1:ChinaDomestic
 Information
 v1:ProductPrice
 v1:Transaction/                     xs:string   Description of purchased products
 v1:ChinaDomestic                    100max
 Information
 v1:ProductDescription
 v1:Transaction/                    xs:anyURI    URL where you want the Chinese platform (PNR) redirect you to after
 v1:ChinaDomestic                                the transaction processing have been competed on their end
 Information
 v1:RedirectUrl
 v1:Transaction/                     xs:string   Identification of the bank, the field is required only for payment
 v1:ChinaDomestic                     8max       methods Union Pay e-banking and Union Pay QuickPay
 Information
 v1:BankId


13.2.32 EMI with ICICI Debit Card
The following elements to be included in a transaction request with Equated Monthly Installments (EMI) payments
with the debit card issued by ICICI.
Please note, that this functionality is only available in India.

 Path/Name                            XML        Description
                                     Schema
                                      type
 v1:Transaction/                     xs:string   EMI Common element contains basic information about EMI, such as
 v1:EMIDetails/                                  mobile number and EMI Indicator
 v1:EMICommon
 v1:Transaction/                     xs:string   Consists of all EMI specific transactions details
 v1:EMIDetails/
 v1:EMIPlan
 v1:Transaction/                     xs:string   EMI Direct Integration element includes the values for EMI, that
 v1:EMIDetails/                                  have been retrieved outside the Gateway and then submitted in the
 v1:DirectIntegration                            transaction request

Web Service API Integration Guide                                                                     13. XML – Tag overv 96
 v1:Transaction/                     xs:string   Customer’s mobile phone number.
 v1:EMIDetails/
 v1:EMICommon
 v1:MobileNumber
 v1:Transaction/                     xs:string   EMI Indicator
 v1:EMIDetails/                       10max
 v1:EMICommon
 v1:Indicator
 v1:Transaction/                     xs:string   Transaction amount = Product Amount - Discount
 v1:EMIDetails/
 v1:EMIPlan
 v1:TransactionAmount
 v1:Transaction/                     xs:string   Full product amount (without discount)
 v1:EMIDetails/
 v1:EMIPlan
 v1:ProductAmount
 v1:Transaction/                     xs:string   Discount offered for the product
 v1:EMIDetails/
 v1:EMIPlan
 v1:DiscountAmount
 v1:Transaction/                      xs:int     Tenure in months, min=1, max=12
 v1:EMIDetails/
 v1:EMIPlan
 v1:Tenure
 v1:Transaction/                    xs:decimal   EMI transaction interest rate
 v1:EMIDetails/
 v1:EMIPlan
 v1:InterestRate
 v1:Transaction/                     xs:string   The processing fee for EMI transaction
 v1:EMIDetails/
 v1:EMIPlan
 v1:ProcessingFee
 v1:Transaction/                     xs:string   Total amount to be paid by a customer; EMI per month x Tenure
 v1:EMIDetails/
 v1:EMIPlan
 v1:TotalAmount
 v1:Transaction/                     xs:string   Transaction amount per month
 v1:EMIDetails/
 v1:EMIPlan
 v1:AmountPerMonth
 v1:Transaction/                     xs:string   Merchant’s reference value, unique transaction ID
 v1:EMIDetails/                       20max
 v1:DirectIntegration
 v1:MerchantReference
 v1:Transaction/                     xs:string   Issuer eligibility reference number for EMI specific transactions
 v1:EMIDetails/                       20max
 v1:DirectIntegration
 v1:IssuerEligibilityReference
 v1:Transaction/                     xs:string   Bank System Trace Audit Number
 v1:EMIDetails/                       12max
 v1:DirectIntegration




Web Service API Integration Guide                                                                     13. XML – Tag overv 97
13.2.33 Boleto
 Path/Name                            XML        Description
                                     Schema
                                      type
 v1:Transaction/                     xs:string   Transaction types enabled for Boleto, available value:sale
 v1:BoletoTxType
 v1:Transaction/                     xs:string   Authorizer code assigned by Software Express, available values for
 v1:AuthorizerId                                 Boleto are:
                                                 Itaú Shopline (authorizer_id = 7)
                                                 Banco do Brasil – Boleto (authorizer_id = 404
For more information about the activation of this payment method please reach out to your local support team.


13.2.33 StandIn Details
The following elements to be included in a transaction request with Standin Instruction payments.
Please note, that this functionality is only available in India.

 Path/Name                            XML        Description
                                     Schema
                                      type
 v1:Transaction/                     xs:string   Indicates standin instruction Type, available values:
 v1: StandInDetails/
                                                 FIXED_AMOUNT
 v1: StandInType
                                                 MAXIMUM_AMOUNT
 v1:Transaction/                     xs:string   Indicates number of standin instruction debits. Possible values can be
 v1: StandInDetails/                             two digit number or UN (Until it is cancelled, only for Visa) or ND (Not
 v1: NumberOfDebits                              defined, only for Visa)
 v1:Transaction/                    xs:boolean Indicates standin instruction validation flag, it can be true or false.
 v1: StandInDetails/                           "false" - Not validated, "true" - Validated
 v1: SIValidated
 v1:Transaction/                     xs:string   Represents maximum debit amount per standin instruction
 v1: StandInDetails/                             transaction
 v1: Maximum
 TransactionAmount
 v1:Transaction/                     xs:string   Unique identifier for SI mandate
 v1: StandInDetails/                 (10max)
 v1: SIHubID
 v1:Transaction/                     xs:string   Indicates frequency of the standin instruction debit, available values:
 v1: StandInDetails/                             WEEKLY
 v1: Frequency                                   FORTNIGHTLY
                                                 MONTHLY
                                                 QUARTERLY
                                                 HALFYEARLY
                                                 YEARLY
                                                 UNSCHEDULED




Web Service API Integration Guide                                                                        13. XML – Tag overv 98
14. Custom Parameters
You can send up to ten additional parameters as individual key-value pairs. The values will be stored so that they
can be returned in Inquiry Actions and be visible in the Virtual Terminal’s Order Details view.
Please refer to the element AdditionalRequestParamaters in the XSD.


14.1.1 Additional parameters for Fraud Detect
In case you use the Fraud Detect product and want to pass mobile device details for the scoring, you need to pass
these with the following parameter naming:
• deviceRiskId
• deviceRiskAPIKey
• deviceRiskHost
The following represents an example of a Sale transaction with Fraud Detect special custom parameters:
<soapenv:Envelope xmlns:soapenv=”http://schemas.xmlsoap.org/soap/envelope/”
xmlns:ipg=”http://ipg-online.com/ipgapi/schemas/ipgapi” xmlns:v1=”http://ipg-online.
com/ipgapi/schemas/v1”>
   <soapenv:Header/>
   <soapenv:Body>
      <ipg:IPGApiOrderRequest>
         <!--You have a CHOICE of the next 2 items at this level-->
         <v1:Transaction>
            <!--You have a CHOICE of the next 9 items at this level-->
            <v1:CreditCardTxType>
               <!--Optional:-->
               <v1:StoreId>120995000</v1:StoreId>
                <v1:Type>sale</v1:Type>
            </v1:CreditCardTxType>
            <!--You have a CHOICE of the next 2 items at this level-->
            <!--Optional:-->
            <v1:CreditCardData>
               <v1:CardNumber>4257********0111</v1:CardNumber>
            <v1:ExpMonth>12</v1:ExpMonth>
            <v1:ExpYear>17</v1:ExpYear>
            <v1:CardCodeValue>XXX</v1:CardCodeValue>
            </v1:CreditCardData>
            <!--Optional:-->
            <v1:Payment>
                <v1:ChargeTotal>10.00</v1:ChargeTotal>
                <v1:Currency>GBP</v1:Currency>
            </v1:Payment>
            <v1:TransactionDetails>
               <v1:AdditionalRequestParameters>
                  <v1:keyValuePair>
                     <v1:key>deviceRiskId</v1:key>
                     <v1:value>********</v1:value>
                  </v1:keyValuePair>
                  <v1:keyValuePair>
                     <v1:key>deviceRiskHost</v1:key>
                     <v1:value>*********</v1:value>
                  </v1:keyValuePair>
                  <v1:keyValuePair>
                     <v1:key>deviceRiskAPIKey</v1:key>
                     <v1:value>************</v1:value>

Web Service API Integration Guide                                                             14. Custom Parameters 99
                   </v1:keyValuePair>
               </v1:AdditionalRequestParameters>
               <v1:TransactionOrigin>ECI</v1:TransactionOrigin>
            </v1:TransactionDetails>
            <!--Optional:-->
            <v1:ClientLocale>
               <!--You may enter the following 2 items in any order-->
               <v1:Language>en</v1:Language>
               <!--Optional:-->
               <v1:Country>GB</v1:Country>
            </v1:ClientLocale>
         </v1:Transaction>
      </ipg:IPGApiOrderRequest>
   </soapenv:Body>
</soapenv:Envelope>




Web Service API Integration Guide                                   14. Custom Parameters 100
15. Building a SOAP Request Message
After building your transaction in XML, a SOAP request message describing the Web Service operation call, you
wish to perform, has to be created. That means while the XML-encoded transaction you have established as
described in the previous chapter represents the operation argument, the SOAP request message encodes the
actual operation call. Building such a SOAP request message is a rather straightforward task. The complete SOAP
message wrapping the XML-Sale-transaction looks as follows:
<?xml version=”1.0” encoding=”UTF-8”?>
<SOAP-ENV:Envelope
xmlns:SOAP-ENV=”http://schemas.xmlsoap.org/soap/envelope/”>
      <SOAP-ENV:Header />
      <SOAP-ENV:Body>
		<ipgapi:IPGApiOrderRequest
		xmlns:v1=”http://ipg-online.com/ipgapi/schemas/v1”
		xmlns:ipgapi=”http://ipg-online.com/ipgapi/schemas/ipgapi”>
			<v1:Transaction>
				<v1:CreditCardTxType>
					<v1:Type>sale</v1:Type>
				</v1:CreditCardTxType>
				<v1:CreditCardData>
					<v1:CardNumber>
						4111********1111
					</v1:CardNumber>
					<v1:ExpMonth>12</v1:ExpMonth>
					<v1:ExpYear>07</v1:ExpYear>
				</v1:CreditCardData>
				<v1:Payment>
					<v1:ChargeTotal>19.00</v1:ChargeTotal>
					<v1:Currency>978</v1:Currency>
				</v1:Payment>
			</v1:Transaction>
		</ipgapi:IPGApiOrderRequest>
      </SOAP-ENV:Body>
</SOAP-ENV:Envelope>
In short, the SOAP request message contains a SOAP envelope consisting of a header and a body. While no
specific header entries are required for calling the Web Service, the SOAP body takes the transaction XML
document as sub element as shown above. Note that there are no further requirements for transactions of a type
other than Sale. That means the general format of the SOAP request message regardless of the actual transaction
type is as follows:
<?xml version=”1.0” encoding=”UTF-8”?>
<SOAP-ENV:Envelope
xmlns:SOAP-ENV=”http://schemas.xmlsoap.org/soap/envelope/”>
      <SOAP-ENV:Header />
      <SOAP-ENV:Body>
		<ipgapi:IPGApiOrderRequest
		xmlns:ipgapi=”http://ipg-online.com/ipgapi/schemas/ipgapi”
		xmlns:v1=”http://ipg-online.com/ipgapi/schemas/v1”>
			<v1:Transaction>
				<!-- transaction content -->
			</v1:Transaction>
		</ipgapi:IPGApiOrderRequest>
      </SOAP-ENV:Body>
</SOAP-ENV:Envelope>
Finally, you may have noticed that there are no specific entries describing which Web Service operation to call. In
fact, the Authipay Gateway automatically maps the ipgapi:IPGApiOrderRequest element to the corresponding Web
Service operation.
Web Service API Integration Guide                                               15. Building a SOAP Request Message 101
16. Reading the SOAP Response Message
The SOAP response message may be understood as the Web Service operation result. Hence, processing the
SOAP request message may have either resulted in a SOAP response message in the success case (i.e. the return
parameter) or a SOAP fault message in case of a failure (i.e. the thrown exception). Both SOAP message types are
contained in the body of the HTTP response message.


16.1.1 SOAP Response Message
A SOAP response message is received as the result to the credit card processor (started by the First DataAuthipay
gateway) having approved your transaction. It always has the following scheme:
<?xml version=”1.0” encoding=”UTF-8”?>
<SOAP-ENV:Envelope
		xmlns:SOAP-ENV=”http://schemas.xmlsoap.org/soap/envelope/”>
      <SOAP-ENV:Header />
      <SOAP-ENV:Body>
		<ipgapi:IPGApiOrderResponse
		xmlns:ipgapi=”http://ipg-online.com/ipgapi/schemas/ipgapi”>
			                <!-- transaction result -->
		</ipgapi:IPGApiOrderResponse>
      </SOAP-ENV:Body>
</SOAP-ENV:Envelope>
If you have send an Action, you get an ipgapi:IPGApiActionResponse.
Again, no headers are defined. The SOAP body contains the actual transaction result contained in the
ipgapi:IPGApiOrderResponse or ipgapi:IPGApiOrderRequest element. Its sub elements and their meanings are
presented in the next chapter. However, in order to provide a quick example, an approved Sale transaction is
wrapped in a SOAP message similar to the following example:
<?xml version=”1.0” encoding=”UTF-8”?>
<SOAP-ENV:Envelope
		xmlns:SOAP-ENV=”http://schemas.xmlsoap.org/soap/envelope/”>
      <SOAP-ENV:Header />
      <SOAP-ENV:Body>
		<ipgapi:IPGApiOrderResponse
		xmlns:ipgapi=”http://ipg-online.com/ipgapi/schemas/ipgapi”>
			<ipgapi:CommercialServiceProvider>
				BNLP
			</ipgapi:CommercialServiceProvider>
			<ipgapi:TransactionTime>
				1192111687392
			</ipgapi:TransactionTime>
			<ipgapi:ProcessorReferenceNumber>
				3105
			</ipgapi:ProcessorReferenceNumber>
			<ipgapi:ProcessorResponseMessage>
				Function performed error-free
			</ipgapi:ProcessorResponseMessage>
			<ipgapi:ErrorMessage />
			<ipgapi:OrderId>
				62e3b5df-2911-4e89-8356-1e49302b1807
			</ipgapi:OrderId>
			<ipgapi:ApprovalCode>
				Y:440368:0000057177:PPXM:0043364291
			</ipgapi:ApprovalCode>
			<ipgapi:AVSResponse>PPX</ipgapi:AVSResponse>
			<ipgapi:TDate>1192140473</ipgapi:TDate>

Web Service API Integration Guide                                             16.Reading the SOAP Response Message 102
			<ipgapi:TransactionResult>
				APPROVED
			</ipgapi:TransactionResult>
			<ipgapi:TerminalID>123456</ipgapi:TerminalID>
			<ipgapi:ProcessorResponseCode>
				00
			</ipgapi:ProcessorResponseCode>
			<ipgapi:ProcessorApprovalCode>
				440368
			</ipgapi:ProcessorApprovalCode>
			<ipgapi:ProcessorReceiptNumber>
				4291
			</ipgapi:ProcessorReceiptNumber>
			<ipgapi:ProcessorTraceNumber>
				004336
			</ipgapi:ProcessorTraceNumber>
		</ipgapi:IPGApiOrderResponse>
      </SOAP-ENV:Body>
</SOAP-ENV:Envelope>


16.1.2 SOAP Fault Message
In general, a SOAP fault message returned by the Web Service API has the following format:
<?xml version=”1.0” encoding=”UTF-8”?>
<SOAP-ENV:Envelope
		xmlns:SOAP-ENV=”http://schemas.xmlsoap.org/soap/envelope/”>
  <SOAP-ENV:Header />
  <SOAP-ENV:Body>
		<SOAP-ENV:Fault>
			 <faultcode>SOAP-ENV:Client</faultcode>
			 <faultstring xml:lang=”en-US”>
				          <!-- fault message -->
			 </faultstring>
			 <detail>
				          <!-- fault message -->
			 </detail>
		</SOAP-ENV:Fault>
  </SOAP-ENV:Body>
</SOAP-ENV:Envelope>
Basically, the faultstring element carries the fault type. According to the fault type, the other elements are set. Note
that not all of the above shown elements have to occur within the SOAP-ENV:Fault element. Which elements exist
for which fault type is described in the upcoming sections.


16.1.3 SOAP-ENV:Server
In general, this fault type indicates that the Web Service has failed to process your transaction due to an internal
system error. If you receive this as response, please contact our support team to resolve the problem.
An InternalException always looks like the example below:
<?xml version=”1.0” encoding=”UTF-8”?>
<SOAP-ENV:Envelope
		xmlns:SOAP-ENV=”http://schemas.xmlsoap.org/soap/envelope/”>
  <SOAP-ENV:Header />
  <SOAP-ENV:Body>
		<SOAP-ENV:Fault>
			 <faultcode>SOAP-ENV:Server</faultcode>
			 <faultstring xml:lang=”en-US”>

Web Service API Integration Guide                                                16.Reading the SOAP Response Message 103
				          unexpected error
			 </faultstring>
		</SOAP-ENV:Fault>
  </SOAP-ENV:Body>
</SOAP-ENV:Envelope>

The SOAP fault message elements – relative to the SOAP-ENV:Envelope/SOAP-ENV:Body/SOAP-ENV:Fault
element – are set as follows:

 Path/Name                           XML        Description
                                    Schema
                                     type
 faultcode                          xs:string   This element is always set to SOAP-ENV:Server, indicating that the
                                                fault cause is due to the system underlying the API having failed.
 faultstring                        xs:string   This element always carries the following fault string:
                                                unexpected error


16.1.4 SOAP-ENV:Client
MerchantException
This fault type occurs if the Gateway can trace back the error to your store having passed incorrect information.
This may have one of the following reasons:
1. o
    ur store is registered as being closed. In case you will receive this information despite your store being
   registered as open, please contact support.
2. The store ID / user ID combination you have provided for HTTPS authorization is syntactically incorrect.
3. The XML does not match the schema.
A MerchantException always looks as shown below:
<?xml version=”1.0” encoding=”UTF-8”?>
<SOAP-ENV:Envelope
		xmlns:SOAP-ENV=”http://schemas.xmlsoap.org/soap/envelope/”>
  <SOAP-ENV:Header />
  <SOAP-ENV:Body>
		<SOAP-ENV:Fault>
			 faultcode>SOAP-ENV:Client</faultcode>
			 <faultstring xml:lang=”en-US”>
				          MerchantException
			 </faultstring>
			 <detail>
				          <!-- detailed explanation. -->
			 </detail>
		</SOAP-ENV:Fault>
  </SOAP-ENV:Body>
</SOAP-ENV:Envelope>
The SOAP fault message elements – relative to the SOAP-ENV:Envelope/SOAP-ENV:Body/SOAP-ENV:Fault
element – are set as follows:

 Path/Name                           XML        Description
                                    Schema
                                     type
 faultcode                          xs:string   This element is always set to SOAP-ENV:Client
 faultstring                        xs:string   This element is always set to MerchantException
 detail/reason                      xs:string   Minimum one reason
See section Merchant Exceptions in the Appendix for detailed analysis of errors.
Web Service API Integration Guide                                                  16.Reading the SOAP Response Message 104
ProcessingException
A fault of this type is raised whenever the Gateway has detected an error while processing your transaction.
The difference to the other fault types is that the transaction passed the check against the xsd.
A ProcessingException always looks as shown below:
<SOAP-ENV:Envelope
		xmlns:SOAP-ENV=”http://schemas.xmlsoap.org/soap/envelope/”>
      <SOAP-ENV:Header />
      <SOAP-ENV:Body>
		<SOAP-ENV:Fault>
			<faultcode>SOAP-ENV:Client</faultcode>
			<faultstring xml:lang=”en-US”>
				ProcessingException: Processing the request
				                    resulted in an error - see SOAP details for more
				information
			</faultstring>
			<detail>
				<ipgapi:IPGApiOrderResponse
		xmlns:ipgapi=”https://ipg-online.com/ipgapi/schemes/ipgapi”>
					<ipgapi:CommercialServiceProvider>
						BNLP
					</ipgapi:CommercialServiceProvider>
					<ipgapi:TransactionTime>
						1192111156423
					</ipgapi:TransactionTime>
					<ipgapi:ProcessorReferenceNumber />
					<ipgapi:ProcessorResponseMessage>
						Card expiry date exceeded
					</ipgapi:ProcessorResponseMessage>
					<ipgapi:ErrorMessage>
						SGS-000033: Card expiry date exceeded
					</ipgapi:ErrorMessage>
					<ipgapi:OrderId>
						62e3b5df-2911-4e89-8356-1e49302b1807
					</ipgapi:OrderId>
					<ipgapi:ApprovalCode />
					<ipgapi:AVSResponse />
					<ipgapi:TDate>1192139943</ipgapi:TDate>
					<ipgapi:TransactionResult>
						FAILED
					</ipgapi:TransactionResult>
					<ipgapi:TerminalID>123456</ipgapi:TerminalID>
					<ipgapi:ProcessorResponseCode/>
					<ipgapi:ProcessorApprovalCode />
					<ipgapi:ProcessorReceiptNumber />
					<ipgapi:ProcessorTraceNumber />
				</ipgapi:IPGApiOrderResponse>
			</detail>
		</SOAP-ENV:Fault>
      </SOAP-ENV:Body>
</SOAP-ENV:Envelope>

The SOAP fault message elements – relative to the SOAP-ENV:Envelope/SOAP-ENV:Body/SOAP-ENV:Fault
element – are set as described below.




Web Service API Integration Guide                                             16.Reading the SOAP Response Message 105
 Path/Name                            XML        Description
                                     Schema
                                      type
 faultcode                           xs:string   This element is always set to SOAP-ENV:Client, indicating that the
                                                 fault cause is likely to be found in invalid transaction data having
                                                 been passed.
 faultstring                         xs:string   This element always carries the following fault string:
                                                 ProcessingException
 detail/ ipgapi:IPGApi              Composite This element contains the error. Since there are numerous causes for
 OrderResponse                       element  raising such an exception, the next chapter will give an overview by
                                              explaining the data contained in this element.
See section Processing Exceptions in the Appendix for detailed analysis of errors.




Web Service API Integration Guide                                                   16.Reading the SOAP Response Message 106
17. Analysing the Transaction Result
17.1.1 Transaction Approval
The SOAP message wrapping a transaction approval has been presented in the previous chapter together with an
example. The transaction status report generated by the Gateway is contained in the ipgapi:IPGApiOrderResponse
element and can be understood as the data returned by the Web Service operation. In the following, its elements
– relative to the ipgapi:IPGApiOrderResponse super element – are described. Note that always the full set of
elements is contained in the response – however, some elements might be empty.

 Path/Name                           XML        Description
                                    Schema
                                     type
 ipgapi:                            xs:string   Indicates your provider.
 CommercialServiceProvider
 ipgapi:TransactionTime             xs:string   The time stamp which is set by the Gateway before returning the
                                                transaction approval.
 ipgapi:                            xs:string   In some cases, this element might be empty. It stores a number
 ProcessorReferenceNumber                       allowing the credit card processor to refer to this transaction. You do
                                                not need to provide this number in any further transaction. However,
                                                have that number ready, in case you detect any problems with your
                                                transaction and you want to contact support.
 ipgapi:                            xs:string   In case of an approval, this element contains either contains the
 ProcessorResponse                              response message provided by the authorisation system (e.g. an auth
 Message                                        code) or in case there is no such message, the string:
                                                Function performed error-free
 ipgapi:                            xs:string   The response code from the credit card processor
 ProcessorResponseCode
 ipgapi:ErrorMessage                xs:string   This element is empty in case of an approval.
 ipgapi:FraudScore                   xs:int     This element contains the fraud score of the transaction, if the Store is
                                                activated for the Fraud Detect product.
 iipgapi:OrderId                    xs:string   This element contains the order ID. For Sale, PreAuth, ForceTicket, and
                                                Credit transactions, a new order ID is returned. For PostAuth, Return,
                                                and Void transactions, supply this number in the v1:OrderId element
                                                for making clear to which transaction you refer. The ipgapi:OrderId
                                                element of a transaction approval to a PostAuth, Return, or Void
                                                transaction simply returns the order ID, such a transaction has
                                                referred to.
 ipgapi:ApprovalCode                xs:string   Stores the approval code the transaction processor has created for
                                                this transaction. You do not need to provide this code in any further
                                                transaction. However, have that number ready, in case you detect any
                                                problems with your transaction and you want to contact support.
 ipgapi:AVSResponse                 xs:string   Returns the address verification system (AVS) response.
 ipgapi:TDate                       xs:string   Stores the TDate you have to supply when voiding this transaction
                                                (which is only possible for Sale and PostAuth transactions). In this
                                                case, pass its value in the v1:TDate element of the Void transaction
                                                you want to build.
 ipgapi:                            xs:string   Stores the transaction result which is always set to APPROVED in case
 TransactionResult                              of an approval or WAITING in case the final result is not yet clear and
                                                will be updated at a later point.
 ipgapi:TerminalID                  xs:string   The Terminal ID used for this transaction.
 iipgapi:PaymentType                xs:string   The payment type used for this transaction.
 ipgapi:Brand                       xs:string   The brand of the card used for this transaction.

Web Service API Integration Guide                                                       17. Analysing the Transaction Result 107
 ipgapi:                            xs:decimal   The Convenience fee value, returned in the response if you have this
 ConvenienceFee                                  feature configured and this is applicable for Sale transaction.
 ipgapi:Country                      xs:string   The country where the card has been issued that has been used for
                                                 this transaction.
 ipgapi:SecurePlusResponse           xs:string   The response from UnionPay SecurePlus authentication
 ipgapi:                             xs:string   Returned in the response by Mastercard for stored credentials
 SchemeTransactionId                             transactions.
 ipgapi: MerchantAdvice              xs:string   The transaction response code for declines for authorizations on
 CodeIndicator                        2 max      Nashville
 ipgapi: UpdatedPrimary              xs:string   The response from Mastercard Account Updater when stored
 AccountNumber                                   credentials are not managed by the Gateway
 ipgapi:                             xs:string   The response from Mastercard Account Updater when stored
 UpdatedExpirationDate                           credentials are not managed by the Gateway
 ipgapi:                             xs:string   The response from Mastercard Account Updater when stored
 UpdatedAccountStatus                            credentials are not managed by the Gateway; available values:
                                                 ACCOUNT_CHANGED
                                                 ACCOUNT_CLOSED
                                                 EXPIRY_CHANGED
                                                 CONTACT_CARDHOLDER
 ipgapi:                             xs:string   The response from Mastercard Account Updater when stored
 UpdatedAccountErrorCode                         credentials are not managed by the Gateway
 ipgapi:RedirectUrl                  xs:string   The URL included in the response from the Gateway, where you are
                                     100max      supposed to redirect the consumer using China domestic or Boleto
                                                 payment methods to, so they can continue with the transaction
                                                 processing
 ipgapi:                             xs:string   Authentication details returned from Standin instruction payment
 StandinResponseDetails                          transaction; available for Indian market only


17.1.2 Transaction Failure
As shown in the previous chapter, a SOAP fault message, resulting from the credit card processor having failed
to process your transaction, contains an ipgapi:IPGApiOrderResponse element passed as child of a SOAP detail
element. Note that its sub elements are exactly the same as in the transaction approval case. Their meaning in the
failure case is described below:

 Path/Name                            XML        Description
                                     Schema
                                      type
 ipgapi:                             xs:string   Indicates your provider.
 CommercialServiceProvider
 ipgapi:TransactionTime              xs:string   The time stamp which is set by the Gateway before returning the
                                                 transaction failure. The format is Unix time
                                                 (https://en.wikipedia.org/wiki/Unix_time).
 ipgapi:                             xs:string   In some cases, this element might be empty. Stores a number
 ProcessorReferenceNumber                        allowing the credit card processor to refer to this transaction. You do
                                                 not need to provide this number in any further transactions. However,
                                                 have that number ready, in case you detect any problems with your
                                                 transaction and you want to contact support.
 ipgapi:                             xs:string   Stores the error message the credit card processor has returned.
 ProcessorResponse                               For instance, in case of an expired credit card this might be:
 Message                                         Card expiry date exceeded
 ipgapi:                             xs:string   The response code from the credit card processor
 ProcessorResponseCode



Web Service API Integration Guide                                                       17. Analysing the Transaction Result 108
 ipgapi:                            xs:string   The approval code from the credit card processor
 ProcessorApprovalCode
 ipgapi:                            xs:string   The receipt number from the credit card processor
 ProcessorReceiptNumber
 ipgapi:                            xs:string   The trace number from the credit card processor
 ProcessorTraceNumber
 ipgapi:ErrorMessage                xs:string   Stores the error message returned by the Gateway. It is always
                                                encoded in the format SGS-XXXXXX: Message with XXXXXX being a
                                                six digit error code and Message describing the error (this description
                                                might be different from the processor response message). For
                                                instance, in the above example the error message SGS-000033: Card
                                                expiry date exceeded is returned. Make sure to have the error code
                                                and message ready when contacting support.
 ipgapi:OrderId                     xs:string   Stores the order ID. In contrast to an approval, this order ID is never
                                                required for any further transaction, but needed for tracing the cause
                                                of the error. Hence, make sure to have it ready when contacting
                                                support.
 ipgapi:ApprovalCode                xs:string   This element is empty in case of a transaction failure.
 ipgapi:AVSResponse                 xs:string   Returns the address verification system (AVS) response.
 ipgapi:TDate                       xs:string   Stores the TDate. Similar to the order ID, the TDate is never required
                                                for any further transaction, but needed for tracing the error cause.
                                                Hence, make sure to have it ready when contacting support.
 iipgapi:TransactionResult          xs:string   In the failure case, there are three possible values:
                                                • DECLINED
                                                • FRAUD
                                                • FAILED
                                                DECLINED is returned in case the credit card processor does not
                                                accept the transaction, e.g. when finding the customer’s funds not to
                                                be sufficient. FRAUD is returned in case a fraud attempt is assumed
                                                by the Gateway. If an internal gateway error should occur, the
                                                returned value is FAILED.
 ipgapi:TerminalID                  xs:string   The Terminal ID used for this transaction.




Web Service API Integration Guide                                                        17. Analysing the Transaction Result 109
18. Building an HTTPS POST Request
Building an HTTPS POST request is a task you rarely have to do “by hand”. There are plenty of tools and libraries
supporting you in the composition of HTTPS requests. Mostly, the required functionality for doing this task is
contained in the standard set of libraries coming with the technological environment in which you develop your
online store.
Since all of these libraries slightly differ in their usage, no general building process can be described. In order
to illustrate the basic concepts, the following chapters will give examples showing how to build a valid HTTPS
request in PHP and ASP. In general, the set of parameters you have to provide for building a valid HTTPS request in
whatever technology is as follows:

 Parameter                          Value     Description
 URL                    https://             This is the full URL of the Web Service API – depending on the
                        test.ipg-online.com/ functionality you use for building HTTP requests, you might have to
                        ipgapi/services      split this URL into host and service and provide this information in the
                                             appropriate HTTP request headers.
                                              Please note, that only TLS secured communication over standard
                                              HTTPS TCP port 443 is accepted.
 Content-Type           text/xml              This is an additional HTTP header needed to be set. This is due to the
                                              SOAP request message being encoded in XML and passed as content
                                              in the HTTP POST request body
 Authorization          Type: Basic           Your store is identified at the Gateway by checking these credentials.
                        Username:             In order to use the Web Service API, you have to provide your store ID,
                        WSstoreID._.userID    user ID, and password as the content of an HTTP Basic authorization
                        Password:             header. For instance, if your store ID is 101, your user ID 007, and your
                        yourPassword          password myPW, the authorization user name is WS101._.007. The
                                              complete HTTP authorization header would be:
                                              Authorization: Basic
                                                  V1MxMDEuXy4wMDc6bXlQVw==
                                              Note that the latter string is the base 64 encoding result of the string
                                              WS101._.007:myPW.
 HTTP Body              SOAP request XML      The HTTP POST request body takes the SOAP request message
Please note, that the Gateway is using GSLB (Global Server Load Balancing) solution to route traffic to different
locations. By default, DNS returns IP address of a primary datacenter. This may change during planned
maintenance or unplanned outage – in such case a different IP is returned, pointing to DR (disaster recovery)
location. It is therefore critical, that you respect IP address and TTL returned by DNS. Please consider that while
setting up firewalls, proxy whitelists etc.


18.1.1 PHP
Doing HTTP communication in PHP is mostly accomplished with the aid of cURL which is shipped both as library
and command line tool. In newer PHP versions, cURL is already included as extension which has to be “activated”,
thus making the cURL functionality available in any PHP script. While this is a rather straightforward task in case
your Web server operates on Microsoft Windows, it might require to compile PHP on Unix/Linux machines.
Therefore, you might consider to call the cURL command line tool from your PHP script instead of using the cURL
extension. Both variants are considered in the following beginning with the usage of the cURL extension in PHP
5.2.4 running on a Windows machine.

Using the cURL PHP Extension
Mostly, activating the cURL extension in PHP 5.2.4 simply requires to uncomment the following line in your php.ini
configuration file:
;extension=php_curl.dll

Web Service API Integration Guide                                                      18. Building an HTTPS POST Request 110
Note that other PHP versions might require other actions in order to enable cURL support in PHP. Refer to your PHP
documentation for more information. After activating cURL, an HTTP request with the above parameters is set up
with the following PHP statements:
<?php
// storing the SOAP message in a variable – note that the plain XML code
// is passed here as string for reasons of simplicity, however, it is
// certainly a good practice to build the XML e.g. with DOM – furthermore,
// when using special characters, you should make sure that the XML string
// gets UTF-8 encoded (which is not done here):
$body = “<SOAP-ENV:Envelope ...>...</SOAP-ENV:Envelope>”;
// initializing cURL with the IPG API URL:
$ch = curl_init(“https://test.ipg-online.com/ipgapi/services”);
// setting the request type to POST:
curl_setopt($ch, CURLOPT_POST, 1);
// setting the content type:
curl_setopt($ch, CURLOPT_HTTPHEADER, array(“Content-Type: text/xml”));
// setting the authorization method to BASIC:
curl_setopt($ch, CURLOPT_HTTPAUTH, CURLAUTH_BASIC);
// supplying your credentials:
curl_setopt($ch, CURLOPT_USERPWD, “WS101._.007:myPW”);
// filling the request body with your SOAP message:
curl_setopt($ch, CURLOPT_POSTFIELDS, $body);
...
?>
Setting the security options which are necessary for enabling TLS communication will be discussed in the next
chapter extending the above script.

Using the cURL Command Line Tool
For the reasons described above, you might consider using the cURL command line tool instead of the extension.
Using the tool does not require any PHP configuration efforts – your PHP script simply has to call the executable
with a set of parameters. Since the security settings are postponed to the next chapter, the following script only
shows how to set up the standard HTTP parameters, i.e. the script is extended with the TLS parameters in the
next chapter.
<?php
// storing the SOAP message in a variable – note that you have to escape
// “ and \n, since the latter makes the command line tool fail,
// furthermore note that the plain XML code is passed here as string
// for reasons of simplicity, however, it is certainly a good practice
// to build the XML e.g. with DOM – finally, when using special
// characters, you should make sure that the XML string gets UTF-8 encoded
// (which is not done here):
$body = “<SOAP-ENV:Envelope ...>...</SOAP-ENV:Envelope>”;
// setting the path to the cURL command line tool – adapt this path to the
// path where you have saved the cURL binaries:
$path = “C:\curl\curl.exe”;
// setting the IPG API URL:
$apiUrl = “ https://test.ipg-online.com/ipgapi/services”;
// setting the content type:
$contentType = “ --header \”Content-Type: text/xml\””;
// setting the authorization method to BASIC and supplying
// your credentials:
$user = “ --basic --user WS101._.007:myPW”;
// setting the request body with your SOAP message – this automatically
// marks the request as POST:
$data = “ --data \””.$body.”\””.
...
?>
Web Service API Integration Guide                                                 18. Building an HTTPS POST Request 111
18.1.2             ASP
There are multiple ways of building an HTTP request in ASP. However, in the following, the usage of WinHTTP 5.1
is described as it ships with Windows Server 2003 and Windows XP SP2. Furthermore, only a few lines of code are
required in order to set up a valid HTTP request. Note that the following code fragment is written in JavaScript.
Using VB Script instead does not fundamentally change the shown statements.
<%@ language=”javascript”%>
<html>...<body>
<%
// storing the SOAP message in a variable – note that the plain XML code
// is passed here as string for reasons of simplicity, however, it is
// certainly a good practice to build the XML e.g. with DOM – furthermore,
// when using special characters, you should make sure that the XML string
// gets UTF-8 encoded (which is not done here):
var body = “<SOAP-ENV:Envelope ...>...</SOAP-ENV:Envelope>”;
// constructing the request object:
var request = Server.createObject(“WinHttp.WinHttpRequest.5.1”);
// initializing the request object with the HTTP method POST
// and the IPG API URL:
request.open(“POST”, “https://test.ipg-online.com/ipgapi/services”);
// setting the content type:
request.setRequestHeader(“Content-Type”, “text/xml”);
// setting the credentials:
request.setCredentials(“WS10036000750._.1001”, “testinger”, 0);
...
%>
</body></html>
Note that the above script is extended in the next chapter by setting the security options which are required for
establishing the TLS channel.




Web Service API Integration Guide                                                  18. Building an HTTPS POST Request 112
19. Establishing a TLS connection
Before sending the HTTP request built in the previous chapter, a secure communication channel has to be
established, guaranteeing both that all data is passed encrypted and that the client (your application) and server
(running the Web Service API) can be sure of communicating with each other and no one else.
Please note, that only TLS secured communication over standard HTTPS TCP port 443 is accepted.
Both are achieved by establishing an TLS connection with the client and server exchanging certificates. A certificate
identifies a communication party uniquely. Basically, this process works as follows:
1. TLS: The client requests access to www.ipg-online.com
2. TLS: The server presents its certificate to the client
3. TLS: The client verifies the server’s certificate (optional)
4. TLS: The server asks the client for a client certificate
5. TLS: The client sends its certificate to the server
6. TLS: The server verifies the client’s credentials
7.	TLS: If successful, the server establishes TLS tunnel to www.ipg-online.com and all the data exchanged between
    parties is encrypted.
8. HTTP: Start HTTP and request the URL part: /ipgapi/services […]
Following this process, your application has to do two things: First, start the communication by sending its client
certificate. Second, verify the received server certificate. How this is accomplished differs from platform to platform.
However, in order to illustrate the basic concepts, the PHP and ASP scripts started in the previous chapter will be
continued by extending them with the relevant statements necessary for setting up a TLS connection.


19.1.1 PHP
Picking up the distinction between using either the PHP cURL extension or the command line tool, the following
two sections will continue the two different ways of enabling secure HTTP communication. However, regardless
of which approach you intend to use, you will be confronted with one special feature of cURL: cURL requires the
client certificate to be passed as PEM file with the Client Certificate Private Key passed in an extra file. Finally, the
Client Certificate Private Key password has to be supplied. Simply spoken, the PEM file contains the list of client
certificates with all information necessary for allowing the server to identify the client. The private key is not really
necessary for this kind of communication. However, it is crucial for making cURL work.

Using the PHP cURL Extension
Building on the script started in the previous chapter, the parameters which are necessary for establishing an TLS
connection with cURL are set in the following statements:
<?php
...
// telling cURL to verify the server certificate:
curl_setopt($ch, CURLOPT_SSL_VERIFYPEER, 1);
// setting the path where cURL can find the certificate to verify the
// received server certificate against:
curl_setopt($ch, CURLOPT_CAINFO, “C:\certs\tlstrust.pem”);
// setting the path where cURL can find the client certificate:
curl_setopt($ch, CURLOPT_SSLCERT, “C:\certs\WS101._.007.pem”);
// setting the path where cURL can find the client certificate’s
// private key:
curl_setopt($ch, CURLOPT_SSLKEY, “C:\certs\WS101._.007.key”);
// setting the key password:
curl_setopt($ch, CURLOPT_SSLKEYPASSWD, “ckp_1193927132”);
...
?>
Note that this script is extended in the next chapter by the statements doing the actual HTTP request.

Web Service API Integration Guide                                                          19. Establishing a TLS connection 113
Using the cURL Command Line Tool
Building on the script started in the previous chapter, the statements which initialize the TLS parameters passed to
the cURL command line tool are as follows:
<?php
...
// setting the path where cURL can find the certificate to verify the
// received server certificate against:
$serverCert = “ --cacert C:\certs\tlstrust.pem”;
// setting the path where cURL can find the client certificate:
$clientCert = “ --cert C:\certs\WS101._.007.pem”;
// setting the path where cURL can find the client certificate’s
// private key:
$clientKey = “ --key C:\certs\WS101._.007.key”;
// setting the key password:
$keyPW = “ --pass ckp_1193927132”;
...
?>
Note that this script is extended in the next chapter by the statements doing the actual HTTP request.


19.1.2 ASP
For making the above TLS initialization process work, ASP requires both the client and the server certificate to be
present in certificate stores. In other words, before ASP can communicate via TLS, both certificates have to be
installed first. The following steps which assume ASP running on Microsoft IIS 5.1 under Windows XP, will guide
you through this set up process:
1. Click Start, click Run..., type mmc and click OK.
2. Open the File menu, select Add/Remove Snap-In.
3. Click Add.
4. Under Snap-In choose Certificates and click Add.
5.	You will be prompted to select the account for which you want to manage the certificates. Since IIS uses the
    computer account, choose Computer Account and click Next.
6. Choose Local Computer and click Finish.
7. Click Close and then OK.
8. Expand the Certificates (Local Computer) tree - the client certificate will be installed in the Personal folder.
9.	Therefore, right click the Certificates folder, select All Tasks, click Import... – this will open the Certificate Import
    Wizard.
10. Click Next. Choose your client certificate p12 file and click Next.
11. Provide the client certificate installation password and click Next.
12.	Select Place all certificates in the following store and browse for the Personal folder if not yet displayed.
     Click Next.
13.	Check the displayed settings and click Finish. Your client certificate is now installed in the local computer’s
    personal certificates store. Here, IIS (running ASP) can lookup the client certificate when communicating with
    another server via HTTP.
14.	Now, the server certificate has to be installed in the Trusted Root Certification Authorities store. The certificates
    in this store are used for verification whenever receiving a certificate from a server. That means the Web Service
    API server certificate has to be installed here. In this way, IIS is able to verify the server certificate received when
    contacting the Web Service. Therefore, choose Trusted Root Certification Authorities from the Certificates
    (Local Computer) tree open the sub folder Certificates.
15.	Right click the Certificates folder, select All Tasks, click Import... – this will open the Certificate Import Wizard
     again.

Web Service API Integration Guide                                                            19. Establishing a TLS connection 114
16. Click Next. Choose the Trust Anchor PKCS#7 file and click Next.
17.	Select Place all certificates in the following store and browse for the Trusted Root Certification Authorities folder
     if not yet displayed. You should trust all client certificates listed to establish a trusted connection to the server.
     Click Next.
18.	Check the displayed settings and click Finish. The server certificate is now installed in the local computer’s
    trusted certificates store. Here, IIS can lookup the server certificate for verification against the Web Service API
    server certificate received during the TLS setup process.
After installing both certificates one could assume that the environment allowing ASP to communicate via TLS is set
up. However, there is still one thing which makes the communication fail: IIS – running your ASP – has a Windows
user which does not have the necessary rights to access the client certificate private key. Although accessing
the private key is not really necessary for establishing the TLS connection to the Gateway, the IIS user needs
access rights for running the authentication process in ASP. For granting rights to a user, Microsoft provides the
WinHttpCertCfg.exe tool you can download for free under:
http://www.microsoft.com/downloads/details.aspx?familyid=c42e27ac-3409-40e9-8667-
c748e422833f&displaylang=en
After installing the tool, open a command prompt, switch to the directory where you have installed the tool, and
type in the following line for granting access to the IIS user:
winhttpcertcfg -g -c LOCAL_MACHINE\My -s WS101._.007 -a IWAM_MyMachine
LOCAL_MACHINE\My determines the key store where the personal certificates for the local machine account are
stored. After installing the client certificate in the personal certificates store as described above, the client certificate
can be found under this path, so there is no need to provide another path. WS101._.007 is the name of the client
certificate. You have to adapt this name to the name of your client certificate. Therefore, check the name displayed
for the client certificate in the mmc console after installing it as described above. Finally, IWAM_MyMachine
denotes the IIS user name. Note that IIS 5.1 uses IWAM_MachineName by default. That means if your machine
has the name IISServerMachine, the IIS user will be called IWAM_IISServerMachine. Note that other IIS versions
might use a different naming scheme. If you do not know your machine name or IIS user name, check the IIS
documentation and contact your administrator.
Now you are ready to use TLS in your ASP code. The code extending the ASP script started in the previous chapter
is reduced to only one additional statement which tells WinHTTP which client certificate to send (and where to find
it) when contacting the Authipay Gateway:
<%@ language=”javascript”%>
<html>...<body>
<%
...
// setting the path where the client certificate to send can be found:
request.setClientCertificate(“LOCAL_MACHINE\\My\\WS101._.007”);
...
%>
</body></html>
Note that if you use VB Script, the code looks almost the same – however, do not forget to replace the doubled backslashes in the path with
single ones (i.e. the path to the certificate would be “LOCAL_MACHINE\My\WS101._.007” instead).
Note that this script is extended in the next chapter by the statements doing the actual HTTP request.




Web Service API Integration Guide                                                                        19. Establishing a TLS connection 115
20. S
     ending the HTTPS POST Request and
    Receiving the Response
The actual communication with the Web Service API takes place when sending the HTTPS request and waiting for
a response. Again, how this is done depends on the technology you are using. Most HTTP libraries fully cover the
underlying communication details and reduce this process to a single operation call returning the HTTP response
as result object.
In any case, the parameters which are required for successfully performing an HTTP POST request over TLS and
receiving the response (carrying a 200 HTTP status code) have been described in the previous two chapters.
Setting invalid or incorrect parameters results in the web server running the Web Service API to return a standard
HTTP error code in the HTTP header of the response or sending an TLS failure. Their meanings can be found in any
HTTP/TLS guide.
Please note, that only TLS secured communication over standard HTTPS TCP port 443 is accepted.
However, there is one important exception: In case the HTTP parameters you have provided are correct, but
the Web Service has failed to process your transaction due to an incorrect value contained in the SOAP request
message (e.g. an invalid credit card number), a SOAP exception is thrown and transferred in the body of an HTTP
response carrying the error code 500. Details about the exception cause are provided in the SOAP fault message
which is described in the context of the next chapter.
In order to complete the PHP and ASP scripts, built gradually in the previous chapters, the following two chapters
will provide the statements necessary for doing an HTTP call using these technologies.


20.1.1 PHP
Again, the distinction between the PHP cURL extension and the cURL command line tool is made in the following:

Using the PHP cURL Extension
The PHP script using the cURL extension is finally completed by doing the call with the statements shown below.
Note that the HTTP call returns a SOAP response or fault message in the HTTP response body.
<?php
...
// telling cURL to return the HTTP response body as operation result
// value when calling curl_exec:
curl_setopt($ch, CURLOPT_RETURNTRANSFER, 1);
// calling cURL and saving the SOAP response message in a variable which
// contains a string like “<SOAP-ENV:Envelope ...>...</SOAP-ENV:Envelope>”:
$result = curl_exec($ch);
// closing cURL:
curl_close($ch);
?>

Using the cURL Command Line Tool
Doing the HTTP call with the cURL command line tool simply requires completing the command line statement and
executing the external tool. However, reading the HTTP response is more complicated as the PHP exec command
saves each line returned by an external program as one element of an array. Concatenating all elements of that
array results in the SOAP response or fault message which has been returned in the HTTP response body. The
following statements handle the HTTP call and complete the script:
<?php
...
// saving the whole command in one variable:
$curl = $path.
$data.
$contentType.

Web Service API Integration Guide                          20.Sending the HTTPS POST Request and Receiving the Response 116
$user.
$serverCert.
$clientCert.
$clientKey.
$keyPW.
$apiUrl;
// preparing the array containing the lines returned by the cURL
// command line tool:
$returnArray = array();
// performing the HTTP call by executing the cURL command line tool:
exec($curl, $returnArray);
// preparing a variable taking the complete result:
$result = “”;
// concatenating the different lines returned by the cURL command
// line tool – this result in the variable $result carrying the entire
// SOAP response message as string:
foreach($returnArray as $item)
$result = $result.$item;
?>


20.1.2 ASP
Doing the actual HTTP call with WinHTTP in ASP is limited to one simple operation call taking the SOAP request
XML as a parameter. After successfully performing the request a SOAP response or fault message is returned which
can be retrieved as a string by accessing the request object’s responseText property. How such a SOAP response
message looks like is described in the next chapter. The following statements complete the ASP script:
<%@ language=”javascript”%>
<html>...<body>
<%
...
// doing the HTTP call with the SOAP request message as input:
request.send(body);
// saving the SOAP response message in a string variable:
var response = request.responseText;
%>
</body></html>




Web Service API Integration Guide                        20.Sending the HTTPS POST Request and Receiving the Response 117
21. Using a Java Client to connect
     to the web service
For quick and simple integration, Authipay provides a Java Client to connect to the Gateway web service.
An instance of the IPGApiClient class manages the connection to the web service, builds XML and the SOAP
messages and evaluates the responses. To construct a transaction or to handle a response, the developer works
with simple Java bean classes.
The IPGApiClient uses the apache http client. Some settings of the http client impact every http client for the same
class loader environment.


21.1.1 Instance an IPGApiClient
There are several constructors available to instantiate the IPGApiClient. The example below illustrates how to use
the easiest one of the constructors. The getBytes method is also included for the completion and simplification of
the example.
String url = “https://test.ipg-online.com/ipgapi/services”;
String storeId = “your store id”;
String password = “your password”;
byte[] key = getBytes(“/path/to/your/keyStore.ks”);
String keyPW = “your key store password”;
IPGApiClient client = new IPGApiClient(url, storeId, password, key, keyPW);
/**
* getBytes
* reads a resource and returns a byte array
* @param resource the resource to read
* @return the resource as byte array
*/
public static byte[] getBytes(final String resource) throws IOException {
   final InputStream input = IO.class.getResourceAsStream(resource);
   if (input == null) {
		throw new IOException(resource);
}
try {
   final byte[] bytes = new byte[input.available()];
   input.read(bytes);
   return bytes;
} finally {
   try {
			 input.close();
   } catch (IOException e) {
			 log.warn(resource);
		}
   }
}


21.1.2 How to construct a transaction and handle the response
There are different classes for transactions with the following card types:
• Credit Card
• German Direct Debit
• UK Debit Cards.
The following factory class can be used to generate the class you need:

Web Service API Integration Guide                                       21. Using a Java Client to connect to the web service 118
de.firstdata.ipgapi.client.transaction.IPGApiTransactionFactory
The following example shows a Credit Card Sale transaction for an amount of 7 Euros:
Amount amount = new Amount(“7”, “978”); // ISO 4217: EUR = 978
CreditCard cC = new CreditCard(“1111222233334444”, “07”, “17”, null);
CCSaleTransaction transaction =
  IPGApiTransactionFactory.createSaleTransactionCredit(amount, cC);
// some transactions may include further information e.g. the customer
transaction.setName(“a name”);
try {
  IPGApiResult result = client.commitTransaction(transaction);
  // now you can read the conclusion
  System.out.println(result.getOrderId());
  System.out.println(result.getTransactionTime());
  // ...
} catch (ProcessingException e) {
  // ERROR: transaction not passed
}


21.1.3 How to construct an action
The following Factory Class can be used to generate the class you need:
de.firstdata.ipgapi.client.transaction.IPGApiActionFactory
To commit an action you need to use the commitAction method of the IPGApiClient. The further process is similar
to payment transactions.


21.1.4 How to connect behind a proxy
Before you use the IPGApiClient behind a proxy you must set the proxy configuration of the client with the
IPGApiClient method:
IPGApiClient.setProxy(
         final String host, final Integer port,
         final String user, final String password,
         final String workstation, final String domain)
The parameters user, password, workstation and domain should be null if no identification needed. If you need
to identify on a MS Windows proxy you must set the parameter domain. To identify on systems like Unix the
parameter domain must be null. For more information see the apache javadoc.
After setting the proxy parameters you must call the IPGApiClient.init() method.




Web Service API Integration Guide                                     21. Using a Java Client to connect to the web service 119
22. Appendix service
XML
The Web Service API uses the XML standard for communication as described on
          http://www.w3.org/standards/xml/core
including the specification of namespaces described on
          http://www.w3.org/TR/2009/REC-xml-names-20091208/
To make the names of the XML tags unique (e.g. in IPG: IPGApiActionRequest, Action, RecurringPayment, etc.),
namespaces are used.
          Example:
          http://ipg-online.com/ipgapi/schemas/ipgapi, http://ipg-online.com/ipgapi/schemas/a1, …
These namespaces are defined in the xsd files like
          xmlns:ipgapi=”http://ipg-online.com/ipgapi/schemas/ipgapi”.
The same namespaces must be declared in the XML files (no parsing with hardcoded namespace references),
starting with keyword xmlns.
To aviod errors with the namespaces we recommend to use libaries to manage the XML messages.
In the course of future product develoment, it may be necessary that we extend the IPGApiRequest or
IPGApiResponse with further members. While extending the request will have no impact on your implemented
code, extending the response might cause errors if you check the response against ipgapi.xsd. We therefore
recommend to deactivate the check.


XML Schemata
The definitions for the XML document building blocks can be found here:

 ipgapi.xsd                                    https://www.ipg-online.com/ipgapi/schemas/ipgapi.xsd
 v1.xsd                                        https://www.ipg-online.com/ipgapi/schemas/v1.xsd
 a1.xsd                                        https://www.ipg-online.com/ipgapi/schemas/a1.xsd


Union Pay SecurePlus
SecurePlus is an eCommerce payment solution designed by UnionPay to reduce the risk of fraudulent transactions,
similar to 3D Secure.
Please note that this feature is not available through all distribution channels.
There are three API request types to support SecurePlus Transactions. Two for authentication:
1. A request to verify enrollment and to send a SMS code to the cardholder
2. A request to verify the SMS code provided by the cardholder, to be the one they have sent before
And then the authorisation which can either be the full SecurePlus sale request, or if the store is allowed to skip,
can be sent without reference to authentication. but then liability is with merchant (ECI 10).
The Web Service API allows you to make following API calls for the required steps:

API call for Step 1
To verify if the card has been enrolled and to receive an SMS code sent by issuer, you need to submit a request
with an AuthenticateTransaction parameter set to “true”, TxType = payerAuth and enter the mobile phone number
registered with the SecurePlus program.
The following represents an example of the full SecurePlusVerification Request with TxType=payerAuth:


Web Service API Integration Guide                                                                       22. Appendix 120
<?xml version=”1.0” encoding=”UTF-8”?><SOAP-ENV:Envelope
xmlns:SOAP-ENV=”http://schemas.xmlsoap.org/soap/envelope/”>
<SOAP-ENV:Header/>
<SOAP-ENV:Body>
<ns4:IPGApiOrderRequest
xmlns:ns4=”http://ipg-online.com/ipgapi/schemas/ipgapi”
xmlns:ns2=”http://ipg-online.com/ipgapi/schemas/a1”
xmlns:ns3=”http://ipg-online.com/ipgapi/schemas/v1”>
  <ns3:Transaction>
		<ns3:CreditCardTxType>
			 <ns3:StoreId>4712300011088</ns3:StoreId>
			 <ns3:Type>payerAuth</ns3:Type>
		</ns3:CreditCardTxType>
		<ns3:CreditCardData>
			 <ns3:CardNumber>6222*****0017</ns3:CardNumber>
			 <ns3:ExpMonth>12</ns3:ExpMonth>
			 <ns3:ExpYear>33</ns3:ExpYear>
			 <ns3:CardCodeValue>XXX</ns3:CardCodeValue>
		</ns3:CreditCardData>
		 <ns3:Upop>           		
  <ns3:AuthenticateTransaction>true</ns3:AuthenticateTransaction>
		</ns3:Upop>
		<ns3:Payment>
			 <ns3:ChargeTotal>100</ns3:ChargeTotal>
			 <ns3:Currency>344</ns3:Currency>
		</ns3:Payment>
			 <ns3:Billing>
				          <ns3:MobilePhone>86-13012345678</ns3:MobilePhone>
			 </ns3:Billing>
  </ns3:Transaction>
</ns4:IPGApiOrderRequest>
</SOAP-ENV:Body>
</SOAP-ENV:Envelope>
The following represents an example of a IPGApiOrderResponse:
<?xml version=”1.0” encoding=”UTF-8”?><SOAP-ENV:Envelope xmlns:SOAP-ENV=”http://
schemas.xmlsoap.org/soap/envelope/”>
    <SOAP-ENV:Header/>
    <SOAP-ENV:Body>
        <ipgapi:IPGApiOrderResponse xmlns:ipgapi=”http://ipg-online.com/ipgapi/
schemas/ipgapi” xmlns:a1=”http://ipg-online.com/ipgapi/schemas/a1” xmlns:v1=”http://
ipg-online.com/ipgapi/schemas/v1”>
            <ipgapi:ApprovalCode>?:waiting authentication</ipgapi:ApprovalCode>
            <ipgapi:Brand>UNIONPAY</ipgapi:Brand>
            <ipgapi:Country>CHN</ipgapi:Country>
            <ipgapi:CommercialServiceProvider>FDMS-HK</
ipgapi:CommercialServiceProvider>
            <ipgapi:OrderId>A-046a5c58-5213-4952-b3ca-fb52de4a2f57</ipgapi:OrderId>
            <ipgapi:IpgTransactionId>8383509671</ipgapi:IpgTransactionId>
            <ipgapi:PaymentType>CREDITCARD</ipgapi:PaymentType>
            <ipgapi:ProcessorResponseCode>00</ipgapi:ProcessorResponseCode>
            <ipgapi:ProcessorResponseMessage>成功[0000000]</
ipgapi:ProcessorResponseMessage>
            <ipgapi:TDate>1523535455</ipgapi:TDate>
            <ipgapi:TDateFormatted>2018.04.12 14:17:35 (MESZ)</ipgapi:TDateFormatted>
            <ipgapi:TransactionResult>WAITING</ipgapi:TransactionResult>
            <ipgapi:TransactionTime>1523535455</ipgapi:TransactionTime>
            <ipgapi:SecurePlusResponse>
                <v1:AuthenticateResponse>
Web Service API Integration Guide                                         22. Appendix 121
                     <v1:smsSent>true</v1:smsSent>
                </v1:AuthenticateResponse>
            </ipgapi:SecurePlusResponse>
        </ipgapi:IPGApiOrderResponse>
    </SOAP-ENV:Body>
</SOAP-ENV:Envelope>

API call for Step 2
In your second API call you need to request Union Pay to verify the SMS code provided by the cardholder.
The following represents an example of a verification request:
<?xml version=”1.0” encoding=”UTF-8”?><SOAP-ENV:Envelope
xmlns:SOAP-ENV=”http://schemas.xmlsoap.org/soap/envelope/”>
<SOAP-ENV:Header/>
<SOAP-ENV:Body>
<ns4:IPGApiOrderRequest
xmlns:ns4=”http://ipg-online.com/ipgapi/schemas/ipgapi”
xmlns:ns2=”http://ipg-online.com/ipgapi/schemas/a1”
xmlns:ns3=”http://ipg-online.com/ipgapi/schemas/v1”>
  <ns3:Transaction>
		<ns3:CreditCardTxType>
			 <ns3:StoreId>471230011088</ns3:StoreId>
			 <ns3:Type>payerAuth</ns3:Type>
		</ns3:CreditCardTxType>
		<ns3:CreditCardData>
			 <ns3:CardCodeValue>XXX</ns3:CardCodeValue>
		</ns3:CreditCardData>
		<ns3:Upop>
			 <ns3:SecurePlusRequest>
				          <ns3:SecurePlusVerifySmsCodeRequest>
					 <ns3:smsCode>111111</ns3:smsCode>
				          </ns3:SecurePlusVerifySmsCodeRequest>
			 </ns3:SecurePlusRequest>
		</ns3:Upop>
		<ns3:Payment>
			 <ns3:ChargeTotal>100</ns3:ChargeTotal>
			 <ns3:Currency>344</ns3:Currency>
		</ns3:Payment>
		<ns3:TransactionDetails>
			 <ns3:IpgTransactionId>8383509671</ns3:IpgTransactionId>
		</ns3:TransactionDetails>
  </ns3:Transaction>
</ns4:IPGApiOrderRequest>
</SOAP-ENV:Body>
</SOAP-ENV:Envelope>
The following represents an example of a IPGApiOrderResponse:
<?xml version=”1.0” encoding=”UTF-8”?><SOAP-ENV:Envelope xmlns:SOAP-ENV=”http://
schemas.xmlsoap.org/soap/envelope/”>
    <SOAP-ENV:Header/>
    <SOAP-ENV:Body>
        <ipgapi:IPGApiOrderResponse xmlns:ipgapi=”http://ipg-online.com/ipgapi/
schemas/ipgapi” xmlns:a1=”http://ipg-online.com/ipgapi/schemas/a1” xmlns:v1=”http://
ipg-online.com/ipgapi/schemas/v1”>
            <ipgapi:ApprovalCode>Y:ECI9:Authenticated</ipgapi:ApprovalCode>
            <ipgapi:Brand>UNIONPAY</ipgapi:Brand>
            <ipgapi:Country>CHN</ipgapi:Country>
            <ipgapi:CommercialServiceProvider>FDMS-HK</
ipgapi:CommercialServiceProvider>
Web Service API Integration Guide                                                                 22. Appendix 122
            <ipgapi:OrderId>A-046a5c58-5213-4952-b3ca-fb52de4a2f57</ipgapi:OrderId>
            <ipgapi:IpgTransactionId>8383509671</ipgapi:IpgTransactionId>
            <ipgapi:PaymentType>CREDITCARD</ipgapi:PaymentType>
            <ipgapi:ProcessorResponseCode>00</ipgapi:ProcessorResponseCode>
            <ipgapi:ProcessorResponseMessage>成功[0000000]</
ipgapi:ProcessorResponseMessage>
            <ipgapi:TDate>1523535455</ipgapi:TDate>
            <ipgapi:TDateFormatted>2018.04.12 14:17:35 (MESZ)</ipgapi:TDateFormatted>
            <ipgapi:TransactionResult>APPROVED</ipgapi:TransactionResult>
            <ipgapi:TransactionTime>1523535455</ipgapi:TransactionTime>
            <ipgapi:SecurePlusResponse>
                <v1:VerifySmsCodeResponse>
                     <v1:responseCode>1</v1:responseCode>
                </v1:VerifySmsCodeResponse>
            </ipgapi:SecurePlusResponse>
        </ipgapi:IPGApiOrderResponse>
    </SOAP-ENV:Body>
</SOAP-ENV:Envelope>

API call for Step 3
In your third API call you submit the authorization, which can either be the full SecurePlus sale request, or if your
store is allowed to skip, can be sent without reference to authentication, but then liability is with merchant (ECI 10).
The following represents an example of an authorization request:
<?xml version=”1.0” encoding=”UTF-8”?><SOAP-ENV:Envelope xmlns:SOAP-ENV=”http://
schemas.xmlsoap.org/soap/envelope/”>
    <SOAP-ENV:Header/>
    <SOAP-ENV:Body>
        <ns4:IPGApiOrderRequest xmlns:ns4=”http://ipg-online.com/ipgapi/schemas/
ipgapi” xmlns:ns2=”http://ipg-online.com/ipgapi/schemas/a1” xmlns:ns3=”http://ipg-
online.com/ipgapi/schemas/v1”>
            <ns3:Transaction>
                <ns3:CreditCardTxType>
                    <ns3:StoreId>4712300011088</ns3:StoreId>
                    <ns3:Type>sale</ns3:Type>
                </ns3:CreditCardTxType>
                <ns3:CreditCardData>
                    <ns3:CardCodeValue>XXX</ns3:CardCodeValue>
                </ns3:CreditCardData>
                <ns3:Payment>
                    <ns3:ChargeTotal>100</ns3:ChargeTotal>
                    <ns3:Currency>344</ns3:Currency>
                </ns3:Payment>
                <ns3:TransactionDetails>
                    <ns3:IpgTransactionId>8383509671</ns3:IpgTransactionId>
                </ns3:TransactionDetails>
            </ns3:Transaction>
        </ns4:IPGApiOrderRequest>
    </SOAP-ENV:Body>
</SOAP-ENV:Envelope>
The following represents an example of a IPGApiOrderResponse:
<?xml version=”1.0” encoding=”UTF-8”?><SOAP-ENV:Envelope
xmlns:SOAP-ENV=”http://schemas.xmlsoap.org/soap/envelope/”>
<SOAP-ENV:Header/>
<SOAP-ENV:Body>
<ipgapi:IPGApiOrderResponse
xmlns:ipgapi=”http://ipg-online.com/ipgapi/schemas/ipgapi”
xmlns:a1=”http://ipg-online.com/ipgapi/schemas/a1”
Web Service API Integration Guide                                                                        22. Appendix 123
xmlns:v1=”http://ipg-online.com/ipgapi/schemas/v1”>
<ipgapi:ApprovalCode>Y:440368:0000057177:PPXM:0043364291</ipgapi:ApprovalCode>
<ipgapi:AVSResponse>PPX</ipgapi:AVSResponse>
<ipgapi:Brand>UNIONPAY</ipgapi:Brand>
<ipgapi:Country>CHN</ipgapi:Country>
<ipgapi:CommercialServiceProvider>FDMS-HK</ipgapi:CommercialServiceProvide
r>
<ipgapi:OrderId>A-046a5c58-5213-4952-b3ca-fb52de4a2f57</ipgapi:OrderId>
<ipgapi:IpgTransactionId>90419835</ipgapi:IpgTransactionId>
<ipgapi:PaymentType>CREDITCARD</ipgapi:PaymentType>
<ipgapi:ProcessorApprovalCode>000000</ipgapi:ProcessorApprovalCode>

<ipgapi:ProcessorResponseCode>00</ipgapi:ProcessorResponseCode>
<ipgapi:ProcessorResponseMessage>Function performed error-free </
ipgapi:ProcessorResponseMessage>
<ipgapi:TDate>1523969700</ipgapi:TDate>
<ipgapi:TDateFormatted>2018.04.17 14:55:00(CEST)</ipgapi:TDateFormatted>
<ipgapi:TerminalID>00001118</ipgapi:TerminalID>
<ipgapi:TransactionResult>APPROVED</ipgapi:TransactionResult>
<ipgapi:TransactionTime>1523969700</ipgapi:TransactionTime>
<ipgapi:SecurePlusResponse>
  <v1:VerifySmsCodeResponse>
		<v1:responseCode>1</v1:responseCode>
  </v1:VerifySmsCodeResponse>
</ipgapi:SecurePlusResponse>
</ipgapi:IPGApiOrderResponse>
</SOAP-ENV:Body>
</SOAP-ENV:Envelope>


Bancontact QR code transactions
The Bancontact card processing capabilities on the Gateway have been enhanced to offer a new option for
QR-Code based payments with the Bancontact App.
Please note, that this feature is not available through all distribution channels and supports only pass-through
authentication model, where the 3-D Secure authentication is handled by external MPI provider.
You are able to include additional parameters in your API requests that support the QR-Code based authorization
with an indicator, if you pay using the Bancontact app or the traditional way, entering the card details.
The following represents an example of an authorization request for payment with Bancontact App:
<?xml version=”1.0” encoding=”UTF-8”?><SOAP-ENV:Envelope
xmlns:SOAP-ENV=”http://schemas.xmlsoap.org/soap/envelope/”>
<SOAP-ENV:Header/>
<SOAP-ENV:Body>
<ns4:IPGApiOrderRequest
xmlns:ns4=”http://ipg-online.com/ipgapi/schemas/ipgapi”
xmlns:ns2=”http://ipg-online.com/ipgapi/schemas/a1”
xmlns:ns3=”http://ipg-online.com/ipgapi/schemas/v1”>
    <ns3:Transaction>
        <ns3:CreditCardTxType>
            <ns3:StoreId>230995000</ns3:StoreId>
            <ns3:Type>sale</ns3:Type>
        </ns3:CreditCardTxType>
        <ns3:CreditCardData>
            <ns3:CardNumber>6703*****4449</ns3:CardNumber>
            <ns3:ExpMonth>07</ns3:ExpMonth>
            <ns3:ExpYear>21</ns3:ExpYear>
            <ns3:CardCodeValue>XXX</ns3:CardCodeValue>
        </ns3:CreditCardData>
        <ns3:CreditCard3DSecure>
Web Service API Integration Guide                                                                     22. Appendix 124
            <ns3:VerificationResponse>Y</ns3:VerificationResponse>
<ns3:PayerAuthenticationResponse>Y</ns3:PayerAuthenticationResponse>
<ns3:AuthenticationValue>BwABC…neJAAAAAAA=</ns3:AuthenticationValue>
            <ns3:XID>nhrtvl22IdlqdioLX6eQmd3jL6U=</ns3:XID>
        </ns3:CreditCard3DSecure>
        <ns3:Payment>
            <ns3:ChargeTotal>708</ns3:ChargeTotal>
            <ns3:Currency>EUR</ns3:Currency>
        </ns3:Payment>
        <ns3:TransactionDetails>
            <ns3:TransactionOrigin>ECI</ns3:TransactionOrigin>
        </ns3:TransactionDetails>
        <ns3:BancontactQR>
  <ns3:TransactionRoutingMeans>QR Code</ns3:TransactionRoutingMeans>                                         		
  <ns3:IssuerCustomerReference>as23..fsdf</ns3:IssuerCustomerReference>
        </ns3:BancontactQR>
    </ns3:Transaction>
</ns4:IPGApiOrderRequest>
The following represents an example of an authorization request for payment with Bancontact card:
<?xml version=”1.0” encoding=”UTF-8”?><SOAP-ENV:Envelope
xmlns:SOAP-ENV=”http://schemas.xmlsoap.org/soap/envelope/”>
<SOAP-ENV:Header/>
<SOAP-ENV:Body>
<ns4:IPGApiOrderRequest
xmlns:ns4=”http://ipg-online.com/ipgapi/schemas/ipgapi”
xmlns:ns2=”http://ipg-online.com/ipgapi/schemas/a1”
xmlns:ns3=”http://ipg-online.com/ipgapi/schemas/v1”>
    <ns3:Transaction>
        <ns3:CreditCardTxType>
            <ns3:StoreId>230995000</ns3:StoreId>
            <ns3:Type>sale</ns3:Type>
        </ns3:CreditCardTxType>
        <ns3:CreditCardData>
            <ns3:CardNumber>6703*****4449</ns3:CardNumber>
            <ns3:ExpMonth>07</ns3:ExpMonth>
            <ns3:ExpYear>21</ns3:ExpYear>
            <ns3:CardCodeValue>XXX</ns3:CardCodeValue>
        </ns3:CreditCardData>
        <ns3:CreditCard3DSecure>
            <ns3:VerificationResponse>Y</ns3:VerificationResponse>
<ns3:PayerAuthenticationResponse>Y</ns3:PayerAuthenticationResponse>
<ns3:AuthenticationValue>BwABC…neJAAAAAAA=</ns3:AuthenticationValue>
            <ns3:XID>nhrtvl22IdlqdioLX6eQmd3jL6U=</ns3:XID>
        </ns3:CreditCard3DSecure>
        <ns3:Payment>
            <ns3:ChargeTotal>708</ns3:ChargeTotal>
            <ns3:Currency>EUR</ns3:Currency>
        </ns3:Payment>
        <ns3:TransactionDetails>
            <ns3:TransactionOrigin>ECI</ns3:TransactionOrigin>
        </ns3:TransactionDetails>
        <ns3:BancontactQR>
		<ns3:TransactionRoutingMeans>URL Intent</ns3:TransactionRoutingMeans>
		<ns3:IssuerCustomerReference>as23..fsdf</ns3:IssuerCustomerReference>
        </ns3:BancontactQR>
    </ns3:Transaction>
</ns4:IPGApiOrderRequest>
An optional element IssuerCustomerReference allows you to also include an identifier for the cardholder.
Web Service API Integration Guide                                                                   22. Appendix 125
China domestic processing
Fiserv has partnered with Huifu, a payment provider in China, to offer the ability to route Chinese transactions and
settle domestically in China.
This solution includes China UnionPay, Alipay and WeChat Pay with a redirection of the consumer to pages in
Chinese language provided by the local partner.
The following represents an example of a transaction request with payment method “CUP Domestic”:
<?xml version=”1.0” encoding=”UTF-8”?>
<ns4:IPGApiOrderRequest
xmlns:ns4=”http://ipg-online.com/ipgapi/schemas/ipgapi” xmlns:ns2=”http://ipg-online.
com/ipgapi/schemas/a1”
xmlns:ns3=”http://ipg-online.com/ipgapi/schemas/v1”>
    <ns3:Transaction>
        <ns3:CUPDomesticTxType>
            <ns3:StoreId>471230011057</ns3:StoreId>
            <ns3:Type>sale</ns3:Type>
        </ns3:CUPDomesticTxType>
        <ns3:CUPDomesticInformation>
            <ns3:CustomerId>123</ns3:CustomerId>
            <ns3:ProductCode>400005</ns3:ProductCode>
            <ns3:ProductQuantity>1</ns3:ProductQuantity>
            <ns3:ProductPrice>1</ns3:ProductPrice>
            <ns3:ProductDescription>product01</ns3:ProductDescription>
            <ns3:RedirectUrl>http://www.testURL.com</ns3:RedirectUrl>
           <ns3:BankId>abc</ns3:BankId>
        </ns3:CUPDomesticInformation>
        <ns3:Payment>
            <ns3:ChargeTotal>2</ns3:ChargeTotal>
            <ns3:Currency>840</ns3:Currency>
        </ns3:Payment>
        <ns3:TransactionDetails>
            <ns3:OrderId>API-TestTxn</ns3:OrderId>
        </ns3:TransactionDetails>
    </ns3:Transaction>
</ns4:IPGApiOrderRequest>
The RedirectURL in the request is where Chinese platform (PNR) is going to redirect you to after the processing is
done at their end.
Please note, that for payment method “CUP_domestic” the element ‘BankId’ is mandatory and the element
‘CustomerId’ is recommended to be submitted in case your consumer knows its value.
Valid ‘ProductCode’ element values are available on the online portal:
https://docs.firstdata.com/org/gateway/node/401
For payment method Alipay please use the elements AlipayTxType AND AlipayDomesticInformation.
For payment method WeChat please use the elements WeChatTxType AND WeChatDomesticInformation.
The following represents an example of a transaction response:
<?xml version=”1.0” encoding=”UTF-8”?><ipgapi:IPGApiOrderResponse
xmlns:ipgapi=”http://ipg-online.com/ipgapi/schemas/ipgapi” xmlns:a1=”http://ipg-
online.com/ipgapi/schemas/a1”
xmlns:v1=”http://ipg-online.com/ipgapi/schemas/v1”>
    <ipgapi:ApprovalCode>?:waiting CHINAPNR</ipgapi:ApprovalCode>
    <ipgapi:AVSResponse>PPX</ipgapi:AVSResponse>
    <ipgapi:CommercialServiceProvider>FDMS-HK</ipgapi:CommercialServiceProvider>
    <ipgapi:OrderId>API-TestTxn</ipgapi:OrderId>
    <ipgapi:IpgTransactionId>8383903542</ipgapi:IpgTransactionId>
    <ipgapi:ProcessorResponseCode>000000</ipgapi:ProcessorResponseCode>

Web Service API Integration Guide                                                                     22. Appendix 126
    <ipgapi:ProcessorResponseMessage>ChinaPnR success</
ipgapi:ProcessorResponseMessage>
    <ipgapi:TDate>1548783233</ipgapi:TDate>
    <ipgapi:TDateFormatted>2019.10.29 18:33:53 (MEZ)</ipgapi:TDateFormatted>
    <ipgapi:TerminalID>0010001</ipgapi:TerminalID>
    <ipgapi:TransactionResult>WAITING</ipgapi:TransactionResult>
    <ipgapi:TransactionTime>1548783233</ipgapi:TransactionTime>
    <ipgapi:RedirectUrl>https://hfgj.chinapnr.com/pay/redirectGw.
htm?sequenceId=2000019929&amp;mac=6783F30480FE694B429ABCA1685ED</ipgapi:RedirectUrl>
</ipgapi:IPGApiOrderResponse>
The ‘RedirectURL’ in the IPG response is where you need to redirect your consumer so that they can continue with
the transaction processing on PNR platform side.
More integration options for China domestic payment methods are described in the Gateway’s Connect
integration guide.


Troubleshooting – Merchant Exceptions
<detail>
        XML is not wellformed: Premature end of message.
</detail>
Possible Explanation:
You have sent an absolutely empty message. The message contains neither a soap message nor an IPG API
message or any other characters in the http body.

<detail>
        XML is not wellformed: Content is not allowed in prolog.
</detail>
Possible Explanation:
The message can’t be interpreted as an XML message.

<detail>
        XML is not wellformed:
        XML document structures must start and end within the same entity.
</detail>
Possible Explanation:
The message starts like an XML message but the end tag of the first open tag is missing.

<detail>
        XML is not wellformed:
	The element type “SOAP-ENV:Body” must be terminated by the matching end-tag
        “&lt;/SOAP-ENV:Body&gt;”.
</detail>
Possible Explanation:
To an open internal tag (not the top level tag) the end tag is missing. In this example the end tag </SOAP-
ENV:Body> is missing.

<detail>
        XML is not wellformed:
        Element type “irgend” must be followed by either attribute specifications, “&gt;” or “/&gt;”.
</detail>
Possible Explanation:
The message isn’t an XML message or a correct XML message. A “>” character is missing for the tag irgend.

<detail>
        XML is not wellformed:
Web Service API Integration Guide                                                                     22. Appendix 127
        Open quote is expected for attribute “xmlns:ns3”
        associated with an element type “ns3:IPGApiOrderRequest”.
</detail>
Possible Explanation:
The value of one attribute isn’t enclosed in quotation marks. In IPG API attributes are only used for the name spaces.

<detail>
        XML is not wellformed:
        The prefix “ipgapi” for element “ipgapi:IPGApiOrderRequest” is not bound.
</detail>
Possible Explanation:
The name space “ipgapi” isn’t declared. To declare a name space use the xmlns prefix. In this case you should
take xmlns:ipgapi=”http://ipg-online.com/ipgapi/schemas/ipgapi” as attribute in the top level tag of the IPG API
message (IPGApiOrderRequest or IPGApiActionRequest).

<detail>
        XML is not wellformed:
        The prefix “xmln” for attribute “xmln:ns2” associated with an element type “ns3:IPGApiOrderRequest”
        is not bound.
</detail>
Possible Explanation:
To declare an own name space, only the predefined name space xmlns allowed. In this case the prefix is written as
xmln and not as xmlns.

<detail>
        XML is not wellformed:
        Unable to create envelope from given source because the namespace was not recognized
</detail>
Possible Explanation:
The message could be interpreted as an XML message and the enclosing soap message is correct, but the
including IPG API message in the soap body has no name spaces or the name spaces are not declared correctly.
The correct name spaces are described in the xsd.

<detail>
        XML is not wellformed:
        The processing instruction target matching “[xX][mM][lL]” is not allowed.
</detail>
Possible Explanation:
The whole message must be a correct XML message so that the including IPG API message must not contains the
xml declaration <?xml … ?>.

<detail>
        Unexpected characters before XML declaration
</detail>
Possible Explanation:
The XML must start with “<?xml”. Please check, if you send an empty line or another white space character in front
of the xml and remove them.

<detail>
        XML is not a SOAP message:
        Unable to create envelope from given source because the root element is not named “Envelope”
</detail>
Possible Explanation:
The message seems to be a correct XML message but only soap messages are accepted. This message must be
enclosed by a soap message.
Web Service API Integration Guide                                                                      22. Appendix 128
<detail>
        XML is not a valid SOAP message:
        Error with the determination of the type.
        Probably the envelope part is not correct.
</detail>
Possible Explanation:
The soap body tag is missing.

<detail>
        Source object passed to ‘’{0}’’ has no contents.
</detail>
Possible Explanation:
The soap body is empty. The including IPG API message is missing.

<detail>
        Included XML is not a valid IPG API message:
        unsupported top level {namespace}tag “irgendwas” in the soap body. Only one of
        [{http://ipg-online.com/ipgapi/schemas/ipgapi}IPGApiActionRequest,
        {http://ipg-online.com/ipgapi/schemas/ipgapi}IPGApiOrderRequest] allowed.
</detail>
Possible Explanation:
The first tag in the including IPG API message must be one of IPGApiActionRequest or IPGApiOrderRequest tag
and not the tag irgendwas. In this case this tag has no namespace.

<detail>
        Included XML is not a valid IPG API message:
	unsupported top level {namespace}tag “{http://firstdata.de/ipgapi/schemas/ipgapi}
        IPGApiOrderRequest” in the soap body. Only one of [{http://ipg-online.com/ipgapi/schemas/ipgapi}
        IPGApiActionRequest, {http://ipg-online.com/ipgapi/schemas/ipgapi}IPGApiOrderRequest] allowed.
</detail>
Possible Explanation:
The top level tag of the included IPG API message no allowed tag. In this case the name space is wrong.

<detail>
        cvc-pattern-valid:
        Value ‘1.234’ is not facet-valid with respect to pattern
        ‘([1-9]([0-9]{0,12}))?[0-9](\.[0-9]{1,2})?’ for type‘#AnonType_ChargeTotalAmount’ cvc-type.3.1.3:
        The value ‘1.234’ of element ‘ns3:ChargeTotal’ is not valid.
</detail>
Possible Explanation:
The value of a tag does not correspond with the declaration in the xsd. The value has three decimal places but the
xsd only allows two.

<detail>
        cvc-complex-type.2.4.a:
        Invalid content was found starting with element ‘ns2:ExpYear’.
        One of ‘{“http://ipg-online.com/ipgapi/schemas/v1”:ExpMonth}’ is expected.
</detail>
Possible Explanation:
The occurrences of the tags must be corresponding to the xsd. We recommend to use the tags in the same
sequence as they are declared in the xsd. In this case the tag ExpMonth is expected and not ExpYear.




Web Service API Integration Guide                                                                   22. Appendix 129
Troubleshooting – Processing Exceptions
<detail>
  <ipgapi:IPGApiOrderResponse
		xmlns:ipgapi=”http://ipg-online.com/ipgapi/schemas/ipgapi”>
		<ipgapi:CommercialServiceProvider />
		<ipgapi:TransactionTime>1233656751183</ipgapi:TransactionTime>
		<ipgapi:ProcessorReferenceNumber />
		<ipgapi:ProcessorResponseMessage />
		<ipgapi:ErrorMessage>
			 SGS-C: 000003:
			       illegal combination of values for the 3DSecure: (VerificationResponse, 		
			       PayerAuthenticationResponse, PayerAuthenticationCode) N Y null
		</ipgapi:ErrorMessage>
		<ipgapi:OrderId />
		<ipgapi:ApprovalCode />
		<ipgapi:AVSResponse />
		<ipgapi:TDate />
		<ipgapi:TransactionResult>FAILED</ipgapi:TransactionResult>
		<ipgapi:TerminalID />
		<ipgapi:ProcessorResponseCode />
<ipgapi:ProcessorApprovalCode />
<ipgapi:ProcessorReceiptNumber />
<ipgapi:ProcessorTraceNumber />
  </ipgapi:IPGApiOrderResponse>
</detail>
Explanation:
The combination of the three values VerificationResponse, PayerAuthenticationResponse and AuthenticationValue
for 3DSecure is wrong. Allowed combinations are

                         Payer-
   Verification-                       Authentication    IPG 3dsecure
                     Authentication-                                                          Comments
    Response                               Value        response code
                       Response
                                                                        Transaction will be passed to auth system without
       null               null             null             n/a         any 3dsecure information
                                                                        No MC ECI, Visa ECI = 7
        N                                                               Cardholder not enrolled
                          null             null              7
                                                                        No MC ECI, Visa ECI = 7
        N                                                               Cardholder not enrolled
                           N               null              7
                                                                        No MC ECI, Visa ECI = 7
        U                                                               Unable to authenticate (DS not accessible)
                          null             null              5
                                                                        No MC ECI, Visa ECI = 7
        Y                                                               Attempt (ACS cannot tell result of authentication)
                           A               null              4
                                                                        MC ECI = 1, Visa ECI = 6
        Y                                                               Attempt (ACS cannot tell result of authentication)
                           A                 x               4
                                                                        MC ECI = 1, Visa ECI = 6
        Y                                                               Unable to authenticate (ACS not accessible)
                           U               null              6
                                                                        No MC ECI, Visa ECI = 7
        Y                                                               Auth Success (no CAAV / UCAF)
                           Y               null              2
                                                                        MC ECI = 2, Visa ECI = 5
        Y                                                               Auth Success
                           Y                 x               1
                                                                        MC ECI = 2, Visa ECI = 5
        Y                                                               Auth Failure (Signature verification incorrect) - IPG
                                                                        declines the transaction
                           N               null              3
                                                                        ( “N:-5101:3D Secure authentication failed” )
                                                                        No MC or Visa ECI

Web Service API Integration Guide                                                                              22. Appendix 130
Other combinations not listed above will be declined by IPG with a IPG 3dsecure response code of 8 and
“N:-5100:Invalid 3D Secure values”.
XID (created by MPI before sending Verification request) needs to be set for VISA transactions.
The payer authentication code x means, that the value is not null.
<detail>
  <ipgapi:IPGApiOrderResponse
  xmlns:ipgapi=”http://ipg-online.com/ipgapi/schemas/ipgapi”>
		<ipgapi:CommercialServiceProvider />
		<ipgapi:TransactionTime>1233659493267</ipgapi:TransactionTime>
		<ipgapi:ProcessorReferenceNumber />
		<ipgapi:ProcessorResponseMessage />
		<ipgapi:ErrorMessage>
			 SGS-005002:
			       The merchant is not setup to support the requested service.
		</ipgapi:ErrorMessage>
		<ipgapi:OrderId>
			 IPGAPI-REQUEST-9c555d62-3850-4726-8589-5a2444c98c5d
		</ipgapi:OrderId>
		<ipgapi:ApprovalCode />
		<ipgapi:AVSResponse />
		<ipgapi:TDate />
		<ipgapi:TransactionResult>FAILED</ipgapi:TransactionResult>
		<ipgapi:TerminalID />
		<ipgapi:ProcessorResponseCode />
		<ipgapi:ProcessorApprovalCode />
		<ipgapi:ProcessorReceiptNumber />
		<ipgapi:ProcessorTraceNumber />
  </ipgapi:IPGApiOrderResponse>
</detail>
Explanation:
This is an example with a German Direct Debit transaction, which is not supported for the merchant. If you should
receive this result for a transaction type which is included in your agreement, please contact our technical support
team.
<detail>
  <ipgapi:IPGApiOrderResponse
  xmlns:ipgapi=”http://ipg-online.com/ipgapi/schemas/ipgapi”>
		<ipgapi:CommercialServiceProvider />
		<ipgapi:TransactionTime>1233656752933</ipgapi:TransactionTime>
		<ipgapi:ProcessorReferenceNumber />
		<ipgapi:ProcessorResponseMessage />
		<ipgapi:ErrorMessage>
			 SGS-005005: Duplicate transaction.
		</ipgapi:ErrorMessage>
		<ipgapi:OrderId>
			 IPGAPI-REQUEST-29351d8e-2634-4725-9d93-91b83704e00d
		</ipgapi:OrderId>
		<ipgapi:ApprovalCode />
		<ipgapi:AVSResponse />
		<ipgapi:TDate />
		<ipgapi:TransactionResult>FRAUD</ipgapi:TransactionResult>
		<ipgapi:TerminalID />
		<ipgapi:ProcessorResponseCode />
		<ipgapi:ProcessorApprovalCode />
		<ipgapi:ProcessorReceiptNumber />
		<ipgapi:ProcessorTraceNumber />
  </ipgapi:IPGApiOrderResponse>
</detail>
Web Service API Integration Guide                                                                     22. Appendix 131
Explanation:
After a transaction further transactions with the same data blocked are for a configurable time span. See User
Guide Virtual Terminal for details about the fraud settings.
<detail>
  <ipgapi:IPGApiOrderResponse
  xmlns:ipgapi=”http://ipg-online.com/ipgapi/schemas/ipgapi”>
		<ipgapi:CommercialServiceProvider />
		<ipgapi:TransactionTime>1233656752308</ipgapi:TransactionTime>
		<ipgapi:ProcessorReferenceNumber />
		<ipgapi:ProcessorResponseMessage />
		<ipgapi:ErrorMessage>
			 SGS-005009:
			       The currency is not allowed for this terminal.
		</ipgapi:ErrorMessage>
		<ipgapi:OrderId>
			 IPGAPI-REQUEST-a58f6631-eb71-49c8-bbca-23fff53252fc
		</ipgapi:OrderId>
		<ipgapi:ApprovalCode />
		<ipgapi:AVSResponse />
		<ipgapi:TDate />
		<ipgapi:TransactionResult>FAILED</ipgapi:TransactionResult>
		<ipgapi:TerminalID />
		<ipgapi:ProcessorResponseCode />
		<ipgapi:ProcessorApprovalCode />
		<ipgapi:ProcessorReceiptNumber />
		<ipgapi:ProcessorTraceNumber />
  </ipgapi:IPGApiOrderResponse>
</detail>
Explanation:
This is an example with US Dollar, which is no allowed currency for this store.
<detail>
  <ipgapi:IPGApiOrderResponse
		xmlns:ipgapi=”http://ipg-online.com/ipgapi/schemas/ipgapi”>
		<ipgapi:CommercialServiceProvider />
		<ipgapi:TransactionTime>1234346305732</ipgapi:TransactionTime>
		<ipgapi:ProcessorReferenceNumber />
		<ipgapi:ProcessorResponseMessage />
		<ipgapi:ErrorMessage>
			 SGS-032000: Unknown processor error occured.
		</ipgapi:ErrorMessage>
		<ipgapi:OrderId>
			 IPGAPI-REQUEST-b3223ee5-156b-4d22-bc3f-910709d59202
		</ipgapi:OrderId>
		<ipgapi:ApprovalCode />
		<ipgapi:AVSResponse />
		<ipgapi:TDate>1234346284</ipgapi:TDate>
		<ipgapi:TransactionResult>DECLINED</ipgapi:TransactionResult>
		<ipgapi:TerminalID />
		<ipgapi:ProcessorResponseCode />
		<ipgapi:ProcessorApprovalCode />
		<ipgapi:ProcessorReceiptNumber />
		<ipgapi:ProcessorTraceNumber />
  </ipgapi:IPGApiOrderResponse>
</detail>
Explanation:
If your transactions are normally executed, one possible explanation is that the number of Terminal IDs assigned
to your store are not sufficient for your transaction volume. Please contact our Sales team to order further Terminal
IDs for load balancing.
Web Service API Integration Guide                                                                     22. Appendix 132
Troubleshooting - Login error messages when using cURL
* About to connect() to test.ipg-online.com port 443 (#0)
* Trying 217.73.32.55... connected
* Connected to test.ipg-online.com (217.73.32.55) port 443 (#0)
* unable to set private key file: ‘C:\API\config\WS120666668._.1.key’ type PEM
* Closing connection #0
curl: (58) unable to set private key file: ‘C:\API\config\WS120666668._.1.key’ type PEM
Explanation:
Keystore and password do not fit. Check if you used the right keystore and password. Please check if you used the
WS<storeId>._.1.pem file. If you append .cer to the file name you can open the certificate with a double click. The
certificate must be exposed for your store. Please remove the extension .cer after the check.
* SSL certificate problem, verify that the CA cert is OK. Details:
error:14090086:SSL routines:SSL3_GET_SERVER_CERTIFICATE:certificate verify failed
* Closing connection #0
curl: (60) SSL certificate problem, verify that the CA cert is OK. Details:
error:14090086:SSL routines:SSL3_GET_SERVER_CERTIFICATE:certificate verify failed
More details here: http://curl.haxx.se/docs/sslcerts.html
curl performs SSL certificate verification by default, using a “bundle” of Certificate Authority (CA) public keys (CA
certs). The default bundle is named curl-ca-bundle.crt; you can specify an alternate file using the --cacert option.
If this HTTPS server uses a certificate signed by a CA represented in the bundle, the certificate verification
probably failed due to a problem with the certificate (it might be expired, or the name might not match the
domain name in the URL).
If you’d like to turn off curl’s verification of the certificate, use the -k (or --insecure) option
Explanation:
The truststore certificate is wrong. Please verify the trustore: Open the file tlstrust.pem and check that one of them
matched the root of the server certificate of the Gateway.
<html>
         <head>
		                <title>Apache Tomcat/5.5.20 - Error report</title>
		<style>
			<!--
H1 {font-family:Tahoma,Arial,sans-serif;color:white;background-color:#525D76;font-size:22px;}
H2 {font-family:Tahoma,Arial,sans-serif;color:white;background-color:#525D76;font-size:16px;}
H3 {font-family:Tahoma,Arial,sans-serif;color:white;background-color:#525D76;font-size:14px;}
BODY {font-family:Tahoma,Arial,sans-serif;color:black;background-color:white;}
B {font-family:Tahoma,Arial,sans-serif;color:white;background-color:#525D76;}
P {font-family:Tahoma,Arial,sans-serif;background:white;color:black;font-size:12px;}
A {color : black;}
A.name {color : black;}
HR {color : #525D76;}
				-->
			</style>
		</head>
		<body>
			                       <h1>HTTP Status 401 - </h1>
			<HR size=”1” noshade=”noshade”>
			<p><b>type</b> Status report</p><p><b>message</b>
				<u></u></p><p><b>description</b>
				                             <u>This request requires HTTP authentication ().</u></p>
			<HR size=”1” noshade=”noshade”>
			<h3>Apache Tomcat/5.5.20</h3>
		</body>
</html>
Explanation:
Your certificates are OK and accepted but your password or your user is wrong.

Web Service API Integration Guide                                                                      22. Appendix 133
Troubleshooting –
Login error messages when using the Java Client
java.io.IOException: Keystore was tampered with, or password was incorrect
Explanation:
Your keystore password doesn’t fit to the keystore or the truststore password to the truststore. You can check the
password with the keytool which is a component of the JDK. You can find it in the bin directory of the JDK. For
testing the password call
c:\Programme\Java\jdk1.6.0_07\bin\keytool.exe -list -v -keystore <your keystore or truststore> -storepass <your
keystore or truststore password>
javax.net.ssl.SSLHandshakeException: sun.security.validator.ValidatorException: No trusted certificate found
Explanation:
Your truststore is wrong. You can inspect your truststore with keytool, a component of the JDK. Call
c:\Programme\Java\jdk1.6.0_07\bin\keytool.exe -list -v -keystore <your truststore> -storepass <your truststore
password>
and you must find the issuer Equifax
OU=Equifax Secure Certificate Authority, O=Equifax, C=US in the output. Check the MD5 and SHA1 values too.
<html>
  <head>
		 <title>Apache Tomcat/5.5.20 - Error report</title>
		<style><!--H1 {font-family:Tahoma,Arial,sans-serif;color:white;background-
color:#525D76;font-size:22px;} H2 {font-family:Tahoma,Arial,sans-
serif;color:white;background-color:#525D76;font-size:16px;} H3 {font-
family:Tahoma,Arial,sans-serif;color:white;background-color:#525D76;font-size:14px;}
BODY {font-family:Tahoma,Arial,sans-serif;color:black;background-color:white;} B
{font-family:Tahoma,Arial,sans-serif;color:white;background-color:#525D76;} P {font-
family:Tahoma,Arial,sans-serif;background:white;color:black;font-size:12px;}A {color
: black;}A.name {color : black;}HR {color : #525D76;}--></style>
  </head>
  <body>
		<h1>HTTP Status 401 -</h1>
		<HR size=”1” noshade=”noshade”>
		<p>
			 <b>type</b>
			 Status report
		</p>
		<p>
			 <b>message</b>
			 <u></u>
		</p>
		<p>
			 <b>description</b>
			 <u>This request requires HTTP authentication ().</u>
		</p>
		<HR size=”1” noshade=”noshade”>
		<h3>Apache Tomcat/5.5.20</h3>
  </body>
</html>


Explanation: Your user id or password is wrong.




Web Service API Integration Guide                                                                      22. Appendix 134
