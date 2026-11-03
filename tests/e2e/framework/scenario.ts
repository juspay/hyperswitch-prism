import type { Scenario } from "./types.js";

/** Defines metadata and setup requirements for one implemented shared scenario. */
export function defineScenario<const T extends Scenario>(definition: T): T {
  return definition;
}
