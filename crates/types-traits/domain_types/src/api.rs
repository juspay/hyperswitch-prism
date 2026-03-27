use std::collections::HashSet;

use crate::{
    payment_method_data::{DefaultPCIHolder, PaymentMethodData},
    router_response_types::RedirectForm,
};

#[derive(Debug, Eq, PartialEq)]
pub struct RedirectionFormData {
    pub redirect_form: RedirectForm,
    pub payment_method_data: Option<PaymentMethodData<DefaultPCIHolder>>,
    pub amount: String,
    pub currency: String,
}

#[derive(Debug, Eq, PartialEq)]
pub enum PaymentLinkAction {
    PaymentLinkFormData(PaymentLinkFormData),
    PaymentLinkStatus(PaymentLinkStatusData),
}

#[derive(Debug, Eq, PartialEq, Clone, serde::Serialize, serde::Deserialize)]
pub struct PaymentLinkFormData {
    pub js_script: String,
    pub css_script: String,
    pub sdk_url: url::Url,
    pub html_meta_tags: String,
}

#[derive(Debug, Eq, PartialEq, Clone, serde::Serialize, serde::Deserialize)]
pub struct PaymentLinkStatusData {
    pub js_script: String,
    pub css_script: String,
}

#[derive(Debug, Eq, PartialEq)]
pub struct GenericLinks {
    pub allowed_domains: HashSet<String>,
    pub data: GenericLinksData,
    pub locale: String,
}

#[derive(Debug, Eq, PartialEq)]
pub enum GenericLinksData {
    ExpiredLink(GenericExpiredLinkData),
    PaymentMethodCollect(GenericLinkFormData),
    PayoutLink(GenericLinkFormData),
    PayoutLinkStatus(GenericLinkStatusData),
    PaymentMethodCollectStatus(GenericLinkStatusData),
    SecurePaymentLink(PaymentLinkFormData),
}

#[derive(Debug, Eq, PartialEq, Clone, serde::Serialize, serde::Deserialize)]
pub struct GenericExpiredLinkData {
    pub title: String,
    pub message: String,
    pub theme: String,
}

#[derive(Debug, Eq, PartialEq, Clone, serde::Serialize, serde::Deserialize)]
pub struct GenericLinkFormData {
    pub js_data: String,
    pub css_data: String,
    pub sdk_url: url::Url,
    pub html_meta_tags: String,
}

#[derive(Debug, Eq, PartialEq, Clone, serde::Serialize, serde::Deserialize)]
pub struct GenericLinkStatusData {
    pub js_data: String,
    pub css_data: String,
}
