use error_stack::ResultExt;

use crate::{
    errors::CustomResult,
    global_id::{CellId, GlobalEntity, GlobalId},
};

/// A global id that can be used to identify a payment method
#[derive(Debug, Clone, Hash, PartialEq, Eq, serde::Serialize, serde::Deserialize)]

pub struct GlobalPaymentMethodId(GlobalId);

/// A global id that can be used to identify a payment method session
#[derive(Debug, Clone, Hash, PartialEq, Eq, serde::Serialize, serde::Deserialize)]

pub struct GlobalPaymentMethodSessionId(GlobalId);

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum GlobalPaymentMethodIdError {
    #[error("Failed to construct GlobalPaymentMethodId")]
    ConstructionError,
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum GlobalPaymentMethodSessionIdError {
    #[error("Failed to construct GlobalPaymentMethodSessionId")]
    ConstructionError,
}

impl GlobalPaymentMethodSessionId {
    /// Create a new GlobalPaymentMethodSessionId from cell id information
    pub fn generate(
        cell_id: &CellId,
    ) -> error_stack::Result<Self, GlobalPaymentMethodSessionIdError> {
        let global_id = GlobalId::generate(cell_id, GlobalEntity::PaymentMethodSession);
        Ok(Self(global_id))
    }

    /// Get the string representation of the id
    pub fn get_string_repr(&self) -> &str {
        self.0.get_string_repr()
    }

    /// Construct a redis key from the id to be stored in redis
    pub fn get_redis_key(&self) -> String {
        format!("payment_method_session:{}", self.get_string_repr())
    }
}

impl crate::events::ApiEventMetric for GlobalPaymentMethodSessionId {
    fn get_api_event_type(&self) -> Option<crate::events::ApiEventsType> {
        Some(crate::events::ApiEventsType::PaymentMethodSession {
            payment_method_session_id: self.clone(),
        })
    }
}

impl GlobalPaymentMethodId {
    /// Create a new GlobalPaymentMethodId from cell id information
    pub fn generate(cell_id: &CellId) -> error_stack::Result<Self, GlobalPaymentMethodIdError> {
        let global_id = GlobalId::generate(cell_id, GlobalEntity::PaymentMethod);
        Ok(Self(global_id))
    }

    /// Get string representation of the id
    pub fn get_string_repr(&self) -> &str {
        self.0.get_string_repr()
    }

    /// Construct a new GlobalPaymentMethodId from a string
    pub fn generate_from_string(value: String) -> CustomResult<Self, GlobalPaymentMethodIdError> {
        let id = GlobalId::from_string(value.into())
            .change_context(GlobalPaymentMethodIdError::ConstructionError)?;
        Ok(Self(id))
    }
}
