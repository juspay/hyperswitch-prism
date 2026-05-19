// AUTO-GENERATED — do not edit by hand.
// Source: services.proto ∩ bindings/uniffi.rs  |  Regenerate: make generate

package payments

import (
	"context"

	pb "github.com/juspay/hyperswitch-prism/sdk/go/generated/payments"
	uniffi "github.com/juspay/hyperswitch-prism/sdk/go/generated/uniffi/connector_service_ffi"
)

// ============================================================================
// CUSTOMERCLIENT
// ============================================================================

// CustomerClient provides methods for CustomerService flows.
type CustomerClient struct {
	*ConnectorClient
}

// NewCustomerClient creates a new CustomerClient.
func NewCustomerClient(config *pb.ConnectorConfig, defaults *pb.RequestConfig) *CustomerClient {
	return &CustomerClient{ConnectorClient: NewConnectorClient(config, defaults)}
}

// Create performs a CustomerService.Create.
func (c *CustomerClient) Create(ctx context.Context, req *pb.CustomerServiceCreateRequest, opts *pb.RequestConfig) (*pb.CustomerServiceCreateResponse, error) {
	res := &pb.CustomerServiceCreateResponse{}
	err := c.ExecuteFlow(ctx, uniffi.CreateReqTransformer, uniffi.CreateResTransformer, req, res, opts)
	return res, err
}

// ============================================================================
// DISPUTECLIENT
// ============================================================================

// DisputeClient provides methods for DisputeService flows.
type DisputeClient struct {
	*ConnectorClient
}

// NewDisputeClient creates a new DisputeClient.
func NewDisputeClient(config *pb.ConnectorConfig, defaults *pb.RequestConfig) *DisputeClient {
	return &DisputeClient{ConnectorClient: NewConnectorClient(config, defaults)}
}

// Accept performs a DisputeService.Accept.
func (c *DisputeClient) Accept(ctx context.Context, req *pb.DisputeServiceAcceptRequest, opts *pb.RequestConfig) (*pb.DisputeServiceAcceptResponse, error) {
	res := &pb.DisputeServiceAcceptResponse{}
	err := c.ExecuteFlow(ctx, uniffi.AcceptReqTransformer, uniffi.AcceptResTransformer, req, res, opts)
	return res, err
}

// Defend performs a DisputeService.Defend.
func (c *DisputeClient) Defend(ctx context.Context, req *pb.DisputeServiceDefendRequest, opts *pb.RequestConfig) (*pb.DisputeServiceDefendResponse, error) {
	res := &pb.DisputeServiceDefendResponse{}
	err := c.ExecuteFlow(ctx, uniffi.DefendReqTransformer, uniffi.DefendResTransformer, req, res, opts)
	return res, err
}

// SubmitEvidence performs a DisputeService.SubmitEvidence.
func (c *DisputeClient) SubmitEvidence(ctx context.Context, req *pb.DisputeServiceSubmitEvidenceRequest, opts *pb.RequestConfig) (*pb.DisputeServiceSubmitEvidenceResponse, error) {
	res := &pb.DisputeServiceSubmitEvidenceResponse{}
	err := c.ExecuteFlow(ctx, uniffi.SubmitEvidenceReqTransformer, uniffi.SubmitEvidenceResTransformer, req, res, opts)
	return res, err
}

// ============================================================================
// EVENTCLIENT
// ============================================================================

// EventClient provides methods for EventService flows.
type EventClient struct {
	*ConnectorClient
}

// NewEventClient creates a new EventClient.
func NewEventClient(config *pb.ConnectorConfig, defaults *pb.RequestConfig) *EventClient {
	return &EventClient{ConnectorClient: NewConnectorClient(config, defaults)}
}

// HandleEvent performs a EventService.HandleEvent.
func (c *EventClient) HandleEvent(ctx context.Context, req *pb.EventServiceHandleRequest, opts *pb.RequestConfig) (*pb.EventServiceHandleResponse, error) {
	res := &pb.EventServiceHandleResponse{}
	err := c.ExecuteDirect(ctx, uniffi.HandleEventTransformer, req, res, opts)
	return res, err
}

