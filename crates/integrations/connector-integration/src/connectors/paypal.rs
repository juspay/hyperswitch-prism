pub mod transformers;

use base64::Engine;
use bytes::Bytes;
use common_enums::{AttemptStatus, CurrencyUnit};
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
    StringMajorUnit,
};
#[allow(unused_imports)]
use domain_types::{
    connector_flow::{
        Accept, Authenticate, Authorize, Capture, ClientAuthenticationToken,
        CreateConnectorCustomer, CreateOrder, DefendDispute, IncrementalAuthorization,
        MandateRevoke, PSync, PaymentMethodToken, PostAuthenticate, PreAuthenticate, RSync, Refund,
        RepeatPayment, ServerAuthenticationToken, ServerSessionAuthenticationToken, SetupMandate,
        SubmitEvidence, VerifyWebhookSource, Void, VoidPC,
    },
    connector_types::{
        ClientAuthenticationTokenRequestData, EventContext, PaymentCreateOrderData,
        PaymentCreateOrderResponse, PaymentFlowData, PaymentVoidData, PaymentsAuthenticateData,
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentsPostAuthenticateData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, RepeatPaymentData, RequestDetails,
        ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData,
        SetupMandateRequestData, VerifyWebhookSourceFlowData,
    },
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, WalletData},
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
    router_request_types::VerifyWebhookSourceRequestData,
    router_response_types::{Response, VerifyWebhookSourceResponseData},
    types::Connectors,
};
use error_stack::{report, ResultExt};
use hyperswitch_masking::{ExposeInterface, Mask, Maskable, PeekInterface, Secret};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use serde::Serialize;
use std::fmt::{Debug, Write};

use super::macros;
use crate::{
    connectors::paypal::transformers::{
        self as paypal, auth_headers, constants as paypal_constants, PaypalAuthResponse,
        PaypalAuthUpdateRequest, PaypalAuthUpdateResponse, PaypalAuthenticateRequest,
        PaypalAuthorizeRequest, PaypalAuthorizeResponse, PaypalCaptureResponse,
        PaypalClientAuthTokenRequest, PaypalClientAuthTokenResponse, PaypalEmptyRequest,
        PaypalOrderCreateRequest, PaypalOrderCreateResponse, PaypalPaymentsCancelResponse,
        PaypalPaymentsCaptureRequest, PaypalPostAuthenticateResponse, PaypalRefundRequest,
        PaypalRepeatPaymentRequest, PaypalRepeatPaymentResponse, PaypalSetupMandateRequest,
        PaypalSetupMandateResponse, PaypalSyncResponse, PaypalVoidRequest, RefundResponse,
        RefundSyncResponse,
    },
    finalize_connector_response,
    types::ResponseRouterData,
    utils::{self, ConnectorErrorType, ConnectorErrorTypeMapping},
    with_error_response_body,
};
use domain_types::errors::{
    ConnectorError, IntegrationError, IntegrationErrorContext, WebhookError,
};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
}

