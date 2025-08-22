/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        'brand-blue': '#35c7ff',
      },
      animation: {
        'twinkle': 'twinkle 3s infinite',
        'typing-twinkle': 'typing-twinkle 1.5s infinite',
      },
      keyframes: {
        twinkle: {
          '0%, 100%': { opacity: '0.3', transform: 'scale(1)' },
          '50%': { opacity: '1', transform: 'scale(1.2)' },
        },
        'typing-twinkle': {
          '0%, 100%': { opacity: '0.6', transform: 'scale(0.8)' },
          '50%': { opacity: '1', transform: 'scale(1.5)' },
        },
      },
    },
  },
  plugins: [],
}