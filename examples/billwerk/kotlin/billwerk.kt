// Auto-generated for billwerk
package examples.billwerk

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