// ParseEvent performs a EventService.ParseEvent.
func (c *EventClient) ParseEvent(ctx context.Context, req *pb.EventServiceParseRequest, opts *pb.RequestConfig) (*pb.EventServiceParseResponse, error) {
	res := &pb.EventServiceParseResponse{}
	err := c.ExecuteDirect(ctx, uniffi.ParseEventTransformer, req, res, opts)
	return res, err
}

// ============================================================================
// MERCHANTAUTHENTICATIONCLIENT
// ============================================================================

// MerchantAuthenticationClient provides methods for MerchantAuthenticationService flows.
type MerchantAuthenticationClient struct {
	*ConnectorClient
}

// NewMerchantAuthenticationClient creates a new MerchantAuthenticationClient.
func NewMerchantAuthenticationClient(config *pb.ConnectorConfig, defaults *pb.RequestConfig) *MerchantAuthenticationClient {
	return &MerchantAuthenticationClient{ConnectorClient: NewConnectorClient(config, defaults)}
}

// CreateClientAuthenticationToken performs a MerchantAuthenticationService.CreateClientAuthenticationToken.
func (c *MerchantAuthenticationClient) CreateClientAuthenticationToken(ctx context.Context, req *pb.MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest, opts *pb.RequestConfig) (*pb.MerchantAuthenticationServiceCreateClientAuthenticationTokenResponse, error) {
	res := &pb.MerchantAuthenticationServiceCreateClientAuthenticationTokenResponse{}
	err := c.ExecuteFlow(ctx, uniffi.CreateClientAuthenticationTokenReqTransformer, uniffi.CreateClientAuthenticationTokenResTransformer, req, res, opts)
	return res, err
}

// CreateServerAuthenticationToken performs a MerchantAuthenticationService.CreateServerAuthenticationToken.
func (c *MerchantAuthenticationClient) CreateServerAuthenticationToken(ctx context.Context, req *pb.MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest, opts *pb.RequestConfig) (*pb.MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse, error) {
	res := &pb.MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse{}
	err := c.ExecuteFlow(ctx, uniffi.CreateServerAuthenticationTokenReqTransformer, uniffi.CreateServerAuthenticationTokenResTransformer, req, res, opts)
	return res, err
}

// CreateServerSessionAuthenticationToken performs a MerchantAuthenticationService.CreateServerSessionAuthenticationToken.
func (c *MerchantAuthenticationClient) CreateServerSessionAuthenticationToken(ctx context.Context, req *pb.MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenRequest, opts *pb.RequestConfig) (*pb.MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenResponse, error) {
	res := &pb.MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenResponse{}
	err := c.ExecuteFlow(ctx, uniffi.CreateServerSessionAuthenticationTokenReqTransformer, uniffi.CreateServerSessionAuthenticationTokenResTransformer, req, res, opts)
	return res, err
}

// ============================================================================
// PAYMENTMETHODAUTHENTICATIONCLIENT
// ============================================================================

// PaymentMethodAuthenticationClient provides methods for PaymentMethodAuthenticationService flows.
type PaymentMethodAuthenticationClient struct {
	*ConnectorClient
}

// NewPaymentMethodAuthenticationClient creates a new PaymentMethodAuthenticationClient.
func NewPaymentMethodAuthenticationClient(config *pb.ConnectorConfig, defaults *pb.RequestConfig) *PaymentMethodAuthenticationClient {
	return &PaymentMethodAuthenticationClient{ConnectorClient: NewConnectorClient(config, defaults)}
}

// Authenticate performs a PaymentMethodAuthenticationService.Authenticate.
func (c *PaymentMethodAuthenticationClient) Authenticate(ctx context.Context, req *pb.PaymentMethodAuthenticationServiceAuthenticateRequest, opts *pb.RequestConfig) (*pb.PaymentMethodAuthenticationServiceAuthenticateResponse, error) {
	res := &pb.PaymentMethodAuthenticationServiceAuthenticateResponse{}
	err := c.ExecuteFlow(ctx, uniffi.AuthenticateReqTransformer, uniffi.AuthenticateResTransformer, req, res, opts)
	return res, err
}

