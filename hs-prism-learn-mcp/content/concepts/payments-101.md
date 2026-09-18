---
{
  "slug": "payments-101",
  "title": "How an online payment actually works",
  "tier": "payments-primer",
  "audience": "everyone",
  "one_liner": "An online card payment has four steps: authorize (reserve the money), capture (take it), settle (move it to the merchant), and optionally refund (give it back).",
  "analogy": "A hotel at check-in. They put a hold on your card (authorize). At check-out they charge the final bill (capture). If you cancel before check-out, they drop the hold (void). If you were over-charged, they send money back (refund).",
  "depth": {
    "tldr": "When you pay online, the shop first asks your bank to RESERVE the money (authorize). Later it actually TAKES the money (capture). If something is wrong it can give it back (refund) or cancel the reservation before taking it (void). Each of these is a separate step a system has to handle.",
    "standard": "Every card payment is a sequence of steps, not one event. (1) Authorize: the merchant asks the processor and the customer's bank to reserve funds and confirm the card is good. (2) Capture: the merchant tells the processor to actually move the reserved money. Some merchants capture immediately (AUTOMATIC); others capture later, e.g. when goods ship (MANUAL). (3) Void: cancel an authorization before capture. (4) Refund: return money after capture. Hyperswitch-prism models each of these as a separate 'flow'. Understanding this sequence is the key to understanding everything else in this repo.",
    "deep": "These steps map one-to-one onto the repo's core flows: Authorize, Capture, Void, Refund, plus 'sync' flows (PSync, RSync) that poll the processor for the current status of a payment or refund. The split between authorize and capture exists because merchants often need to confirm stock, run fraud checks, or wait for fulfilment before taking money. A decline is not a crash: the processor returns a status (e.g. FAILURE) inside a normal response. See the status-codes and error-model cards for how that is represented."
  },
  "prerequisites": [],
  "related": ["what-is-a-payment-processor", "flow", "flow-authorize", "flow-capture", "flow-refund"],
  "go_deeper": [
    {"path": "docs/FAQs.md", "why": "common payment questions answered in plain language"},
    {"path": "docs/getting-started/first-payment.md", "why": "a real first authorize call end to end"}
  ],
  "verify_anchors": []
}
---
