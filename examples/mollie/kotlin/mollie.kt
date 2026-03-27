// Auto-generated for mollie
package examples.mollie

import payments.PaymentClient

fun processCheckoutAutocapture(txnId: String, config: ConnectorConfig) {
    // Card Payment (Automatic Capture)
    val client = PaymentClient(config)
}
fun processRefund(txnId: String, config: ConnectorConfig) {
    // Refund a Payment
    val client = PaymentClient(config)
}
fun processVoidPayment(txnId: String, config: ConnectorConfig) {
    // Void a Payment
    val client = PaymentClient(config)
}
fun processGetPayment(txnId: String, config: ConnectorConfig) {
    // Get Payment Status
    val client = PaymentClient(config)
}