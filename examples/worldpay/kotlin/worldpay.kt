// Auto-generated for worldpay
package examples.worldpay

import payments.PaymentClient

fun processCheckoutCard(txnId: String, config: ConnectorConfig) {
    // Card Payment (Authorize + Capture)
    val client = PaymentClient(config)
}
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
fun processVoidPayment(txnId: String, config: ConnectorConfig) {
    // Void a Payment
    val client = PaymentClient(config)
}
fun processGetPayment(txnId: String, config: ConnectorConfig) {
    // Get Payment Status
    val client = PaymentClient(config)
}