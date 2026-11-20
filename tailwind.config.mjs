/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{js,jsx,ts,tsx}"],
  theme: {
    extend: {
      fontFamily: {
        sans: ["-apple-system", "BlinkMacSystemFont", '"SF Pro Display"', '"SF Pro Text"', '"Segoe UI Variable Display"', '"Segoe UI"', "system-ui", "sans-serif"],
        mono: ['"SF Mono"', '"Cascadia Mono"', "ui-monospace", "monospace"],
      },
    },
  },
  plugins: [],
};
