import PrismSpec.Authorize.No3dsCard.Raw

/-!
# Authorize · No3DS card — deterministic test vectors

Every vector is a fixed `Raw` request plus the rule it is *meant* to break (`none` = valid).
The theorems at the bottom are checked by `lake build`; if any vector is wrong, or a rule
has no vector, the build fails and no JSON is generated.
-/

namespace PrismSpec.Authorize.No3dsCard

/-! ## Helpers -/

def digitsStr (ds : List Nat) : String :=
  String.mk (ds.map fun d => Char.ofNat (d + 48))

/-- Append the Luhn check digit to `body`. -/
def withCheck (body : List Nat) : String :=
  digitsStr (body ++ [((List.range 10).find? fun d => luhn (body ++ [d])).getD 0])

def visa16 : String := "4242424242424242"
def master16 : String := "5555555555554444"
def amex15 : String := "378282246310005"

def baseCard : RawCard :=
  { number := some visa16, expMonth := some "03", expYear := some "2030",
    cvc := some "737", holder := some "Test Name" }

def base : Raw :=
  { merchantTransactionId := some "test_id", amount := some ⟨6000, .usd⟩, capture := .automatic, authType := .noThreeDs,
    authData := false, card := some baseCard }

def withCard (f : RawCard → RawCard) : Raw :=
  { base with card := some (f baseCard) }

structure Vector where
  id       : String
  kind     : String   -- "field_class" | "network_pair" | "single_fault"
  raw      : Raw
  intended : Option Rule

def ok (id kind : String) (raw : Raw) : Vector := ⟨id, kind, raw, none⟩
def bad (id : String) (raw : Raw) (r : Rule) : Vector := ⟨id, "single_fault", raw, some r⟩

/-! ## Valid requests: one per field class -/

def positives : List Vector :=
  [ ok "base_visa_usd_automatic" "field_class" base
  , ok "amount_min_1" "field_class" { base with amount := some ⟨1, .usd⟩ }
  , ok "amount_int64_max" "field_class" { base with amount := some ⟨9223372036854775807, .usd⟩ }
  , ok "currency_jpy_exp0" "field_class" { base with amount := some ⟨6000, .jpy⟩ }
  , ok "currency_kwd_exp3" "field_class" { base with amount := some ⟨6000, .kwd⟩ }
  , ok "capture_manual" "field_class" { base with capture := .manual }
  , ok "capture_unspecified_defaults_automatic" "field_class" { base with capture := .unspecified }
  , ok "auth_type_unspecified_defaults_no3ds" "field_class" { base with authType := .unspecified }
  , ok "card_number_min_len_8" "field_class" (withCard ({ · with number := some (withCheck [4,0,0,0,0,0,0]) }))
  , ok "card_number_max_len_19" "field_class"
      (withCard ({ · with number := some (withCheck (4 :: List.replicate 17 1)) }))
  , ok "exp_month_single_digit" "field_class" (withCard ({ · with expMonth := some "1" }))
  , ok "exp_month_12" "field_class" (withCard ({ · with expMonth := some "12" }))
  , ok "exp_year_2_digit" "field_class" (withCard ({ · with expYear := some "30" }))
  , ok "holder_absent" "field_class" (withCard ({ · with holder := none }))
  , ok "holder_unicode" "field_class" (withCard ({ · with holder := some "Tëst Nàme 名前" }))
  , ok "holder_empty" "field_class" (withCard ({ · with holder := some "" }))
  -- network × CVC length (the dependent pair)
  , ok "mastercard_cvc3" "network_pair" (withCard ({ · with number := some master16 }))
  , ok "amex_cvc4" "network_pair" (withCard ({ · with number := some amex15, cvc := some "7373" }))
  ]

/-! ## Invalid requests: each breaks exactly one rule -/

def negatives : List Vector :=
  [ bad "amount_missing" { base with amount := none } .amountMissing
  , bad "amount_zero" { base with amount := some ⟨0, .usd⟩ } .amountNonPositive
  , bad "amount_negative" { base with amount := some ⟨-1, .usd⟩ } .amountNonPositive
  , bad "currency_unspecified" { base with amount := some ⟨6000, .unspecified⟩ } .currencyUnspecified
  , bad "payment_method_missing" { base with card := none } .paymentMethodMissing
  , bad "card_number_missing" (withCard ({ · with number := none })) .cardNumberMissing
  , bad "card_number_hyphens" (withCard ({ · with number := some "4242-4242-4242-4242" })) .cardNumberNonDigit
  , bad "card_number_letters" (withCard ({ · with number := some "4242abcd42424242" })) .cardNumberNonDigit
  , bad "card_number_empty" (withCard ({ · with number := some "" })) .cardNumberLength
  , bad "card_number_len_7" (withCard ({ · with number := some (withCheck [4,0,0,0,0,0]) })) .cardNumberLength
  , bad "card_number_len_20"
      (withCard ({ · with number := some (withCheck (4 :: List.replicate 18 1)) })) .cardNumberLength
  , bad "card_number_luhn" (withCard ({ · with number := some "4242424242424241" })) .cardNumberLuhn
  , bad "exp_month_missing" (withCard ({ · with expMonth := none })) .expMonthMissing
  , bad "exp_month_00" (withCard ({ · with expMonth := some "00" })) .expMonthInvalid
  , bad "exp_month_13" (withCard ({ · with expMonth := some "13" })) .expMonthInvalid
  , bad "exp_month_letters" (withCard ({ · with expMonth := some "ab" })) .expMonthInvalid
  , bad "exp_year_missing" (withCard ({ · with expYear := none })) .expYearMissing
  , bad "exp_year_3_digit" (withCard ({ · with expYear := some "203" })) .expYearInvalid
  , bad "exp_year_5_digit" (withCard ({ · with expYear := some "20300" })) .expYearInvalid
  , bad "exp_year_letters" (withCard ({ · with expYear := some "20a0" })) .expYearInvalid
  , bad "cvc_missing" (withCard ({ · with cvc := none })) .cvcMissing
  , bad "cvc_letters" (withCard ({ · with cvc := some "7a7" })) .cvcNonDigit
  , bad "visa_cvc_4" (withCard ({ · with cvc := some "7373" })) .cvcLengthForNetwork
  , bad "visa_cvc_2" (withCard ({ · with cvc := some "73" })) .cvcLengthForNetwork
  , bad "amex_cvc_3" (withCard ({ · with number := some amex15, cvc := some "737" })) .cvcLengthForNetwork
  , bad "authentication_data_with_no3ds" { base with authData := true } .authDataWithNo3ds
  ]

def vectors : List Vector := positives ++ negatives

/-! ## Checked properties of the suite -/

/-- The spec (`decode`) agrees with what each vector was written to test. -/
theorem vectors_match_intent :
    vectors.all (fun v => outcome (decode v.raw) == v.intended) = true := by
  native_decide

/-- Every valid vector breaks nothing; every invalid vector breaks exactly its one rule. -/
theorem vectors_single_fault :
    vectors.all (fun v => violations v.raw == v.intended.toList) = true := by
  native_decide

/-- Every rule has at least one vector that breaks it. -/
theorem every_rule_tested : ∀ r : Rule, ∃ v ∈ vectors, v.intended = some r := by
  intro r; cases r <;> native_decide

/-- Vector ids are unique (they name the Rust test failures). -/
theorem ids_unique : (vectors.map (·.id)).eraseDups.length = vectors.length := by
  native_decide

end PrismSpec.Authorize.No3dsCard
