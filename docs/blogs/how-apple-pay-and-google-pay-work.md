# What actually happens when someone taps "Pay" on Apple Pay or Google Pay

Most people describe these wallets as "your card, but faster." That's the user-facing story. Underneath, something more interesting is happening: your real card number is never in the transaction at all.

Here's the mechanism, end to end.

## The core idea: the card number is replaced, not hidden

When you add a card to either wallet, the wallet doesn't store your card. It asks the card network's Token Service Provider (Visa's VTS, Mastercard's MDES, and the equivalents) to issue a **network token** — a different 16-digit number that routes to the same account but is bound to a specific device and, in some schemes, a specific merchant.

That token is called a DPAN (Device Primary Account Number). Your real number, the FPAN, stays with your issuer.

This is why a leaked wallet transaction is close to worthless. The token only works alongside a one-time cryptogram, and it only works from the context it was provisioned for.

## Apple Pay, step by step

**1. Provisioning.** You add a card. Apple passes the details to the issuer, the issuer verifies you (sometimes with an OTP step, "yellow path"), and the network issues a DPAN. The DPAN lands in the **Secure Element** — a separate tamper-resistant chip on the device. Apple's servers never hold your FPAN, and neither does the app you're paying in.

**2. Authentication.** At payment time, Face ID, Touch ID, or the device passcode unlocks the Secure Element. No biometric match, no cryptogram. This is a hardware gate, not an app-level check.

**3. Cryptogram generation.** The Secure Element generates a one-time cryptogram over the transaction: the token, an unpredictable number, the amount. It is valid for that transaction only. Replaying it fails.

**4. Encryption to the merchant.** The merchant receives a `PKPaymentToken`. Its `paymentData` field is a base64 blob — ECDH-derived AES-256-GCM ciphertext, encrypted to the public key of the merchant's **Apple Pay payment processing certificate**. The merchant (or their payment provider, if the certificate is theirs) decrypts it to get:

- `applicationPrimaryAccountNumber` — the DPAN, not your card
- `applicationExpirationDate` — the token's expiry, not your card's
- `paymentData.onlinePaymentCryptogram` — the one-time cryptogram
- `paymentData.eciIndicator` — how strongly the payer was authenticated

**5. Authorization.** Those fields go into a normal card authorization message. The cryptogram travels in the field the network reserves for authentication data — the same field 3-D Secure uses. The network validates the cryptogram, swaps the DPAN back to the FPAN, and hands the issuer a request that looks like an ordinary card payment with strong authentication attached.

On the web there's one extra step: **merchant validation**. Before the sheet opens, your server calls Apple with a merchant identity certificate to prove the domain is registered. This is why Apple Pay on the web requires domain verification and Apple Pay in an app does not.

## Google Pay, step by step

Structurally similar, with one consequential difference: **Google Pay has two token types, and only one of them is a network token.**

**`CRYPTOGRAM_3DS`** — a device-provisioned network token plus a one-time cryptogram. This is the Apple Pay equivalent. Google uses Host Card Emulation rather than a dedicated secure chip, so the credential is cloud-backed with short-lived keys on the device instead of being sealed in hardware.

**`PAN_ONLY`** — a card saved to your Google account, not provisioned to the device. What comes back is the **real card number**, encrypted, with no cryptogram at all. It is a card-on-file payment wearing a wallet's clothes.

You can tell them apart from `assuranceDetails` in the response:

| | `cardHolderAuthenticated` | Cryptogram / ECI |
|---|---|---|
| `CRYPTOGRAM_3DS` | `true` | present |
| `PAN_ONLY` | `false` | absent |

This distinction matters more than it looks. A `PAN_ONLY` payment carries no authentication evidence, so it gets no liability shift, is likelier to be declined, and in regulated markets may need separate authentication to clear. Plenty of "Google Pay is failing but cards work" incidents are really "this processor requires a cryptogram and we allowed `PAN_ONLY`."

Google also splits **who decrypts**:

- `PAYMENT_GATEWAY` — encrypted to your payment provider's key; they decrypt it.
- `DIRECT` — encrypted to your own key; you decrypt it, and you inherit the PCI scope that comes with holding a real PAN.

The payload itself is a signed envelope: `protocolVersion` (`ECv2`), an `intermediateSigningKey`, an ECDSA `signature`, and a `signedMessage` containing the ECIES-encrypted data. You verify the signature chain against Google's published root keys **before** decrypting. Skipping that check is the classic integration bug — it means accepting a payload anyone could have forged.

## Where the two genuinely differ

| | Apple Pay | Google Pay |
|---|---|---|
| Credential storage | Secure Element (hardware) | Cloud-backed, HCE on device |
| User authentication | Always — biometric or passcode | Not always; depends on token type, amount, device |
| Token type | Always DPAN + cryptogram | `CRYPTOGRAM_3DS` or `PAN_ONLY` |
| Encryption target | Merchant's payment processing certificate | Gateway key, or merchant key in `DIRECT` |
| Web prerequisite | Domain verification + merchant session | Origin allowlisting, no session handshake |

## What this means if you're integrating

**A wallet is not a payment method to the acquirer — it's a card with extra evidence attached.** The auth message is a card auth. Which is why wallet payments can fail in ways that look bizarre: the acquirer needs a contract for the wallet variant, or the processor expects the downgraded card-plus-cryptogram shape rather than the wallet payload, and the decline arrives with an error about an account or a missing field rather than about the wallet.

**The cryptogram is the whole point.** Everything valuable — replay resistance, liability shift, the higher approval rates wallets are known for — comes from the cryptogram and its ECI. If your integration path drops it (`PAN_ONLY`, or a decryption step that forwards only the PAN), you have kept the wallet's UX and thrown away its economics.

**Decryption is a choice with consequences.** Letting your provider decrypt keeps the PAN out of your systems. Decrypting yourself buys flexibility and buys you PCI scope. Neither is wrong; picking one by accident usually is.

The wallets are, in the end, a well-engineered indirection: hardware-gated authentication on one side, a number that means nothing outside its context in the middle, and an ordinary card authorization coming out the other end.