// PostAuthenticate performs a PaymentMethodAuthenticationService.PostAuthenticate.
func (c *PaymentMethodAuthenticationClient) PostAuthenticate(ctx context.Context, req *pb.PaymentMethodAuthenticationServicePostAuthenticateRequest, opts *pb.RequestConfig) (*pb.PaymentMethodAuthenticationServicePostAuthenticateResponse, error) {
	res := &pb.PaymentMethodAuthenticationServicePostAuthenticateResponse{}
	err := c.ExecuteFlow(ctx, uniffi.PostAuthenticateReqTransformer, uniffi.PostAuthenticateResTransformer, req, res, opts)
	return res, err
}

// PreAuthenticate performs a PaymentMethodAuthenticationService.PreAuthenticate.
func (c *PaymentMethodAuthenticationClient) PreAuthenticate(ctx context.Context, req *pb.PaymentMethodAuthenticationServicePreAuthenticateRequest, opts *pb.RequestConfig) (*pb.PaymentMethodAuthenticationServicePreAuthenticateResponse, error) {
	res := &pb.PaymentMethodAuthenticationServicePreAuthenticateResponse{}
	err := c.ExecuteFlow(ctx, uniffi.PreAuthenticateReqTransformer, uniffi.PreAuthenticateResTransformer, req, res, opts)
	return res, err
}

// ============================================================================
// PAYMENTMETHODCLIENT
// ============================================================================

// PaymentMethodClient provides methods for PaymentMethodService flows.
type PaymentMethodClient struct {
	*ConnectorClient
}

// NewPaymentMethodClient creates a new PaymentMethodClient.
func NewPaymentMethodClient(config *pb.ConnectorConfig, defaults *pb.RequestConfig) *PaymentMethodClient {
	return &PaymentMethodClient{ConnectorClient: NewConnectorClient(config, defaults)}
}

// Tokenize performs a PaymentMethodService.Tokenize.
func (c *PaymentMethodClient) Tokenize(ctx context.Context, req *pb.PaymentMethodServiceTokenizeRequest, opts *pb.RequestConfig) (*pb.PaymentMethodServiceTokenizeResponse, error) {
	res := &pb.PaymentMethodServiceTokenizeResponse{}
	err := c.ExecuteFlow(ctx, uniffi.TokenizeReqTransformer, uniffi.TokenizeResTransformer, req, res, opts)
	return res, err
}

// ============================================================================
// PAYMENTCLIENT
// ============================================================================

// PaymentClient provides methods for PaymentService flows.
type PaymentClient struct {
	*ConnectorClient
}

// NewPaymentClient creates a new PaymentClient.
func NewPaymentClient(config *pb.ConnectorConfig, defaults *pb.RequestConfig) *PaymentClient {
	return &PaymentClient{ConnectorClient: NewConnectorClient(config, defaults)}
}

// Authorize performs a PaymentService.Authorize.
func (c *PaymentClient) Authorize(ctx context.Context, req *pb.PaymentServiceAuthorizeRequest, opts *pb.RequestConfig) (*pb.PaymentServiceAuthorizeResponse, error) {
	res := &pb.PaymentServiceAuthorizeResponse{}
	err := c.ExecuteFlow(ctx, uniffi.AuthorizeReqTransformer, uniffi.AuthorizeResTransformer, req, res, opts)
	return res, err
}

// Capture performs a PaymentService.Capture.
func (c *PaymentClient) Capture(ctx context.Context, req *pb.PaymentServiceCaptureRequest, opts *pb.RequestConfig) (*pb.PaymentServiceCaptureResponse, error) {
	res := &pb.PaymentServiceCaptureResponse{}
	err := c.ExecuteFlow(ctx, uniffi.CaptureReqTransformer, uniffi.CaptureResTransformer, req, res, opts)
	return res, err
}

// CreateOrder performs a PaymentService.CreateOrder.
func (c *PaymentClient) CreateOrder(ctx context.Context, req *pb.PaymentServiceCreateOrderRequest, opts *pb.RequestConfig) (*pb.PaymentServiceCreateOrderResponse, error) {
	res := &pb.PaymentServiceCreateOrderResponse{}
	err := c.ExecuteFlow(ctx, uniffi.CreateOrderReqTransformer, uniffi.CreateOrderResTransformer, req, res, opts)
	return res, err
}

