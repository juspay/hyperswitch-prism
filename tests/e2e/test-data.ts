import { randomUUID } from "node:crypto";
import type { CardData } from "./framework/types.js";

export const testReturnUrl = "https://example.com";

const address = {
  first_name: "Test",
  last_name: "Customer",
  line1: "1",
  line2: "Test Street",
  city: "San Francisco",
  state: "CA",
  zip: "94107",
  country: "US",
};

export function createAddress(overrides: Partial<typeof address> = {}) {
  return { ...address, ...overrides };
}

export function createContact() {
  return {
    name: "Test Customer",
    email: "test@example.com",
    phone: "2025550100",
    phone_country_code: "+1",
  };
}

export function createCustomer(
  overrides: Partial<ReturnType<typeof createContact>> = {},
) {
  return {
    id: `cus_${randomUUID().replaceAll("-", "")}`,
    ...createContact(),
    ...overrides,
  };
}

export function createCard(overrides: Partial<CardData> = {}): CardData {
  return {
    card_number: "4111111111111111",
    card_exp_month: "03",
    card_exp_year: "30",
    card_holder_name: "Test Customer",
    card_cvc: "737",
    ...overrides,
  };
}

const browserInfo = {
  accept_header: "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
  language: "en-US",
  screen_height: 1080,
  screen_width: 1920,
  color_depth: 24,
  user_agent: "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 Chrome/130.0.0.0 Safari/537.36",
  time_zone: 0,
  java_enabled: false,
  ip_address: "192.0.2.1",
};

export function createBrowserInfo(overrides: Partial<typeof browserInfo> = {}) {
  return { ...browserInfo, ...overrides };
}
