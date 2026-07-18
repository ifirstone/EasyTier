/** @type {import('tailwindcss').Config} */
export default {
  content: [
    './index.html',
    './src/**/*.{vue,js,ts,jsx,tsx}',
    '!./src/modules/github-releases.ts',
    '!./src/modules/hostname-decrypt.ts',
  ],
  theme: {
    extend: {},
  },
  plugins: [require('tailwindcss-primeui')],
}
