import PrismSpec.Authorize.No3dsCard.Types

/-!
# Authorize · No3DS card — raw request, rules, decoder

`Raw` mirrors the proto as sent on the wire: every field optional, strings unparsed.
`decode : Raw → Except Rule No3dsCardAuthorize` is the spec. Because its success type is
the dependent `No3dsCardAuthorize`, every accepted request satisfies every invariant.

`violations` lists **all** broken rules (not just the first), so a test vector can be
proven to break exactly one rule.
-/

namespace PrismSpec.Authorize.No3dsCard

/-! ## Raw (proto-shaped) request -/

inductive CurrencyTag
  | unspecified
  | usd
  | jpy
  | kwd
  deriving DecidableEq, Repr

def CurrencyTag.toCurrency? : CurrencyTag → Option Currency
  | .unspecified => none
  | .usd => some .usd
  | .jpy => some .jpy
  | .kwd => some .kwd

/-- proto enum name (`Currency` in payment.proto). -/
def CurrencyTag.proto : CurrencyTag → String
  | .unspecified => "UNSPECIFIED"
  | .usd => "USD"
  | .jpy => "JPY"
  | .kwd => "KWD"

inductive CaptureTag
  | unspecified
  | automatic
  | manual
  deriving DecidableEq, Repr

def CaptureTag.proto : CaptureTag → String
  | .unspecified => "UNSPECIFIED"
  | .automatic => "AUTOMATIC"
  | .manual => "MANUAL"

/-- Unset capture method defaults to automatic (same as UCS today, types.rs:1089). -/
def CaptureTag.normalize : CaptureTag → Capture
  | .unspecified => .automatic
  | .automatic => .automatic
  | .manual => .manual

inductive AuthTag
  | unspecified
  | noThreeDs
  deriving DecidableEq, Repr

def AuthTag.proto : AuthTag → String
  | .unspecified => "UNSPECIFIED"
  | .noThreeDs => "NO_THREE_DS"

structure RawMoney where
  minor    : Int
  currency : CurrencyTag
  deriving DecidableEq, Repr

structure RawCard where
  number   : Option String
  expMonth : Option String
  expYear  : Option String
  cvc      : Option String
  holder   : Option String
  deriving DecidableEq, Repr

structure Raw where
  /-- `merchant_transaction_id`: opaque, always a fixed test id in the vectors. -/
  merchantTransactionId : Option String
  amount   : Option RawMoney
  capture  : CaptureTag
  authType : AuthTag
  /-- `authentication_data` present? -/
  authData : Bool
  /-- `payment_method.card`; `none` = `payment_method` absent. -/
  card     : Option RawCard
  deriving DecidableEq, Repr

/-! ## Rules -/

inductive Rule
  | amountMissing
  | amountNonPositive
  | currencyUnspecified
  | paymentMethodMissing
  | cardNumberMissing
  | cardNumberNonDigit
  | cardNumberLength
  | cardNumberLuhn
  | expMonthMissing
  | expMonthInvalid
  | expYearMissing
  | expYearInvalid
  | cvcMissing
  | cvcNonDigit
  | cvcLengthForNetwork
  | authDataWithNo3ds
  deriving DecidableEq, Repr

def Rule.all : List Rule :=
  [.amountMissing, .amountNonPositive, .currencyUnspecified, .paymentMethodMissing,
   .cardNumberMissing, .cardNumberNonDigit, .cardNumberLength, .cardNumberLuhn,
   .expMonthMissing, .expMonthInvalid, .expYearMissing, .expYearInvalid,
   .cvcMissing, .cvcNonDigit, .cvcLengthForNetwork, .authDataWithNo3ds]

theorem Rule.all_complete : ∀ r : Rule, r ∈ Rule.all := by
  intro r; cases r <;> decide

def Rule.id : Rule → String
  | .amountMissing => "amount_missing"
  | .amountNonPositive => "amount_non_positive"
  | .currencyUnspecified => "currency_unspecified"
  | .paymentMethodMissing => "payment_method_missing"
  | .cardNumberMissing => "card_number_missing"
  | .cardNumberNonDigit => "card_number_non_digit"
  | .cardNumberLength => "card_number_length"
  | .cardNumberLuhn => "card_number_luhn"
  | .expMonthMissing => "exp_month_missing"
  | .expMonthInvalid => "exp_month_invalid"
  | .expYearMissing => "exp_year_missing"
  | .expYearInvalid => "exp_year_invalid"
  | .cvcMissing => "cvc_missing"
  | .cvcNonDigit => "cvc_non_digit"
  | .cvcLengthForNetwork => "cvc_length_for_network"
  | .authDataWithNo3ds => "authentication_data_with_no3ds"

