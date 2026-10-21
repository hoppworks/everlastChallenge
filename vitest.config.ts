import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import policy from "./config/coverage.json" with { type: "json" };

export default defineConfig({
  plugins: [react()],
  test: {
    environment: "jsdom",
    include: ["src/**/*.test.{ts,tsx}"],
    coverage: {
      provider: "v8",
      include: ["src/**/*.{ts,tsx}"],
      exclude: [
        ...policy.exclusions.map((item: { path: string }) => item.path),
        "src/**/*.test.{ts,tsx}",
      ],
      reporter: ["text", "json-summary"],
      thresholds: policy.frontendMinimum,
    },
  },
});
