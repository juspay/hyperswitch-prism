// Auto-generated for xendit
package examples.xendit

import payments.PaymentClient

fun processCheckoutCard(txnId: String, config: ConnectorConfig) {
    // Card Payment (Authorize + Capture)
    val client = PaymentClient(config)
}
fun processCheckoutAutocapture(txnId: String, config: ConnectorConfig) {
    // Card Payment (Automatic Capture)
    val client = PaymentClient(config)
}
fun processRefund(txnId: String, config: ConnectorConfig) {
    // Refund a Payment
    val client = PaymentClient(config)
}
fun processGetPayment(txnId: String, config: ConnectorConfig) {
    // Get Payment Status
    val client = PaymentClient(config)
}