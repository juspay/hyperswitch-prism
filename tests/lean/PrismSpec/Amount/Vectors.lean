import PrismSpec.Amount.Conversion

/-!
# Amount conversion — deterministic test vectors

Three kinds, each run against UCS's real `AmountConvertor`s by
`tests/lean/harness/tests/amount_conversion.rs`:

* `send`: a minor amount and the connector string UCS must produce. The harness also reads
  that string back and expects the original amount.
* `receive_major`: a connector major-unit string and the minor amount it means, or a
  rejection when it has more decimals than the currency allows.
* `receive_minor`: the same for a connector minor-unit string.
-/

namespace PrismSpec.Amount

inductive Kind
  | send
  | receiveMajor
  | receiveMinor
  deriving DecidableEq, Repr

def Kind.name : Kind → String
  | .send => "send"
  | .receiveMajor => "receive_major"
  | .receiveMinor => "receive_minor"

structure Vector where
  id       : String
  kind     : Kind
  currency : Currency
  /-- the connector amount string -/
  wire     : String
  /-- the minor amount, or `none` when the spec rejects `wire` -/
  minor    : Option Nat

/-- What the spec says `wire` means. -/
def Vector.specMinor (v : Vector) : Option Nat :=
  match v.kind, Dec.parse v.wire with
  | .receiveMinor, some d => (fromMinor d).toOption
  | _, some d => (fromMajor v.currency d).toOption
  | _, none => none

/-! ## Builders -/

def send (c : Currency) (m : Nat) : Vector :=
  ⟨s!"send_{c.code.toLower}_{m}", .send, c, (toMajor c m).render, some m⟩

def recv (c : Currency) (wire : String) (minor : Option Nat) : Vector :=
  let tag := match minor with | some _ => "ok" | none => "reject"
  ⟨s!"receive_{c.code.toLower}_{wire}_{tag}", .receiveMajor, c, wire, minor⟩

def recvMinor (wire : String) (minor : Option Nat) : Vector :=
  let tag := match minor with | some _ => "ok" | none => "reject"
  ⟨s!"receive_minor_{wire}_{tag}", .receiveMinor, .usd, wire, minor⟩

/-! ## Vectors -/

def sendAmounts : List Nat := [0, 1, 5, 100, 12345, 999999999]

def sends : List Vector :=
  Currency.all.flatMap fun c => sendAmounts.map (send c)

def receives : List Vector :=
  [ -- exact values, including trailing zeros and fewer places than the exponent
    recv .usd "10.99" (some 1099)
  , recv .usd "10.9" (some 1090)
  , recv .usd "10" (some 1000)
  , recv .usd "10.990" (some 1099)
  , recv .usd "0.29" (some 29)
  , recv .usd "1.15" (some 115)
  , recv .usd "19.99" (some 1999)
  , recv .jpy "10" (some 10)
  , recv .jpy "10.0" (some 10)
  , recv .kwd "1.234" (some 1234)
  , recv .kwd "1.2" (some 1200)
  , recv .clf "1.2345" (some 12345)
  , recv .clf "1.23" (some 12300)
  , recv .clf "0.0001" (some 1)
    -- more decimals than the currency allows: must be rejected, not truncated
  , recv .usd "10.999" none
  , recv .usd "10.009" none
  , recv .usd "0.001" none
  , recv .jpy "10.5" none
  , recv .kwd "1.2345" none
  , recv .clf "1.23456" none
  , recvMinor "12" (some 12)
  , recvMinor "12.0" (some 12)
  , recvMinor "12.7" none
  ]

def vectors : List Vector := sends ++ receives

/-! ## Theorems over the vectors (checked by `lake build`) -/

/-- Every vector's expected minor amount is what the spec computes from its wire string. -/
theorem vectors_match_spec : vectors.all (fun v => v.specMinor == v.minor) = true := by
  native_decide

/-- Every send string parses back to exactly the decimal `toMajor` produced: the rendering
loses nothing. -/
theorem send_render_parses_back :
    sends.all (fun v => v.minor.all fun m => Dec.parse v.wire == some (toMajor v.currency m)) = true := by
  native_decide

/-- Every currency has send vectors and an accepted receive vector. -/
theorem every_currency_tested :
    Currency.all.all (fun c =>
      sends.any (·.currency == c) && receives.any (fun v => v.currency == c && v.minor.isSome)) = true := by
  native_decide

/-- Every currency with decimals has a vector whose extra digits must be rejected. -/
theorem every_decimal_currency_rejects_excess :
    (Currency.all.filter (exponent · > 0)).all (fun c =>
      receives.any fun v => v.currency == c && v.kind == .receiveMajor && v.minor.isNone) = true := by
  native_decide

/-- Vector ids are unique (they name the Rust test failures). -/
theorem ids_unique : (vectors.map (·.id)).eraseDups.length = vectors.length := by
  native_decide

end PrismSpec.Amount
