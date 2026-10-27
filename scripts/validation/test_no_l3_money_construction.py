"""Tests for the L3 domain Money construction guard."""

import unittest

from no_l3_money_construction import violations


class MoneyConstructionGuardTests(unittest.TestCase):
    def test_rejects_qualified_and_imported_money(self):
        source = """use common_utils::types::Money;
let first = Money { amount, currency };
let second = common_utils::types::Money {
    amount,
    currency,
};
let third = Money::default();
"""
        self.assertEqual(violations(source), [2, 3, 7])

    def test_ignores_comments_strings_and_connector_payload_money(self):
        source = '''// Money { amount, currency }
const EXAMPLE: &str = "Money { amount, currency }";
/* nested /* Money { amount, currency } */ comment */
pub struct Money { amount: String }
let connector_amount = Money { amount: "10".into() };
'''
        self.assertEqual(violations(source), [])

    def test_rejects_domain_money_even_with_local_money_type(self):
        source = """struct Money { amount: String }
let domain_amount = common_utils::types::Money { amount, currency };
"""
        self.assertEqual(violations(source), [2])

    def test_rejects_import_alias(self):
        source = """use common_utils::types::Money as DomainMoney;
let amount = DomainMoney { amount, currency };
"""
        self.assertEqual(violations(source), [2])


if __name__ == "__main__":
    unittest.main()
