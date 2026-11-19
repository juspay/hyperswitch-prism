pub mod adyen;
pub mod braintree;
pub mod calida;
pub mod cashtocode;
pub mod phonepe;
pub mod rapyd;
pub use self::{
    adyen::Adyen, braintree::Braintree, calida::Calida, cashtocode::Cashtocode, phonepe::Phonepe,
    rapyd::Rapyd,
};
