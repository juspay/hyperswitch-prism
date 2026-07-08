# Skill: Route Between Connectors

## Description
Implement intelligent routing to switch between payment providers dynamically.

## When to Use
- Supporting multiple payment processors
- Implementing failover logic
- Optimizing for cost/routing rules

## Parameters
- language: Programming language
- primary_connector: Primary payment provider
- fallback_connector: Fallback provider (optional)
- routing_criteria: currency, region, amount_threshold, etc.

## Prompt Template

Create a dynamic routing implementation in {{language}} using Hyperswitch Prism.

Routing Logic:
- Primary: {{primary_connector}}
- Fallback: {{fallback_connector}}
- Criteria: {{routing_criteria}}

Requirements:
1. Define configuration objects for both connectors
2. Create a routing function that selects connector based on:
   {{routing_criteria}}
3. Common patterns to demonstrate:
   - Currency-based routing (USD → Stripe, EUR → Adyen)
   - Region-based routing
   - Amount-based routing (high value → specific processor)
   - Random/load balancing
4. Implement fallback logic:
   - Try primary connector
   - On specific failures, retry with fallback
   - Track which connector succeeded
5. Show how to switch connectors in ONE line of code

Output: Complete routing implementation with example criteria.
