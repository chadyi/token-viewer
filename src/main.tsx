import { createRoot } from 'react-dom/client'

import App from './App'
import './index.css'

const container = document.getElementById('app')
if (!container) throw new Error('Missing #app root element')

createRoot(container).render(<App />)

