import PrismSpec

/-!
`lake exe gen [out_dir]` — writes the vectors for the Rust tests in `tests/lean/harness/tests/`:
`authorize_no3ds_card.json` and `amount_conversion.json` (default `out_dir`: `vectors`).
-/

open PrismSpec.Authorize.No3dsCard

inductive J
  | null
  | bool (b : Bool)
  | num (i : Int)
  | str (s : String)
  | arr (xs : List J)
  | obj (kvs : List (String × J))

def hex4 (n : Nat) : String :=
  let h := (Nat.toDigits 16 n)
  String.mk (List.replicate (4 - h.length) '0' ++ h)

def escape (s : String) : String :=
  s.foldl (fun acc c =>
    acc ++ match c with
      | '"' => "\\\""
      | '\\' => "\\\\"
      | '\n' => "\\n"
      | c => if c.toNat < 0x20 then "\\u" ++ hex4 c.toNat else c.toString) ""

partial def J.render : J → String
  | .null => "null"
  | .bool b => toString b
  | .num i => toString i
  | .str s => "\"" ++ escape s ++ "\""
  | .arr xs => "[" ++ ", ".intercalate (xs.map J.render) ++ "]"
  | .obj kvs => "{" ++ ", ".intercalate (kvs.map fun (k, v) => J.render (.str k) ++ ": " ++ v.render) ++ "}"

def optStr : Option String → J
  | some s => .str s
  | none => .null

def rawJson (r : Raw) : J :=
  .obj [
    ("merchant_transaction_id", optStr r.merchantTransactionId),
    ("amount", match r.amount with
      | some m => .obj [("minor_amount", .num m.minor), ("currency", .str m.currency.proto)]
      | none => .null),
    ("capture_method", .str r.capture.proto),
    ("auth_type", .str r.authType.proto),
    ("authentication_data", .bool r.authData),
    ("card", match r.card with
      | some c => .obj [
          ("card_number", optStr c.number), ("card_exp_month", optStr c.expMonth),
          ("card_exp_year", optStr c.expYear), ("card_cvc", optStr c.cvc),
          ("card_holder_name", optStr c.holder)]
      | none => .null)]

/-- What UCS should produce for an accepted request: the normalized values from the spec. -/
def expectedJson (r : Raw) : J :=
  match decode r with
  | .ok a => .obj [
      ("minor_amount", .num a.amount.minor),
      ("currency", .str a.amount.currency.code),
      ("capture_method", .str a.capture.name)]
  | .error _ => .null

def vectorJson (v : Vector) : J :=
  .obj [
    ("id", .str v.id),
    ("kind", .str v.kind),
    ("rule", optStr (v.intended.map Rule.id)),
    ("field", optStr (v.intended.map Rule.field)),
    ("request", rawJson v.raw),
    ("expected", expectedJson v.raw)]

def document : String :=
  let header := J.obj [
    ("generator", .str "tests/lean/PrismSpec/Authorize/No3dsCard/Vectors.lean"),
    ("regenerate", .str "cd tests/lean && lake build && lake exe gen"),
    ("group", .str "PaymentService/Authorize · No3DS · card"),
    ("rules", .arr (Rule.all.map fun r => .obj [("id", .str r.id), ("field", .str r.field)]))]
  let body := ",\n    ".intercalate (vectors.map fun v => (vectorJson v).render)
  -- header object minus its closing brace, then the vectors one per line (readable diffs)
  (header.render.dropRight 1) ++ ",\n  \"vectors\": [\n    " ++ body ++ "\n  ]\n}\n"

/-! ## Amount conversion -/

def amountVectorJson (v : PrismSpec.Amount.Vector) : J :=
  .obj [
    ("id", .str v.id),
    ("kind", .str v.kind.name),
    ("currency", .str v.currency.code),
    ("wire", .str v.wire),
    ("minor", match v.minor with | some m => .num m | none => .null)]

def amountDocument : String :=
  let header := J.obj [
    ("generator", .str "tests/lean/PrismSpec/Amount/Vectors.lean"),
    ("regenerate", .str "cd tests/lean && lake build && lake exe gen"),
    ("group", .str "AmountConvertor convert / convert_back")]
  let body := ",\n    ".intercalate (PrismSpec.Amount.vectors.map fun v => (amountVectorJson v).render)
  (header.render.dropRight 1) ++ ",\n  \"vectors\": [\n    " ++ body ++ "\n  ]\n}\n"

def main (args : List String) : IO Unit := do
  let dir : System.FilePath := args.head?.getD "vectors"
  IO.FS.createDirAll dir
  IO.FS.writeFile (dir / "authorize_no3ds_card.json") document
  IO.println s!"wrote {vectors.length} vectors to {dir / "authorize_no3ds_card.json"}"
  IO.FS.writeFile (dir / "amount_conversion.json") amountDocument
  IO.println s!"wrote {PrismSpec.Amount.vectors.length} vectors to {dir / "amount_conversion.json"}"
