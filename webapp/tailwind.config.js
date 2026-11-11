/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  darkMode: 'class',
  theme: {
    extend: {
      colors: {
        austria: {
          red: '#ED2939',
          gold: '#F39C12',
          navy: '#1A252F',
        }
      }
    },
  },
  plugins: [],
}
