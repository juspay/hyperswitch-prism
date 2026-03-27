// Auto-generated for helcim
package examples.helcim

import payments.PaymentClient

fun processCheckoutAutocapture(txnId: String, config: ConnectorConfig) {
    // Card Payment (Automatic Capture)
    val client = PaymentClient(config)
}
fun processGetPayment(txnId: String, config: ConnectorConfig) {
    // Get Payment Status
    val client = PaymentClient(config)
}