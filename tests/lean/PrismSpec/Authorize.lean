inductive PaymentStatus where
  | authorized
  | pending
  | refunded
  deriving Repr, DecidableEq

def authorize (amount : Int) (currency : String) : PaymentStatus :=
  if amount > 0 then
    PaymentStatus.authorized
  else
    PaymentStatus.pending

theorem authorize_returns_valid_status
    (amount : Nat) (currency : String) :
    authorize amount currency = PaymentStatus.authorized ∨
    authorize amount currency = PaymentStatus.pending := by
  unfold authorize
  split <;> simp
