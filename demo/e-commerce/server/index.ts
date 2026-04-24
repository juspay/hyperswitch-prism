import express from 'express';
import cors from 'cors';
import path from 'path';
import { fileURLToPath } from 'url';
import dotenv from 'dotenv';
import routes from './routes/index.js';

// Load environment variables
dotenv.config();

// ES module equivalent of __dirname
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Create Express app
const app = express();

// Middleware
app.use(cors());
app.use(express.json());
app.use(express.urlencoded({ extended: true }));

// Serve static files from client directory
const clientPath = path.join(__dirname, '..', 'client');
app.use(express.static(clientPath));

// API routes
app.use('/api', routes);

// Serve index.html for root route
app.get('/', (req, res) => {
  res.sendFile(path.join(clientPath, 'index.html'));
});

// Serve checkout.html for checkout route
app.get('/checkout', (req, res) => {
  res.sendFile(path.join(clientPath, 'checkout.html'));
});

// Health check endpoint
app.get('/health', (req, res) => {
  res.json({ status: 'ok', timestamp: new Date().toISOString() });
});

// Error handling middleware
app.use((err: Error, req: express.Request, res: express.Response, next: express.NextFunction) => {
  console.error('[Server Error]', err);
  res.status(500).json({ 
    error: 'Internal server error',
    message: err.message 
  });
});

// Start server
const PORT = parseInt(process.env.PORT || '3000', 10);
const HOST = '0.0.0.0';

app.listen(PORT, HOST, () => {
  console.log(`🚀 E-commerce Payment Demo running at http://localhost:${PORT}`);
  console.log(`📦 Environment: ${process.env.NODE_ENV || 'development'}`);
  console.log(`💳 Connectors: Stripe (USD), GlobalPay (EUR)`);
  console.log('');
  console.log('API Endpoints:');
  console.log(`  GET  /api/auth/sdk-session     - Get SDK session for client tokenization`);
  console.log(`  POST /api/payments/token-authorize - Authorize payment with token`);
  console.log(`  POST /api/payments/refund      - Refund a payment`);
  console.log(`  GET  /api/payments/:id         - Get payment status`);
});