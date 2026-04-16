use cards::CardNumber;
use common_utils::Email;
use hyperswitch_masking::Secret;

/// The payout method information required for carrying out a payout
#[derive(Debug, Clone)]
pub enum PayoutMethodData {
    Card(CardPayout),
    Bank(Bank),
    Wallet(Wallet),
    BankRedirect(BankRedirect),
    Passthrough(Passthrough),
}

impl Default for PayoutMethodData {
    fn default() -> Self {
        Self::Card(CardPayout::default())
    }
}

#[derive(Default, Eq, PartialEq, Clone, Debug)]
pub struct CardPayout {
    /// The card number
    pub card_number: CardNumber,

    /// The card's expiry month
    pub expiry_month: Secret<String>,

    /// The card's expiry year
    pub expiry_year: Secret<String>,

    /// The card holder's name
    pub card_holder_name: Option<Secret<String>>,

    /// The card's network
    pub card_network: Option<common_enums::CardNetwork>,
}

#[derive(Eq, PartialEq, Clone, Debug)]
pub enum Bank {
    Ach(AchBankTransfer),
    Bacs(BacsBankTransfer),
    Sepa(SepaBankTransfer),
    Pix(PixBankTransfer),
}

#[derive(Default, Eq, PartialEq, Clone, Debug)]
pub struct AchBankTransfer {
    /// Bank name
    pub bank_name: Option<common_enums::BankNames>,

    /// Bank country code
    pub bank_country_code: Option<common_enums::CountryAlpha2>,

    /// Bank city
    pub bank_city: Option<String>,

    /// Bank account number is an unique identifier assigned by a bank to a customer.
    pub bank_account_number: Secret<String>,

    /// [9 digits] Routing number - used in USA for identifying a specific bank.
    pub bank_routing_number: Secret<String>,
}

#[derive(Default, Eq, PartialEq, Clone, Debug)]
pub struct BacsBankTransfer {
    /// Bank name
    pub bank_name: Option<common_enums::BankNames>,

    /// Bank country code
    pub bank_country_code: Option<common_enums::CountryAlpha2>,

    /// Bank city
    pub bank_city: Option<String>,

    /// Bank account number is an unique identifier assigned by a bank to a customer.
    pub bank_account_number: Secret<String>,

    /// [6 digits] Sort Code - used in UK and Ireland for identifying a bank and it's branches.
    pub bank_sort_code: Secret<String>,
}

#[derive(Default, Eq, PartialEq, Clone, Debug)]
// The SEPA (Single Euro Payments Area) is a pan-European network that allows you to send and receive payments in euros between two cross-border bank accounts in the eurozone.
pub struct SepaBankTransfer {
    /// Bank name
    pub bank_name: Option<common_enums::BankNames>,

    /// Bank country code
    pub bank_country_code: Option<common_enums::CountryAlpha2>,

    /// Bank city
    pub bank_city: Option<String>,

    /// International Bank Account Number (iban) - used in many countries for identifying a bank along with it's customer.
    pub iban: Secret<String>,

    /// [8 / 11 digits] Bank Identifier Code (bic) / Swift Code - used in many countries for identifying a bank and it's branches
    pub bic: Option<Secret<String>>,
}

#[derive(Default, Eq, PartialEq, Clone, Debug)]
pub struct PixBankTransfer {
    /// Bank name
    pub bank_name: Option<common_enums::BankNames>,

    /// Bank branch
    pub bank_branch: Option<String>,

    /// Bank account number is an unique identifier assigned by a bank to a customer.
    pub bank_account_number: Option<Secret<String>>,

    /// Unique key for pix customer
    pub pix_key: Option<Secret<String>>,

    /// EMV data for pix
    pub pix_emv: Option<Secret<String>>,

    /// Individual taxpayer identification number
    pub tax_id: Option<Secret<String>>,
}

#[derive(Eq, PartialEq, Clone, Debug)]
pub enum Wallet {
    ApplePayDecrypt(ApplePayDecrypt),
    Paypal(Paypal),
    Venmo(Venmo),
}

#[derive(Eq, PartialEq, Clone, Debug)]
pub enum BankRedirect {
    Interac(Interac),
    OpenBankingUk(OpenBankingUk),
}

#[derive(Default, Eq, PartialEq, Clone, Debug)]
pub struct Interac {
    /// Customer email linked with interac account
    pub email: Email,
}

#[derive(Default, Eq, PartialEq, Clone, Debug)]
pub struct OpenBankingUk {
    /// Account holder name
    pub account_holder_name: Secret<String>,
    /// International Bank Account Number (iban) - used in many countries for identifying a bank along with it's customer.
    pub iban: Secret<String>,
}

#[derive(Eq, PartialEq, Clone, Debug)]
pub struct Passthrough {
    /// PSP token generated for the payout method
    pub psp_token: Secret<String>,

    /// Payout method type of the token
    pub token_type: common_enums::PaymentMethodType,
}

#[derive(Default, Eq, PartialEq, Clone, Debug)]
pub struct Paypal {
    /// Email linked with paypal account
    pub email: Option<Email>,

    /// mobile number linked to paypal account
    pub telephone_number: Option<Secret<String>>,

    /// id of the paypal account
    pub paypal_id: Option<Secret<String>>,
}

#[derive(Default, Eq, PartialEq, Clone, Debug)]
pub struct Venmo {
    /// mobile number linked to venmo account
    pub telephone_number: Option<Secret<String>>,
}

#[derive(Default, Eq, PartialEq, Clone, Debug)]
pub struct ApplePayDecrypt {
    /// The dpan number associated with card number
    pub dpan: CardNumber,

    /// The card's expiry month
    pub expiry_month: Secret<String>,

    /// The card's expiry year
    pub expiry_year: Secret<String>,

    /// The card holder's name
    pub card_holder_name: Option<Secret<String>>,

    /// The card's network
    pub card_network: Option<common_enums::CardNetwork>,
}
