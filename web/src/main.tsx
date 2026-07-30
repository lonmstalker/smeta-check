import './index.css'
import './lib/i18n'
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { BrowserRouter } from 'react-router'
import App from './App'
import { installGlobalErrorLogging } from './lib/logger'
import { applyStoredTheme } from './lib/theme'

installGlobalErrorLogging()
// тема до рендера — чтобы страница не мигала светлым
applyStoredTheme()

const root = document.getElementById('root')
if (!root) throw new Error('index.html без <div id="root">')

createRoot(root).render(
  <StrictMode>
    <BrowserRouter>
      <App />
    </BrowserRouter>
  </StrictMode>,
)
