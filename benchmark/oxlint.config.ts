import { defineConfig } from "oxlint";

export default defineConfig({
  plugins: ["typescript", "unicorn", "oxc", "import"],
  categories: {
    correctness: "error",
    suspicious: "error",
    perf: "error",
  },
  options: {
    typeAware: true,
    typeCheck: true,
  },
  rules: {
    "eslint/no-underscore-dangle": ["error", { allow: ["_meta"] }],
    "typescript/no-explicit-any": "error",
    "typescript/no-floating-promises": "error",
    "typescript/no-misused-promises": "error",
  },
  env: {
    node: true,
  },
  ignorePatterns: ["results/**"],
});
