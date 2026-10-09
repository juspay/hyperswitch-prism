# Lean spec tests

Lean 4 specs for parts of UCS. Lean proves the theorems below, generates deterministic test
vectors, and the Rust harness runs those vectors through the real UCS code:

- **PaymentService/Authorize · No3DS · card**: the Authorize request conversion.
- **Amount conversion**: `AmountConvertor` `convert` / `convert_back` (minor units ↔ connector amounts).

```
cd tests/lean && lake build && lake exe gen   # check proofs, regenerate vectors/
cargo test -p lean-spec-harness               # run the vectors against UCS
```

| Path | Contents |
|---|---|
| `PrismSpec/Authorize/No3dsCard/Types.lean` | The valid request as a dependent type |
| `PrismSpec/Authorize/No3dsCard/Raw.lean` | Proto-shaped raw request, the 16 rules, `decode`, `violations` |
| `PrismSpec/Authorize/No3dsCard/Vectors.lean` | The 44 Authorize vectors and the theorems about them |
| `PrismSpec/Amount/Conversion.lean` | Exponents, `toMajor` / `fromMajor` / `fromMinor`, and the theorems about them |
| `PrismSpec/Amount/Vectors.lean` | The 47 amount vectors and the theorems about them |
| `Gen.lean` | Writes `vectors/authorize_no3ds_card.json` and `vectors/amount_conversion.json` |
| `harness/` | Rust test crate that runs the vectors. Each test has a `KNOWN_GAPS` list of where UCS differs from the spec today |

## Theorems: Authorize · No3DS · card

| Theorem | Statement (Lean) | Plain-words explanation |
|---|---|---|
| `amount_positive` | [`∀ r : No3dsCardAuthorize, 0 < r.amount.minor`](PrismSpec/Authorize/No3dsCard/Types.lean#L130) | Every valid request has an amount greater than zero. |
| `card_valid` | [`∀ r : No3dsCardAuthorize, let n := r.card.number.digits; n.all (· < 10) ∧ (minCardLen ≤ n.length ∧ n.length ≤ maxCardLen) ∧ luhn n ∧ (1 ≤ r.card.expMonth.val ∧ r.card.expMonth.val ≤ 12) ∧ ((r.card.expYear.text.length = 2 ∨ r.card.expYear.text.length = 4) ∧ r.card.expYear.text.all Char.isDigit) ∧ r.card.cvc.digits.all (· < 10) ∧ r.card.cvc.digits.length = cvcLen (networkOf n)`](PrismSpec/Authorize/No3dsCard/Types.lean#L135) | Every valid request has a valid card: the number is 8 to 19 digits and passes the Luhn checksum, the expiry month is 1 to 12, the expiry year is 2 or 4 digits, and the CVC is digits only, 4 long for Amex (34/37) and 3 for other networks. |
| `ids_unique` | [`(vectors.map (·.id)).eraseDups.length = vectors.length`](PrismSpec/Authorize/No3dsCard/Vectors.lean#L122) | No two vectors share a name, so a failing test always points to exactly one vector. |

The first two hold for every possible request and are checked by Lean's kernel. `ids_unique` is
checked over the 44 fixed vectors with `native_decide`, which runs a compiled check.

## Theorems: amount conversion

In the statements, `c` is a currency, `m` a minor amount, `e` a currency exponent and `d` a
connector amount `⟨mant, places⟩`, meaning `mant / 10^places`.

| Theorem | Statement (Lean) | Plain-words explanation |
|---|---|---|
| `send_uses_exponent` | [`∀ c m, (toMajor c m).places = exponent c`](PrismSpec/Amount/Conversion.lean#L78) | When sending, an amount is written with exactly the currency's number of decimals (0 for JPY, 2 for USD, 3 for KWD, 4 for CLF), the same number used when reading it back. |
| `round_trip` | [`∀ c m, fromMajor c (toMajor c m) = .ok m`](PrismSpec/Amount/Conversion.lean#L82) | Sending any amount in any currency and reading it back gives the same amount. |
| `fromDec_exact` | [`∀ e d m, fromDec e d = .ok m → m * 10 ^ d.places = d.mant * 10 ^ e`](PrismSpec/Amount/Conversion.lean#L87) | Reading back never rounds or cuts off: the minor amount returned has exactly the value of the connector's amount. |
| `fromDec_complete` | [`∀ e d m, m * 10 ^ d.places = d.mant * 10 ^ e → fromDec e d = .ok m`](PrismSpec/Amount/Conversion.lean#L105) | Reading back accepts every connector amount that is a whole number of minor units, including ones with trailing zeros (`"10.990"`) or fewer decimals (`"10.9"`). |
| `fromDec_rejects_excess` | [`∀ e d, e < d.places → ¬ 10 ^ (d.places - e) ∣ d.mant → fromDec e d = .error .tooManyDecimals`](PrismSpec/Amount/Conversion.lean#L125) | A connector amount with more decimals than the currency allows, where the extra digits are not all zero (`"10.999"` USD), is an error. |
| `vectors_match_spec` | [`vectors.all (fun v => v.specMinor == v.minor) = true`](PrismSpec/Amount/Vectors.lean#L98) | Every vector's expected minor amount is what the spec computes from its connector string. |
| `send_render_parses_back` | [`sends.all (fun v => v.minor.all fun m => Dec.parse v.wire == some (toMajor v.currency m)) = true`](PrismSpec/Amount/Vectors.lean#L103) | Every connector string the spec writes parses back to exactly the amount it came from. |
| `every_currency_tested` | [`Currency.all.all (fun c => sends.any (·.currency == c) && receives.any (fun v => v.currency == c && v.minor.isSome)) = true`](PrismSpec/Amount/Vectors.lean#L108) | Every currency has send vectors and at least one accepted receive vector. |
| `every_decimal_currency_rejects_excess` | [`(Currency.all.filter (exponent · > 0)).all (fun c => receives.any fun v => v.currency == c && v.kind == .receiveMajor && v.minor.isNone) = true`](PrismSpec/Amount/Vectors.lean#L114) | Every currency with decimals has a vector with too many decimals that must be rejected. |
| `ids_unique` | [`(vectors.map (·.id)).eraseDups.length = vectors.length`](PrismSpec/Amount/Vectors.lean#L120) | No two vectors share a name. |

The first five hold for every currency and every amount and are checked by Lean's kernel. The
last five are checked over the 47 fixed vectors with `native_decide`.

`∣` in `fromDec_rejects_excess` is "divides". The statements are copied from the `.lean` files;
the `∀` binders are written out here, while the source declares them as theorem arguments. In
`card_valid` the README shortens `r.card.number.digits` to `n` and drops `= true` after each Bool
check.

Each statement links to the theorem in its `.lean` file. The links use line numbers, so update
them when lines are added or removed above a theorem.