// Get performs a PaymentService.Get.
func (c *PaymentClient) Get(ctx context.Context, req *pb.PaymentServiceGetRequest, opts *pb.RequestConfig) (*pb.PaymentServiceGetResponse, error) {
	res := &pb.PaymentServiceGetResponse{}
	err := c.ExecuteFlow(ctx, uniffi.GetReqTransformer, uniffi.GetResTransformer, req, res, opts)
	return res, err
}

// IncrementalAuthorization performs a PaymentService.IncrementalAuthorization.
func (c *PaymentClient) IncrementalAuthorization(ctx context.Context, req *pb.PaymentServiceIncrementalAuthorizationRequest, opts *pb.RequestConfig) (*pb.PaymentServiceIncrementalAuthorizationResponse, error) {
	res := &pb.PaymentServiceIncrementalAuthorizationResponse{}
	err := c.ExecuteFlow(ctx, uniffi.IncrementalAuthorizationReqTransformer, uniffi.IncrementalAuthorizationResTransformer, req, res, opts)
	return res, err
}

// ProxyAuthorize performs a PaymentService.ProxyAuthorize.
func (c *PaymentClient) ProxyAuthorize(ctx context.Context, req *pb.PaymentServiceProxyAuthorizeRequest, opts *pb.RequestConfig) (*pb.PaymentServiceAuthorizeResponse, error) {
	res := &pb.PaymentServiceAuthorizeResponse{}
	err := c.ExecuteFlow(ctx, uniffi.ProxyAuthorizeReqTransformer, uniffi.ProxyAuthorizeResTransformer, req, res, opts)
	return res, err
}

// ProxySetupRecurring performs a PaymentService.ProxySetupRecurring.
func (c *PaymentClient) ProxySetupRecurring(ctx context.Context, req *pb.PaymentServiceProxySetupRecurringRequest, opts *pb.RequestConfig) (*pb.PaymentServiceSetupRecurringResponse, error) {
	res := &pb.PaymentServiceSetupRecurringResponse{}
	err := c.ExecuteFlow(ctx, uniffi.ProxySetupRecurringReqTransformer, uniffi.ProxySetupRecurringResTransformer, req, res, opts)
	return res, err
}

// Refund performs a PaymentService.Refund.
func (c *PaymentClient) Refund(ctx context.Context, req *pb.PaymentServiceRefundRequest, opts *pb.RequestConfig) (*pb.RefundResponse, error) {
	res := &pb.RefundResponse{}
	err := c.ExecuteFlow(ctx, uniffi.RefundReqTransformer, uniffi.RefundResTransformer, req, res, opts)
	return res, err
}

// Reverse performs a PaymentService.Reverse.
func (c *PaymentClient) Reverse(ctx context.Context, req *pb.PaymentServiceReverseRequest, opts *pb.RequestConfig) (*pb.PaymentServiceReverseResponse, error) {
	res := &pb.PaymentServiceReverseResponse{}
	err := c.ExecuteFlow(ctx, uniffi.ReverseReqTransformer, uniffi.ReverseResTransformer, req, res, opts)
	return res, err
}

// SetupRecurring performs a PaymentService.SetupRecurring.
func (c *PaymentClient) SetupRecurring(ctx context.Context, req *pb.PaymentServiceSetupRecurringRequest, opts *pb.RequestConfig) (*pb.PaymentServiceSetupRecurringResponse, error) {
	res := &pb.PaymentServiceSetupRecurringResponse{}
	err := c.ExecuteFlow(ctx, uniffi.SetupRecurringReqTransformer, uniffi.SetupRecurringResTransformer, req, res, opts)
	return res, err
}

// TokenAuthorize performs a PaymentService.TokenAuthorize.
func (c *PaymentClient) TokenAuthorize(ctx context.Context, req *pb.PaymentServiceTokenAuthorizeRequest, opts *pb.RequestConfig) (*pb.PaymentServiceAuthorizeResponse, error) {
	res := &pb.PaymentServiceAuthorizeResponse{}
	err := c.ExecuteFlow(ctx, uniffi.TokenAuthorizeReqTransformer, uniffi.TokenAuthorizeResTransformer, req, res, opts)
	return res, err
}

