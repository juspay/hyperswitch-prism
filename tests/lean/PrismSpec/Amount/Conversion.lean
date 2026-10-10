/-!
# Amount conversion — minor units ↔ connector amounts

How UCS should turn a `MinorUnit` into the amount a connector sends (`convert`) and turn a
connector's amount back into a `MinorUnit` (`convert_back`).

Rust source: `AmountConvertor` and its impls in `crates/common/common_utils/src/types.rs`.

A connector amount is modelled as a decimal `⟨mant, places⟩`, meaning `mant / 10^places`
("10.99" is `⟨1099, 2⟩`). Every function here uses exact Nat arithmetic: nothing is
rounded, so a value that cannot be expressed in minor units is an error, never truncated.
-/

namespace PrismSpec.Amount

/-! ## Currencies, one per exponent group -/

inductive Currency
  | jpy  -- exponent 0
  | usd  -- exponent 2
  | kwd  -- exponent 3
  | clf  -- exponent 4
  deriving DecidableEq, Repr

def Currency.all : List Currency := [.jpy, .usd, .kwd, .clf]

def Currency.code : Currency → String
  | .jpy => "JPY"
  | .usd => "USD"
  | .kwd => "KWD"
  | .clf => "CLF"

/-- ISO 4217 exponent. Mirrors `Currency::number_of_digits_after_decimal_point`. -/
def exponent : Currency → Nat
  | .jpy => 0
  | .usd => 2
  | .kwd => 3
  | .clf => 4

/-! ## Decimals -/

/-- `mant / 10^places`. -/
structure Dec where
  mant   : Nat
  places : Nat
  deriving DecidableEq, Repr

inductive ConvError
  | tooManyDecimals
  deriving DecidableEq, Repr

/-- Send: minor units → connector amount. It uses the currency's exponent, the same one
`fromMajor` reads back. -/
def toMajor (c : Currency) (m : Nat) : Dec :=
  ⟨m, exponent c⟩

/-- Read back a decimal at scale `e`: the minor amount if `d` is a whole number of
`10^-e` units, else `tooManyDecimals`. -/
def fromDec (e : Nat) (d : Dec) : Except ConvError Nat :=
  if d.places ≤ e then
    .ok (d.mant * 10 ^ (e - d.places))
  else if 10 ^ (d.places - e) ∣ d.mant then
    .ok (d.mant / 10 ^ (d.places - e))
  else
    .error .tooManyDecimals

/-- Receive: connector major amount → minor units. -/
def fromMajor (c : Currency) (d : Dec) : Except ConvError Nat :=
  fromDec (exponent c) d

/-- Receive: connector minor-unit string (`StringMinorUnit`) → minor units. -/
def fromMinor (d : Dec) : Except ConvError Nat :=
  fromDec 0 d

/-! ## Theorems (kernel-checked, for every currency and every amount) -/

/-- Send and receive use the same scale: `toMajor` writes exactly `exponent c` decimals. -/
theorem send_uses_exponent (c : Currency) (m : Nat) : (toMajor c m).places = exponent c :=
  rfl

/-- Sending an amount and reading it back gives the same amount, for every currency. -/
theorem round_trip (c : Currency) (m : Nat) : fromMajor c (toMajor c m) = .ok m := by
  simp [fromMajor, fromDec, toMajor]

/-- Whatever `fromDec` returns has exactly the value of the input decimal: nothing is
rounded or truncated. `m / 10^e = mant / 10^places`, cross-multiplied. -/
theorem fromDec_exact (e : Nat) (d : Dec) (m : Nat) (h : fromDec e d = .ok m) :
    m * 10 ^ d.places = d.mant * 10 ^ e := by
  unfold fromDec at h
  split at h
  · rename_i hle
    cases h
    rw [Nat.mul_assoc, ← Nat.pow_add, Nat.sub_add_cancel hle]
  · rename_i hgt
    split at h
    · rename_i hdvd
      cases h
      have : 10 ^ d.places = 10 ^ (d.places - e) * 10 ^ e := by
        rw [← Nat.pow_add, Nat.sub_add_cancel (by omega)]
      rw [this, ← Nat.mul_assoc, Nat.div_mul_cancel hdvd]
    · cases h

/-- `fromDec` accepts every decimal that *is* a whole number of minor units. With
`fromDec_exact`, it accepts exactly those and nothing else. -/
theorem fromDec_complete (e : Nat) (d : Dec) (m : Nat)
    (h : m * 10 ^ d.places = d.mant * 10 ^ e) : fromDec e d = .ok m := by
  have hpos : ∀ n, 0 < 10 ^ n := fun _ => Nat.pow_pos (by decide)
  unfold fromDec
  split
  · rename_i hle
    have : 10 ^ e = 10 ^ (e - d.places) * 10 ^ d.places := by
      rw [← Nat.pow_add, Nat.sub_add_cancel hle]
    rw [this, ← Nat.mul_assoc] at h
    exact congrArg Except.ok (Nat.eq_of_mul_eq_mul_right (hpos _) h).symm
  · rename_i hgt
    have : 10 ^ d.places = 10 ^ (d.places - e) * 10 ^ e := by
      rw [← Nat.pow_add, Nat.sub_add_cancel (by omega)]
    rw [this, ← Nat.mul_assoc] at h
    have hm : m * 10 ^ (d.places - e) = d.mant := Nat.eq_of_mul_eq_mul_right (hpos _) h
    have hdvd : 10 ^ (d.places - e) ∣ d.mant := ⟨m, by rw [← hm, Nat.mul_comm]⟩
    rw [if_pos hdvd, ← hm, Nat.mul_div_cancel _ (hpos _)]

/-- A decimal with more places than the currency allows, whose extra digits are not all
zero, is rejected. No truncation. -/
theorem fromDec_rejects_excess (e : Nat) (d : Dec) (hgt : e < d.places)
    (hnd : ¬ 10 ^ (d.places - e) ∣ d.mant) : fromDec e d = .error .tooManyDecimals := by
  unfold fromDec
  rw [if_neg (by omega), if_neg hnd]

/-! ## Connector wire format (strings) -/

def pad (n : Nat) (s : String) : String :=
  String.mk (List.replicate (n - s.length) '0') ++ s

/-- `⟨1099, 2⟩` → `"10.99"`; `⟨5, 0⟩` → `"5"`. -/
def Dec.render (d : Dec) : String :=
  if d.places = 0 then toString d.mant
  else
    let s := 10 ^ d.places
    toString (d.mant / s) ++ "." ++ pad d.places (toString (d.mant % s))

def isDigits (s : String) : Bool :=
  !s.isEmpty && s.all Char.isDigit

/-- `"10.99"` → `⟨1099, 2⟩`, `"10"` → `⟨10, 0⟩`. Anything else (sign, exponent, empty part)
is not a connector amount. -/
def Dec.parse (s : String) : Option Dec :=
  match s.splitOn "." with
  | [i] => if isDigits i then some ⟨i.toNat!, 0⟩ else none
  | [i, f] => if isDigits i && isDigits f then some ⟨(i ++ f).toNat!, f.length⟩ else none
  | _ => none

end PrismSpec.Amount
