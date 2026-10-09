/-!
# Authorize · No3DS card — typed (dependent) request

The *valid* No3DS card Authorize request, with every invariant carried in the type.
A value of `No3dsCardAuthorize` cannot exist unless all invariants hold, so anything
`decode` (Raw.lean) returns is valid by construction — the type checker is the proof.

Proto source: `PaymentServiceAuthorizeRequest` (payment.proto:2697),
`CardDetails` (payment_methods.proto:288).
-/

namespace PrismSpec.Authorize.No3dsCard

/-! ## Card number -/

/-- Mirrors `common_utils::consts::{MIN,MAX}_CARD_NUMBER_LENGTH`. -/
def minCardLen : Nat := 8
def maxCardLen : Nat := 19

def digitOf? (c : Char) : Option Nat :=
  if c.isDigit then some (c.toNat - '0'.toNat) else none

def digitsOf? (s : String) : Option (List Nat) :=
  s.toList.mapM digitOf?

def luhnTerm (idx d : Nat) : Nat :=
  if idx % 2 = 1 then (2 * d) / 10 + (2 * d) % 10 else d

def luhnAux : Nat → List Nat → Nat
  | _, [] => 0
  | i, d :: ds => luhnTerm i d + luhnAux (i + 1) ds

/-- Mirrors `cards::validate::luhn` (crates/types-traits/cards/src/validate.rs:219):
digits are walked right-to-left; odd positions are doubled and digit-summed. -/
def luhn (digits : List Nat) : Bool :=
  luhnAux 0 digits.reverse % 10 == 0

structure CardNumber where
  digits  : List Nat
  h_digit : digits.all (· < 10) = true
  h_len   : minCardLen ≤ digits.length ∧ digits.length ≤ maxCardLen
  h_luhn  : luhn digits = true

/-! ## Network → CVC length (the dependent part) -/

inductive Network
  | amex
  | other
  deriving DecidableEq, Repr

/-- AMEX BINs start 34 / 37. Everything else in this group uses a 3-digit CVC. -/
def networkOf : List Nat → Network
  | 3 :: 4 :: _ => .amex
  | 3 :: 7 :: _ => .amex
  | _ => .other

def cvcLen : Network → Nat
  | .amex => 4
  | .other => 3

/-- A CVC whose length is fixed by the card's network. -/
structure Cvc (net : Network) where
  digits  : List Nat
  h_digit : digits.all (· < 10) = true
  h_len   : digits.length = cvcLen net

/-! ## Expiry -/

structure ExpMonth where
  val : Nat
  h   : 1 ≤ val ∧ val ≤ 12

/-- 2-digit ("30") or 4-digit ("2030") year. Not compared to today's date:
that would make the vectors time-dependent. -/
structure ExpYear where
  text : String
  h    : (text.length = 2 ∨ text.length = 4) ∧ text.all Char.isDigit = true

/-! ## Card -/

structure Card where
  number   : CardNumber
  expMonth : ExpMonth
  expYear  : ExpYear
  cvc      : Cvc (networkOf number.digits)
  holder   : Option String

/-! ## Amount -/

inductive Currency
  | usd  -- exponent 2
  | jpy  -- exponent 0
  | kwd  -- exponent 3
  deriving DecidableEq, Repr

def Currency.code : Currency → String
  | .usd => "USD"
  | .jpy => "JPY"
  | .kwd => "KWD"

/-- `Money.minor_amount` is `int64`. -/
def maxMinor : Nat := 9223372036854775807

structure Amount where
  currency : Currency
  minor    : Nat
  h_pos    : 0 < minor
  h_max    : minor ≤ maxMinor

/-! ## Request -/

inductive Capture
  | automatic
  | manual
  deriving DecidableEq, Repr

def Capture.name : Capture → String
  | .automatic => "Automatic"
  | .manual => "Manual"

/-- A valid No3DS card Authorize request. `auth_type` is not a field: the group fixes it
to No3DS, and `authentication_data` has no slot at all. -/
structure No3dsCardAuthorize where
  amount  : Amount
  capture : Capture
  card    : Card

/-! ## Properties that follow from the types alone -/

theorem amount_positive (r : No3dsCardAuthorize) : 0 < r.amount.minor :=
  r.amount.h_pos

/-- Every card guarantee at once: number digits / length / Luhn, expiry month and year,
and a CVC of digits whose length matches the card's network. -/
theorem card_valid (r : No3dsCardAuthorize) :
    r.card.number.digits.all (· < 10) = true ∧
    (minCardLen ≤ r.card.number.digits.length ∧ r.card.number.digits.length ≤ maxCardLen) ∧
    luhn r.card.number.digits = true ∧
    (1 ≤ r.card.expMonth.val ∧ r.card.expMonth.val ≤ 12) ∧
    ((r.card.expYear.text.length = 2 ∨ r.card.expYear.text.length = 4) ∧
      r.card.expYear.text.all Char.isDigit = true) ∧
    r.card.cvc.digits.all (· < 10) = true ∧
    r.card.cvc.digits.length = cvcLen (networkOf r.card.number.digits) :=
  ⟨r.card.number.h_digit, r.card.number.h_len, r.card.number.h_luhn, r.card.expMonth.h,
   r.card.expYear.h, r.card.cvc.h_digit, r.card.cvc.h_len⟩

end PrismSpec.Authorize.No3dsCard
