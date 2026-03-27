# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 2026.03.04.0

### Features

- **connector:**
  - [Adyen] Add Apple pay and Google pay Decrypt for Adyen ([#509](https://github.com/juspay/connector-service/pull/509)) ([`ada0603`](https://github.com/juspay/connector-service/commit/ada06037ada6e62f18bb270dfa3e0366aeb49720))
  - [Truelayer] Integrate OpenBankingUK flows ([#519](https://github.com/juspay/connector-service/pull/519)) ([`40eb645`](https://github.com/juspay/connector-service/commit/40eb64581346db3fd0e4c9424160fdc7ef4218bc))
- Ach bankdebit integration for bluesnap ([#552](https://github.com/juspay/connector-service/pull/552)) ([`d1ed349`](https://github.com/juspay/connector-service/commit/d1ed349a8b5d86ae011a1c76ed511b7ccb836c65))
- Ach bankdebit integration for nuvei ([#551](https://github.com/juspay/connector-service/pull/551)) ([`28cd250`](https://github.com/juspay/connector-service/commit/28cd250652e4d5b6eba50eb4a8205627dd76221d))

**Full Changelog:** [`2026.03.03.0...2026.03.04.0`](https://github.com/juspay/connector-service/compare/2026.03.03.0...2026.03.04.0)

- - -

## 2026.03.27.0

### Features

- **core:** Implement NTID flow for Decrypted Wallet Token and also Implement for checkout connector ([#793](https://github.com/juspay/connector-service/pull/793)) ([`5cd633a`](https://github.com/juspay/connector-service/commit/5cd633a7dc8b04a3a62a605222ba9e9c29c0df0f))

### Bug Fixes

- Webhook api response trait and adyen webhook source verification ([#541](https://github.com/juspay/connector-service/pull/541)) ([`a15c291`](https://github.com/juspay/connector-service/commit/a15c291beb7671c8b276eb68f6b5b17f4d2cf0cf))

### Miscellaneous Tasks

- **rename:** Rename hs playlib ([#794](https://github.com/juspay/connector-service/pull/794)) ([`fab7753`](https://github.com/juspay/connector-service/commit/fab775303e1557aaf7ba000696ebd5d11f0c1a8c))
- Change package version ([#818](https://github.com/juspay/connector-service/pull/818)) ([`2426348`](https://github.com/juspay/connector-service/commit/242634870764a548b557688744e465819d8e19f7))

**Full Changelog:** [`2026.03.26.0...2026.03.27.0`](https://github.com/juspay/connector-service/compare/2026.03.26.0...2026.03.27.0)

- - -

## 2026.03.26.0

### Features

- **client:** Add grpc client support in js, rs, kt and py ([#764](https://github.com/juspay/connector-service/pull/764)) ([`7982ec3`](https://github.com/juspay/connector-service/commit/7982ec31e81be6e2583fa1599df45088966acfef))
- **framework:**
  - Superposition toml parsing implementation ([#591](https://github.com/juspay/connector-service/pull/591)) ([`65a07ae`](https://github.com/juspay/connector-service/commit/65a07aedd7245b62f28a627ced3e832a83621aae))
  - Added ConnectorSpecificConfig for Fiservcommercehub ([#789](https://github.com/juspay/connector-service/pull/789)) ([`7d9c7e9`](https://github.com/juspay/connector-service/commit/7d9c7e918313cf07caf995ee41b68bc4f3d24ce0))
- **payouts:** Add payout flows ([#717](https://github.com/juspay/connector-service/pull/717)) ([`965091e`](https://github.com/juspay/connector-service/commit/965091eb8c8a5029dc8d6009be9922c09fd84adc))
- [GRACE] Add skills to Hyperswitch-Prism ([#781](https://github.com/juspay/connector-service/pull/781)) ([`0b61a93`](https://github.com/juspay/connector-service/commit/0b61a93ee2633e03b69a70ea5d4a25731434173a))

### Documentation

- **README.md:** Reference org repo and minor edits ([#787](https://github.com/juspay/connector-service/pull/787)) ([`806454b`](https://github.com/juspay/connector-service/commit/806454b58fbf1a5194b6f174b615f453f205b5da))
- Restructure and enhance documentation ([#783](https://github.com/juspay/connector-service/pull/783)) ([`cc4b137`](https://github.com/juspay/connector-service/commit/cc4b137cffd6050d7ff764bf79926afc381e06bb))
- Add comprehensive SDK reference documentation for all 4 languages ([#730](https://github.com/juspay/connector-service/pull/730)) ([`3d53997`](https://github.com/juspay/connector-service/commit/3d53997b4f9468706a563c8118cfdd7e8959bde0))

**Full Changelog:** [`2026.03.25.0...2026.03.26.0`](https://github.com/juspay/connector-service/compare/2026.03.25.0...2026.03.26.0)

- - -

## 2026.03.25.0

### Features

- **payout:** Create payout flow ([#659](https://github.com/juspay/connector-service/pull/659)) ([`6f1c7f5`](https://github.com/juspay/connector-service/commit/6f1c7f51fc2a8d504c87cbcc6fc1d4f6b29833e4))

### Bug Fixes

- **connector:** [REVOLUT] amount and id fixes for revolut euler-ucs ([#778](https://github.com/juspay/connector-service/pull/778)) ([`6181a56`](https://github.com/juspay/connector-service/commit/6181a56f8be37f951d0df64731b834b9b8a3ed97))
- Migrate `connector_feature_data` mca configs to `ConnectorSpecificConfig` ([#723](https://github.com/juspay/connector-service/pull/723)) ([`a05e5e1`](https://github.com/juspay/connector-service/commit/a05e5e178cf133b44070d845c75f418f4666f0f0))

**Full Changelog:** [`2026.03.24.0...2026.03.25.0`](https://github.com/juspay/connector-service/compare/2026.03.24.0...2026.03.25.0)

- - -

## 2026.03.24.0

### Features

- **connector:** Add fiservcommercehub cards ([#725](https://github.com/juspay/connector-service/pull/725)) ([`c1e03e9`](https://github.com/juspay/connector-service/commit/c1e03e91275b1815abc0369ff46b871005487b69))

### Bug Fixes

- **proto:** Resolve proto consistency issues from review ([#720](https://github.com/juspay/connector-service/pull/720)) ([`bb44fb1`](https://github.com/juspay/connector-service/commit/bb44fb1ab40dfa2cfc55f606dcc764e8282b2e5b))

### Refactors

- **peachpayments:** Extract webhook body parsing to helper function and remove unused struct ([#685](https://github.com/juspay/connector-service/pull/685)) ([`4bf3703`](https://github.com/juspay/connector-service/commit/4bf37031dc0fe03cfed194d7af62a77134be6d11))

### Miscellaneous Tasks

- Folder restructure ([#756](https://github.com/juspay/connector-service/pull/756)) ([`d3a555d`](https://github.com/juspay/connector-service/commit/d3a555d57217f6119ff83e9fd05f261689572421))

**Full Changelog:** [`2026.03.23.0...2026.03.24.0`](https://github.com/juspay/connector-service/compare/2026.03.23.0...2026.03.24.0)

- - -

## 2026.03.23.0

### Features

- **connector:** Adyen network token ([#631](https://github.com/juspay/connector-service/pull/631)) ([`b706342`](https://github.com/juspay/connector-service/commit/b7063424b41b8a1aa193cf3c4094bf2119146e32))
- **framework:** Add GRACE AI to connector-service ([#718](https://github.com/juspay/connector-service/pull/718)) ([`d2a922c`](https://github.com/juspay/connector-service/commit/d2a922c02b29f058bec695517cdb6cf8d7b61d75))
- **proto:** Add payouts proto contract ([#616](https://github.com/juspay/connector-service/pull/616)) ([`dcf3aaf`](https://github.com/juspay/connector-service/commit/dcf3aafa09c22572fb14005faf597dd12e05ad9a))

### Documentation

- Restructure and improve documentation ([#728](https://github.com/juspay/connector-service/pull/728)) ([`f18221a`](https://github.com/juspay/connector-service/commit/f18221adf3c2aaf158a7718c6f4cd1d7e8302ed4))

### Miscellaneous Tasks

- **sdk-error:** Update sdk error to new proto ([#719](https://github.com/juspay/connector-service/pull/719)) ([`4984edc`](https://github.com/juspay/connector-service/commit/4984edcbdddbe62de2f2fd26220394ffb7ca5d0b))

**Full Changelog:** [`2026.03.19.0...2026.03.23.0`](https://github.com/juspay/connector-service/compare/2026.03.19.0...2026.03.23.0)

- - -

## 2026.03.19.0

### Features

- Error proto refactor ([#669](https://github.com/juspay/connector-service/pull/669)) ([`1790d55`](https://github.com/juspay/connector-service/commit/1790d5575fd1b400fb8ecf1bfaa0bfeafb33e791))

### Refactors

- **docs:** Add support for connector wise request example in multiple languages ([#637](https://github.com/juspay/connector-service/pull/637)) ([`2d66cc8`](https://github.com/juspay/connector-service/commit/2d66cc889ce9a31cd4a9b5be02510ffe90bdb93f))

### Documentation

- Restructure documentation with /docs and /docs-generated separation ([#684](https://github.com/juspay/connector-service/pull/684)) ([`d48fa46`](https://github.com/juspay/connector-service/commit/d48fa4628111cd58e93aca725acb00c4882c2929))

**Full Changelog:** [`2026.03.18.0...2026.03.19.0`](https://github.com/juspay/connector-service/compare/2026.03.18.0...2026.03.19.0)

- - -

## 2026.03.18.0

### Features

- **connector:**
  - [peachpayments] add no 3ds cards, network token payment methods ([#607](https://github.com/juspay/connector-service/pull/607)) ([`b9de702`](https://github.com/juspay/connector-service/commit/b9de702805ba45ed1accedb342362364b1aebe7a))
  - PPRO connector integration ([#568](https://github.com/juspay/connector-service/pull/568)) ([`70e4f6e`](https://github.com/juspay/connector-service/commit/70e4f6e70ad7568b9f51bf59487a3aac1b8387c9))
- **framework:** Use hyperswitch_masking from crates.io instead of git dependency ([#660](https://github.com/juspay/connector-service/pull/660)) ([`4782efa`](https://github.com/juspay/connector-service/commit/4782efa705106a827bce38d8a89d351e22ea434e))

### Miscellaneous Tasks

- Implemented Refund And RefundGet Flow For Composite Service ([#608](https://github.com/juspay/connector-service/pull/608)) ([`d0aa0a5`](https://github.com/juspay/connector-service/commit/d0aa0a55e8ea792037b500229c5d6e0ccca12d65))
- Implemented Void and Capture Flows For Composite Service ([#624](https://github.com/juspay/connector-service/pull/624)) ([`c0c71fc`](https://github.com/juspay/connector-service/commit/c0c71fc7fef7e5e661a917a8d9418b50025c7cac))

**Full Changelog:** [`2026.03.17.0...2026.03.18.0`](https://github.com/juspay/connector-service/compare/2026.03.17.0...2026.03.18.0)

- - -

## 2026.03.17.0

### Features

- **framework:** Add merchant_transaction_id in PaymentServiceGetRequest & PaymentServiceGetResponse ([#654](https://github.com/juspay/connector-service/pull/654)) ([`c43491b`](https://github.com/juspay/connector-service/commit/c43491baea60975cd2a0c5534de20ca513c9aa73))
- Http client sanity runner ([#621](https://github.com/juspay/connector-service/pull/621)) ([`6565db7`](https://github.com/juspay/connector-service/commit/6565db7363334e62252cef28480379f3e6eb10d7))

### Refactors

- **codegen:** Organize templates into per-language subdirectories ([#652](https://github.com/juspay/connector-service/pull/652)) ([`ec40f8b`](https://github.com/juspay/connector-service/commit/ec40f8bbaf45799caf86913346e2c9cd4ea7c13f))

### Miscellaneous Tasks

- **error:** Ffi error handling ([#661](https://github.com/juspay/connector-service/pull/661)) ([`985c55f`](https://github.com/juspay/connector-service/commit/985c55fad84bcfd2b109918535cb81713ba7af89))
- **uniffii:** Revert error handling ([#656](https://github.com/juspay/connector-service/pull/656)) ([`9774c5b`](https://github.com/juspay/connector-service/commit/9774c5b81eaa95214febda7bd8705bbb6d747317))

**Full Changelog:** [`2026.03.16.0...2026.03.17.0`](https://github.com/juspay/connector-service/compare/2026.03.16.0...2026.03.17.0)

- - -

## 2026.03.16.0

### Features

- **docs:** Add automated connector docs ([#625](https://github.com/juspay/connector-service/pull/625)) ([`00b9804`](https://github.com/juspay/connector-service/commit/00b98047742438f29dee827484bc069bfad4fa1d))
- **domain:** Unify ConnectorSpecificAuth → ConnectorSpecificConfig ([#627](https://github.com/juspay/connector-service/pull/627)) ([`a7a696c`](https://github.com/juspay/connector-service/commit/a7a696c3a3f546616f74a85584ab123ddf1cda15))

### Refactors

- **generate.py:** Refactor code generation to use Jinja2 templates ([#645](https://github.com/juspay/connector-service/pull/645)) ([`30cef9b`](https://github.com/juspay/connector-service/commit/30cef9baaf7acec58b0256a24386f3fc11a3048e))

### Documentation

- Add Unified Payment Protocol (UPP) specification RFC ([#646](https://github.com/juspay/connector-service/pull/646)) ([`9f140bd`](https://github.com/juspay/connector-service/commit/9f140bd54c174fce53ab4fc7374f9842e3459c1e))
- Launch blog for review ([#642](https://github.com/juspay/connector-service/pull/642)) ([`29e2fb9`](https://github.com/juspay/connector-service/commit/29e2fb9c33315bcd180c5798efce37adc528bb87))
- Update SDK javascript README ([#640](https://github.com/juspay/connector-service/pull/640)) ([`36ef23e`](https://github.com/juspay/connector-service/commit/36ef23e3369d06c8f61573875ad17bc1f1495429))
- Update API reference documentation and navigation ([#633](https://github.com/juspay/connector-service/pull/633)) ([`f57de07`](https://github.com/juspay/connector-service/commit/f57de079efd841b47afbcc725e9ed90a485c5847))

### Miscellaneous Tasks

- Scenario Based Test framework for UCS ([#580](https://github.com/juspay/connector-service/pull/580)) ([`01c4768`](https://github.com/juspay/connector-service/commit/01c4768db0ef751306ba5b0f0fa0eef24529efd9))

**Full Changelog:** [`2026.03.13.0...2026.03.16.0`](https://github.com/juspay/connector-service/compare/2026.03.13.0...2026.03.16.0)

- - -

## 2026.03.13.0

### Features

- **connector:** [Truelayer] Implement webhooks for payments and refunds ([#602](https://github.com/juspay/connector-service/pull/602)) ([`ffe6888`](https://github.com/juspay/connector-service/commit/ffe6888f6a8d339fb931188980221aa20a9e6784))

### Miscellaneous Tasks

- **error:** Add request and response error proto for ffi implementation ([#610](https://github.com/juspay/connector-service/pull/610)) ([`ac90f77`](https://github.com/juspay/connector-service/commit/ac90f773fbd450caa74a1ed2f3da145282a4fa7a))
- **remove:** Remove unused code ([#628](https://github.com/juspay/connector-service/pull/628)) ([`ee6e1b9`](https://github.com/juspay/connector-service/commit/ee6e1b9fadaef928dde3f9718a540f39283e562d))

**Full Changelog:** [`2026.03.12.0...2026.03.13.0`](https://github.com/juspay/connector-service/compare/2026.03.12.0...2026.03.13.0)

- - -

## 2026.03.12.0

### Features

- **framework:** Added all available services in app.rs ([#618](https://github.com/juspay/connector-service/pull/618)) ([`b05c372`](https://github.com/juspay/connector-service/commit/b05c37205e42d501d4bda5ebb010cfc529062879))
- **payment_methods:** Add support for Samsung Pay ([#558](https://github.com/juspay/connector-service/pull/558)) ([`e35afcc`](https://github.com/juspay/connector-service/commit/e35afccda14851fb1f6321cf9116a19398deb427))

### Refactors

- **proto:** Refactor id_type to string ([#604](https://github.com/juspay/connector-service/pull/604)) ([`3c66b11`](https://github.com/juspay/connector-service/commit/3c66b11ce7ee18bc93fc10e62b4bf295248e25b1))

**Full Changelog:** [`2026.03.11.0...2026.03.12.0`](https://github.com/juspay/connector-service/compare/2026.03.11.0...2026.03.12.0)

- - -

## 2026.03.11.0

### Features

- **connector:** [Checkout] Add l2_l3 data support in checkout ([#565](https://github.com/juspay/connector-service/pull/565)) ([`ed05bce`](https://github.com/juspay/connector-service/commit/ed05bceb35ecbbb37c50eb34af9629748bf4ade3))
- [AUTHORIZEDOTNET] ACH BankDebit ([#549](https://github.com/juspay/connector-service/pull/549)) ([`54df6fa`](https://github.com/juspay/connector-service/commit/54df6fa301550120678b10565f3b9dc9b5ffaafd))
- [FINIX] CARDS NO3DS , ACH BankDebit ([#564](https://github.com/juspay/connector-service/pull/564)) ([`6181e60`](https://github.com/juspay/connector-service/commit/6181e601d90f339c55887f70e742f9418d8ebfe2))

**Full Changelog:** [`2026.03.10.0...2026.03.11.0`](https://github.com/juspay/connector-service/compare/2026.03.10.0...2026.03.11.0)

- - -

## 2026.03.10.0

### Features

- **payment_methods:** Add ACH (eCheck) support to Forte connector ([#576](https://github.com/juspay/connector-service/pull/576)) ([`d696886`](https://github.com/juspay/connector-service/commit/d696886a6db355c631d49069a3f0c0fbfa353d5b))
- **proto:** Change the proto package name ([#603](https://github.com/juspay/connector-service/pull/603)) ([`19101f3`](https://github.com/juspay/connector-service/commit/19101f3bf2900dd6f1953db48cb26900051ef8ac))
- Proto changes for sdk configs overridable vs non overridable ([#589](https://github.com/juspay/connector-service/pull/589)) ([`017e0e3`](https://github.com/juspay/connector-service/commit/017e0e3601cb68215c9a371c411486e9b21c0b5a))

### Refactors

- Simplify header handling by making them optional and inferring connector from `x-connector-auth` header ([#590](https://github.com/juspay/connector-service/pull/590)) ([`6fd2e37`](https://github.com/juspay/connector-service/commit/6fd2e37f51f1f6fa149270c1c667560b88c46aaa))

### Miscellaneous Tasks

- Added Composite Get Flow ([#575](https://github.com/juspay/connector-service/pull/575)) ([`1179bb7`](https://github.com/juspay/connector-service/commit/1179bb78718adf692df1c465cea02c9e40e01d65))

**Full Changelog:** [`2026.03.09.0...2026.03.10.0`](https://github.com/juspay/connector-service/compare/2026.03.09.0...2026.03.10.0)

- - -

## 2026.03.09.0

### Bug Fixes

- **clippy:** Fix clippy error ([#596](https://github.com/juspay/connector-service/pull/596)) ([`eb197f7`](https://github.com/juspay/connector-service/commit/eb197f7e187315dcbe3a0e86749660eb99a952e4))

### Refactors

- **client:** Per-service SDK clients from services.proto boundaries ([#595](https://github.com/juspay/connector-service/pull/595)) ([`911373a`](https://github.com/juspay/connector-service/commit/911373a896eebc787a8c2b592863c1e46be58d34))

**Full Changelog:** [`2026.03.06.0...2026.03.09.0`](https://github.com/juspay/connector-service/compare/2026.03.06.0...2026.03.09.0)

- - -

## 2026.03.06.0

### Features

- Ach bankdebit integration for checkout ([#547](https://github.com/juspay/connector-service/pull/547)) ([`4ba9117`](https://github.com/juspay/connector-service/commit/4ba9117fcbb6d2bcde93fcb2eff6588960f98808))
- Ach bankdebit integration for jpmorgan ([#553](https://github.com/juspay/connector-service/pull/553)) ([`61aec3b`](https://github.com/juspay/connector-service/commit/61aec3bf5f2e76aeefe2f5cad930601acb88d533))
- Helper for multi form data as bytes to support with ffi lang ag… ([#566](https://github.com/juspay/connector-service/pull/566)) ([`d44785e`](https://github.com/juspay/connector-service/commit/d44785e87fbdc0b6e8ea66b3dadf3944865fc557))
- FFI implementation ([#515](https://github.com/juspay/connector-service/pull/515)) ([`00e5edf`](https://github.com/juspay/connector-service/commit/00e5edf0369cb6007010b23fb26bc43dda1adf8f))
- [PAYSAFE] ACH BankDebit ([#556](https://github.com/juspay/connector-service/pull/556)) ([`f9300f3`](https://github.com/juspay/connector-service/commit/f9300f3d040c78aee5d793ec10a56bc68325120b))

### Bug Fixes

- Redirect response removed ([#514](https://github.com/juspay/connector-service/pull/514)) ([`b275d05`](https://github.com/juspay/connector-service/commit/b275d05066690e080e5dfb658ccf567e9d3aea24))

**Full Changelog:** [`2026.03.04.0...2026.03.06.0`](https://github.com/juspay/connector-service/compare/2026.03.04.0...2026.03.06.0)

- - -

## 2026.03.03.0

### Features

- **connector:** [Revolut] Rename RevolutAuth "api_key" field and add "signing_secret" for webhook source verification ([#570](https://github.com/juspay/connector-service/pull/570)) ([`6bc06bd`](https://github.com/juspay/connector-service/commit/6bc06bdc3f5f332d605a5541dc3e57bce20903a6))

**Full Changelog:** [`2026.03.02.0...2026.03.03.0`](https://github.com/juspay/connector-service/compare/2026.03.02.0...2026.03.03.0)

- - -

## 2026.03.02.0

### Features

- **connector:** [revolv3] add recurring support for non-3ds card payments ([#554](https://github.com/juspay/connector-service/pull/554)) ([`6a85fd4`](https://github.com/juspay/connector-service/commit/6a85fd47eeaacc6fa40fd38c91930c373eb7a476))
- Webhook support for paypal ([#440](https://github.com/juspay/connector-service/pull/440)) ([`7e9496f`](https://github.com/juspay/connector-service/commit/7e9496f2b0d261c0092241efc8875d23b4e6161d))
- [NOVALNET] ACH BankDebit ([#563](https://github.com/juspay/connector-service/pull/563)) ([`c4f52ae`](https://github.com/juspay/connector-service/commit/c4f52ae0a89ba63a87efca7b3cdd53413da1647a))
- Typed ConnectorSpecificAuth with header-based auth resolution via X-Connector-Auth ([#555](https://github.com/juspay/connector-service/pull/555)) ([`23fb46a`](https://github.com/juspay/connector-service/commit/23fb46a5c6e36b3454124b413e6db585ebcd0cac))

### Miscellaneous Tasks

- **connector:** Add warning comment about dtd validation to redsys soap api ([#560](https://github.com/juspay/connector-service/pull/560)) ([`a941b53`](https://github.com/juspay/connector-service/commit/a941b531eacc40684ed8800a93c373f84201f20e))

**Full Changelog:** [`2026.02.26.0...2026.03.02.0`](https://github.com/juspay/connector-service/compare/2026.02.26.0...2026.03.02.0)

- - -

## 2026.02.26.0

### Features

- **connector:** [revolv3] add no-threeds card payments ([#520](https://github.com/juspay/connector-service/pull/520)) ([`4cf7158`](https://github.com/juspay/connector-service/commit/4cf7158a744fc77bf23765a0c00951059197cb8a))
- **core:** Added Missing BankTransfer, BankDebit & BankRedirect Payment Method Types ([#538](https://github.com/juspay/connector-service/pull/538)) ([`84493fe`](https://github.com/juspay/connector-service/commit/84493fefd9acfa016d380683a7a8c5e2e32d6b1f))
- [STAX] ACH BankDebit ([#548](https://github.com/juspay/connector-service/pull/548)) ([`50bf11c`](https://github.com/juspay/connector-service/commit/50bf11c04e0158e6e0b425b22d0695df380b9522))

### Miscellaneous Tasks

- Added Composite Authorize Flow ([#517](https://github.com/juspay/connector-service/pull/517)) ([`fedc4ad`](https://github.com/juspay/connector-service/commit/fedc4ad617862addc81c08016635380031accf12))

**Full Changelog:** [`2026.02.25.0...2026.02.26.0`](https://github.com/juspay/connector-service/compare/2026.02.25.0...2026.02.26.0)

- - -

## 2026.02.25.0

### Features

- **connector:** [Checkout] Implement googlepay and applepay decrypt flow and card ntid flow ([#546](https://github.com/juspay/connector-service/pull/546)) ([`576dfbe`](https://github.com/juspay/connector-service/commit/576dfbe4c3e3113a30d607c84d1bdcd43e26412b))
- Ach bankdebit integration for nmi ([#545](https://github.com/juspay/connector-service/pull/545)) ([`e07b1c3`](https://github.com/juspay/connector-service/commit/e07b1c3b71d02396fc6c8284dddee8958b8e3e40))

### Miscellaneous Tasks

- Refactored the wallet Payment Method ([#526](https://github.com/juspay/connector-service/pull/526)) ([`bb898de`](https://github.com/juspay/connector-service/commit/bb898deefab57b6100ca07754c1427a8035cfe50))

**Full Changelog:** [`2026.02.24.0...2026.02.25.0`](https://github.com/juspay/connector-service/compare/2026.02.24.0...2026.02.25.0)

- - -

## 2026.02.24.0

### Features

- **connector:** Adyen voucher paymentmethod added ([#500](https://github.com/juspay/connector-service/pull/500)) ([`948bd45`](https://github.com/juspay/connector-service/commit/948bd45c0a5ba816a25f2793265c2469609f4e69))

**Full Changelog:** [`2026.02.23.0...2026.02.24.0`](https://github.com/juspay/connector-service/compare/2026.02.23.0...2026.02.24.0)

- - -

## 2026.02.23.0

### Features

- **connector:** [trustpay] introduce wallet support - apple pay and google pay ([#503](https://github.com/juspay/connector-service/pull/503)) ([`5976300`](https://github.com/juspay/connector-service/commit/5976300a6eb3746990502970ca089b4eac4b4e24))

**Full Changelog:** [`2026.02.20.0...2026.02.23.0`](https://github.com/juspay/connector-service/compare/2026.02.20.0...2026.02.23.0)

- - -



### Bug Fixes

- Add protoc installation in ci

- Fmt

- Clippy and spell checks 

- Run ci checks in merge queue 

- **core:** Fixed the rust client library and its usage 

- **connector:** [ADYEN] Fix Error Response Status 

- **config:** Add list parse key for proxy.bypass_proxy_urls environment variable 

- Proto fixes (Add Implementations for RefundService and DisputeService) 

- Revoked the ability of child to mutate payment flow data 

- Order_id is made optional 

- Changing default return status type to authorizing 

- Removed the default non deterministic fallback from amount converter 

- Status code not optional 

- Razorpay error status fix 

- Paytm naming 

- **connector-integration:** Update expand_fn_handle_response macro with preprocess_response logic 

- Sanitize the branch name with Slash for image tag creation 

- **connector:** Fix authorizedotnet payment flows with adding preprocess response bytes method 

- Raw connector response changes 

- Initialize Kafka metrics at startup and resolve Clippy warnings in common-util crate 

- Convert _DOT_ to . for audit event nested keys ENV parsing 

- Convert _DOT_ to . for audit event nested keys ENV parsing for transformation and extraction 

- Added masked_serialize for audit events 

- Razorpay reference id 

- Initializing event publisher only if config.event is enabled 

- Improve flow mapping and make audit events fail-safe 

- Capture method optional handling 

- Customer_id for authorizedotnet 

- Email consumption from payment method billing in Razorpay 

- Docker public repo fix 

- **configs:** Add Bluecode's base url in sandbox and production configs 

- **cybersource:** Use minor_refund_amount instead of minor_payment_amount in refund transformer 

- Resolve disparity in Authorizedotnet flows (Authorize, RepeatPayment, SetupMandate) 

- **Access_token_flow:** Added proto field to accept expires_in_seconds in request 

- **cybersource:** Use security_code and state_code in authorize flow 

- **audit:** Ensure grpc audit events emit even for early request parsing failures 

- Authentication flow request and response handling fix 

- Fixed xendit tests for pending cases 

- Noon expiry year and fiuu three ds 

- **stripe:** Update error handling to use message instead of code for response errors 

- **noon:** Update error response message handling to use the correct message field 

- **cybersource:** Update error handling to use message instead of reason 

- Add optional error_reason field to payment responses 

- Diff fixes for Novalnet Authorize flow 

- **noon:** Refund diff check for connector noon 

- **razorpay:** Change payment_capture field type from boolean to integer 

- Capture body changes and baseurl changes 

- Adyen Diff Check Resolve 

- **Braintree:** Refund diff check for connector Braintree 

- Mapping wrongly done for hipay in types.rs 

- Stripe connector_response diff fix 

- Change address type for Customer Create and PaymenMethodToken Create Request 

- Sandbox url fix 

- [WORLDPAYVANTIV] sandbox url fix 

- **Trustpay:** AccessToken creation fix 

- **Rapyd:** Authorize diff check fix 

- Merchant_reference_payment_id proto change 

- Removed git from dockerignore to add build versions in health check 

- **Fiserv:** Authorize, Capture, Void, Refund diff check for connector Fiserv 

- Reverting merchant_reference_payment_id field addition 

- Populate payment method token for AuthorizeOnly request 

- Fix Customer_Acceptance conversion from proto to connector_type 

- Diff correction for multisafepay 

- **bluesnap:** Address `merchantTransactionId` being `IRRELEVANT_ATTEMPT_ID` instead of actual `attempt_id` 

- Adyen prod diff check parity 

- Diff checker changes in hipay 

- Fixed metadata to accept all values in Authorize flow 

- Checkout Diff check fixes 

- Removed extra ; in payments.proto file 

- **connector:** [paysafe] make payment method token calls work for authorizeonly flow 

- Status handling to use router_data.status during error case 2xx 

- Diff check fixes for Xendit Authorize flow 

- Adyen brand name lower case to match hyperswitch diff 

- **connector:** [bluesnap] pass `connector_request_ref_id` instead of `payment_id` 

- **connector:** Fiserv RSync flow Diff fix 

- Correct mapping of metadata 

- Capture, Void, Refund Request 

- Removed the authorization_indicator_type field from Authdotnet Req 

- **connector:** Paypal Capture & Void flow 

- [WORLPAYVANTIV] Diff Checks 

- Diff check fixes for Dlocal 

- Adyen url on non test mode for authorize,void,etc 

- Remove the parallel execution of test in Run test 

- Remove unused field 

- Added Capture Method in Cybersource Repeat Payment Response 

- CavvAlgorithm in proto missing field 

- Resolved RouterData diffs in Prod for Authorizedotnet  

- **connector:** Fix Razorpay metadata to accept all values 

- RepeatPayment Merchant configured Currency Handling 

- Adyen shoppername to none for bankredirect, repeatpayment 

- RouterData diff fix for Novalnet & Cashtocode 

- RouterData diff fix for Fiuu PSync 

- Add secondary base url for Fiuu 

- Diff fix for adyen and paypal repeat payments 

- [CYBERSOURCE] PSYNC DIFF FIX 

- Trustpay refund fix 

- Paypal missing redirect_uri logic in form_fields for 3DS flow 

- **payload:** Do not pass `content-type` header in sync calls 

- **connector:** Map `Ds_State` to status in Redsys PSync when `Ds_Response` is absent 

- **connector:** Rapyd amount type in request 

- Adyen webhook fix 

- Added missing proto to domain conversion of merchant_account_metadata for setupmandate 

- **connector:** [NOVALNET] Populating connector transaction id during 2xx failures 

- **connector:** Request diff fix for Stripe & Cybersource 

- **connector:** [NEXIXPAY] DIFF FIX 

- **connector:** [Fiuu] Fixed payment status being sent as Pending for Fiuu when the connector response is FiuuPaymentsResponse::Error 

- Handled metadata Parsing Err Gracefully in Core 

- Revert "Handled metadata Parsing Err Gracefully in Core" 

- PAYPAL Authorize 2xx error handling and connector_metadata diff in psync 

- **payment_method:** Blik and sofort bank redirect payment method type defaulting to card 

- **connector:** Paypal Router Data Fix in Authorize and RepeatPayment Flow 

- Populate connector response for Repeat Everything Flow's Err response 

- **connector:** Mifinity 5xx Error Handling 

- **connector:** Fixed Volt Default Response and PSync Response Handling 

- **connector:** Noon RSync Url & Default Status 

- Incremental_authorization_allowed and cybersource repeatpayment diff fix 

- **redsys:** Correct XML element ordering in SOAP sync requests to comply with DTD validation 

- Add dev tools via nix

- Standardize setup instructions to use 'make run' in SDK makefiles and READMEs

- Addressing comments of pr #515 

- Install libpq for macOS builds

- Make SDK Makefiles work from any directory


### Documentation

- Add memory banks for folder on interests 

- **setup.md:** Add setup instructions for local development setup 

- **setup.md:** Toml always prod.toml issue fix for docker 

- Remove example directory references from SDK READMEs


### Features

- **core:** Added macros and Adyen authorize with macros 

- **core:** Add Setup Mandate Flow 

- **core:** Added accept dispute (L2) and accept dispute for Adyen (L3) 

- **core:** Added Submit evidence (L2) and Submit evidence for Adyen (L3) 

- **core:** Implement Error Framework 

- **connector:** Added macros for adyen flows 

- Add macro implementations for granular apis in L2 layer 

- **docs:** Connector Integration With Macros Guide Doc 

- **core:** Added Defend Dispute flow (L2) and Adyen Defend Dispute(L3) 

- **core:** Added Dispute Webhooks flow (L2) and Dispute Webhooks for Adyen (L3) 

- **core:** [ADYEN, RAZORPAY] Added util functions for Connector Specifications & Validations 

- **core:** Added Google Pay and Apple Pay Wallets(L2) and Adyen (L3) flow 

- Add all_keys_required and raw_connector_response 

- **core:** Added response preprocessing in macros 

- **connector:** Added cards flow and unit tests for Fiserv 

- **connector:** Added cards flow and unit tests for elavon 

- **core:** Downgrade Resolver to Fix compatibility with Hyperswitch 

- **connector:** Added cards flow and unit tests for Xendit 

- Add HTTP health endpoint for Kubernetes probes 

- **connector:** Added Authorization flow and tests for checkout 

- Add structured logs 

- Adding integrity framework support 

- Added Metrics to the UCS 

- Adding source verification framework 

- **connector:** Added cards flow and unit tests for Authorizedotnet 

- Razorpay integration v2/v1 

- Phonepe UPI integration 

- Cashfree upi integration 

- **connector:** Added cards flow and tests for Fiuu 

- **connector:** [PAYU] Payu Connector Integration 

- Network status being passed 

- **connector:** Added authorize flow and tests for Cashtocode and Reward PaymentMethod 

- Headers Passing 

- **connector:** Added cards flow and tests for Novalnet 

- **config:** Add Coderabbit Configuration 

- Add new trait for payment method data type  

- **connector:** [NEXINETS] Connector Integration 

- Patym upi integration 

- **connector:** [NOON] Connector Integration 

- Add audit logging and direct Kafka logging with tracing-kafka 

- **connector:** [PAYU] Payu PSync flow 

- Adding sync for phone pe 

- **connector:** [MIFINITY] Connector Integration 

- **core:** Implemented CardNumber type in proto 

- **core:** Added Secret String Type in Proto 

- **core:** Renamed cards, common_enums and common_utils crate 

- **config:** Updated Coderabbit Guidelines 

- **connector:** Added wallet payments support for Novalnet 

- **core:** Added Masked Serialize for Golden Log Lines and Added SecretString type to Emails and Phone Number in Proto 

- **core:** Setup G2H to use compile_protos_with_config() function 

- Implement lineage ID tracking for distributed request tracing 

- **core:** Added SecretString type for first_name and last_name 

- **core:** Injector crate addition 

- **connector:** [BRAINTREE] Connector Integration and PaymentMethodToken flow 

- Setup automated nightly release workflows 

- **core:** Access token flow 

- **connector:** [VOLT] Connector Integration  

- **connector:** [BLUECODE] Added Bluecode Wallet in UCS 

- Introduce production/sandbox configs 

- **core:** Implement two step payment webhooks processing 

- **connector:** Added authorize, psync and tests for Cryptopay and CryptoCurrency PaymentMethod 

- Added raw_connector_request in ucs response 

- Emit event for grpc request and refactor event publisher to synchronous 

- **connector:** [HELCIM] Connector Integration  

- **core:** PreAuthenticate, Authenticate and PostAuthenticate flow 

- **connector:** [Dlocal] Connector Integration 

- **connector:** [Placetopay] Connector Integration 

- Emitting lineage id an reference id to kafka metadata in events 

- **connector:** [Rapyd] Connector Integration 

- **framework:** Run UCS in Shadow mode  

- **connector:** [Aci] Connector Integration 

- **connector:** [TRUSTPAY] Connector Integration PSync flow 

- **connector:** Added AccessToken flow for trustpay 

- **connector:** Added cards flow and tests for Stripe 

- **core:** Added SecretString type for raw_connector_request and raw_connector_response 

- **connector:** [CYBERSOURCE] Connector Integration 

- **core:** Added Create connector customer flow 

- Adding_new_field_for_Merchant_account_metadata 

- **connector:** Diff check fixes for Stripe, Cybersource & Novalnet 

- **connector:** [Worldpay] Connector Integration  

- **connector:** [Worldpayvantiv] Connector Integration and VoidPostCapture flow implemented 

- **connector:** Added SetupMandate, RepeatPayment and CreateConnectorCustomer flows for stripe 

- **connector:** Added RepeatPayment flow for cybersource 

- **connector:** [payload] implement core flows, card payment method and webhooks 

- Unmask x-shadow-mode header in logs 

- **connector:** [FISERVEMEA] Connector Integration  

- Add test mode and mock PG API integration 

- **connector:** [DATATRANS] Connector Integration  

- **connector:** [AUTHIPAY] Connector Integration 

- **connector:** Added Refund flow for Authorizedotnet 

- Add wait screen information for UPI payments 

- **connector:** [SILVERFLOW] Connector Integration  

- **connector:** [CELEROCOMMERCE] Connector Integration 

- Introduce session token create grpc function 

- Introduce access token create grpc function 

- **connector:** [Paypal] Connector Integration 

- **connector:** [STAX] Connector Integration 

- **connector:** [Stripe] Add Apple pay, Google pay & PaymentMethodtoken flow for Stripe 

- Introduce payment authorize only create grpc function 

- Client creation based on proxy 

- **trustpay:** Implement error type mapping and enhance error handling 

- Introduce connector customer create grpc function 

- Encoded data in psync separate field 

- Introduce create order grpc function 

- **connector:** [MULTISAFEPAY] Connector Integration 

- Introduce create payment method token create grpc function 

- Introduce register only grpc function 

- **connector:** [HIPAY] Connector Integration  

- **connector:** [TRUSTPAYMENTS] Connector Integration 

- **connector:** [GLOBALPAY] Connector Integration 

- **connector:** Add bluesnap -- no3ds authorize, void, capture, refund, psync, rsync and webhooks 

- **connector:** [paysafe] integrate no3ds card, refund, void, capture 

- Added Config Overrides 

- **connector:** Billwerk Connector Integration 

- **connector:** [NMI] Connector Integration 

- Enhance gRPC payment requests with order_id, payment_method_token, and access_token support 

- **connector:** Add Forte Connector 

- **connector:** [SHIFT4] Connector Integration 

- **connector:** Added bamboraapac integration 

- **connector:** [IATAPAY] Connector Integration 

- **connector:** [NEXIXPAY] Connector Integration 

- **core:** Added SdkSessionToken Flow support 

- **connector:** [NUVEI] Connector Integration  

- GooglePayThirdPartySdk, ApplePayThirdPartySdk, PaypalSdk wallet support for braintree 

- **connector:** Introduce barclaycard  

- Paypal refund rsync flow 

- **connector:** [AIRWALLEX] Connector Integration 

- **framework:** Implemented Custom HTTP Integration Layer 

- **connector:** Trustpay Refund & RSync flow 

- **connector:** Bankofamerica Connector Integration 

- **connector:** [Powertranz] Connector Integration  

- Paypal Threeds flow Added 

- **connector:** Nexinets void flow & PSync, Capture, Refund, RSyns diff check fix 

- Setupmandate and repeat payment flow for paypal 

- **connector:** [BAMBORA] Connector Integration 

- Enable clippy for connector integration crate 

- **connector:** [Checkout] Added Setupmandate & Repeatpayment flows for Checkout 

- **connector:** [PAYME] Connector Integration 

- **connector:** [TSYS] Connector Integration 

- **connector:** Refactored Volt connector and Refund & RSync flow implementation 

- **connector:** [WORLDPAYXML] Connector Integration 

- **connector:** [Stripe] Add Banktransfer, BNPL, BankRedirect PMs for stripe 

- **connector:** [SHIFT4] Bank-Redirect 

- **connector:** Jpmorgan 

- **connector:** Revolut Connector Integration  

- **connector:** Revolut pay fix 

- Added upi_source for cc/cl 

- **core:** Add support for NetworkTokenWithNTI and NetworkMandateId in RepeatPayment 

- **connector:** [AIRWALLEX] Bank-Redirect 

- **connector:** [GLOBALPAY] Bank-Redirect 

- **connector:** Refactor Calida 

- **core:** Add connector_order_reference_id for Psync 

- **connector:** [PAYPAL] Bank-Redirect 

- **connector:** Trustpay Bank Transfer & Bank Redirect Payment Method 

- Adyen bankredirect payment method 

- **connector:** [PAYBOX] Connector Integration 

- **connector:** [LOONIO] Connector Integration  

- **connector:** Braintree RepeatPayment Flow 

- **connector:** [GIGADAT] Connector Integration 

- Repeatpayment, nti flow for adyen 

- Add granular Claude rules for connector integration 

- **framework:** Added IncrementalAuthorization Flow support 

- **core:** MandateRevoke flow 

- Added  Network-level error details in proto 

- **connector:** [Fiuu] Added RepeatPayment flow 

- **connector:** [GETNETGLOBAL] Connector Integration  

- **core:** Changed Metadata Type to SecretString 

- **wellsfargo:** Connector integration 

- **connector:** Refactored Cybersource Mandate Payments 

- **connector:** [Adyen] Implement Bank debits  

- Add bank transfer support in adyen 

- **connector:** [NovalNet] Implement Bank Debits 

- **connector:** [ADYEN] card redirect Integration  

- **connector:** Braintree Card 3DS PaymentMethod 

- **connector:** [MOLLIE] Connector Integration 

- Disable gzip decompression in test mode 

- Noon repeateverything flow implementation 

- **framework:** Added redirection_data field in PSync response and test_mode field in PSync request 

- **connector:** [Hyperpg] Integrate Card flows 

- **connector:** Phonepe upi cc/cl response handling 

- Adyen gift card 

- **connector:** Razorpay - added pay mode handling in upi sync response  

- **framework:** Added VerifyRedirectResponse flow 

- **connector:** Implement incoming webhooks for trustpay 

- **framework:** Added missing CardNetwork Types 

- **connector:** Zift Connector Integration 

- **payment_method_data:** [adyen] Auth code in payment response 

- **connector:** Gigadat Macro Implementation 

- **framework:** Introduce BodyDecoding trait 

- **connector:** Added Adyen paylater paymentmethod 

- Uniffi working implementation for JS/Java/Python 

- **framework:** Changed access_token type from String to SecretString in proto and connector_types 

- **connector:** Added ConnectorResponse for Connector Loonio 

- Add flake.lock

- **ci:** Set up GitHub release workflow with multi-platform builds 

- Enable release workflow on branches

- **connector:** [trustpay] introduce wallet support - apple pay and google pay 

- Make examples work across directories


### Miscellaneous Tasks

- Address Rust 1.88.0 clippy lints 

- Wrapper for log 

- Log sanity (Updated code) 

- Added setupmandate flow to authorizedotnet 

- Added support for raw connector response for Authorizedotnet 

- Status of SetupMandate changed from authorize to charged 

- Added webhooks support in cashtocode 

- Added amount converter 

- Added webhooks support in Novalnet 

- **core:** Removing debug logging which is set manually 

- **version:** 2025.09.17.0

- Add amount conversion wrapper and integrity checks for Xendit 

- Update git tag for hyperswitch repo 

- **version:** 2025.09.18.0

- **version:** 2025.09.19.0

- **version:** 2025.09.22.0

- Added webhooks support in Fiuu 

- **version:** 2025.09.23.0

- **version:** 2025.09.24.0

- **version:** 2025.09.25.0

- Added OnlineBankingFpx, DuitNow payment methods support 

- **version:** 2025.09.25.1

- **version:** 2025.09.26.0

- Update git tag for hyperswitch repo 

- **version:** 2025.09.29.0

- **version:** 2025.09.30.0

- **version:** 2025.10.01.0

- **version:** 2025.10.02.0

- **version:** 2025.10.08.0

- Added webhooks support in Noon 

- **version:** 2025.10.09.0

- **version:** 2025.10.10.0

- **version:** 2025.10.10.1

- **version:** 2025.10.14.0

- Added webhooks support in Cryptopay 

- **version:** 2025.10.16.0

- **version:** 2025.10.17.0

- **version:** 2025.10.23.0

- **version:** 2025.10.27.0

- **version:** 2025.10.28.0

- **version:** 2025.10.29.0

- **version:** 2025.10.30.0

- **version:** 2025.10.31.0

- **version:** 2025.11.04.0

- **version:** 2025.11.04.1

- **version:** 2025.11.05.0

- **version:** 2025.11.10.0

- **version:** 2025.11.11.0

- **version:** 2025.11.12.0

- **version:** 2025.11.13.0

- Fixed Void and Capture flow as per diff checker 

- **version:** 2025.11.14.0

- **version:** 2025.11.17.0

- **version:** 2025.11.17.1

- **version:** 2025.11.18.0

- **version:** 2025.11.19.0

- **version:** 2025.11.19.1

- Added dynamic content type selection and authorize flow for Trustpay 

- **version:** 2025.11.19.2

- **version:** 2025.11.21.0

- **version:** 2025.11.24.0

- **version:** 2025.11.25.0

- **core:** Updating tokio and hyperswitch dependency 

- **version:** 2025.11.25.1

- **version:** 2025.11.26.0

- **version:** 2025.11.27.0

- **version:** 2025.11.28.0

- **version:** 2025.12.01.0

- **version:** 2025.12.02.0

- **version:** 2025.12.03.0

- Add trigger to push image to ghcr when tag is created 

- **version:** 2025.12.03.1

- **version:** 2025.12.04.0

- **version:** 2025.12.05.0

- **version:** 2025.12.08.0

- **version:** 2025.12.09.0

- **version:** 2025.12.10.0

- **version:** 2025.12.10.1

- **version:** 2025.12.11.0

- **version:** 2025.12.11.1

- **version:** 2025.12.12.0

- **version:** 2025.12.15.0

- **version:** 2025.12.16.0

- **version:** 2025.12.17.0

- **version:** 2025.12.18.0

- **version:** 2025.12.19.0

- **version:** 2025.12.23.0

- **version:** 2025.12.24.0

- **version:** 2025.12.25.0

- **version:** 2025.12.30.0

- **version:** 2025.12.31.0

- **version:** 2026.01.01.0

- **version:** 2026.01.05.0

- **version:** 2026.01.08.0

- **version:** 2026.01.09.0

- **version:** 2026.01.12.0

- **version:** 2026.01.12.1

- **version:** 2026.01.13.0

- **version:** 2026.01.13.1

- **version:** 2026.01.13.2

- **version:** 2026.01.14.0

- **version:** 2026.01.14.1

- **version:** 2026.01.15.0

- **version:** 2026.01.19.0

- **version:** 2026.01.21.0

- Proto code owners 

- **version:** 2026.01.22.0

- **version:** 2026.01.23.0

- **version:** 2026.01.26.0

- **version:** 2026.01.27.0

- **version:** 2026.01.28.0

- Populate connector response field in error response 

- **version:** 2026.01.29.0

- **version:** 2026.01.30.0

- **version:** 2026.02.02.0

- **version:** 2026.02.03.0

- [Auth.net] Response field made optional 

- **version:** 2026.02.04.0

- **version:** 2026.02.05.0

- Updated the creds file 

- **version:** 2026.02.06.0

- **version:** 2026.02.06.1

- Added Resource ID, Service Name, and Service Type for UCS Events 

- **version:** 2026.02.10.0

- Adding failure status to customer create response 

- **version:** 2026.02.11.0

- **version:** 2026.02.11.1

- **version:** 2026.02.12.0

- **version:** 2026.02.13.0

- **version:** 2026.02.13.1

- **version:** 2026.02.16.0

- Directory organization/naming

- Added Crate for Composite Flows 

- **version:** 2026.02.18.0

- **version:** 2026.02.18.1

- **version:** 2026.02.20.0

- Disable strict conventional commits requirement

- Use right toolchain action

- Use right toolchain action

- Use right toolchain action

- **fmt:** Run formatter

- Add protoc setup and use cargo build for native Linux

- Remove obsolete ci-makefiles directory


### Performance

- Optimize release workflow with parallel SDK packaging and caching


### Refactor

- **proto:** Improve consistency and conventions in payment.proto 

- Removing hyperswitch dependency 

- Adding getter function for domain types and adding some util functions 

- Remove unnecessary qualifications in interfaces crate 

- **connector:** [RAZORPAY] populate error for success response in sync 

- Added Webhook Events 

- Added proper referer handling 

- **connector:** [PHONEPE] refactor phonepe and add UPI_QR support 

- **connector:** Update phonepe sandbox endpoint 

- **connector:** [RAZORPAY] update Razorpay connector diffs 

- Use typed connector response with masking for events 

- **connector:** [PHONEPE] refactor status mapping 

- **connector:** [PAYTM] refactor UPI flows for Paytm 

- Flattened the payment method in proto 

- Use namespace imports for connectors in types.rs 

- Made mandatory fields in authorize flow optional 

- Refactor config override functionality 

- **connector:** Add url safe base64 decoding support 

- Use proper error mapping instead of hardcoded connector_errors for Authorize 

- **connector:** [redsys] skip serializing fields that are `none` and sort fields in alphabetical order 

- Event publisher to log processed event even when publisher is disabled 

- **connector:** [PHONEPE] add Phonepe specific headers and target_app for upi request 

- Rename x86 targets to x86_64 and limit to native platforms

- Consolidate SDK build and packaging into sdk/ directory

<!-- generated by git-cliff -->
