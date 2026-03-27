<!--
---
title: "Prism: The Other Side of the Rainbow"
description: "The story behind naming Hyperswitch's payment abstraction library after Newton's discovery—seeing unity in payment diversity"
author: "Loki"
date: 2026-03-21
og_image: /images/prism-header.png
tags: ["payments", "engineering", "story"]
---
-->

In 1665, Isaac Newton performed an experiment that would echo through centuries. He passed a beam of white light through a glass prism and watched it decompose into a spectrum—red, orange, yellow, green, blue, indigo, violet—onto the wall behind him.

But here's what most people forget: **Newton looked at the experiment from both sides.**

When you stand where Newton stood, you see the rainbow. Beautiful, distinct colors, each different from the others. But when you step to the other side—when you place a SECOND prism in the path of the scattered light—you discover the profound truth: **all those colors recombine into white light again.**

The rainbow wasn't chaos. It was white light disguised as color.

---

## The Rainbow of Payments

Every developer who has built payments sees the rainbow first.

Stripe is one color. Adyen is another. Braintree, PayPal, Checkout.com, Worldpay, Cybersource—each distinct, each beautiful, each different. The payment landscape isn't noise. It's a **spectrum**. And each color does the same fundamental things:

- **Authorize** — reserve funds
- **Capture** — collect funds
- **Refund** — return funds
- **3DS** — authenticate the cardholder
- **Void** — cancel before capture

Every processor does these. Stripe. Adyen. All 70 of them. Red does what orange does. Blue does what violet does.

**The colors are not the problem. They're the beauty.**

---

## The Other Side of the Rainbow

Here's where most payment abstractions get it wrong. They see the rainbow and think: "Too many colors. Must simplify. Must reduce."

But we looked from the other side.

We asked a different question: **What if all these colors are just one thing?**

The answer changed everything.

When you unify authorization, capture, refund, void, and 3DS across every processor—you don't lose the rainbow. You gain the ability to see both the diversity AND the unity. You can work with Stripe, then switch to Adyen, without rewriting your code. The colors remain beautiful. But now you also see the white light that contains them all.

---

## What Makes a Prism

A prism is magical because it works in **both directions**:

1. **White → Rainbow**: Decompose, see the diversity
2. **Rainbow → White**: Recompose, see the unity

Newton's second prism proved the first. Hyperswitch Prism does the same:

```python
# See the colors (diversity)
client = PaymentClient(connector='stripe')

# See the light (unity)
# Same code works for adyen, paypal, braintree...
```

You're not choosing between the rainbow and white light. **You're seeing both.**

---

## Why Developers Need This

The payment world isn't broken. It doesn't need fixing. It needs **seeing**.

Every processor is a valid choice. Every color is beautiful. The problem isn't diversity—it's being trapped in one color. When you're stuck on Stripe, you can't easily move to Adyen. When you're locked into Braintree, PayPal feels like a different universe.

**Hyperswitch Prism gives you the second prism.**

You can work with any processor. You can see the unity underneath. You can switch when you need to. You can use multiple at once for redundancy.

The rainbow stays. The light stays. You get both.

---

## The Unity Beneath

What did Newton prove? That white light **contains** all colors. They don't come from nowhere. They're always there, folded into one beam, waiting to be seen.

The same is true for payments:

| Operation | What it is | Every processor |
|-----------|------------|-----------------|
| Authorize | Reserve funds | All 70+ |
| Capture | Collect reserved | All 70+ |
| Refund | Return captured | All 70+ |
| 3DS | Authenticate | Most |
| Void | Cancel | Most |

**The unity was always there.** We just had to look from the right angle.

---

## Your Payment Rainbow

Here's the invitation: Don't choose between beauty and simplicity. See both.

When you use Hyperswitch Prism, you're not reducing the payment world. You're not forcing everyone into one color. You're seeing what Newton saw—that the diversity was always supported by unity.

The rainbow is real. The white light is real. **Prism lets you work with both.**

---

Welcome to the other side of the rainbow.

---

*Hyperswitch Prism is open source. Start integrating at [connector.juspay.io](https://connector.juspay.io).*
