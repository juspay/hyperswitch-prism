package payments
//
import pb "github.com/juspay/hyperswitch-prism/sdk/go/generated/payments"

// IntegrationError is returned when a req_transformer fails.
type IntegrationError struct {
	Proto *pb.IntegrationError
}

func (e *IntegrationError) Error() string {
	return e.Proto.GetErrorMessage()
}

func (e *IntegrationError) ErrorCode() string {
	return e.Proto.GetErrorCode()
}

func (e *IntegrationError) SuggestedAction() string {
	return e.Proto.GetSuggestedAction()
}

func (e *IntegrationError) DocURL() string {
	return e.Proto.GetDocUrl()
}

// ConnectorError is returned when a res_transformer fails.
type ConnectorError struct {
	Proto *pb.ConnectorError
}

func (e *ConnectorError) Error() string {
	return e.Proto.GetErrorMessage()
}

func (e *ConnectorError) ErrorCode() string {
	return e.Proto.GetErrorCode()
}

func (e *ConnectorError) HTTPStatusCode() uint32 {
	return e.Proto.GetHttpStatusCode()
}

// NetworkError is returned for HTTP transport failures.
type NetworkError struct {
	Code    pb.NetworkErrorCode
	Message string
	Status  uint32
}

func (e *NetworkError) Error() string {
	return e.Message
}
