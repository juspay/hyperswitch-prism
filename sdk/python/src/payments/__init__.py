# Hyperswitch Payments SDK
#
# Export structure:
#   - PaymentClient, MerchantAuthenticationClient (per-service high-level API)
#   - Direct imports via wildcard from generated proto files
#   - Exception classes (IntegrationError, ConnectorResponseTransformationError) from connector_client

from payments._generated_service_clients import (
    CustomerClient,
    DisputeClient,
    MerchantAuthenticationClient,
    PaymentClient,
    PaymentMethodAuthenticationClient,
    PaymentMethodClient,
    RecurringPaymentClient,
)
from payments.grpc_client import GrpcClient, GrpcConfig

# Direct access to all types via wildcard imports
from payments.generated.payment_pb2 import *
from payments.generated.payment_methods_pb2 import *
from payments.generated.sdk_config_pb2 import *
from payments.generated.connector_service_ffi import *

# Exception classes - override protobuf message types with proper Exception classes
from payments.connector_client import IntegrationError, ConnectorResponseTransformationError
from payments.http_client import NetworkError, NetworkErrorCode

# Expose proto modules for namespaced access (e.g., payments.Connector, configs.Environment)
from payments.generated import payment_pb2 as payments
from payments.generated import payment_methods_pb2 as payment_methods
from payments.generated import sdk_config_pb2 as configs
