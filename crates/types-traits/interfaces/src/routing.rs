use common_enums::RoutableConnectors;
use utoipa::ToSchema;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, ToSchema)]
#[serde(from = "RoutableChoiceSerde", into = "RoutableChoiceSerde")]
pub struct RoutableConnectorChoice {
    #[serde(skip)]
    pub choice_kind: RoutableChoiceKind,
    pub connector: RoutableConnectors,
    #[schema(value_type = Option<String>)]
    pub merchant_connector_id: Option<common_utils::id_type::MerchantConnectorAccountId>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, ToSchema, PartialEq)]
pub enum RoutableChoiceKind {
    OnlyConnector,
    FullStruct,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(untagged)]
pub enum RoutableChoiceSerde {
    OnlyConnector(Box<RoutableConnectors>),
    FullStruct {
        connector: RoutableConnectors,
        merchant_connector_id: Option<common_utils::id_type::MerchantConnectorAccountId>,
    },
}

impl From<RoutableChoiceSerde> for RoutableConnectorChoice {
    fn from(value: RoutableChoiceSerde) -> Self {
        match value {
            RoutableChoiceSerde::OnlyConnector(connector) => Self {
                choice_kind: RoutableChoiceKind::OnlyConnector,
                connector: *connector,
                merchant_connector_id: None,
            },

            RoutableChoiceSerde::FullStruct {
                connector,
                merchant_connector_id,
            } => Self {
                choice_kind: RoutableChoiceKind::FullStruct,
                connector,
                merchant_connector_id,
            },
        }
    }
}

impl From<RoutableConnectorChoice> for RoutableChoiceSerde {
    fn from(value: RoutableConnectorChoice) -> Self {
        match value.choice_kind {
            RoutableChoiceKind::OnlyConnector => Self::OnlyConnector(Box::new(value.connector)),
            RoutableChoiceKind::FullStruct => Self::FullStruct {
                connector: value.connector,
                merchant_connector_id: value.merchant_connector_id,
            },
        }
    }
}
