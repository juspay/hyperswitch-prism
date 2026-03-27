use error_stack::ResultExt;

use crate::{errors, global_id::CellId};

/// A global id that can be used to identify a refund
#[derive(Debug, Clone, Hash, PartialEq, Eq, serde::Serialize, serde::Deserialize)]

pub struct GlobalRefundId(super::GlobalId);

impl GlobalRefundId {
    /// Get string representation of the id
    pub fn get_string_repr(&self) -> &str {
        self.0.get_string_repr()
    }

    /// Generate a new GlobalRefundId from a cell id
    pub fn generate(cell_id: &CellId) -> Self {
        let global_id = super::GlobalId::generate(cell_id, super::GlobalEntity::Refund);
        Self(global_id)
    }
}

// TODO: refactor the macro to include this id use case as well
impl TryFrom<std::borrow::Cow<'static, str>> for GlobalRefundId {
    type Error = error_stack::Report<errors::ValidationError>;
    fn try_from(value: std::borrow::Cow<'static, str>) -> Result<Self, Self::Error> {
        let merchant_ref_id = super::GlobalId::from_string(value).change_context(
            errors::ValidationError::IncorrectValueProvided {
                field_name: "refund_id",
            },
        )?;
        Ok(Self(merchant_ref_id))
    }
}