pub const BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPostAuthenticateV2<T> for Paypal<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Paypal<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Paypal<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Paypal<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ClientAuthentication for Paypal<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Paypal<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Paypal<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Paypal<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Paypal<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Paypal<T>
{
    fn next_authentication_step(
        &self,
        auth_type: common_enums::AuthenticationType,
        payment_method: common_enums::PaymentMethod,
        redirect_state: connector_types::RedirectState,
        completed_step: Option<connector_types::AuthenticationStep>,
    ) -> connector_types::AuthenticationStep {
        use connector_types::{AuthenticationStep, RedirectState};
        if auth_type == common_enums::AuthenticationType::ThreeDs
            && payment_method == common_enums::PaymentMethod::Card
        {
            match (redirect_state, completed_step) {
                (RedirectState::InitialRequest, None) => AuthenticationStep::Authenticate,
                (RedirectState::InitialRequest, Some(AuthenticationStep::Authenticate)) => {
                    AuthenticationStep::PostAuthenticate
                }
                (RedirectState::RedirectWithParams, None) => AuthenticationStep::PostAuthenticate,
                (RedirectState::RedirectWithoutParams, None) => {
                    AuthenticationStep::PostAuthenticate
                }
                (RedirectState::RedirectWithParams, Some(AuthenticationStep::Authenticate)) => {
                    AuthenticationStep::PostAuthenticate
                }
                (RedirectState::RedirectWithoutParams, Some(AuthenticationStep::Authenticate)) => {
                    AuthenticationStep::PostAuthenticate
                }
                _ => AuthenticationStep::Authorize,
            }
        } else {
            match (redirect_state, completed_step) {
                (RedirectState::RedirectWithParams, None) => AuthenticationStep::PostAuthenticate,
                (RedirectState::RedirectWithoutParams, None) => {
                    AuthenticationStep::PostAuthenticate
                }
                _ => AuthenticationStep::Authorize,
            }
        }
    }
    fn should_do_access_token(&self, _payment_method: Option<common_enums::PaymentMethod>) -> bool {
        true
    }
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Paypal<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Paypal<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Paypal<T>
{
    fn verify_webhook_source(
        &self,
        _request: RequestDetails,
        _connector_webhook_secret: Option<domain_types::connector_types::ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, error_stack::Report<WebhookError>> {
        // This is a fallback for connectors that don't require external verification
        // For PayPal, this should never be called due to requires_external_verification check
        Err::<bool, _>(
            error_stack::report!(WebhookError::WebhookSourceVerificationFailed)
                .attach_printable(
                    "PayPal requires external API call for webhook verification, not internal verification",
                ),
        )
    }

    fn sample_webhook_body(&self) -> &'static [u8] {
        br#"{"event_type":"PAYMENT.CAPTURE.COMPLETED","resource":{"id":"probe_capture_001","status":"COMPLETED","amount":{"value":"10.00","currency_code":"USD"}}}"#
    }

    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<domain_types::connector_types::EventType, error_stack::Report<WebhookError>> {
        let envelope: paypal::PaypalWebooksEventType = request
            .body
            .parse_struct("PaypalWebhookEvent")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;
        use paypal::PaypalWebhookEventType as E;
        match envelope.event_type {
            E::PaymentAuthorizationVoided => {
                let details: paypal::PaypalWebhooksBody = request
                    .body
                    .parse_struct("PaypalAuthorizationVoidedWebhook")
                    .change_context(WebhookError::WebhookBodyDecodingFailed)?;
                let paypal::PaypalResource::PaypalCardWebhooks(resource) = details.resource else {
                    return Err(report!(WebhookError::WebhookResourceObjectNotFound));
                };
                paypal_webhook_order_id(&resource)?;
                paypal_check_webhook_resource_status(
                    envelope.event_type,
                    resource.status.as_deref(),
                )?;
                Ok(domain_types::connector_types::EventType::PaymentIntentProcessing)
            }
            E::CustomerDisputeCreated | E::CustomerDisputeResolved | E::CustomerDisputedUpdated => {
                let details: paypal::PaypalWebhooksBody = request
                    .body
                    .parse_struct("PaypalDisputeWebhook")
                    .change_context(WebhookError::WebhookBodyDecodingFailed)?;
                let paypal::PaypalResource::PaypalDisputeWebhooks(resource) = details.resource
                else {
                    return Err(report!(WebhookError::WebhookResourceObjectNotFound));
                };
                let resolved = resource.status == paypal::DisputeStatus::Resolved;
                if envelope.event_type == E::CustomerDisputeResolved && !resolved {
                    return Err(report!(WebhookError::WebhookProcessingFailed)
                        .attach_printable("PayPal dispute event and resource status disagree"));
                }
                if matches!(envelope.event_type, E::CustomerDisputeResolved) || resolved {
                    Ok(get_paypal_event_type(
                        E::CustomerDisputeResolved,
                        resource.dispute_outcome.map(|value| value.outcome_code),
                    ))
                } else if resource.status == paypal::DisputeStatus::Other {
                    Ok(domain_types::connector_types::EventType::IncomingWebhookEventUnspecified)
                } else {
                    Ok(domain_types::connector_types::EventType::DisputeOpened)
                }
            }
            E::CheckoutOrderCompleted => {
                let details: paypal::PaypalWebhooksBody = request
                    .body
                    .parse_struct("PaypalOrderWebhook")
                    .change_context(WebhookError::WebhookBodyDecodingFailed)?;
                let paypal::PaypalResource::PaypalRedirectsWebhooks(resource) = details.resource
                else {
                    return Err(report!(WebhookError::WebhookResourceObjectNotFound));
                };
                let (status, _) = resource.webhook_observation()?;
                Ok(match status {
                    AttemptStatus::Charged => {
                        domain_types::connector_types::EventType::PaymentIntentSuccess
                    }
                    AttemptStatus::Authorized => {
                        domain_types::connector_types::EventType::PaymentIntentAuthorizationSuccess
                    }
                    _ => domain_types::connector_types::EventType::IncomingWebhookEventUnspecified,
                })
            }
            _ => Ok(get_paypal_event_type(envelope.event_type, None)),
        }
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<
        Option<domain_types::connector_types::WebhookResourceReference>,
        error_stack::Report<WebhookError>,
    > {
        let event: paypal::PaypalWebooksEventType = request
            .body
            .parse_struct("PaypalWebhookEvent")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;
        if get_paypal_event_type(event.event_type, None)
            == domain_types::connector_types::EventType::IncomingWebhookEventUnspecified
            && !matches!(
                event.event_type,
                paypal::PaypalWebhookEventType::CustomerDisputeResolved
                    | paypal::PaypalWebhookEventType::CheckoutOrderCompleted
            )
        {
            return Ok(None);
        }
        let details: paypal::PaypalWebhooksBody = request
            .body
            .parse_struct("PaypalWebhookResource")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;
        use domain_types::connector_types::{
            DisputeWebhookReference, PaymentWebhookReference, RefundWebhookReference,
            WebhookResourceReference as R,
        };
        let reference = match details.resource {
            paypal::PaypalResource::PaypalCardWebhooks(resource) => {
                R::Payment(PaymentWebhookReference {
                    connector_transaction_id: Some(paypal_webhook_order_id(&resource)?.to_owned()),
                    merchant_transaction_id: None,
                })
            }
            paypal::PaypalResource::PaypalRedirectsWebhooks(resource) => {
                R::Payment(PaymentWebhookReference {
                    connector_transaction_id: Some(paypal_webhook_id(&resource.id)?.to_owned()),
                    merchant_transaction_id: None,
                })
            }
            paypal::PaypalResource::PaypalRefundWebhooks(resource) => {
                R::Refund(RefundWebhookReference {
                    connector_refund_id: Some(paypal_webhook_id(&resource.id)?.to_owned()),
                    merchant_refund_id: None,
                    connector_transaction_id: None,
                    merchant_transaction_id: None,
                })
            }
            paypal::PaypalResource::PaypalDisputeWebhooks(resource) => {
                let (seller, invoice) = paypal_dispute_identity(&resource)?;
                R::Dispute(DisputeWebhookReference {
                    connector_dispute_id: Some(paypal_webhook_id(&resource.dispute_id)?.to_owned()),
                    connector_transaction_id: Some(seller.to_owned()),
                    merchant_transaction_id: invoice.map(str::to_owned),
                })
            }
            paypal::PaypalResource::Unknown => return Ok(None),
        };
        Ok(Some(reference))
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<domain_types::connector_types::ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        event_context: Option<EventContext>,
    ) -> Result<
        domain_types::connector_types::WebhookDetailsResponse,
        error_stack::Report<WebhookError>,
    > {
        let details: paypal::PaypalWebhooksBody = request
            .body
            .parse_struct("PaypalPaymentWebhook")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;
        let (id, status, amount_captured, reference, reason) = match details.resource {
            paypal::PaypalResource::PaypalCardWebhooks(resource) => {
                let id = paypal_webhook_order_id(&resource)?.to_owned();
                if details.event_type == paypal::PaypalWebhookEventType::PaymentAuthorizationVoided
                {
                    let context = event_context.as_ref().ok_or_else(|| {
                        report!(WebhookError::WebhookMissingRequiredContext {
                            field: "payment.connector_transaction_id",
                            origin: "stored canonical PayPal order"
                        })
                    })?;
                    if context.connector_transaction_id.as_deref() != Some(id.as_str()) {
                        return Err(report!(WebhookError::WebhookSourceVerificationFailed)
                            .attach_printable("PayPal voided authorization order does not belong to the stored payment"));
                    }
                }
                let status = get_paypal_payment_webhook_status(details.event_type)?;
                paypal_check_webhook_resource_status(
                    details.event_type,
                    resource.status.as_deref(),
                )?;
                let amount = if status == AttemptStatus::Charged {
                    paypal_webhook_money(resource.amount)?
                } else {
                    None
                };
                let reason = resource
                    .status_details
                    .and_then(|value| value.reason)
                    .filter(|_| status == AttemptStatus::Failure);
                (id, status, amount, resource.invoice_id, reason)
            }
            paypal::PaypalResource::PaypalRedirectsWebhooks(resource) => {
                let id = paypal_webhook_id(&resource.id)?.to_owned();
                let (status, amount) = resource.webhook_observation()?;
                if !matches!(
                    details.event_type,
                    paypal::PaypalWebhookEventType::CheckoutOrderCompleted
                        | paypal::PaypalWebhookEventType::CheckoutOrderProcessed
                ) {
                    return Err(report!(WebhookError::WebhookEventTypeNotFound));
                }
                (id, status, amount, None, None)
            }
            _ => return Err(report!(WebhookError::WebhookResourceObjectNotFound)),
        };
        Ok(domain_types::connector_types::WebhookDetailsResponse {
            connector_returned_payment_method_details: None,
            resource_id: Some(
                domain_types::connector_types::ResponseId::ConnectorTransactionId(id),
            ),
            status,
            connector_response_reference_id: reference,
            connector_request_reference_id: None,
            mandate_reference: None,
            error_code: reason.clone(),
            error_message: reason.clone(),
            error_reason: reason,
            // Original bytes are returned only through the typed raw-response field;
            // invalid UTF-8 is refused, never replaced before verification or hashing.
            raw_connector_response: Some(
                String::from_utf8(request.body.to_vec())
                    .change_context(WebhookError::WebhookBodyDecodingFailed)?,
            ),
            status_code: 200,
            response_headers: None,
            amount_captured,
            network_txn_id: None,
            payment_method_update: None,
            sender_payment_instrument_id: None,
        })
    }

    fn process_refund_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<domain_types::connector_types::ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<
        domain_types::connector_types::RefundWebhookDetailsResponse,
        error_stack::Report<WebhookError>,
    > {
        let details: paypal::PaypalWebhooksBody = request
            .body
            .parse_struct("PaypalRefundWebhook")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;
        let paypal::PaypalResource::PaypalRefundWebhooks(resource) = details.resource else {
            return Err(report!(WebhookError::WebhookResourceObjectNotFound));
        };
        use paypal::PaypalWebhookEventType as E;
        let (status, expected) = match details.event_type {
            E::PaymentCaptureRefunded => (common_enums::RefundStatus::Success, "COMPLETED"),
            E::PaymentRefundPending => (common_enums::RefundStatus::Pending, "PENDING"),
            E::PaymentRefundFailed => (common_enums::RefundStatus::Failure, "FAILED"),
            _ => return Err(report!(WebhookError::WebhookEventTypeNotFound)),
        };
        if resource
            .status
            .as_deref()
            .is_some_and(|value| value != expected)
        {
            return Err(report!(WebhookError::WebhookProcessingFailed)
                .attach_printable("PayPal refund event and resource status disagree"));
        }
        let reason = resource.status_details.and_then(|value| value.reason);
        Ok(
            domain_types::connector_types::RefundWebhookDetailsResponse {
                connector_refund_id: Some(paypal_webhook_id(&resource.id)?.to_owned()),
                refund_amount: paypal_webhook_money(resource.amount)?,
                merchant_transaction_id: None,
                status,
                connector_response_reference_id: None,
                error_code: reason
                    .clone()
                    .filter(|_| status == common_enums::RefundStatus::Failure),
                error_message: reason.filter(|_| status == common_enums::RefundStatus::Failure),
                raw_connector_response: Some(
                    String::from_utf8(request.body.to_vec())
                        .change_context(WebhookError::WebhookBodyDecodingFailed)?,
                ),
                status_code: 200,
                response_headers: None,
            },
        )
    }

    fn process_dispute_webhook(
        &self,
        request: RequestDetails,
        connector_webhook_secret: Option<domain_types::connector_types::ConnectorWebhookSecrets>,
        connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<
        domain_types::connector_types::DisputeWebhookDetailsResponse,
        error_stack::Report<WebhookError>,
    > {
        // Old callers get the actionable missing-context error, never a claim
        // that the implemented dispute flow is unimplemented.
        self.process_dispute_webhook_with_context(
            request,
            connector_webhook_secret,
            connector_account_details,
            None,
        )
    }

    fn process_dispute_webhook_with_context(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<domain_types::connector_types::ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        event_context: Option<EventContext>,
    ) -> Result<
        domain_types::connector_types::DisputeWebhookDetailsResponse,
        error_stack::Report<WebhookError>,
    > {
        let details: paypal::PaypalWebhooksBody = request
            .body
            .parse_struct("PaypalDisputeWebhook")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;
        let paypal::PaypalResource::PaypalDisputeWebhooks(dispute) = details.resource else {
            return Err(report!(WebhookError::WebhookResourceObjectNotFound));
        };
        let (seller, _) = paypal_dispute_identity(&dispute)?;
        let context = event_context.ok_or_else(|| {
            report!(WebhookError::WebhookMissingRequiredContext {
                field: "dispute.connector_transaction_id / related_connector_transaction_ids",
                origin: "stored payment attempt"
            })
        })?;
        if context
            .connector_transaction_id
            .as_deref()
            .is_none_or(|id| id.trim().is_empty())
            && context.related_connector_transaction_ids.is_empty()
        {
            return Err(report!(WebhookError::WebhookMissingRequiredContext {
                field: "dispute.connector_transaction_id / related_connector_transaction_ids",
                origin: "stored payment attempt"
            }));
        }
        if context.connector_transaction_id.as_deref() != Some(seller)
            && !context
                .related_connector_transaction_ids
                .iter()
                .any(|id| id == seller)
        {
            return Err(report!(WebhookError::WebhookSourceVerificationFailed).attach_printable("PayPal disputed seller transaction does not belong to the caller's stored payment"));
        }
        let seller = seller.to_owned();
        let status = if dispute.status == paypal::DisputeStatus::Resolved
            || matches!(
                details.event_type,
                paypal::PaypalWebhookEventType::CustomerDisputeResolved
            ) {
            dispute
                .dispute_outcome
                .as_ref()
                .and_then(|outcome| get_paypal_dispute_status_from_outcome(&outcome.outcome_code))
                .ok_or_else(|| {
                    report!(WebhookError::WebhookEventTypeNotFound)
                        .attach_printable("Unsupported or missing PayPal dispute resolution")
                })?
        } else if dispute.status != paypal::DisputeStatus::Other {
            common_enums::DisputeStatus::DisputeOpened
        } else {
            return Err(report!(WebhookError::WebhookEventTypeNotFound));
        };
        let minor = domain_types::utils::convert_back_amount_to_minor_units_for_webhook(
            &common_utils::types::StringMajorUnitForConnector,
            dispute.dispute_amount.value.clone(),
            dispute.dispute_amount.currency_code,
        )?;
        if minor.get_amount_as_i64() < 0 {
            return Err(report!(WebhookError::WebhookAmountConversionFailed {
                reason: "Negative PayPal dispute amount".to_owned()
            }));
        }
        let amount = domain_types::utils::convert_amount_for_webhook(
            &common_utils::types::StringMinorUnitForConnector,
            minor,
            dispute.dispute_amount.currency_code,
        )?;
        let stage = match dispute.dispute_life_cycle_stage {
            paypal::DisputeLifeCycleStage::Inquiry => common_enums::DisputeStage::PreDispute,
            paypal::DisputeLifeCycleStage::Chargeback => common_enums::DisputeStage::Dispute,
            paypal::DisputeLifeCycleStage::PreArbitration
            | paypal::DisputeLifeCycleStage::Arbitration => {
                common_enums::DisputeStage::PreArbitration
            }
        };
        Ok(
            domain_types::connector_types::DisputeWebhookDetailsResponse {
                connector_transaction_id: Some(seller),
                amount,
                currency: dispute.dispute_amount.currency_code,
                dispute_id: paypal_webhook_id(&dispute.dispute_id)?.to_owned(),
                status,
                stage,
                connector_response_reference_id: None,
                dispute_message: dispute.reason.clone(),
                connector_reason_code: dispute.reason,
                raw_connector_response: Some(
                    String::from_utf8(request.body.to_vec())
                        .change_context(WebhookError::WebhookBodyDecodingFailed)?,
                ),
                status_code: 200,
                response_headers: None,
                additional_details: None,
            },
        )
    }

    fn get_webhook_resource_object(
        &self,
        request: RequestDetails,
    ) -> Result<Box<dyn hyperswitch_masking::ErasedMaskSerialize>, error_stack::Report<WebhookError>>
    {
        let details: paypal::PaypalWebhooksBody = request
            .body
            .parse_struct("PaypalWebhookResource")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;
        Ok(Box::new(details))
    }
}

fn paypal_webhook_id(id: &str) -> Result<&str, error_stack::Report<WebhookError>> {
    if id.is_empty()
        || id
            .chars()
            .any(|value| value.is_whitespace() || value.is_control())
    {
        return Err(
            report!(WebhookError::WebhookReferenceIdNotFound).attach_printable(
                "PayPal webhook requires a nonempty unambiguous actual resource ID",
            ),
        );
    }
    Ok(id)
}
fn paypal_webhook_order_id(
    resource: &paypal::PaypalCardWebhooks,
) -> Result<&str, error_stack::Report<WebhookError>> {
    paypal_webhook_id(&resource.id)?;
    let order = resource.supplementary_data.as_ref().ok_or_else(|| {
        report!(WebhookError::WebhookReferenceIdNotFound).attach_printable(
            "PayPal payment webhook requires supplementary_data.related_ids.order_id",
        )
    })?;
    paypal_webhook_id(&order.related_ids.order_id)
}
fn paypal_dispute_identity(
    dispute: &paypal::PaypalDisputeWebhooks,
) -> Result<(&str, Option<&str>), error_stack::Report<WebhookError>> {
    paypal_webhook_id(&dispute.dispute_id)?;
    let first = dispute
        .disputed_transactions
        .first()
        .ok_or_else(|| report!(WebhookError::WebhookReferenceIdNotFound))?;
    let seller = paypal_webhook_id(&first.seller_transaction_id)?;
    if dispute.disputed_transactions.iter().any(|transaction| {
        transaction.seller_transaction_id != seller
            || transaction.invoice_number != first.invoice_number
    }) {
        return Err(
            report!(WebhookError::WebhookReferenceIdNotFound).attach_printable(
                "Ambiguous PayPal disputed seller transactions or invoice references",
            ),
        );
    }
    let invoice = first
        .invoice_number
        .as_deref()
        .map(paypal_webhook_id)
        .transpose()?;
    Ok((seller, invoice))
}
fn paypal_webhook_money(
    amount: Option<paypal::OrderAmount>,
) -> Result<Option<common_utils::types::Money>, error_stack::Report<WebhookError>> {
    amount
        .map(|amount| {
            let minor = domain_types::utils::convert_back_amount_to_minor_units_for_webhook(
                &common_utils::types::StringMajorUnitForConnector,
                amount.value,
                amount.currency_code,
            )?;
            if minor.get_amount_as_i64() < 0 {
                return Err(report!(WebhookError::WebhookAmountConversionFailed {
                    reason: "Negative actual PayPal webhook amount".to_owned()
                }));
            }
            Ok(common_utils::types::Money {
                amount: minor,
                currency: amount.currency_code,
            })
        })
        .transpose()
}
fn get_paypal_event_type(
    event: paypal::PaypalWebhookEventType,
    outcome: Option<paypal::OutcomeCode>,
) -> domain_types::connector_types::EventType {
    use domain_types::connector_types::EventType as R;
    use paypal::PaypalWebhookEventType as E;
    match event {
        E::PaymentCaptureCompleted | E::CheckoutOrderCompleted => R::PaymentIntentSuccess,
        E::PaymentCapturePending | E::CheckoutOrderProcessed => R::PaymentIntentProcessing,
        E::PaymentCaptureDeclined | E::PaymentCaptureDenied | E::PaymentCaptureReversed => {
            R::PaymentIntentFailure
        }
        E::PaymentAuthorizationCreated => R::PaymentIntentAuthorizationSuccess,
        E::PaymentAuthorizationVoided => R::PaymentIntentProcessing,
        E::PaymentCaptureRefunded => R::RefundSuccess,
        E::PaymentRefundPending => R::RefundProcessing,
        E::PaymentRefundFailed => R::RefundFailure,
        E::CustomerDisputeCreated | E::CustomerDisputedUpdated => R::DisputeOpened,
        E::CustomerDisputeResolved => {
            match outcome.and_then(|value| get_paypal_dispute_status_from_outcome(&value)) {
                Some(common_enums::DisputeStatus::DisputeLost) => R::DisputeLost,
                Some(common_enums::DisputeStatus::DisputeWon) => R::DisputeWon,
                Some(common_enums::DisputeStatus::DisputeCancelled) => R::DisputeCancelled,
                Some(common_enums::DisputeStatus::DisputeAccepted) => R::DisputeAccepted,
                _ => R::IncomingWebhookEventUnspecified,
            }
        }
        E::CheckoutOrderApproved | E::RiskDisputeCreated | E::Unknown => {
            R::IncomingWebhookEventUnspecified
        }
    }
}
fn get_paypal_payment_webhook_status(
    event: paypal::PaypalWebhookEventType,
) -> Result<AttemptStatus, error_stack::Report<WebhookError>> {
    use paypal::PaypalWebhookEventType as E;
    Ok(match event {
        E::PaymentCaptureCompleted => AttemptStatus::Charged,
        E::PaymentCapturePending | E::CheckoutOrderProcessed => AttemptStatus::Pending,
        E::PaymentCaptureDeclined | E::PaymentCaptureDenied => AttemptStatus::Failure,
        E::PaymentAuthorizationCreated => AttemptStatus::Authorized,
        // An authorization closure is not evidence of the whole payment's
        // captured amount. The caller must reconcile the canonical order.
        E::PaymentAuthorizationVoided => AttemptStatus::Pending,
        E::PaymentCaptureReversed => AttemptStatus::Unresolved,
        _ => return Err(report!(WebhookError::WebhookEventTypeNotFound)),
    })
}
fn paypal_check_webhook_resource_status(
    event: paypal::PaypalWebhookEventType,
    status: Option<&str>,
) -> Result<(), error_stack::Report<WebhookError>> {
    use paypal::PaypalWebhookEventType as E;
    let expected = match event {
        E::PaymentCaptureCompleted => "COMPLETED",
        E::PaymentCapturePending => "PENDING",
        E::PaymentCaptureDeclined => "DECLINED",
        E::PaymentCaptureDenied => "DENIED",
        E::PaymentCaptureReversed => "REVERSED",
        E::PaymentAuthorizationCreated => "CREATED",
        E::PaymentAuthorizationVoided => "VOIDED",
        _ => return Err(report!(WebhookError::WebhookEventTypeNotFound)),
    };
    if status.is_some_and(|status| status != expected)
        || (event == E::PaymentAuthorizationVoided && status.is_none())
    {
        return Err(report!(WebhookError::WebhookProcessingFailed)
            .attach_printable("PayPal payment webhook event and resource status disagree"));
    }
    Ok(())
}
fn get_paypal_dispute_status_from_outcome(
    outcome: &paypal::OutcomeCode,
) -> Option<common_enums::DisputeStatus> {
    use common_enums::DisputeStatus as S;
    use paypal::OutcomeCode as O;
    match outcome {
        O::ResolvedBuyerFavour => Some(S::DisputeLost),
        O::ResolvedSellerFavour => Some(S::DisputeWon),
        O::CanceledByBuyer => Some(S::DisputeCancelled),
        O::ACCEPTED => Some(S::DisputeAccepted),
        O::DENIED | O::NONE | O::ResolvedWithPayout | O::Unknown => None,
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Paypal<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Paypal<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Paypal<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentOrderCreate for Paypal<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerAuthentication for Paypal<T>
{
}
macros::create_all_prerequisites!(
    connector_name: Paypal,
    generic_type: T,
    api: [
        (flow: Authenticate, request_body: PaypalAuthenticateRequest<T>, response_body: PaypalAuthResponse, router_data: RouterDataV2<Authenticate, PaymentFlowData, PaymentsAuthenticateData<T>, PaymentsResponseData>,),
        (flow: PostAuthenticate, request_body: PaypalEmptyRequest, response_body: PaypalPostAuthenticateResponse, router_data: RouterDataV2<PostAuthenticate, PaymentFlowData, PaymentsPostAuthenticateData<T>, PaymentsResponseData>,),

        (
            flow: CreateOrder,
            request_body: PaypalOrderCreateRequest,
            response_body: PaypalOrderCreateResponse,
            router_data: RouterDataV2<CreateOrder, PaymentFlowData, PaymentCreateOrderData, PaymentCreateOrderResponse>,
        ),
        (
            flow: Authorize,
            request_body: PaypalAuthorizeRequest<T>,
            response_body: PaypalAuthorizeResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: PaypalSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: PaypalPaymentsCaptureRequest,
            response_body: PaypalCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: PaypalVoidRequest,
            response_body: PaypalPaymentsCancelResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: ServerAuthenticationToken,
            request_body: PaypalAuthUpdateRequest,
            response_body: PaypalAuthUpdateResponse,
            router_data: RouterDataV2<ServerAuthenticationToken, MerchantAuthenticationFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>,
        ),
        (
            flow: Refund,
            request_body: PaypalRefundRequest,
            response_body: RefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            response_body: RefundSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ),
        (
            flow: SetupMandate,
            request_body: PaypalSetupMandateRequest,
            response_body: PaypalSetupMandateResponse,
            router_data: RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ),
        (
            flow: RepeatPayment,
            request_body: PaypalRepeatPaymentRequest<T>,
            response_body: PaypalRepeatPaymentResponse,
            router_data: RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ),
        (
            flow: ClientAuthenticationToken,
            request_body: PaypalClientAuthTokenRequest,
            response_body: PaypalClientAuthTokenResponse,
            router_data: RouterDataV2<ClientAuthenticationToken, MerchantAuthenticationFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: StringMajorUnit
    ],
    member_functions: {
        pub fn preprocess_response_bytes(
            &self,
            _req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            response_bytes: Bytes,
            status_code: u16,
        ) -> CustomResult<Bytes, ConnectorError> {
            // HTTP204 is the actual void outcome. This adapter supplies only empty
            // JSON for the macro deserializer, never an invented vendor status.
            if status_code == 204 { Ok(Bytes::from_static(b"{}")) } else { Ok(response_bytes) }
        }

        pub fn build_headers(
            &self,
            access_token: &str,
            connector_request_reference_id: &str,
            connector_config: &ConnectorSpecificConfig,
            connector_metadata: Option<&serde_json::Value>,
            endpoint: &str,
            mutating: bool,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let auth = paypal::PaypalAuthType::try_from(connector_config)?;
            let mut headers = vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    self.common_get_content_type().to_string().into(),
                ),
                (
                    headers::AUTHORIZATION.to_string(),
                    format!("Bearer {access_token}").into_masked(),
                ),
                (
                    auth_headers::PREFER.to_string(),
                    "return=representation".to_string().into(),
                ),

            ];
            if mutating {
                use sha2::Digest;
                // Endpoint and operation reference are length-delimited before hashing: retries
                // of one stage reuse the key, while capture/authorize/vault stages cannot collide.
                let preimage = format!("{}:{}{}:{}", connector_request_reference_id.len(),
                    connector_request_reference_id, endpoint.len(), endpoint);
                let key = hex::encode(sha2::Sha256::digest(preimage.as_bytes()));
                headers.push((auth_headers::PAYPAL_REQUEST_ID.to_owned(), key[..38].to_owned().into_masked()));
            }
            if let Ok(paypal::PaypalConnectorCredentials::PartnerIntegration(credentials)) =
                auth.get_credentials()
            {
                let auth_assertion_header =
                    construct_auth_assertion_header(&credentials.payer_id, &credentials.client_id);

                let partner_attribution_id = connector_metadata
                    .and_then(|metadata| metadata.get("paypal_partner_attribution_id"))
                    .and_then(|value| value.as_str())
                    .unwrap_or(paypal_constants::DEFAULT_PARTNER_ATTRIBUTION_ID);

                headers.extend(vec![
                    (
                        auth_headers::PAYPAL_AUTH_ASSERTION.to_string(),
                        auth_assertion_header.to_string().into_masked(),
                    ),
                    (
                        auth_headers::PAYPAL_PARTNER_ATTRIBUTION_ID.to_string(),
                        partner_attribution_id.to_string().into(),
                    ),
                ])
            } else {
                let legacy_attribution_id = connector_metadata
                    .and_then(|metadata| metadata.get("paypal_legacy_partner_attribution_id"))
                    .and_then(|value| value.as_str())
                    .unwrap_or(paypal_constants::DEFAULT_LEGACY_PARTNER_ATTRIBUTION_ID);

                headers.extend(vec![(
                    auth_headers::PAYPAL_PARTNER_ATTRIBUTION_ID.to_string(),
                    legacy_attribution_id.to_string().into(),
                )])
            }
            Ok(headers)
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.paypal.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.paypal.base_url
        }

        pub fn connector_base_url_merchant_auth<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, MerchantAuthenticationFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.paypal.base_url
        }

     pub fn get_order_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        //Handled error response separately for Orders as the end point is different for Orders - (Authorize) and Payments - (Capture, void, refund, rsync).
        //Error response have different fields for Orders and Payments.
        let response: paypal::PaypalOrderErrorResponse = res
            .response
            .parse_struct("Paypal ErrorResponse")
            .change_context(utils::response_handling_fail_for_connector(res.status_code, "paypal"))
            ?;

        with_error_response_body!(event_builder, response);

        let typed = macros::serialize_typed_connector_payload(&response, "typed_connector_response");
        let error_reason = response.details.clone().map(|order_errors| {
            order_errors
                .iter()
                .map(|error| {
                    paypal::order_error_context(error)
                })
                .collect::<String>()
        });
        let attempt_status = response.details.as_ref().and_then(|details| {
            details.iter().find_map(|detail| match detail.issue.as_str() {
                "ORDER_ALREADY_CAPTURED" => Some(FlowStatus::Payment(AttemptStatus::Charged)),
                "ORDER_ALREADY_AUTHORIZED" => Some(FlowStatus::Payment(AttemptStatus::Authorized)),
                "INSTRUMENT_DECLINED" | "PAYMENT_DENIED" | "PAYER_CANNOT_PAY" => Some(FlowStatus::Payment(AttemptStatus::Failure)),
                _ => None,
            })
        });
        let errors_list = response.details.unwrap_or_default();
        let option_error_code_message =
            utils::get_error_code_error_message_based_on_priority(
                self.clone(),
                errors_list
                    .into_iter()
                    .map(|errors| errors.into())
                    .collect(),
            );
        Ok(ErrorResponse {
            status_code: res.status_code,
            code: option_error_code_message
                .clone()
                .map(|error_code_message| error_code_message.error_code)
                .unwrap_or(NO_ERROR_CODE.to_string()),
            message: option_error_code_message
                .map(|error_code_message| error_code_message.error_message)
                .unwrap_or(NO_ERROR_MESSAGE.to_string()),
            reason: error_reason.or(Some(response.message)),
            attempt_status: attempt_status.or(Some(FlowStatus::Payment(AttemptStatus::Pending))),
            connector_transaction_id: None,
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
            typed_connector_response: typed,
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
        })
    }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Paypal,
    curl_request: Json(PaypalAuthorizeRequest<T>),
    curl_response: PaypalAuthorizeResponse,
    flow_name: Authorize,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsAuthorizeData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
    fn get_headers(
        &self,
        req: &RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let access_token = req.resource_common_data.access_token.clone().ok_or(
            IntegrationError::FailedToObtainAuthType {
                context: IntegrationErrorContext { additional_context: Some("Authorize requires the preceding OAuth access token".to_owned()), ..Default::default() },
            },
        )?;
        let connector_metadata = req
            .resource_common_data
            .connector_feature_data
            .as_ref()
            .map(|secret| secret.clone().expose());
        let original_setup_reference=paypal::paypal_authorize_meta(req)?.filter(|metadata|metadata.continuation.as_ref().is_some_and(|continuation|continuation.operation==paypal::PaypalContinuationOperation::SetupRecurring)).and_then(|metadata|metadata.reference_id);
        self.build_headers(
            &access_token.access_token.expose(),
            original_setup_reference.as_deref().unwrap_or(&req.resource_common_data.connector_request_reference_id),
            &req.connector_config,
            connector_metadata.as_ref(),
            &self.get_url(req)?,
            true,
        )
    }

    fn get_url(
        &self,
        req: &RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        // Determine the action based on capture method
        let action = if req.request.is_auto_capture() {
            "capture"
        } else {
            "authorize"
        };

        let base = self.connector_base_url_payments(req);

        let metadata = paypal::paypal_authorize_meta(req)?;
        let continuation = metadata.as_ref().and_then(|metadata| metadata.continuation.as_ref());
        let path = if let Some(continuation) = continuation {
            match continuation.stage {
                paypal::PaypalContinuationStage::SetupApproved => "v3/vault/payment-tokens".to_owned(),
                paypal::PaypalContinuationStage::PaymentTokenCreated => "v2/checkout/orders".to_owned(),
                paypal::PaypalContinuationStage::OrderApproved => {
                    let original = metadata.as_ref().ok_or(IntegrationError::MissingRequiredField {
                        field_name: "connector_feature_data", context: IntegrationErrorContext { additional_context: Some("Retain original approved order".to_owned()), ..Default::default() },
                    })?;
                    let order = original.order_id.as_ref().filter(|id| !id.is_empty()).ok_or(IntegrationError::MissingRequiredField {
                        field_name: "connector_order_id", context: IntegrationErrorContext { additional_context: Some("Order approval must identify the original order".to_owned()), ..Default::default() },
                    })?;
                    if matches!(req.request.payment_method_data, PaymentMethodData::Card(_) | PaymentMethodData::CardWithNoCvc(_)) && original.authentication_result.as_ref().is_none_or(|result| !paypal::accepted_native_authentication(result)) {
                        return Err(IntegrationError::InvalidDataFormat {
                            field_name: "connector_feature_data.authentication_result", context: IntegrationErrorContext { additional_context: Some("PostAuthenticate validation is required before completion".to_owned()), ..Default::default() },
                        }.into());
                    }
                    format!("v2/checkout/orders/{order}/{action}")
                }
                paypal::PaypalContinuationStage::SetupApprovalRequired | paypal::PaypalContinuationStage::OrderApprovalRequired | paypal::PaypalContinuationStage::Completed => {
                    return Err(IntegrationError::InvalidDataFormat {
                        field_name: "connector_feature_data.continuation.stage",
                        context: IntegrationErrorContext {
                            additional_context: Some("Pending approval must be validated; completed payments must be synchronized".to_owned()), ..Default::default()
                        },
                    }.into());
                }
            }
        } else if paypal::is_encrypted_apple(req) {
            "v3/vault/setup-tokens".to_owned()
        } else if let PaymentMethodData::Wallet(WalletData::PaypalSdk(paypal_wallet_data)) =
            &req.request.payment_method_data
        {
            // Case 1: PaypalSdk wallet - complete order using SDK token
            format!("v2/checkout/orders/{}/{}", paypal_wallet_data.token, action)
        } else if let Some(order_id) = &req.resource_common_data.connector_order_id {
            if order_id.is_empty() || metadata.as_ref().and_then(|meta| meta.order_id.as_ref()) != Some(order_id) {
                return Err(IntegrationError::InvalidDataFormat {
                    field_name: "connector_order_id",
                    context: IntegrationErrorContext {
                        additional_context: Some("Completion requires the original PayPal order identity and approved continuation metadata".to_owned()), ..Default::default()
                    },
                }.into());
            }
            let original = metadata.as_ref().ok_or(IntegrationError::InvalidDataFormat {
                field_name: "connector_feature_data",
                context: IntegrationErrorContext { additional_context: Some("Retain original PayPal order metadata".to_owned()), ..Default::default() },
            })?;
            let expected_intent = if req.request.is_auto_capture() { paypal::PaypalPaymentIntent::Capture } else { paypal::PaypalPaymentIntent::Authorize };
            if original.psync_flow != expected_intent {
                return Err(IntegrationError::InvalidDataFormat {
                    field_name: "capture_method",
                    context: IntegrationErrorContext { additional_context: Some("Completion must preserve the original order intent".to_owned()), ..Default::default() },
                }.into());
            }
            if req.resource_common_data.auth_type == common_enums::AuthenticationType::ThreeDs
                && matches!(req.request.payment_method_data,PaymentMethodData::Card(_) | PaymentMethodData::CardWithNoCvc(_))
                && !original.authentication_result.as_ref().is_some_and(paypal::accepted_native_authentication) {
                return Err(IntegrationError::InvalidDataFormat {
                    field_name: "connector_feature_data.authentication_result",
                    context: IntegrationErrorContext { additional_context: Some("Native completion requires a corroborated accepted PostAuthenticate outcome".to_owned()), ..Default::default() },
                }.into());
            }
            format!("v2/checkout/orders/{order_id}/{action}")
        } else {
            // Case 3: Creating new order
            "v2/checkout/orders".to_owned()
        };

        Ok(format!("{base}{path}"))
    }

    fn next_http_stage(
        &self,
        data: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    ) -> Option<String> {
        if data.resource_common_data.status != AttemptStatus::Pending { return None; }
        if !matches!(&data.response, Ok(PaymentsResponseData::TransactionResponse { redirection_data: None, .. })) { return None; }
        let metadata = paypal::paypal_authorize_meta(data).ok().flatten()?;
        let continuation = metadata.continuation?;
        match continuation.stage {
            paypal::PaypalContinuationStage::SetupApproved => continuation.setup_token_id.map(|id| format!("exchange:{}", id.expose())),
            paypal::PaypalContinuationStage::PaymentTokenCreated if continuation.operation == paypal::PaypalContinuationOperation::Authorize =>
                continuation.payment_token_id.map(|id| format!("order:{}", id.expose())),
            paypal::PaypalContinuationStage::SetupApprovalRequired | paypal::PaypalContinuationStage::PaymentTokenCreated | paypal::PaypalContinuationStage::OrderApprovalRequired | paypal::PaypalContinuationStage::OrderApproved | paypal::PaypalContinuationStage::Completed => None,
        }
    }
    fn get_error_response_v2(
        &self, res: Response, event_builder: Option<&mut events::Event>, _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.get_order_error_response(res, event_builder)
    }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Paypal,
    curl_request: FormUrlEncoded(PaypalAuthUpdateRequest),
    curl_response: PaypalAuthUpdateResponse,
    flow_name: ServerAuthenticationToken,
    resource_common_data: MerchantAuthenticationFlowData,
    flow_request: ServerAuthenticationTokenRequestData,
    flow_response: ServerAuthenticationTokenResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<ServerAuthenticationToken, MerchantAuthenticationFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let auth = paypal::PaypalAuthType::try_from(&req.connector_config)?;
            let credentials = auth.get_credentials()?;
            let auth_val = credentials.generate_authorization_value();

            Ok(vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    "application/x-www-form-urlencoded".to_string().into(),
                ),
                (headers::AUTHORIZATION.to_string(), auth_val.into_masked()),
            ])
        }
        fn get_url(
            &self,
            req: &RouterDataV2<ServerAuthenticationToken, MerchantAuthenticationFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}v1/oauth2/token", self.connector_base_url_merchant_auth(req)))
        }
        fn get_error_response_v2(
                &self,
                res: Response,
                event_builder: Option<&mut events::Event>,
                _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
                let response: paypal::PaypalAccessTokenErrorResponse = res
                .response
                .parse_struct("Paypal AccessTokenErrorResponse")
                .change_context(utils::response_handling_fail_for_connector(res.status_code, "paypal"))?;

        with_error_response_body!(event_builder, response);

        let typed = macros::serialize_typed_connector_payload(&response, "typed_connector_response");
        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.error.clone(),
            message: response.error.clone(),
            reason: Some(response.error_description),
            attempt_status: None,
            connector_transaction_id: None,
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
            typed_connector_response: typed,
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
        })
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
    for Paypal<T>
{
    fn get_http_method(&self) -> common_utils::request::Method {
        common_utils::request::Method::Get
    }
    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder, connector_config)
    }
    fn handle_response_v2(
        &self,
        data: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ConnectorError,
    > {
        use super::macros::BridgeRequestResponse;
        let metadata = paypal::sync_metadata(data).change_context(
            utils::response_handling_fail_for_connector(res.status_code, "paypal"),
        )?;
        let target = paypal::sync_target(data, &metadata).change_context(
            utils::response_handling_fail_for_connector(res.status_code, "paypal"),
        )?;
        let vault = match target {
            paypal::PaypalSyncTarget::SetupToken(_) => {
                Some(paypal::PaypalVaultSyncResponse::Setup(
                    res.response
                        .parse_struct::<paypal::PaypalSetupTokenSyncResponse>(
                            "PaypalSetupTokenSyncResponse",
                        )
                        .change_context(utils::response_handling_fail_for_connector(
                            res.status_code,
                            "paypal",
                        ))?,
                ))
            }
            paypal::PaypalSyncTarget::PaymentToken(_) => {
                Some(paypal::PaypalVaultSyncResponse::Payment(
                    res.response
                        .parse_struct::<paypal::PaypalPaymentTokenSyncResponse>(
                            "PaypalPaymentTokenSyncResponse",
                        )
                        .change_context(utils::response_handling_fail_for_connector(
                            res.status_code,
                            "paypal",
                        ))?,
                ))
            }
            _ => None,
        };
        if let Some(response) = vault {
            tracing::info!(response=?response, "response from connector");
            finalize_connector_response!(event_builder, response, data, res.status_code)
        } else {
            let response = self.p_sync.response(res.response, res.status_code)?;
            tracing::info!(response=?response, "response from connector");
            finalize_connector_response!(event_builder, response, data, res.status_code)
        }
    }
    fn get_headers(
        &self,
        req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let access_token = req.resource_common_data.access_token.clone().ok_or(
            IntegrationError::FailedToObtainAuthType {
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "PSync requires the preceding OAuth access token".to_owned(),
                    ),
                    ..Default::default()
                },
            },
        )?;
        let connector_metadata = req
            .resource_common_data
            .connector_feature_data
            .as_ref()
            .map(|secret| secret.clone().expose());
        self.build_headers(
            &access_token.access_token.expose(),
            &req.resource_common_data.connector_request_reference_id,
            &req.connector_config,
            connector_metadata.as_ref(),
            &self.get_url(req)?,
            false,
        )
    }
    fn get_url(
        &self,
        req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
    ) -> CustomResult<String, IntegrationError> {
        let metadata = paypal::sync_metadata(req)?;
        let target = paypal::sync_target(req, &metadata)?;
        let endpoint = format!("{}{}", self.connector_base_url_payments(req), target.path());
        let parsed =
            url::Url::parse(&endpoint).map_err(|_| IntegrationError::InvalidDataFormat {
                field_name: "paypal.sync_url",
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "PSync requires a valid unfiltered resource URL".to_owned(),
                    ),
                    ..Default::default()
                },
            })?;
        // C25 requires a full unfiltered Orders GET. Configuration must not
        // smuggle fields/filter/pagination into the otherwise fixed target.
        if parsed.query().is_some() || parsed.fragment().is_some() {
            return Err(IntegrationError::InvalidDataFormat {
                field_name: "paypal.sync_url",
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "PSync requires a valid unfiltered resource URL".to_owned(),
                    ),
                    ..Default::default()
                },
            }
            .into());
        }
        Ok(endpoint)
    }
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Paypal,
    curl_request: Json(PaypalPaymentsCaptureRequest),
    curl_response: PaypalCaptureResponse,
    flow_name: Capture,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsCaptureData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_error_response_v2(&self, res: Response, event_builder: Option<&mut events::Event>, connector_config: &ConnectorSpecificConfig) -> CustomResult<ErrorResponse, ConnectorError> {
            let mut error = self.build_error_response(res, event_builder, connector_config)?;
            // An already-done error is not proof of collected money. PSync must corroborate
            // the actual capture; explicit Pending prevents the caller defaulting to Failure.
            let status = match error.code.as_str() {
                "INSTRUMENT_DECLINED" | "CARD_EXPIRED" | "AUTHORIZATION_EXPIRED" | "MAX_CAPTURE_AMOUNT_EXCEEDED" | "CAPTURE_AMOUNT_EXCEEDED" => AttemptStatus::CaptureFailed,
                _ => AttemptStatus::Pending,
            };
            error.attempt_status = Some(FlowStatus::Payment(status));
            Ok(error)
        }
        fn get_headers(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let access_token = req.resource_common_data
                .access_token
                .clone()
                .ok_or(IntegrationError::FailedToObtainAuthType { context: Default::default() })?;
            let connector_metadata = req.resource_common_data.connector_feature_data
                .as_ref()
                .map(|secret| secret.clone().expose());
            let metadata = paypal::capture_metadata(req)?;
            let saved_mode = metadata.capture_context.as_ref().and_then(|context| context.capture_method);
            let operation_reference = if req.request.capture_method.or(saved_mode) == Some(common_enums::CaptureMethod::ManualMultiple)
                || req.request.multiple_capture_data.is_some()
            {
                req.request.multiple_capture_data.as_ref()
                    .map(|data| data.capture_reference.as_str())
                    .filter(|reference| !reference.trim().is_empty())
                    .ok_or_else(|| IntegrationError::MissingRequiredField {
                        field_name: "multiple_capture_data.capture_reference",
                        context: IntegrationErrorContext {
                            additional_context: Some("Each partial capture requires its own persisted operation reference".to_owned()),
                            suggested_action: Some("Provide a unique capture_reference for each capture and reuse it unchanged when retrying that capture".to_owned()),
                            doc_url: Some("https://developer.paypal.com/api/rest/reference/idempotency/".to_owned()),
                        },
                    })?
            } else {
                &req.resource_common_data.connector_request_reference_id
            };
            self.build_headers(
                &access_token.access_token.expose(),
                operation_reference,
                &req.connector_config,
                connector_metadata.as_ref(),
            &self.get_url(req)?,
            true,
            )
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let paypal_meta = paypal::capture_metadata(req)?;
            let authorize_id = paypal_meta.authorize_id.ok_or_else(|| IntegrationError::MissingRequiredField {field_name: "connector_feature_data.authorize_id",context: IntegrationErrorContext {additional_context:Some("Capture requires original authorization identity".to_owned()),..Default::default()}})?;
            Ok(format!(
                "{}v2/payments/authorizations/{}/capture",
                self.connector_base_url_payments(req),
                authorize_id
            ))
            }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Paypal,
    curl_request: Json(PaypalVoidRequest),
    curl_response: PaypalPaymentsCancelResponse,
    flow_name: Void,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentVoidData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_error_response_v2(&self, res: Response, event_builder: Option<&mut events::Event>, connector_config: &ConnectorSpecificConfig) -> CustomResult<ErrorResponse, ConnectorError> {
            let mut error = self.build_error_response(res, event_builder, connector_config)?;
            // No request context is available here to corroborate an already-done
            // state. Explicit Pending requires PSync and avoids the HS Failure default.
            let status = match error.code.as_str() {
                "AUTHORIZATION_ALREADY_VOIDED" | "AUTHORIZATION_VOIDED" | "AUTHORIZATION_ALREADY_CAPTURED" | "AUTHORIZATION_FULLY_CAPTURED" | "PREVIOUS_REQUEST_IN_PROGRESS" => AttemptStatus::Pending,
                _ if error.status_code >= 500 => AttemptStatus::Pending,
                _ => AttemptStatus::VoidFailed,
            };
            error.attempt_status = Some(FlowStatus::Payment(status));
            Ok(error)
        }
        fn get_headers(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let access_token = req.resource_common_data
                .access_token
                .clone()
                .ok_or(IntegrationError::FailedToObtainAuthType { context: IntegrationErrorContext {additional_context:Some("Void requires a valid PayPal OAuth access token".to_owned()),suggested_action:Some("Acquire a server authentication token before voiding".to_owned()),doc_url:Some("https://developer.paypal.com/api/rest/authentication/".to_owned())} })?;
            let connector_metadata = req.resource_common_data.connector_feature_data
                .as_ref()
                .map(|secret| secret.clone().expose());
            self.build_headers(
                &access_token.access_token.expose(),
                &req.resource_common_data.connector_request_reference_id,
                &req.connector_config,
                connector_metadata.as_ref(),
            &self.get_url(req)?,
            true,
            )
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let paypal_meta = paypal::void_metadata(req)?;
            let authorize_id = paypal_meta.authorize_id.ok_or_else(||IntegrationError::MissingConnectorTransactionID {context:IntegrationErrorContext {additional_context:Some("Void requires original authorization identity".to_owned()),..Default::default()}})?;
            Ok(format!(
                "{}v2/payments/authorizations/{}/void",
                self.connector_base_url_payments(req),
                authorize_id,
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Paypal,
    curl_request: Json(PaypalRefundRequest),
    curl_response: RefundResponse,
    flow_name: Refund,
    resource_common_data: RefundFlowData,
    flow_request: RefundsData,
    flow_response: RefundsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_error_response_v2(&self,res:Response,event_builder:Option<&mut events::Event>,connector_config:&ConnectorSpecificConfig) -> CustomResult<ErrorResponse,ConnectorError> {
            let mut error=self.build_error_response(res,event_builder,connector_config)?;
            let status=match error.code.as_str() {
                "REFUND_ALREADY_PROCESSED" | "CAPTURE_FULLY_REFUNDED" | "PREVIOUS_REQUEST_IN_PROGRESS" | "DUPLICATE_REQUEST_ID" => common_enums::RefundStatus::Pending,
                _ if error.status_code>=500 || error.status_code==409 => common_enums::RefundStatus::Pending,
                _ => common_enums::RefundStatus::Failure,
            };
            error.attempt_status=Some(FlowStatus::Refund(status));
            Ok(error)
        }

        fn get_headers(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let access_token = req.resource_common_data
                .access_token
                .clone()
                .ok_or(IntegrationError::FailedToObtainAuthType { context: IntegrationErrorContext {additional_context:Some("Refund requires a valid PayPal OAuth access token".to_owned()),suggested_action:Some("Acquire server authentication before refund".to_owned()),doc_url:Some("https://developer.paypal.com/api/rest/authentication/".to_owned())} })?;
            let connector_metadata = req.resource_common_data.connector_feature_data
                .as_ref()
                .map(|secret| secret.clone().expose());
            self.build_headers(
                &access_token.access_token.expose(),
                &req.request.refund_id,
                &req.connector_config,
                connector_metadata.as_ref(),
            &self.get_url(req)?,
            true,
            )
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let paypal_meta=paypal::refund_metadata(req)?;
            let capture_id=paypal_meta.capture_id.ok_or_else(||IntegrationError::MissingRequiredField {field_name:"connector_feature_data.capture_id",context:IntegrationErrorContext {additional_context:Some("Refund requires the actual capture resource ID".to_owned()),..Default::default()}})?;
            Ok(format!(
                "{}v2/payments/captures/{}/refund",
                self.connector_base_url_refunds(req),
                capture_id,
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Paypal,
    curl_response: RefundSyncResponse,
    flow_name: RSync,
    resource_common_data: RefundFlowData,
    flow_request: RefundSyncData,
    flow_response: RefundsResponseData,
    http_method: Get,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_error_response_v2(&self,res:Response,event_builder:Option<&mut events::Event>,connector_config:&ConnectorSpecificConfig) -> CustomResult<ErrorResponse,ConnectorError> {
            let mut error=self.build_error_response(res,event_builder,connector_config)?;
            // Retrieval/authentication errors do not prove that the refund failed.
            error.attempt_status=Some(FlowStatus::Refund(common_enums::RefundStatus::Pending));
            Ok(error)
        }

        fn get_headers(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let access_token = req.resource_common_data
                .access_token
                .clone()
                .ok_or(IntegrationError::FailedToObtainAuthType { context: IntegrationErrorContext {additional_context:Some("Refund sync requires a valid PayPal OAuth access token".to_owned()),suggested_action:Some("Acquire server authentication before refund sync".to_owned()),doc_url:Some("https://developer.paypal.com/api/rest/authentication/".to_owned())} })?;
            let connector_metadata = req.resource_common_data.connector_feature_data
                .as_ref()
                .map(|secret| secret.clone().expose());
            self.build_headers(
                &access_token.access_token.expose(),
                &req.resource_common_data.connector_request_reference_id,
                &req.connector_config,
                connector_metadata.as_ref(),
            &self.get_url(req)?,
            false,
            )
        }
        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}v2/payments/refunds/{}",
                self.connector_base_url_refunds(req),
                paypal::rsync_refund_id(req)?
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Paypal,
    curl_request: Json(PaypalSetupMandateRequest),
    curl_response: PaypalSetupMandateResponse,
    flow_name: SetupMandate,
    resource_common_data: PaymentFlowData,
    flow_request: SetupMandateRequestData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn next_http_stage(&self,data:&RouterDataV2<SetupMandate,PaymentFlowData,SetupMandateRequestData<T>,PaymentsResponseData>) -> Option<String> {
            if data.resource_common_data.status!=AttemptStatus::Pending || !matches!(&data.response,Ok(PaymentsResponseData::TransactionResponse {redirection_data:None,..})) {return None;}
            let metadata=paypal::setup_mandate_metadata(data).ok().flatten()?;
            let continuation=metadata.continuation?;
            if continuation.stage!=paypal::PaypalContinuationStage::SetupApproved {return None;}
            continuation.setup_token_id.map(|id|format!("exchange:{}",id.expose()))
        }

        fn get_headers(
            &self,
            req: &RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let access_token = req.resource_common_data
                .access_token
                .clone()
                .ok_or(IntegrationError::FailedToObtainAuthType { context:IntegrationErrorContext {additional_context:Some("Vault setup requires a valid PayPal OAuth token".to_owned()),doc_url:Some("https://developer.paypal.com/api/rest/authentication/".to_owned()),..Default::default()} })?;
            let connector_metadata = req.resource_common_data.connector_feature_data
                .as_ref()
                .map(|secret| secret.clone().expose());
            let original_reference=paypal::setup_mandate_metadata(req)?.and_then(|metadata|metadata.reference_id);
            self.build_headers(
                &access_token.access_token.expose(),
                original_reference.as_deref().unwrap_or(&req.resource_common_data.connector_request_reference_id),
                &req.connector_config,
                connector_metadata.as_ref(),
            &self.get_url(req)?,
            true,
            )
        }
        fn get_url(
            &self,
            req: &RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let metadata=paypal::setup_mandate_metadata(req)?;
            let exchange=metadata.as_ref().and_then(|metadata|metadata.continuation.as_ref());
            let path=if let Some(continuation)=exchange {
                if continuation.stage!=paypal::PaypalContinuationStage::SetupApproved {return Err(IntegrationError::InvalidDataFormat {field_name:"connector_feature_data.continuation.stage",context:IntegrationErrorContext {additional_context:Some("Setup token must be authoritatively approved before exchange, and completed setup must not be repeated".to_owned()),..Default::default()}}.into());}
                "v3/vault/payment-tokens"
            }else if paypal::is_setup_encrypted_apple(req) {"v3/vault/setup-tokens"}else{"v3/vault/payment-tokens"};
            Ok(format!("{}{path}",self.connector_base_url_payments(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Paypal,
    curl_request: Json(PaypalRepeatPaymentRequest<T>),
    curl_response: PaypalRepeatPaymentResponse,
    flow_name: RepeatPayment,
    resource_common_data: PaymentFlowData,
    flow_request: RepeatPaymentData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let access_token = req.resource_common_data
                .access_token
                .clone()
                .ok_or(IntegrationError::FailedToObtainAuthType { context: Default::default() })?;
            let connector_metadata = req.resource_common_data.connector_feature_data
                .as_ref()
                .map(|secret| secret.clone().expose());
            self.build_headers(
                &access_token.access_token.expose(),
                &req.resource_common_data.connector_request_reference_id,
                &req.connector_config,
                connector_metadata.as_ref(),
            &self.get_url(req)?,
            true,
            )
        }
        fn get_url(
            &self,
            req: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}v2/checkout/orders", self.connector_base_url_payments(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Paypal,
    curl_request: Json(PaypalOrderCreateRequest),
    curl_response: PaypalOrderCreateResponse,
    flow_name: CreateOrder,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentCreateOrderData,
    flow_response: PaymentCreateOrderResponse,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<CreateOrder, PaymentFlowData, PaymentCreateOrderData, PaymentCreateOrderResponse>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let access_token = req.resource_common_data.access_token.clone().ok_or(
                IntegrationError::FailedToObtainAuthType {
                    context: Default::default(),
                },
            )?;
            let connector_metadata = req
                .resource_common_data
                .connector_feature_data
                .as_ref()
                .map(|secret| secret.clone().expose());
            self.build_headers(
                &access_token.access_token.expose(),
                &req.resource_common_data.connector_request_reference_id,
                &req.connector_config,
                connector_metadata.as_ref(),
            &self.get_url(req)?,
            true,
            )
        }

        fn get_url(
            &self,
            req: &RouterDataV2<CreateOrder, PaymentFlowData, PaymentCreateOrderData, PaymentCreateOrderResponse>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}v2/checkout/orders", self.connector_base_url_payments(req)))
        }

        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
            self.get_order_error_response(res, event_builder)
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Paypal,
    curl_request: Json(PaypalClientAuthTokenRequest),
    curl_response: PaypalClientAuthTokenResponse,
    flow_name: ClientAuthenticationToken,
    resource_common_data: MerchantAuthenticationFlowData,
    flow_request: ClientAuthenticationTokenRequestData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<ClientAuthenticationToken, MerchantAuthenticationFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            // Use Basic auth (client_id:client_secret) for v1/identity/generate-token
            // since the ClientAuthenticationToken gRPC request does not carry a pre-obtained access token.
            let auth = paypal::PaypalAuthType::try_from(&req.connector_config)?;
            let credentials = auth.get_credentials()?;
            let auth_val = credentials.generate_authorization_value();

            Ok(vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    "application/json".to_string().into(),
                ),
                (headers::AUTHORIZATION.to_string(), auth_val.into_masked()),
            ])
        }
        fn get_url(
            &self,
            req: &RouterDataV2<ClientAuthenticationToken, MerchantAuthenticationFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}v1/identity/generate-token", self.connector_base_url_merchant_auth(req)))
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthenticateV2<T> for Paypal<T>
{
}
macros::macro_connector_implementation!(
    connector_default_implementations:[get_content_type],
    connector:Paypal,curl_request:Json(PaypalAuthenticateRequest<T>),curl_response:PaypalAuthResponse,
    flow_name:Authenticate,resource_common_data:PaymentFlowData,flow_request:PaymentsAuthenticateData<T>,flow_response:PaymentsResponseData,http_method:Post,generic_type:T,
    [PaymentMethodDataTypes+Debug+Sync+Send+'static+Serialize],
    other_functions:{
        fn get_error_response_v2(&self,res:Response,event_builder:Option<&mut events::Event>,_connector_config:&ConnectorSpecificConfig)->CustomResult<ErrorResponse,ConnectorError>{
            let mut error=self.get_order_error_response(res,event_builder)?;
            // debug_id is a diagnostic trace, never a payment resource identifier.
            error.connector_transaction_id=None;
            if error.attempt_status.is_none(){error.attempt_status=Some(FlowStatus::Payment(AttemptStatus::Pending));}
            Ok(error)
        }

        fn get_headers(&self,req:&RouterDataV2<Authenticate,PaymentFlowData,PaymentsAuthenticateData<T>,PaymentsResponseData>)->CustomResult<Vec<(String,Maskable<String>)>,IntegrationError>{
            let adapted=paypal::native_authorize_router(req)?;
            let token=adapted.resource_common_data.access_token.as_ref().ok_or(IntegrationError::FailedToObtainAuthType{context:IntegrationErrorContext { additional_context: Some("PayPal native authentication requires OAuth access token from the prerequisite token call".to_owned()), suggested_action: Some("Run CreateServerAuthenticationToken and forward its access token through request state before Authenticate or PostAuthenticate".to_owned()), doc_url: Some("https://developer.paypal.com/api/rest/authentication/".to_owned()) }})?;
            self.build_headers(token.access_token.peek(),&adapted.resource_common_data.connector_request_reference_id,&req.connector_config,adapted.resource_common_data.connector_feature_data.as_ref().map(|v|v.peek()),&self.get_url(req)?,true)
        }
        fn get_url(&self,req:&RouterDataV2<Authenticate,PaymentFlowData,PaymentsAuthenticateData<T>,PaymentsResponseData>)->CustomResult<String,IntegrationError>{
            Ok(format!("{}v2/checkout/orders",self.connector_base_url_payments(req)))
        }
    }
);
macros::macro_connector_implementation!(
    connector_default_implementations:[get_content_type],connector:Paypal,curl_response:PaypalPostAuthenticateResponse,
    flow_name:PostAuthenticate,resource_common_data:PaymentFlowData,flow_request:PaymentsPostAuthenticateData<T>,flow_response:PaymentsResponseData,http_method:Get,generic_type:T,
    [PaymentMethodDataTypes+Debug+Sync+Send+'static+Serialize],
    other_functions:{
        fn get_error_response_v2(&self,res:Response,event_builder:Option<&mut events::Event>,_connector_config:&ConnectorSpecificConfig)->CustomResult<ErrorResponse,ConnectorError>{
            let mut error=self.get_order_error_response(res,event_builder)?;
            // debug_id is a diagnostic trace, never a payment resource identifier.
            error.connector_transaction_id=None;
            if error.attempt_status.is_none(){error.attempt_status=Some(FlowStatus::Payment(AttemptStatus::Pending));}
            Ok(error)
        }

        fn get_headers(&self,req:&RouterDataV2<PostAuthenticate,PaymentFlowData,PaymentsPostAuthenticateData<T>,PaymentsResponseData>)->CustomResult<Vec<(String,Maskable<String>)>,IntegrationError>{
            let token=req.resource_common_data.access_token.as_ref().ok_or(IntegrationError::FailedToObtainAuthType{context:IntegrationErrorContext { additional_context: Some("PayPal native authentication requires OAuth access token from the prerequisite token call".to_owned()), suggested_action: Some("Run CreateServerAuthenticationToken and forward its access token through request state before Authenticate or PostAuthenticate".to_owned()), doc_url: Some("https://developer.paypal.com/api/rest/authentication/".to_owned()) }})?;
            self.build_headers(token.access_token.peek(),&req.resource_common_data.connector_request_reference_id,&req.connector_config,req.resource_common_data.connector_feature_data.as_ref().map(|v|v.peek()),&self.get_url(req)?,false)
        }
        fn get_url(&self,req:&RouterDataV2<PostAuthenticate,PaymentFlowData,PaymentsPostAuthenticateData<T>,PaymentsResponseData>)->CustomResult<String,IntegrationError>{
            Ok(format!("{}{}",self.connector_base_url_payments(req),paypal::post_authenticate_path(req)?))
        }
    }
);

// VerifyWebhookSource implementation using ConnectorIntegrationV2
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        VerifyWebhookSource,
        VerifyWebhookSourceFlowData,
        VerifyWebhookSourceRequestData,
        VerifyWebhookSourceResponseData,
    > for Paypal<T>
{
    fn get_url(
        &self,
        req: &RouterDataV2<
            VerifyWebhookSource,
            VerifyWebhookSourceFlowData,
            VerifyWebhookSourceRequestData,
            VerifyWebhookSourceResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        let base_url = self.base_url(&req.resource_common_data.connectors);
        Ok(format!(
            "{}v1/notifications/verify-webhook-signature",
            base_url
        ))
    }

    fn get_headers(
        &self,
        req: &RouterDataV2<
            VerifyWebhookSource,
            VerifyWebhookSourceFlowData,
            VerifyWebhookSourceRequestData,
            VerifyWebhookSourceResponseData,
        >,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let token = req
            .resource_common_data
            .access_token
            .as_ref()
            .filter(|token| !token.access_token.peek().trim().is_empty())
            .ok_or_else(|| IntegrationError::MissingRequiredField {
                field_name: "access_token",
                context: IntegrationErrorContext {
                    additional_context: Some("Remote PayPal webhook postback verification requires its preceding OAuth access token".to_owned()),
                    suggested_action: Some("Acquire PayPal server authentication before calling the remote webhook verifier".to_owned()),
                    doc_url: Some("https://developer.paypal.com/api/rest/authentication/".to_owned()),
                },
            })?;
        let auth_val = format!("Bearer {}", token.access_token.peek());

        Ok(vec![
            (
                headers::CONTENT_TYPE.to_string(),
                "application/json".to_string().into(),
            ),
            (headers::AUTHORIZATION.to_string(), auth_val.into_masked()),
        ])
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<
            VerifyWebhookSource,
            VerifyWebhookSourceFlowData,
            VerifyWebhookSourceRequestData,
            VerifyWebhookSourceResponseData,
        >,
    ) -> CustomResult<Option<common_utils::request::ConnectorRequestData>, IntegrationError> {
        let verification_request = paypal::PaypalSourceVerificationRequest::try_from(&req.request)?;
        let typed = events::MaskedSerdeValue::from_masked_optional(
            &verification_request,
            "typed_connector_request",
        );
        Ok(Some(common_utils::request::ConnectorRequestData::new(
            common_utils::request::RequestContent::RawBytes(verification_request.to_wire_body()?),
            typed,
        )))
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<
            VerifyWebhookSource,
            VerifyWebhookSourceFlowData,
            VerifyWebhookSourceRequestData,
            VerifyWebhookSourceResponseData,
        >,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<
            VerifyWebhookSource,
            VerifyWebhookSourceFlowData,
            VerifyWebhookSourceRequestData,
            VerifyWebhookSourceResponseData,
        >,
        ConnectorError,
    > {
        let verification_response: paypal::PaypalSourceVerificationResponse = res
            .response
            .parse_struct("PaypalSourceVerificationResponse")
            .change_context(utils::response_handling_fail_for_connector(
                res.status_code,
                "paypal",
            ))?;
        finalize_connector_response!(event_builder, verification_response, data, res.status_code)
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder, _connector_config)
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyWebhookSourceV2 for Paypal<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorErrorTypeMapping for Paypal<T>
{
    fn get_connector_error_type(
        &self,
        error_code: String,
        _error_message: String,
    ) -> ConnectorErrorType {
        match error_code.as_str() {
            "CANNOT_BE_NEGATIVE" => ConnectorErrorType::UserError,
            "CANNOT_BE_ZERO_OR_NEGATIVE" => ConnectorErrorType::UserError,
            "CARD_EXPIRED" => ConnectorErrorType::UserError,
            "DECIMAL_PRECISION" => ConnectorErrorType::UserError,
            "DUPLICATE_INVOICE_ID" => ConnectorErrorType::UserError,
            "INSTRUMENT_DECLINED" => ConnectorErrorType::BusinessError,
            "INTERNAL_SERVER_ERROR" => ConnectorErrorType::TechnicalError,
            "INVALID_ACCOUNT_STATUS" => ConnectorErrorType::BusinessError,
            "INVALID_CURRENCY_CODE" => ConnectorErrorType::UserError,
            "INVALID_PARAMETER_SYNTAX" => ConnectorErrorType::UserError,
            "INVALID_PARAMETER_VALUE" => ConnectorErrorType::UserError,
            "INVALID_RESOURCE_ID" => ConnectorErrorType::UserError,
            "INVALID_STRING_LENGTH" => ConnectorErrorType::UserError,
            "MISSING_REQUIRED_PARAMETER" => ConnectorErrorType::UserError,
            "PAYER_ACCOUNT_LOCKED_OR_CLOSED" => ConnectorErrorType::BusinessError,
            "PAYER_ACCOUNT_RESTRICTED" => ConnectorErrorType::BusinessError,
            "PAYER_CANNOT_PAY" => ConnectorErrorType::BusinessError,
            "PERMISSION_DENIED" => ConnectorErrorType::BusinessError,
            "INVALID_ARRAY_MAX_ITEMS" => ConnectorErrorType::UserError,
            "INVALID_ARRAY_MIN_ITEMS" => ConnectorErrorType::UserError,
            "INVALID_COUNTRY_CODE" => ConnectorErrorType::UserError,
            "NOT_SUPPORTED" => ConnectorErrorType::BusinessError,
            "PAYPAL_REQUEST_ID_REQUIRED" => ConnectorErrorType::UserError,
            "MALFORMED_REQUEST_JSON" => ConnectorErrorType::UserError,
            "PERMISSION_DENIED_FOR_DONATION_ITEMS" => ConnectorErrorType::BusinessError,
            "MALFORMED_REQUEST" => ConnectorErrorType::TechnicalError,
            "AMOUNT_MISMATCH" => ConnectorErrorType::UserError,
            "BILLING_ADDRESS_INVALID" => ConnectorErrorType::UserError,
            "CITY_REQUIRED" => ConnectorErrorType::UserError,
            "DONATION_ITEMS_NOT_SUPPORTED" => ConnectorErrorType::BusinessError,
            "DUPLICATE_REFERENCE_ID" => ConnectorErrorType::UserError,
            "INVALID_PAYER_ID" => ConnectorErrorType::UserError,
            "ITEM_TOTAL_REQUIRED" => ConnectorErrorType::UserError,
            "MAX_VALUE_EXCEEDED" => ConnectorErrorType::UserError,
            "MISSING_PICKUP_ADDRESS" => ConnectorErrorType::UserError,
            "MULTI_CURRENCY_ORDER" => ConnectorErrorType::BusinessError,
            "MULTIPLE_ITEM_CATEGORIES" => ConnectorErrorType::UserError,
            "MULTIPLE_SHIPPING_ADDRESS_NOT_SUPPORTED" => ConnectorErrorType::UserError,
            "MULTIPLE_SHIPPING_TYPE_NOT_SUPPORTED" => ConnectorErrorType::BusinessError,
            "PAYEE_ACCOUNT_INVALID" => ConnectorErrorType::UserError,
            "PAYEE_ACCOUNT_LOCKED_OR_CLOSED" => ConnectorErrorType::UserError,
            "REFERENCE_ID_REQUIRED" => ConnectorErrorType::UserError,
            "PAYMENT_SOURCE_CANNOT_BE_USED" => ConnectorErrorType::BusinessError,
            "PAYMENT_SOURCE_DECLINED_BY_PROCESSOR" => ConnectorErrorType::BusinessError,
            "PAYMENT_SOURCE_INFO_CANNOT_BE_VERIFIED" => ConnectorErrorType::BusinessError,
            "POSTAL_CODE_REQUIRED" => ConnectorErrorType::UserError,
            "SHIPPING_ADDRESS_INVALID" => ConnectorErrorType::UserError,
            "TAX_TOTAL_MISMATCH" => ConnectorErrorType::UserError,
            "TAX_TOTAL_REQUIRED" => ConnectorErrorType::UserError,
            "UNSUPPORTED_INTENT" => ConnectorErrorType::BusinessError,
            "UNSUPPORTED_PAYMENT_INSTRUCTION" => ConnectorErrorType::UserError,
            "SHIPPING_TYPE_NOT_SUPPORTED_FOR_CLIENT" => ConnectorErrorType::BusinessError,
            "UNSUPPORTED_SHIPPING_TYPE" => ConnectorErrorType::BusinessError,
            "PREFERRED_SHIPPING_OPTION_AMOUNT_MISMATCH" => ConnectorErrorType::UserError,
            "CARD_CLOSED" => ConnectorErrorType::BusinessError,
            "ORDER_CANNOT_BE_SAVED" => ConnectorErrorType::BusinessError,
            "SAVE_ORDER_NOT_SUPPORTED" => ConnectorErrorType::BusinessError,
            "FIELD_NOT_PATCHABLE" => ConnectorErrorType::UserError,
            "AMOUNT_NOT_PATCHABLE" => ConnectorErrorType::UserError,
            "INVALID_PATCH_OPERATION" => ConnectorErrorType::UserError,
            "PAYEE_ACCOUNT_NOT_SUPPORTED" => ConnectorErrorType::UserError,
            "PAYEE_ACCOUNT_NOT_VERIFIED" => ConnectorErrorType::UserError,
            "PAYEE_NOT_CONSENTED" => ConnectorErrorType::UserError,
            "INVALID_JSON_POINTER_FORMAT" => ConnectorErrorType::BusinessError,
            "INVALID_PARAMETER" => ConnectorErrorType::UserError,
            "NOT_PATCHABLE" => ConnectorErrorType::BusinessError,
            "PATCH_VALUE_REQUIRED" => ConnectorErrorType::UserError,
            "PATCH_PATH_REQUIRED" => ConnectorErrorType::UserError,
            "REFERENCE_ID_NOT_FOUND" => ConnectorErrorType::UserError,
            "SHIPPING_OPTION_NOT_SELECTED" => ConnectorErrorType::UserError,
            "SHIPPING_OPTIONS_NOT_SUPPORTED" => ConnectorErrorType::BusinessError,
            "MULTIPLE_SHIPPING_OPTION_SELECTED" => ConnectorErrorType::UserError,
            "ORDER_ALREADY_COMPLETED" => ConnectorErrorType::BusinessError,
            "ACTION_DOES_NOT_MATCH_INTENT" => ConnectorErrorType::BusinessError,
            "AGREEMENT_ALREADY_CANCELLED" => ConnectorErrorType::BusinessError,
            "BILLING_AGREEMENT_NOT_FOUND" => ConnectorErrorType::BusinessError,
            "DOMESTIC_TRANSACTION_REQUIRED" => ConnectorErrorType::BusinessError,
            "ORDER_NOT_APPROVED" => ConnectorErrorType::UserError,
            "MAX_NUMBER_OF_PAYMENT_ATTEMPTS_EXCEEDED" => ConnectorErrorType::TechnicalError,
            "PAYEE_BLOCKED_TRANSACTION" => ConnectorErrorType::BusinessError,
            "TRANSACTION_LIMIT_EXCEEDED" => ConnectorErrorType::UserError,
            "TRANSACTION_RECEIVING_LIMIT_EXCEEDED" => ConnectorErrorType::BusinessError,
            "TRANSACTION_REFUSED" => ConnectorErrorType::TechnicalError,
            "ORDER_ALREADY_AUTHORIZED" => ConnectorErrorType::BusinessError,
            "AUTH_CAPTURE_NOT_ENABLED" => ConnectorErrorType::BusinessError,
            "AMOUNT_CANNOT_BE_SPECIFIED" => ConnectorErrorType::BusinessError,
            "AUTHORIZATION_AMOUNT_EXCEEDED" => ConnectorErrorType::UserError,
            "AUTHORIZATION_CURRENCY_MISMATCH" => ConnectorErrorType::UserError,
            "MAX_AUTHORIZATION_COUNT_EXCEEDED" => ConnectorErrorType::BusinessError,
            "ORDER_COMPLETED_OR_VOIDED" => ConnectorErrorType::BusinessError,
            "ORDER_EXPIRED" => ConnectorErrorType::BusinessError,
            "INVALID_PICKUP_ADDRESS" => ConnectorErrorType::UserError,
            "CONSENT_NEEDED" => ConnectorErrorType::UserError,
            "COMPLIANCE_VIOLATION" => ConnectorErrorType::BusinessError,
            "REDIRECT_PAYER_FOR_ALTERNATE_FUNDING" => ConnectorErrorType::TechnicalError,
            "ORDER_ALREADY_CAPTURED" => ConnectorErrorType::UserError,
            "TRANSACTION_BLOCKED_BY_PAYEE" => ConnectorErrorType::BusinessError,
            "NOT_ENABLED_FOR_CARD_PROCESSING" => ConnectorErrorType::BusinessError,
            "PAYEE_NOT_ENABLED_FOR_CARD_PROCESSING" => ConnectorErrorType::BusinessError,
            _ => ConnectorErrorType::UnknownError,
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Paypal<T>
{
    fn id(&self) -> &'static str {
        "paypal"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.paypal.base_url.as_ref()
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = paypal::PaypalAuthType::try_from(auth_type)?;
        let credentials = auth.get_credentials()?;

        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            credentials.get_client_secret().into_masked(),
        )])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: paypal::PaypalPaymentErrorResponse = res
            .response
            .parse_struct("Paypal ErrorResponse")
            .change_context(utils::response_handling_fail_for_connector(
                res.status_code,
                "paypal",
            ))?;

        with_error_response_body!(event_builder, response);

        let typed =
            macros::serialize_typed_connector_payload(&response, "typed_connector_response");
        let error_reason = response
            .details
            .clone()
            .map(|error_details| {
                error_details
                    .iter()
                    .try_fold(String::new(), |mut acc, error| {
                        if let Some(description) = &error.description {
                            write!(acc, "description - {description} ;")
                                .change_context(utils::response_handling_fail_for_connector(
                                    res.status_code,
                                    "paypal",
                                ))
                                .attach_printable("Failed to concatenate error details")
                                .map(|_| acc)
                        } else {
                            Ok(acc)
                        }
                    })
            })
            .transpose()?;
        let reason = match error_reason {
            Some(err_reason) => err_reason
                .is_empty()
                .then(|| response.message.to_owned())
                .or(Some(err_reason)),
            None => Some(response.message.to_owned()),
        };
        let errors_list = response.details.unwrap_or_default();
        let option_error_code_message = utils::get_error_code_error_message_based_on_priority(
            self.clone(),
            errors_list
                .into_iter()
                .map(|errors| errors.into())
                .collect(),
        );

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: option_error_code_message
                .clone()
                .map(|error_code_message| error_code_message.error_code)
                .unwrap_or(NO_ERROR_CODE.to_string()),
            message: option_error_code_message
                .map(|error_code_message| error_code_message.error_message)
                .unwrap_or(NO_ERROR_MESSAGE.to_string()),
            reason,
            attempt_status: None,
            connector_transaction_id: None,
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
            typed_connector_response: typed,
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
        })
    }
}

fn construct_auth_assertion_header(
    payer_id: &Secret<String>,
    client_id: &Secret<String>,
) -> String {
    let algorithm = BASE64_ENGINE.encode("{\"alg\":\"none\"}").to_string();
    let merchant_credentials = format!(
        "{{\"iss\":\"{}\",\"payer_id\":\"{}\"}}",
        client_id.clone().expose(),
        payer_id.clone().expose()
    );
    let encoded_credentials = BASE64_ENGINE.encode(merchant_credentials).to_string();
    format!("{algorithm}.{encoded_credentials}.")
}

macros::macro_connector_flow_status_impls!(
    connector: Paypal,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        IncrementalAuthorization,
        Accept,
        SubmitEvidence,
        DefendDispute,
        CreateConnectorCustomer,
        GetConnectorCustomer,
        PaymentMethodToken,
        PreAuthenticate,
        MandateRevoke,
    ],
    not_supported: [
        VoidPostRefund,
        ServerSessionAuthenticationToken,
        VoidPC,
    ],
);
