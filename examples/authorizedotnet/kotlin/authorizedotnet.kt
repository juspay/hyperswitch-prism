// Auto-generated for authorizedotnet
package examples.authorizedotnet

import payments.PaymentClient

fun processCheckoutCard(txnId: String, config: ConnectorConfig) {
    // Card Payment (Authorize + Capture)
    val client = PaymentClient(config)
}
fun processCheckoutAutocapture(txnId: String, config: ConnectorConfig) {
    // Card Payment (Automatic Capture)
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
fun processRecurring(txnId: String, config: ConnectorConfig) {
    // Recurring / Mandate Payments
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
fun processCreateCustomer(txnId: String, config: ConnectorConfig) {
    // Create Customer
    val client = PaymentClient(config)
}