import { Router } from 'express';
import authRoutes from './auth.js';
import paymentRoutes from './payments.js';

const router = Router();

router.use('/auth', authRoutes);
router.use('/payments', paymentRoutes);

export default router;