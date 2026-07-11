import clsx from "clsx";

export function cn(...inputs: Array<string | false | null | undefined>) {
  return clsx(inputs);
}

export function formatPct(value: number, signed = true) {
  const prefix = signed && value > 0 ? "+" : "";
  return `${prefix}${value.toFixed(2)}%`;
}

export function formatPrice(value: number) {
  return value.toFixed(2);
}

export function marketLabel(market: string) {
  return market === "SH" ? "沪" : "深";
}