// TokenSetupRecurring performs a PaymentService.TokenSetupRecurring.
func (c *PaymentClient) TokenSetupRecurring(ctx context.Context, req *pb.PaymentServiceTokenSetupRecurringRequest, opts *pb.RequestConfig) (*pb.PaymentServiceSetupRecurringResponse, error) {
	res := &pb.PaymentServiceSetupRecurringResponse{}
	err := c.ExecuteFlow(ctx, uniffi.TokenSetupRecurringReqTransformer, uniffi.TokenSetupRecurringResTransformer, req, res, opts)
	return res, err
}

// Void performs a PaymentService.Void.
func (c *PaymentClient) Void(ctx context.Context, req *pb.PaymentServiceVoidRequest, opts *pb.RequestConfig) (*pb.PaymentServiceVoidResponse, error) {
	res := &pb.PaymentServiceVoidResponse{}
	err := c.ExecuteFlow(ctx, uniffi.VoidReqTransformer, uniffi.VoidResTransformer, req, res, opts)
	return res, err
}

// VerifyRedirectResponse performs a PaymentService.VerifyRedirectResponse.
func (c *PaymentClient) VerifyRedirectResponse(ctx context.Context, req *pb.PaymentServiceVerifyRedirectResponseRequest, opts *pb.RequestConfig) (*pb.PaymentServiceVerifyRedirectResponseResponse, error) {
	res := &pb.PaymentServiceVerifyRedirectResponseResponse{}
	err := c.ExecuteDirect(ctx, uniffi.VerifyRedirectResponseTransformer, req, res, opts)
	return res, err
}

// ============================================================================
// PAYOUTCLIENT
// ============================================================================

// PayoutClient provides methods for PayoutService flows.
type PayoutClient struct {
	*ConnectorClient
}

// NewPayoutClient creates a new PayoutClient.
func NewPayoutClient(config *pb.ConnectorConfig, defaults *pb.RequestConfig) *PayoutClient {
	return &PayoutClient{ConnectorClient: NewConnectorClient(config, defaults)}
}

// PayoutCreate performs a PayoutService.Create.
func (c *PayoutClient) PayoutCreate(ctx context.Context, req *pb.PayoutServiceCreateRequest, opts *pb.RequestConfig) (*pb.PayoutServiceCreateResponse, error) {
	res := &pb.PayoutServiceCreateResponse{}
	err := c.ExecuteFlow(ctx, uniffi.PayoutCreateReqTransformer, uniffi.PayoutCreateResTransformer, req, res, opts)
	return res, err
}

// PayoutCreateLink performs a PayoutService.CreateLink.
func (c *PayoutClient) PayoutCreateLink(ctx context.Context, req *pb.PayoutServiceCreateLinkRequest, opts *pb.RequestConfig) (*pb.PayoutServiceCreateLinkResponse, error) {
	res := &pb.PayoutServiceCreateLinkResponse{}
	err := c.ExecuteFlow(ctx, uniffi.PayoutCreateLinkReqTransformer, uniffi.PayoutCreateLinkResTransformer, req, res, opts)
	return res, err
}

// PayoutCreateRecipient performs a PayoutService.CreateRecipient.
func (c *PayoutClient) PayoutCreateRecipient(ctx context.Context, req *pb.PayoutServiceCreateRecipientRequest, opts *pb.RequestConfig) (*pb.PayoutServiceCreateRecipientResponse, error) {
	res := &pb.PayoutServiceCreateRecipientResponse{}
	err := c.ExecuteFlow(ctx, uniffi.PayoutCreateRecipientReqTransformer, uniffi.PayoutCreateRecipientResTransformer, req, res, opts)
	return res, err
}

// PayoutEnrollDisburseAccount performs a PayoutService.EnrollDisburseAccount.
func (c *PayoutClient) PayoutEnrollDisburseAccount(ctx context.Context, req *pb.PayoutServiceEnrollDisburseAccountRequest, opts *pb.RequestConfig) (*pb.PayoutServiceEnrollDisburseAccountResponse, error) {
	res := &pb.PayoutServiceEnrollDisburseAccountResponse{}
	err := c.ExecuteFlow(ctx, uniffi.PayoutEnrollDisburseAccountReqTransformer, uniffi.PayoutEnrollDisburseAccountResTransformer, req, res, opts)
	return res, err
}

