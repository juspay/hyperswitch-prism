// Auto-generated for revolut
package examples.revolut

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
fun processCheckoutBank(txnId: String, config: ConnectorConfig) {
    // Bank Transfer (SEPA / ACH / BACS)
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
fun processTokenizedCheckout(txnId: String, config: ConnectorConfig) {
    // Tokenized Payment (Authorize + Capture)
    val client = PaymentClient(config)
}