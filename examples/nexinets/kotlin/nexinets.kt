// Auto-generated for nexinets
package examples.nexinets

import payments.PaymentClient

fun processCheckoutAutocapture(txnId: String, config: ConnectorConfig) {
    // Card Payment (Automatic Capture)
    val client = PaymentClient(config)
}
fun processCheckoutWallet(txnId: String, config: ConnectorConfig) {
    // Wallet Payment (Google Pay / Apple Pay)
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