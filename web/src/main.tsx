import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { createBrowserRouter } from 'react-router';
import { App } from './App';
import { createQueryClient } from './api/queryClient';
import { appRoutes } from './routes';
import './styles/global.css';

const container = document.getElementById('root');
if (container === null) {
  throw new Error('Root element not found');
}

createRoot(container).render(
  <StrictMode>
    <App router={createBrowserRouter(appRoutes)} queryClient={createQueryClient()} />
  </StrictMode>,
);
