/**
 * Re-exports for SDK configuration types.
 *
 * Usage:
 *   import payments.FfiOptions
 *   import payments.ClientConfig
 *   import payments.FfiConnectorHttpRequest
 *
 * Mirrors the JavaScript `configs` namespace and Python `ConfigsNamespace`.
 */
@file:Suppress("unused")

package payments

import types.SdkConfig

typealias Environment = SdkConfig.Environment
typealias ConnectorConfig = SdkConfig.ConnectorConfig
typealias SdkOptions = SdkConfig.SdkOptions
typealias RequestConfig = SdkConfig.RequestConfig
typealias HttpConfig = SdkConfig.HttpConfig
typealias CaCert = SdkConfig.CaCert
typealias ProxyOptions = SdkConfig.ProxyOptions
typealias FfiOptions = SdkConfig.FfiOptions
typealias FfiConnectorHttpRequest = SdkConfig.FfiConnectorHttpRequest
typealias FfiConnectorHttpResponse = SdkConfig.FfiConnectorHttpResponse
typealias HttpDefault = SdkConfig.HttpDefault
typealias NetworkErrorCode = SdkConfig.NetworkErrorCode
typealias FfiResult = SdkConfig.FfiResult
typealias IntegrationErrorProto = SdkConfig.IntegrationError
typealias ConnectorResponseTransformationErrorProto = SdkConfig.ConnectorResponseTransformationError