/-- Proto field path the rule is about. -/
def Rule.field : Rule → String
  | .amountMissing | .amountNonPositive => "amount.minor_amount"
  | .currencyUnspecified => "amount.currency"
  | .paymentMethodMissing => "payment_method"
  | .cardNumberMissing | .cardNumberNonDigit | .cardNumberLength | .cardNumberLuhn =>
      "payment_method.card.card_number"
  | .expMonthMissing | .expMonthInvalid => "payment_method.card.card_exp_month"
  | .expYearMissing | .expYearInvalid => "payment_method.card.card_exp_year"
  | .cvcMissing | .cvcNonDigit | .cvcLengthForNetwork => "payment_method.card.card_cvc"
  | .authDataWithNo3ds => "authentication_data"

/-! ## Per-field parsers (each returns a *typed* value or the rule it broke) -/

def parseAmount : Option RawMoney → Except Rule Amount
  | none => .error .amountMissing
  | some m =>
    match m.currency.toCurrency? with
    | none => .error .currencyUnspecified
    | some c =>
      if h : 0 < m.minor.toNat ∧ m.minor.toNat ≤ maxMinor then
        .ok ⟨c, m.minor.toNat, h.1, h.2⟩
      else .error .amountNonPositive

def parseCardNumber : Option String → Except Rule CardNumber
  | none => .error .cardNumberMissing
  | some s =>
    match digitsOf? s with
    | none => .error .cardNumberNonDigit
    | some ds =>
      if hd : ds.all (· < 10) = true then
        if hl : minCardLen ≤ ds.length ∧ ds.length ≤ maxCardLen then
          if hk : luhn ds = true then .ok ⟨ds, hd, hl, hk⟩
          else .error .cardNumberLuhn
        else .error .cardNumberLength
      else .error .cardNumberNonDigit

def digitsToNat (ds : List Nat) : Nat :=
  ds.foldl (fun acc d => acc * 10 + d) 0

def parseExpMonth : Option String → Except Rule ExpMonth
  | none => .error .expMonthMissing
  | some s =>
    match digitsOf? s with
    | some ds =>
      if ds.length = 1 ∨ ds.length = 2 then
        if h : 1 ≤ digitsToNat ds ∧ digitsToNat ds ≤ 12 then .ok ⟨digitsToNat ds, h⟩
        else .error .expMonthInvalid
      else .error .expMonthInvalid
    | none => .error .expMonthInvalid

def parseExpYear : Option String → Except Rule ExpYear
  | none => .error .expYearMissing
  | some s =>
    if h : (s.length = 2 ∨ s.length = 4) ∧ s.all Char.isDigit = true then .ok ⟨s, h⟩
    else .error .expYearInvalid

def parseCvc (net : Network) : Option String → Except Rule (Cvc net)
  | none => .error .cvcMissing
  | some s =>
    match digitsOf? s with
    | none => .error .cvcNonDigit
    | some ds =>
      if hd : ds.all (· < 10) = true then
        if hl : ds.length = cvcLen net then .ok ⟨ds, hd, hl⟩
        else .error .cvcLengthForNetwork
      else .error .cvcNonDigit

/-! ## Decoder (the spec) -/

/-- The CVC parser's type depends on the network of the *already parsed* card number. -/
def parseCard (c : RawCard) : Except Rule Card := do
  let number ← parseCardNumber c.number
  let expMonth ← parseExpMonth c.expMonth
  let expYear ← parseExpYear c.expYear
  let cvc ← parseCvc (networkOf number.digits) c.cvc
  pure ⟨number, expMonth, expYear, cvc, c.holder⟩

def decode (r : Raw) : Except Rule No3dsCardAuthorize := do
  let amount ← parseAmount r.amount
  if r.authData then throw .authDataWithNo3ds
  let rc ← match r.card with
    | none => throw .paymentMethodMissing
    | some c => pure c
  let card ← parseCard rc
  pure { amount, capture := r.capture.normalize, card }

def outcome {α} : Except Rule α → Option Rule
  | .ok _ => none
  | .error e => some e

/-! ## All violations (independent per field) -/

def errs {α} (e : Except Rule α) : List Rule :=
  match e with
  | .ok _ => []
  | .error r => [r]

def amountViolations : Option RawMoney → List Rule
  | none => [.amountMissing]
  | some m =>
    (if m.currency = .unspecified then [.currencyUnspecified] else []) ++
    (if m.minor ≤ 0 then [.amountNonPositive] else [])

/-- Network from the raw number when it is all digits; `.other` otherwise. -/
def rawNetwork (n : Option String) : Network :=
  match n.bind digitsOf? with
  | some ds => networkOf ds
  | none => .other

def cardViolations (c : RawCard) : List Rule :=
  errs (parseCardNumber c.number) ++ errs (parseExpMonth c.expMonth) ++
  errs (parseExpYear c.expYear) ++ errs (parseCvc (rawNetwork c.number) c.cvc)

def violations (r : Raw) : List Rule :=
  amountViolations r.amount ++
  (if r.authData then [.authDataWithNo3ds] else []) ++
  (match r.card with
   | none => [.paymentMethodMissing]
   | some c => cardViolations c)

end PrismSpec.Authorize.No3dsCard
