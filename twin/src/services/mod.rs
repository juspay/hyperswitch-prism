//! Business logic for each connector mock.
//!
//! Handlers in `crate::handlers` translate HTTP/protocol details and delegate
//! into a `services::<connector>::<flow>` function. The split lets us add new
//! protocols (today: Stripe-shape HTTP; tomorrow: anything) without rewriting
//! the scenario logic.

pub mod dummy;
