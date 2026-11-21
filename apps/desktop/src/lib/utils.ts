import { clsx, type ClassValue } from "clsx";

/**
 * Joins class names. It does not resolve conflicts: a later class does not beat an earlier one of
 * the same property, so a component must never pass two (tokens.test.ts checks the generated
 * ones and every button variant).
 */
export function cn(...inputs: ClassValue[]): string {
  return clsx(inputs);
}
