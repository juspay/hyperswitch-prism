#[derive(Debug, Clone)]
pub struct CreateOrder;

#[derive(Debug, Clone)]
pub struct Authorize;

#[derive(Debug, Clone)]
pub struct PSync;

#[derive(Debug, Clone)]
pub struct Void;

#[derive(Debug, Clone)]
pub struct RSync;

#[derive(Debug, Clone)]
pub struct Refund;

#[derive(Debug, Clone)]
pub struct Capture;

#[derive(Debug, Clone)]
pub struct SetupMandate;

#[derive(Debug, Clone)]
pub struct RepeatPayment;

#[derive(Debug, Clone)]
pub struct Accept;

#[derive(Debug, Clone)]
pub struct SubmitEvidence;

#[derive(Debug, Clone)]
pub struct DefendDispute;

#[derive(Debug, Clone)]
pub struct CreateSessionToken;

#[derive(Debug, Clone)]
pub struct CreateAccessToken;

#[derive(Debug, Clone)]
pub struct CreateConnectorCustomer;

#[derive(Debug, Clone)]
pub struct PaymentMethodToken;

#[derive(Debug, Clone)]
pub struct PreAuthenticate;

#[derive(Debug, Clone)]
pub struct Authenticate;

#[derive(Debug, Clone)]
pub struct PostAuthenticate;

#[derive(Debug, Clone)]
pub struct VoidPC;

#[derive(Debug, Clone)]
pub struct SdkSessionToken;

#[derive(Debug, Clone)]
pub struct IncrementalAuthorization;

#[derive(Debug, Clone)]
pub struct MandateRevoke;

#[derive(Debug, Clone)]
pub struct MandateStatusCheck;

#[derive(Debug, Clone)]
pub struct VerifyWebhookSource;

#[derive(strum::Display)]
#[strum(serialize_all = "snake_case")]
pub enum FlowName {
    Authorize,
    Refund,
    Rsync,
    Psync,
    Void,
    VoidPc,
    SetupMandate,
    RepeatPayment,
    Capture,
    AcceptDispute,
    SubmitEvidence,
    DefendDispute,
    CreateOrder,
    IncomingWebhook,
    Dsync,
    CreateSessionToken,
    CreateAccessToken,
    CreateConnectorCustomer,
    PaymentMethodToken,
    PreAuthenticate,
    Authenticate,
    PostAuthenticate,
    SdkSessionToken,
    IncrementalAuthorization,
    MandateRevoke,
    MandateStatusCheck,
}
