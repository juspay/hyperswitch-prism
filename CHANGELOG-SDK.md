# SDK Changelog

All notable changes to the **Hyperswitch Prism SDKs** (`hyperswitch-prism` on npm, PyPI, Maven Central) are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

> **Note:** This file tracks **SemVer SDK releases** (`X.Y.Z`).
> Nightly CalVer releases (`YYYY.MM.DD.N`) are tracked separately in [`CHANGELOG.md`](./CHANGELOG.md).

---

## [Unreleased]

## [0.4.0] — 2026-04-08

**Full diff:** [`0.3.0...0.4.0`](https://github.com/juspay/hyperswitch-prism/compare/0.3.0...0.4.0)

### Features

- feat(connector): implement CreateClientAuthenticationToken for Globalpay (#957) ([`dd456e9ae`](https://github.com/juspay/hyperswitch-prism/commit/dd456e9ae))
- feat: add proxy cache logic to all httpclient of sdk, previously each… (#859) ([`090d520ef`](https://github.com/juspay/hyperswitch-prism/commit/090d520ef))
- feat(tests): comprehensive connector test harness with 57 connectors, 22 suites, and credential masking (#771) ([`74b8f0de7`](https://github.com/juspay/hyperswitch-prism/commit/74b8f0de7))
- feat: rename connectortransformationerror to connectorerror and connector 4xx and 5xx as connectorerror exception in sdk (#928) ([`93cab3244`](https://github.com/juspay/hyperswitch-prism/commit/93cab3244))
- feat(connector): [Trustly] Implement Trustly flows (#752) ([`981a52bd3`](https://github.com/juspay/hyperswitch-prism/commit/981a52bd3))
- feat: stripe pm token (#776) ([`447a3b713`](https://github.com/juspay/hyperswitch-prism/commit/447a3b713))
- feat(connector): implement BankDebit for multisafepay (#869) ([`6d984ec9c`](https://github.com/juspay/hyperswitch-prism/commit/6d984ec9c))
- feat(connector): [Bluesnap] implement BankDebit (ACH + SEPA) with PSync alt-transactions routing (#875) ([`2fb4f3776`](https://github.com/juspay/hyperswitch-prism/commit/2fb4f3776))
- feat(connector): implement MIT and CreateCustomer for shift4 (#882) ([`280978406`](https://github.com/juspay/hyperswitch-prism/commit/280978406))
- feat(connector): implement BankDebit for dlocal (#889) ([`ff25fc75a`](https://github.com/juspay/hyperswitch-prism/commit/ff25fc75a))
- feat(connector): implement BankDebit for payload (#873) ([`2f1e86a92`](https://github.com/juspay/hyperswitch-prism/commit/2f1e86a92))
- feat(connector): implement MIT for Billwerk (#871) ([`878de3f8e`](https://github.com/juspay/hyperswitch-prism/commit/878de3f8e))
- feat(connector): implement MIT for dlocal (#878) ([`63d1d55cd`](https://github.com/juspay/hyperswitch-prism/commit/63d1d55cd))
- feat: split connectorerror to connectorrequesterror and connectorresponseerror (#765) ([`d201c6704`](https://github.com/juspay/hyperswitch-prism/commit/d201c6704))
- feat(connector): add config specific to Itaubank (#864) ([`7ccb6df05`](https://github.com/juspay/hyperswitch-prism/commit/7ccb6df05))
- feat(connector): implement GooglePay for nmi (#876) ([`ed8c2cd96`](https://github.com/juspay/hyperswitch-prism/commit/ed8c2cd96))
- feat(connector): implement googlepay for finix (#866) ([`927623d3f`](https://github.com/juspay/hyperswitch-prism/commit/927623d3f))
- feat(connector): [NMI] add 3DS support for Card payments (#760) ([`266d7cc3f`](https://github.com/juspay/hyperswitch-prism/commit/266d7cc3f))
- feat(pr-reviewer): add scenario-aware PR review system with multi-tool skill wiring (#792) ([`09afd88f0`](https://github.com/juspay/hyperswitch-prism/commit/09afd88f0))
- feat(client): add support for non-pci payment client (#774) ([`f044c7cf9`](https://github.com/juspay/hyperswitch-prism/commit/f044c7cf9))
- feat(connectors): [revolv3] add external 3ds support  (#815) ([`253ead5b7`](https://github.com/juspay/hyperswitch-prism/commit/253ead5b7))
- feat(connector): [Itaubank] add payout flows (#826) ([`962763b3d`](https://github.com/juspay/hyperswitch-prism/commit/962763b3d))

### Bug Fixes

- fix: remove rust release pipeline for package (#971) ([`850324d9b`](https://github.com/juspay/hyperswitch-prism/commit/850324d9b))
- fix: unify errorcodes, errrohandling in same doc (#933) ([`5d737a515`](https://github.com/juspay/hyperswitch-prism/commit/5d737a515))
- fix: authorize error inconsistency and remove ApplicationErrorResponse (#892) ([`bdb8635d4`](https://github.com/juspay/hyperswitch-prism/commit/bdb8635d4))
- fix: avoid panic when kafka is not available during start up with event enabled config set (#887) ([`41a7c5a29`](https://github.com/juspay/hyperswitch-prism/commit/41a7c5a29))
- fix: restore merchant_secret fallback and webhook_uri in webhook (#822) ([`e849b7f36`](https://github.com/juspay/hyperswitch-prism/commit/e849b7f36))

### Refactors

- refactor(docs): auto update docs in CI check itself (#942) ([`b492c3ff5`](https://github.com/juspay/hyperswitch-prism/commit/b492c3ff5))
- refactor(docs): remove services.desc and manifest.json from docs generation (#940) ([`33dc492a4`](https://github.com/juspay/hyperswitch-prism/commit/33dc492a4))
- refactor(client): refactor rust connector client (#939) ([`6edb68c14`](https://github.com/juspay/hyperswitch-prism/commit/6edb68c14))
- refactor(docs): update all_connector.md for newly added flows (#929) ([`64bdb0922`](https://github.com/juspay/hyperswitch-prism/commit/64bdb0922))
- refactor(connector): added PproConfig (#893) ([`0043d778f`](https://github.com/juspay/hyperswitch-prism/commit/0043d778f))
- refactor(auth): rename authentication token abstractions + implement Stripe ClientAuthentication (#855) ([`c9e1025e3`](https://github.com/juspay/hyperswitch-prism/commit/c9e1025e3))
- refactor(connector): added pms to ForeignTryFrom<grpc_api_types::payments::PaymentMethod> (#860) ([`fe221f4b2`](https://github.com/juspay/hyperswitch-prism/commit/fe221f4b2))
- refactor(connector): use RedirectForm::Uri instead of Form for redire… (#811) ([`02842f752`](https://github.com/juspay/hyperswitch-prism/commit/02842f752))
- refactor(connector): [Fiservcommercehub]  fix card_expiry_year and access_token expiry  (#828) ([`06e7de9fa`](https://github.com/juspay/hyperswitch-prism/commit/06e7de9fa))

### Documentation

- docs: clarify first-payment non-PCI client auth flow (#938) ([`870337ebb`](https://github.com/juspay/hyperswitch-prism/commit/870337ebb))
- docs: error-handling after proto change (#821) ([`d81557f6f`](https://github.com/juspay/hyperswitch-prism/commit/d81557f6f))
- docs(readme): add routing demo GIF with tagline ([`b07dc3a23`](https://github.com/juspay/hyperswitch-prism/commit/b07dc3a23))

### Chores

- chore(release): prepare for 0.4.0 ([`bd6fa5254`](https://github.com/juspay/hyperswitch-prism/commit/bd6fa5254))
- chore(version): 2026.04.08.0 ([`3173e84bb`](https://github.com/juspay/hyperswitch-prism/commit/3173e84bb))
- chore: add publish command ci (#975) ([`1c0d1777d`](https://github.com/juspay/hyperswitch-prism/commit/1c0d1777d))
- chore(version): 2026.04.07.1 ([`c7075d896`](https://github.com/juspay/hyperswitch-prism/commit/c7075d896))
- chore: add publish command ci (#974) ([`c2af0f42a`](https://github.com/juspay/hyperswitch-prism/commit/c2af0f42a))
- chore(version): 2026.04.07.0 ([`fd20f13ba`](https://github.com/juspay/hyperswitch-prism/commit/fd20f13ba))
- chore: resolve manual docs (#935) ([`9840d0f81`](https://github.com/juspay/hyperswitch-prism/commit/9840d0f81))
- chore: add publish command (#945) ([`8a77eaae9`](https://github.com/juspay/hyperswitch-prism/commit/8a77eaae9))
- chore(version): 2026.04.06.1 ([`dc65fa848`](https://github.com/juspay/hyperswitch-prism/commit/dc65fa848))
- chore(version): 2026.04.06.0 ([`88f74334e`](https://github.com/juspay/hyperswitch-prism/commit/88f74334e))
- chore(version): 2026.04.03.0 ([`e479d36ac`](https://github.com/juspay/hyperswitch-prism/commit/e479d36ac))
- chore(version): 2026.04.02.0 ([`74c33bae7`](https://github.com/juspay/hyperswitch-prism/commit/74c33bae7))
- chore(version): 2026.04.01.0 ([`c89d3b03a`](https://github.com/juspay/hyperswitch-prism/commit/c89d3b03a))
- chore: Feature-gate superposition deps to reduce transitive dependencies (#857) ([`0de0cb9ae`](https://github.com/juspay/hyperswitch-prism/commit/0de0cb9ae))
- chore(version): 2026.03.31.0 ([`b7fb9e9dd`](https://github.com/juspay/hyperswitch-prism/commit/b7fb9e9dd))
- chore(version): 2026.03.30.0 ([`1f5f36a47`](https://github.com/juspay/hyperswitch-prism/commit/1f5f36a47))

---

## [0.3.0] — 2026-03-27

**Full diff:** [`0.2.0...0.3.0`](https://github.com/juspay/hyperswitch-prism/compare/0.2.0...0.3.0)

### Chores

- chore(release): prepare for 0.3.0 ([`7fdae6c59`](https://github.com/juspay/hyperswitch-prism/commit/7fdae6c59))
- chore(version): 2026.03.27.0 ([`b55050c60`](https://github.com/juspay/hyperswitch-prism/commit/b55050c60))

---

## [0.2.0] — 2026-03-26

**Full diff:** [`0.1.0...0.2.0`](https://github.com/juspay/hyperswitch-prism/compare/0.1.0...0.2.0)

### Features

- feat(core): Implement NTID flow for Decrypted Wallet Token and also Implement for checkout connector (#793) ([`5cd633a7d`](https://github.com/juspay/hyperswitch-prism/commit/5cd633a7d))
- feat(payouts): add payout flows (#717) ([`965091eb8`](https://github.com/juspay/hyperswitch-prism/commit/965091eb8))
- feat(framework): Added ConnectorSpecificConfig for Fiservcommercehub  (#789) ([`7d9c7e918`](https://github.com/juspay/hyperswitch-prism/commit/7d9c7e918))
- feat(client): add grpc client support in js, rs, kt and py (#764) ([`7982ec31e`](https://github.com/juspay/hyperswitch-prism/commit/7982ec31e))
- feat: [GRACE] Add skills to Hyperswitch-Prism (#781) ([`0b61a93ee`](https://github.com/juspay/hyperswitch-prism/commit/0b61a93ee))
- feat(framework): Superposition toml parsing implementation (#591) ([`65a07aedd`](https://github.com/juspay/hyperswitch-prism/commit/65a07aedd))
- feat(payout): create payout flow (#659) ([`6f1c7f51f`](https://github.com/juspay/hyperswitch-prism/commit/6f1c7f51f))
- feat(connector): add fiservcommercehub cards (#725) ([`c1e03e912`](https://github.com/juspay/hyperswitch-prism/commit/c1e03e912))
- feat(proto): add payouts proto contract (#616) ([`dcf3aafa0`](https://github.com/juspay/hyperswitch-prism/commit/dcf3aafa0))
- Feat(connector): adyen  network token (#631) ([`b7063424b`](https://github.com/juspay/hyperswitch-prism/commit/b7063424b))
- feat(framework): Add GRACE AI to connector-service (#718) ([`d2a922c02`](https://github.com/juspay/hyperswitch-prism/commit/d2a922c02))
- feat: error proto refactor (#669) ([`1790d5575`](https://github.com/juspay/hyperswitch-prism/commit/1790d5575))
- feat(connector): PPRO connector integration (#568) ([`70e4f6e70`](https://github.com/juspay/hyperswitch-prism/commit/70e4f6e70))
- feat(framework): Use hyperswitch_masking from crates.io instead of git dependency (#660) ([`4782efa70`](https://github.com/juspay/hyperswitch-prism/commit/4782efa70))
- feat(connector): [peachpayments] add no 3ds cards, network token payment methods (#607) ([`b9de70280`](https://github.com/juspay/hyperswitch-prism/commit/b9de70280))
- feat(framework): Add merchant_transaction_id in PaymentServiceGetRequest & PaymentServiceGetResponse (#654) ([`c43491bae`](https://github.com/juspay/hyperswitch-prism/commit/c43491bae))
- feat: http client sanity runner (#621) ([`6565db736`](https://github.com/juspay/hyperswitch-prism/commit/6565db736))
- feat(domain): unify ConnectorSpecificAuth → ConnectorSpecificConfig (#627) ([`a7a696c3a`](https://github.com/juspay/hyperswitch-prism/commit/a7a696c3a))
- feat(docs): add automated connector docs (#625) ([`00b980477`](https://github.com/juspay/hyperswitch-prism/commit/00b980477))
- feat(connector): [Truelayer] Implement webhooks for payments and refunds (#602) ([`ffe6888f6`](https://github.com/juspay/hyperswitch-prism/commit/ffe6888f6))
- feat(framework): Added all available services in app.rs (#618) ([`b05c37205`](https://github.com/juspay/hyperswitch-prism/commit/b05c37205))
- feat(payment_methods): add support for Samsung Pay (#558) ([`e35afccda`](https://github.com/juspay/hyperswitch-prism/commit/e35afccda))
- feat(connector): [Checkout] Add l2_l3 data support in checkout (#565) ([`ed05bceb3`](https://github.com/juspay/hyperswitch-prism/commit/ed05bceb3))
- feat: [FINIX] CARDS NO3DS , ACH BankDebit (#564) ([`6181e601d`](https://github.com/juspay/hyperswitch-prism/commit/6181e601d))
- feat: [AUTHORIZEDOTNET] ACH BankDebit (#549) ([`54df6fa30`](https://github.com/juspay/hyperswitch-prism/commit/54df6fa30))
- feat(payment_methods): Add ACH (eCheck) support to Forte connector (#576) ([`d696886a6`](https://github.com/juspay/hyperswitch-prism/commit/d696886a6))
- feat(proto): Change the proto package name (#603) ([`19101f3bf`](https://github.com/juspay/hyperswitch-prism/commit/19101f3bf))
- feat: proto changes for sdk configs overridable vs non overridable (#589) ([`017e0e360`](https://github.com/juspay/hyperswitch-prism/commit/017e0e360))
- feat: [PAYSAFE] ACH BankDebit (#556) ([`f9300f3d0`](https://github.com/juspay/hyperswitch-prism/commit/f9300f3d0))
- feat: FFI implementation (#515) ([`00e5edf03`](https://github.com/juspay/hyperswitch-prism/commit/00e5edf03))
- feat: helper for multi form data as bytes to support with ffi lang ag… (#566) ([`d44785e87`](https://github.com/juspay/hyperswitch-prism/commit/d44785e87))
- feat: ach bankdebit integration for jpmorgan (#553) ([`61aec3bf5`](https://github.com/juspay/hyperswitch-prism/commit/61aec3bf5))
- feat: ach bankdebit integration for checkout (#547) ([`4ba9117fc`](https://github.com/juspay/hyperswitch-prism/commit/4ba9117fc))
- feat: ach bankdebit integration for nuvei (#551) ([`28cd25065`](https://github.com/juspay/hyperswitch-prism/commit/28cd25065))
- feat: ach bankdebit integration for bluesnap (#552) ([`d1ed349a8`](https://github.com/juspay/hyperswitch-prism/commit/d1ed349a8))
- feat(connector): [Truelayer] Integrate OpenBankingUK flows (#519) ([`40eb64581`](https://github.com/juspay/hyperswitch-prism/commit/40eb64581))
- feat(connector): [Adyen] Add Apple pay and Google pay Decrypt for Adyen (#509) ([`ada06037a`](https://github.com/juspay/hyperswitch-prism/commit/ada06037a))
- feat(connector): [Revolut] Rename RevolutAuth "api_key" field and add "signing_secret" for webhook source verification (#570) ([`6bc06bdc3`](https://github.com/juspay/hyperswitch-prism/commit/6bc06bdc3))
- feat(connector): [revolv3] add recurring support for non-3ds card payments (#554) ([`6a85fd47e`](https://github.com/juspay/hyperswitch-prism/commit/6a85fd47e))
- feat: typed ConnectorSpecificAuth with header-based auth resolution via X-Connector-Auth (#555) ([`23fb46a5c`](https://github.com/juspay/hyperswitch-prism/commit/23fb46a5c))
- feat: [NOVALNET] ACH BankDebit (#563) ([`c4f52ae0a`](https://github.com/juspay/hyperswitch-prism/commit/c4f52ae0a))
- feat: Webhook support for paypal (#440) ([`7e9496f2b`](https://github.com/juspay/hyperswitch-prism/commit/7e9496f2b))
- feat: [STAX] ACH BankDebit  (#548) ([`50bf11c04`](https://github.com/juspay/hyperswitch-prism/commit/50bf11c04))
- feat(connector): [revolv3] add no-threeds card payments (#520) ([`4cf7158a7`](https://github.com/juspay/hyperswitch-prism/commit/4cf7158a7))
- feat(core): Added Missing BankTransfer, BankDebit & BankRedirect Payment Method Types (#538) ([`84493fefd`](https://github.com/juspay/hyperswitch-prism/commit/84493fefd))
- feat: ach bankdebit integration for nmi (#545) ([`e07b1c3b7`](https://github.com/juspay/hyperswitch-prism/commit/e07b1c3b7))
- feat(connector): [Checkout] Implement googlepay and applepay decrypt flow and card ntid flow (#546) ([`576dfbe4c`](https://github.com/juspay/hyperswitch-prism/commit/576dfbe4c))
- Feat(connector): adyen voucher paymentmethod added (#500) ([`948bd45c0`](https://github.com/juspay/hyperswitch-prism/commit/948bd45c0))

### Bug Fixes

- fix: webhook api response trait and adyen webhook source verification (#541) ([`a15c291be`](https://github.com/juspay/hyperswitch-prism/commit/a15c291be))
- fix(connector): [REVOLUT] amount and id fixes for revolut euler-ucs (#778) ([`6181a56f8`](https://github.com/juspay/hyperswitch-prism/commit/6181a56f8))
- fix: migrate `connector_feature_data` mca configs to `ConnectorSpecificConfig` (#723) ([`a05e5e178`](https://github.com/juspay/hyperswitch-prism/commit/a05e5e178))
- fix(proto): resolve proto consistency issues from review (#720) ([`bb44fb1ab`](https://github.com/juspay/hyperswitch-prism/commit/bb44fb1ab))
- fix(workflow): simplify docs sync change detection logic ([`a2b45fdaa`](https://github.com/juspay/hyperswitch-prism/commit/a2b45fdaa))
- fix(clippy): fix clippy error (#596) ([`eb197f7e1`](https://github.com/juspay/hyperswitch-prism/commit/eb197f7e1))
- fix: redirect response removed (#514) ([`b275d0506`](https://github.com/juspay/hyperswitch-prism/commit/b275d0506))

### Refactors

- refactor(peachpayments): extract webhook body parsing to helper function and remove unused struct (#685) ([`4bf37031d`](https://github.com/juspay/hyperswitch-prism/commit/4bf37031d))
- refactor(docs): add support for connector wise request example in multiple languages (#637) ([`2d66cc889`](https://github.com/juspay/hyperswitch-prism/commit/2d66cc889))
- refactor(codegen): organize templates into per-language subdirectories (#652) ([`ec40f8bba`](https://github.com/juspay/hyperswitch-prism/commit/ec40f8bba))
- refactor(generate.py): Refactor code generation to use Jinja2 templates (#645) ([`30cef9baa`](https://github.com/juspay/hyperswitch-prism/commit/30cef9baa))
- refactor(proto): refactor id_type to string (#604) ([`3c66b11ce`](https://github.com/juspay/hyperswitch-prism/commit/3c66b11ce))
- refactor: simplify header handling by making them optional and inferring connector from `x-connector-auth` header (#590) ([`6fd2e37f5`](https://github.com/juspay/hyperswitch-prism/commit/6fd2e37f5))
- refactor(client): per-service SDK clients from services.proto boundaries (#595) ([`911373a89`](https://github.com/juspay/hyperswitch-prism/commit/911373a89))

### Documentation

- docs: Add comprehensive SDK reference documentation for all 4 languages (#730) ([`3d53997b4`](https://github.com/juspay/hyperswitch-prism/commit/3d53997b4))
- docs: Restructure and enhance documentation (#783) ([`cc4b137cf`](https://github.com/juspay/hyperswitch-prism/commit/cc4b137cf))
- docs(README.md): reference org repo and minor edits (#787) ([`806454b58`](https://github.com/juspay/hyperswitch-prism/commit/806454b58))
- docs: Add hyperlinks to intelligent routing and smart retries docs ([`c33a4aab1`](https://github.com/juspay/hyperswitch-prism/commit/c33a4aab1))
- docs: Add routing rule examples for Stripe/Adyen currency-based routing ([`ea8307e71`](https://github.com/juspay/hyperswitch-prism/commit/ea8307e71))
- docs: Use sample values in Node.js createOrder example ([`e29c8d2c8`](https://github.com/juspay/hyperswitch-prism/commit/e29c8d2c8))
- docs: Change Node.js example to use createOrder API ([`489a4400d`](https://github.com/juspay/hyperswitch-prism/commit/489a4400d))
- docs: Update README examples to show currency-based routing ([`fd190a123`](https://github.com/juspay/hyperswitch-prism/commit/fd190a123))
- docs: Update README code snippets to match SDK patterns ([`799a2556e`](https://github.com/juspay/hyperswitch-prism/commit/799a2556e))
- docs: Restructure and improve documentation (#728) ([`f18221adf`](https://github.com/juspay/hyperswitch-prism/commit/f18221adf))
- docs: Restructure documentation with /docs and /docs-generated separation (#684) ([`d48fa4628`](https://github.com/juspay/hyperswitch-prism/commit/d48fa4628))
- docs: Update API reference documentation and navigation (#633) ([`f57de079e`](https://github.com/juspay/hyperswitch-prism/commit/f57de079e))
- docs: Update SDK javascript README (#640) ([`36ef23e33`](https://github.com/juspay/hyperswitch-prism/commit/36ef23e33))
- docs: launch blog for review (#642) ([`29e2fb9c3`](https://github.com/juspay/hyperswitch-prism/commit/29e2fb9c3))
- docs: Add Unified Payment Protocol (UPP) specification RFC (#646) ([`9f140bd54`](https://github.com/juspay/hyperswitch-prism/commit/9f140bd54))

### CI

- ci(docs): Update workflow to use CONNECTOR_SERVICE_DOCS_CI secret (#623) ([`6b530f84e`](https://github.com/juspay/hyperswitch-prism/commit/6b530f84e))
- ci: make release build manual and add sdk test to PR check (#592) ([`b41f3e7b2`](https://github.com/juspay/hyperswitch-prism/commit/b41f3e7b2))
- ci(docs): add workflow to sync docs to hyperswitch-docs ([`3f0c79c5e`](https://github.com/juspay/hyperswitch-prism/commit/3f0c79c5e))
- ci: fix java tests to match latest generation spec (#606) ([`9a87d836c`](https://github.com/juspay/hyperswitch-prism/commit/9a87d836c))

### Chores

- chore(release): prepare for 0.2.0 ([`253ff4a74`](https://github.com/juspay/hyperswitch-prism/commit/253ff4a74))
- chore: change package version (#818) ([`242634870`](https://github.com/juspay/hyperswitch-prism/commit/242634870))
- chore(rename): rename hs playlib (#794) ([`fab775303`](https://github.com/juspay/hyperswitch-prism/commit/fab775303))
- chore(version): 2026.03.26.0 ([`9d7ffe6b0`](https://github.com/juspay/hyperswitch-prism/commit/9d7ffe6b0))
- chore(version): 2026.03.25.0 ([`91457a956`](https://github.com/juspay/hyperswitch-prism/commit/91457a956))
- chore(version): 2026.03.24.0 ([`959ddc895`](https://github.com/juspay/hyperswitch-prism/commit/959ddc895))
- chore: folder restructure (#756) ([`d3a555d57`](https://github.com/juspay/hyperswitch-prism/commit/d3a555d57))
- chore(version): 2026.03.23.0 ([`0c63af12f`](https://github.com/juspay/hyperswitch-prism/commit/0c63af12f))
- chore(sdk-error): update sdk error to new proto (#719) ([`4984edcbd`](https://github.com/juspay/hyperswitch-prism/commit/4984edcbd))
- chore(version): 2026.03.19.0 ([`a77fe0808`](https://github.com/juspay/hyperswitch-prism/commit/a77fe0808))
- chore(version): 2026.03.18.0 ([`c6dac68ac`](https://github.com/juspay/hyperswitch-prism/commit/c6dac68ac))
- chore: Implemented Void and Capture Flows For Composite Service (#624) ([`c0c71fc7f`](https://github.com/juspay/hyperswitch-prism/commit/c0c71fc7f))
- chore: Implemented Refund And RefundGet Flow For Composite Service (#608) ([`d0aa0a55e`](https://github.com/juspay/hyperswitch-prism/commit/d0aa0a55e))
- chore(version): 2026.03.17.0 ([`b354e0e21`](https://github.com/juspay/hyperswitch-prism/commit/b354e0e21))
- chore(error): ffi error handling  (#661) ([`985c55fad`](https://github.com/juspay/hyperswitch-prism/commit/985c55fad))
- chore(uniffii): revert error handling (#656) ([`9774c5b81`](https://github.com/juspay/hyperswitch-prism/commit/9774c5b81))
- chore(version): 2026.03.16.0 ([`a656942c8`](https://github.com/juspay/hyperswitch-prism/commit/a656942c8))
- chore: Scenario Based Test framework for UCS (#580) ([`01c4768db`](https://github.com/juspay/hyperswitch-prism/commit/01c4768db))
- chore(version): 2026.03.13.0 ([`f49d63f93`](https://github.com/juspay/hyperswitch-prism/commit/f49d63f93))
- chore(error): add request and response error proto for ffi implementation (#610) ([`ac90f773f`](https://github.com/juspay/hyperswitch-prism/commit/ac90f773f))
- chore(remove): remove unused code (#628) ([`ee6e1b9fa`](https://github.com/juspay/hyperswitch-prism/commit/ee6e1b9fa))
- chore(version): 2026.03.12.0 ([`ebe7cd405`](https://github.com/juspay/hyperswitch-prism/commit/ebe7cd405))
- chore(version): 2026.03.11.0 ([`dab23b442`](https://github.com/juspay/hyperswitch-prism/commit/dab23b442))
- chore(version): 2026.03.10.0 ([`2449ae68f`](https://github.com/juspay/hyperswitch-prism/commit/2449ae68f))
- chore: Added Composite Get Flow (#575) ([`1179bb787`](https://github.com/juspay/hyperswitch-prism/commit/1179bb787))
- chore(version): 2026.03.09.0 ([`d8c63d0b1`](https://github.com/juspay/hyperswitch-prism/commit/d8c63d0b1))
- chore(version): 2026.03.06.0 ([`11136248a`](https://github.com/juspay/hyperswitch-prism/commit/11136248a))
- chore(version): 2026.03.04.0 ([`01b01a716`](https://github.com/juspay/hyperswitch-prism/commit/01b01a716))
- chore(version): 2026.03.03.0 ([`95144b082`](https://github.com/juspay/hyperswitch-prism/commit/95144b082))
- chore(version): 2026.03.02.0 ([`ed6ac6de9`](https://github.com/juspay/hyperswitch-prism/commit/ed6ac6de9))
- chore(connector): add warning comment about dtd validation to redsys soap api (#560) ([`a941b531e`](https://github.com/juspay/hyperswitch-prism/commit/a941b531e))
- chore(version): 2026.02.26.0 ([`9bd74a4d1`](https://github.com/juspay/hyperswitch-prism/commit/9bd74a4d1))
- chore: Added Composite Authorize Flow  (#517) ([`fedc4ad61`](https://github.com/juspay/hyperswitch-prism/commit/fedc4ad61))
- chore(version): 2026.02.25.0 ([`49e9aa7be`](https://github.com/juspay/hyperswitch-prism/commit/49e9aa7be))
- chore: Refactored the wallet Payment Method (#526) ([`bb898deef`](https://github.com/juspay/hyperswitch-prism/commit/bb898deef))
- chore(version): 2026.02.24.0 ([`ade5389d4`](https://github.com/juspay/hyperswitch-prism/commit/ade5389d4))
- chore(version): 2026.02.23.0 ([`55db1cc2e`](https://github.com/juspay/hyperswitch-prism/commit/55db1cc2e))

### Other

- Merge main into docs-review2203 - resolve conflicts ([`258f1fdb5`](https://github.com/juspay/hyperswitch-prism/commit/258f1fdb5))
- Merge branch 'main' of https://github.com/juspay/connector-service ([`c919a204c`](https://github.com/juspay/hyperswitch-prism/commit/c919a204c))
- Merge branch 'main' of https://github.com/juspay/connector-service ([`1e2d005e8`](https://github.com/juspay/hyperswitch-prism/commit/1e2d005e8))

---

## [0.1.0] — 2026-02-21

Initial public release of the Hyperswitch Prism (Unified Connector Service) SDKs.

For the full list of changes included in this initial release, see the [GitHub Release page](https://github.com/juspay/hyperswitch-prism/releases/tag/0.1.0).

[unreleased]: https://github.com/juspay/hyperswitch-prism/compare/0.4.0...HEAD
[0.4.0]: https://github.com/juspay/hyperswitch-prism/releases/tag/0.4.0
[0.3.0]: https://github.com/juspay/hyperswitch-prism/releases/tag/0.3.0
[0.2.0]: https://github.com/juspay/hyperswitch-prism/releases/tag/0.2.0
[0.1.0]: https://github.com/juspay/hyperswitch-prism/releases/tag/0.1.0