// PayoutGet performs a PayoutService.Get.
func (c *PayoutClient) PayoutGet(ctx context.Context, req *pb.PayoutServiceGetRequest, opts *pb.RequestConfig) (*pb.PayoutServiceGetResponse, error) {
	res := &pb.PayoutServiceGetResponse{}
	err := c.ExecuteFlow(ctx, uniffi.PayoutGetReqTransformer, uniffi.PayoutGetResTransformer, req, res, opts)
	return res, err
}

// PayoutStage performs a PayoutService.Stage.
func (c *PayoutClient) PayoutStage(ctx context.Context, req *pb.PayoutServiceStageRequest, opts *pb.RequestConfig) (*pb.PayoutServiceStageResponse, error) {
	res := &pb.PayoutServiceStageResponse{}
	err := c.ExecuteFlow(ctx, uniffi.PayoutStageReqTransformer, uniffi.PayoutStageResTransformer, req, res, opts)
	return res, err
}

// PayoutTransfer performs a PayoutService.Transfer.
func (c *PayoutClient) PayoutTransfer(ctx context.Context, req *pb.PayoutServiceTransferRequest, opts *pb.RequestConfig) (*pb.PayoutServiceTransferResponse, error) {
	res := &pb.PayoutServiceTransferResponse{}
	err := c.ExecuteFlow(ctx, uniffi.PayoutTransferReqTransformer, uniffi.PayoutTransferResTransformer, req, res, opts)
	return res, err
}

// PayoutVoid performs a PayoutService.Void.
func (c *PayoutClient) PayoutVoid(ctx context.Context, req *pb.PayoutServiceVoidRequest, opts *pb.RequestConfig) (*pb.PayoutServiceVoidResponse, error) {
	res := &pb.PayoutServiceVoidResponse{}
	err := c.ExecuteFlow(ctx, uniffi.PayoutVoidReqTransformer, uniffi.PayoutVoidResTransformer, req, res, opts)
	return res, err
}

// ============================================================================
// RECURRINGPAYMENTCLIENT
// ============================================================================

// RecurringPaymentClient provides methods for RecurringPaymentService flows.
type RecurringPaymentClient struct {
	*ConnectorClient
}

// NewRecurringPaymentClient creates a new RecurringPaymentClient.
func NewRecurringPaymentClient(config *pb.ConnectorConfig, defaults *pb.RequestConfig) *RecurringPaymentClient {
	return &RecurringPaymentClient{ConnectorClient: NewConnectorClient(config, defaults)}
}

// Charge performs a RecurringPaymentService.Charge.
func (c *RecurringPaymentClient) Charge(ctx context.Context, req *pb.RecurringPaymentServiceChargeRequest, opts *pb.RequestConfig) (*pb.RecurringPaymentServiceChargeResponse, error) {
	res := &pb.RecurringPaymentServiceChargeResponse{}
	err := c.ExecuteFlow(ctx, uniffi.ChargeReqTransformer, uniffi.ChargeResTransformer, req, res, opts)
	return res, err
}

// RecurringRevoke performs a RecurringPaymentService.Revoke.
func (c *RecurringPaymentClient) RecurringRevoke(ctx context.Context, req *pb.RecurringPaymentServiceRevokeRequest, opts *pb.RequestConfig) (*pb.RecurringPaymentServiceRevokeResponse, error) {
	res := &pb.RecurringPaymentServiceRevokeResponse{}
	err := c.ExecuteFlow(ctx, uniffi.RecurringRevokeReqTransformer, uniffi.RecurringRevokeResTransformer, req, res, opts)
	return res, err
}

// ============================================================================
// REFUNDCLIENT
// ============================================================================

// RefundClient provides methods for RefundService flows.
type RefundClient struct {
	*ConnectorClient
}

// NewRefundClient creates a new RefundClient.
func NewRefundClient(config *pb.ConnectorConfig, defaults *pb.RequestConfig) *RefundClient {
	return &RefundClient{ConnectorClient: NewConnectorClient(config, defaults)}
}

// RefundGet performs a RefundService.Get.
func (c *RefundClient) RefundGet(ctx context.Context, req *pb.RefundServiceGetRequest, opts *pb.RequestConfig) (*pb.RefundResponse, error) {
	res := &pb.RefundResponse{}
	err := c.ExecuteFlow(ctx, uniffi.RefundGetReqTransformer, uniffi.RefundGetResTransformer, req, res, opts)
	return res, err
}
