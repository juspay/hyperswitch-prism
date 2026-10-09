# Lean spec tests

A Lean 4 spec of the **PaymentService/Authorize · No3DS · card** request. Lean proves the
theorems below, generates deterministic test vectors, and the Rust harness runs those vectors
through the UCS Authorize request conversion.

```
cd tests/lean && lake build && lake exe gen   # check proofs, regenerate vectors/
cargo test -p lean-spec-harness               # run the vectors against UCS
```

| Path | Contents |
|---|---|
| `PrismSpec/Authorize/No3dsCard/Types.lean` | The valid request as a dependent type |
| `PrismSpec/Authorize/No3dsCard/Raw.lean` | Proto-shaped raw request, the 16 rules, `decode`, `violations` |
| `PrismSpec/Authorize/No3dsCard/Vectors.lean` | The 44 test vectors and the theorems about them |
| `Gen.lean` | Writes `vectors/authorize_no3ds_card.json` |
| `harness/` | Rust test crate that runs the vectors |

## Theorems

| Theorem | Plain-words explanation |
|---|---|
| `amount_positive` | Every valid request has an amount greater than zero. |
| `card_number_passes_luhn` | Every valid request has a card number that passes the Luhn checksum. |
| `Rule.all_complete` | The list of rules is complete: no rule is missing from it, so "every rule is tested" really means every rule. |
| `vectors_match_intent` | Every test vector gets the result it was written for: valid ones are accepted, invalid ones are rejected by the rule they target. |
| `vectors_single_fault` | Valid vectors break no rule, and each invalid vector breaks exactly one rule. No vector can hide a second error behind the first. |
| `every_rule_tested` | Every rule has at least one vector that breaks it. |
| `ids_unique` | No two vectors share a name, so a failing test always points to exactly one vector. |

The first three hold for every possible request and are checked by Lean's kernel. The last four
are checked over the 44 fixed vectors with `native_decide`, which runs a compiled check.
