// Auto-generated for hipay
package examples.hipay

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
fun processVoidPayment(txnId: String, config: ConnectorConfig) {
    // Void a Payment
    val client = PaymentClient(config)
}
fun processGetPayment(txnId: String, config: ConnectorConfig) {
    // Get Payment Status
    val client = PaymentClient(config)
}
fun processTokenize(txnId: String, config: ConnectorConfig) {
    // Tokenize Payment Method
    val client = PaymentClient(config)
}
fun processTokenizedCheckout(txnId: String, config: ConnectorConfig) {
    // Tokenized Payment (Authorize + Capture)
    val client = PaymentClient(config)
}