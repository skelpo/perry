/**
 * Blockchain Demo Application
 *
 * This is a demonstration application that showcases TypeScript features
 * and library usage patterns for testing the Compilets TypeScript compiler.
 *
 * IMPORTANT: This is NOT a real blockchain application. All data is simulated
 * and no actual blockchain interactions occur.
 */

import { EventEmitter } from 'events';
import { ethers } from 'ethers';
import WebSocket from 'ws';
import Redis from 'ioredis';
import { LRUCache } from 'lru-cache';
import { createPublicClient, http, parseAbiItem } from 'viem';
import { mainnet } from 'viem/chains';
import Big from 'big.js';
import Decimal from 'decimal.js-light';
import JSBI from 'jsbi';
import invariant from 'tiny-invariant';
import { backOff } from 'exponential-backoff';
import { Command } from 'commander';
import 'dotenv/config';

// ============================================================================
// Type Definitions
// ============================================================================

/** Supported blockchain networks */
type NetworkName = 'ethereum' | 'polygon' | 'arbitrum' | 'optimism' | 'base';

/** Connection tier for WebSocket subscriptions */
type TierType = 'premium' | 'standard';

/** Asset types in the demo */
type AssetType = 'token' | 'nft' | 'liquidity_position';

/** Order status in the simulated order book */
type OrderStatus = 'pending' | 'filled' | 'cancelled' | 'expired';

/** Log event from blockchain subscription */
interface LogEvent {
  address: string;
  topics: string[];
  data: string;
  blockNumber: string;
  transactionHash: string;
  logIndex: string;
  removed: boolean;
}

/** Processed event with context */
interface ProcessedEvent {
  network: NetworkName;
  contractAddress: string;
  transactionHash: string;
  blockNumber: number;
  eventSignature: string;
  data: string;
  topics: string[];
  timestamp: number;
  source: TierType;
}

/** Connection status for monitoring */
interface ConnectionStatus {
  connected: boolean;
  network: NetworkName;
  subscriptionId: string | null;
  lastMessageTime: number;
  messageCount: number;
  reconnectCount: number;
}

/** Simulated gas price data */
interface GasPriceData {
  baseFeePerGas: string;
  maxPriorityFeePerGas: string;
  maxFeePerGas: string;
  gasPrice: string;
  timestamp: number;
  isLegacy: boolean;
}

/** Configuration for the demo application */
interface DemoConfig {
  networks: NetworkName[];
  cacheSize: number;
  reconnectAttempts: number;
  healthCheckInterval: number;
  simulationMode: boolean;
}

// ============================================================================
// Data Classes
// ============================================================================

/**
 * Represents a token in the demo system
 */
export class TokenData {
  public symbol: string;
  public address: string;
  public decimals: number;
  public name: string;
  public id: number | null;

  constructor(
    symbol: string,
    address: string,
    decimals: number,
    name: string,
    id: number | null = null
  ) {
    this.symbol = symbol;
    this.address = address;
    this.decimals = decimals;
    this.name = name;
    this.id = id;
  }

  /** Format a raw amount to human-readable string */
  public formatAmount(rawAmount: bigint): string {
    return ethers.formatUnits(rawAmount, this.decimals);
  }

  /** Parse a human-readable amount to raw bigint */
  public parseAmount(amount: string): bigint {
    return ethers.parseUnits(amount, this.decimals);
  }
}

/**
 * Represents a liquidity pool between two tokens
 */
export class PoolData {
  public id: number;
  public token1: TokenData;
  public token2: TokenData;
  public network: NetworkName;
  public pairAddress: string;
  public reserve1: bigint;
  public reserve2: bigint;
  public fee: bigint;
  public liquidity: bigint;
  public tick: bigint;
  public sqrtRatioX96: bigint;
  public lastUpdate: Date;
  public isActive: boolean;

  constructor(data: Partial<PoolData> = {}) {
    this.id = data.id ?? 0;
    this.token1 = data.token1 ?? new TokenData('', '', 18, '');
    this.token2 = data.token2 ?? new TokenData('', '', 18, '');
    this.network = data.network ?? 'ethereum';
    this.pairAddress = data.pairAddress ?? '';
    this.reserve1 = data.reserve1 ?? 0n;
    this.reserve2 = data.reserve2 ?? 0n;
    this.fee = data.fee ?? 3000n; // 0.3% default
    this.liquidity = data.liquidity ?? 0n;
    this.tick = data.tick ?? 0n;
    this.sqrtRatioX96 = data.sqrtRatioX96 ?? 0n;
    this.lastUpdate = data.lastUpdate ?? new Date();
    this.isActive = data.isActive ?? true;
  }

  /** Calculate the price of token1 in terms of token2 */
  public getPrice(): number {
    if (this.reserve2 === 0n) return 0;
    const r1 = Number(this.reserve1) / Math.pow(10, this.token1.decimals);
    const r2 = Number(this.reserve2) / Math.pow(10, this.token2.decimals);
    return r2 / r1;
  }

  /** Get total value locked in USD (simulated) */
  public getTVL(): Big {
    // Simulated TVL calculation
    const simulatedPrice = 1000; // Pretend token1 is worth $1000
    const r1 = new Big(this.reserve1.toString()).div(Math.pow(10, this.token1.decimals));
    return r1.times(simulatedPrice).times(2); // TVL = 2 * reserve1 value
  }
}

/**
 * Represents a simulated order in the demo order book
 */
export class Order {
  public id: string;
  public network: NetworkName;
  public tokenIn: TokenData;
  public tokenOut: TokenData;
  public amountIn: bigint;
  public amountOutMin: bigint;
  public deadline: number;
  public status: OrderStatus;
  public createdAt: Date;
  public executedAt: Date | null;
  public transactionHash: string | null;

  constructor(
    id: string,
    network: NetworkName,
    tokenIn: TokenData,
    tokenOut: TokenData,
    amountIn: bigint,
    amountOutMin: bigint,
    deadline: number
  ) {
    this.id = id;
    this.network = network;
    this.tokenIn = tokenIn;
    this.tokenOut = tokenOut;
    this.amountIn = amountIn;
    this.amountOutMin = amountOutMin;
    this.deadline = deadline;
    this.status = 'pending';
    this.createdAt = new Date();
    this.executedAt = null;
    this.transactionHash = null;
  }

  /** Check if the order has expired */
  public isExpired(): boolean {
    return Date.now() / 1000 > this.deadline;
  }

  /** Mark order as filled */
  public fill(transactionHash: string): void {
    this.status = 'filled';
    this.executedAt = new Date();
    this.transactionHash = transactionHash;
  }

  /** Mark order as cancelled */
  public cancel(): void {
    this.status = 'cancelled';
  }
}

// ============================================================================
// WebSocket Connection Manager
// ============================================================================

/**
 * Manages WebSocket connections for blockchain event subscriptions.
 * This is a demonstration class - no actual connections are made.
 */
class DemoWebSocketConnection extends EventEmitter {
  private ws: WebSocket | null = null;
  private network: NetworkName;
  private tier: TierType;
  private subscriptionId: string | null = null;
  private reconnectAttempts: number = 0;
  private maxReconnectAttempts: number;
  private reconnectDelay: number = 5000;
  private isShuttingDown: boolean = false;
  private healthCheckInterval: NodeJS.Timeout | null = null;

  // Statistics
  private messageCount: number = 0;
  private lastMessageTime: number = 0;
  private reconnectCount: number = 0;

  constructor(network: NetworkName, tier: TierType, maxReconnectAttempts: number = 5) {
    super();
    this.network = network;
    this.tier = tier;
    this.maxReconnectAttempts = maxReconnectAttempts;
  }

  /**
   * Simulate connecting to a WebSocket endpoint
   */
  public async connect(): Promise<boolean> {
    if (this.isShuttingDown) {
      return false;
    }

    console.log(`[${this.network}] [${this.tier}] Simulating WebSocket connection...`);

    // Simulate connection delay
    await sleep(100);

    // Simulate successful connection
    this.subscriptionId = `demo-sub-${Date.now()}`;
    this.lastMessageTime = Date.now();
    this.reconnectAttempts = 0;

    console.log(`[${this.network}] [${this.tier}] Connected (simulated)`);

    // Start simulated event generation
    this.startSimulatedEvents();
    this.startHealthCheck();

    return true;
  }

  /**
   * Generate simulated blockchain events
   */
  private startSimulatedEvents(): void {
    const eventInterval = setInterval(() => {
      if (this.isShuttingDown) {
        clearInterval(eventInterval);
        return;
      }

      // Generate a fake event every 5 seconds
      const event: ProcessedEvent = {
        network: this.network,
        contractAddress: `0x${randomHex(40)}`,
        transactionHash: `0x${randomHex(64)}`,
        blockNumber: Math.floor(Math.random() * 1000000) + 18000000,
        eventSignature: '0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef',
        data: `0x${randomHex(64)}`,
        topics: [`0x${randomHex(64)}`, `0x${randomHex(64)}`],
        timestamp: Date.now(),
        source: this.tier,
      };

      this.messageCount++;
      this.lastMessageTime = Date.now();
      this.emit('event', event);
    }, 5000);
  }

  /**
   * Start periodic health checks
   */
  private startHealthCheck(): void {
    this.healthCheckInterval = setInterval(() => {
      const timeSinceLastMessage = Date.now() - this.lastMessageTime;
      if (timeSinceLastMessage > 30000) {
        console.warn(`[${this.network}] No messages for ${timeSinceLastMessage}ms`);
      }
    }, 10000);
  }

  /**
   * Get current connection status
   */
  public getStatus(): ConnectionStatus {
    return {
      connected: this.subscriptionId !== null,
      network: this.network,
      subscriptionId: this.subscriptionId,
      lastMessageTime: this.lastMessageTime,
      messageCount: this.messageCount,
      reconnectCount: this.reconnectCount,
    };
  }

  /**
   * Disconnect and cleanup
   */
  public disconnect(): void {
    this.isShuttingDown = true;

    if (this.healthCheckInterval) {
      clearInterval(this.healthCheckInterval);
    }

    this.subscriptionId = null;
    console.log(`[${this.network}] [${this.tier}] Disconnected`);
  }
}

/**
 * Manages multiple WebSocket connections across networks
 */
class MultiNetworkConnectionManager extends EventEmitter {
  private connections: Map<NetworkName, DemoWebSocketConnection> = new Map();
  private config: DemoConfig;

  constructor(config: DemoConfig) {
    super();
    this.config = config;
  }

  /**
   * Initialize connections for all configured networks
   */
  public async initialize(): Promise<void> {
    console.log('Initializing multi-network connection manager...');

    const connectionPromises = this.config.networks.map(async (network) => {
      const connection = new DemoWebSocketConnection(network, 'premium', this.config.reconnectAttempts);

      connection.on('event', (event: ProcessedEvent) => {
        this.emit('event', event);
      });

      await connection.connect();
      this.connections.set(network, connection);
    });

    await Promise.all(connectionPromises);

    console.log(`Initialized ${this.connections.size} network connections`);
  }

  /**
   * Get status of all connections
   */
  public getStatus(): Map<NetworkName, ConnectionStatus> {
    const status = new Map<NetworkName, ConnectionStatus>();
    for (const [network, connection] of this.connections) {
      status.set(network, connection.getStatus());
    }
    return status;
  }

  /**
   * Shutdown all connections
   */
  public shutdown(): void {
    console.log('Shutting down all connections...');
    for (const connection of this.connections.values()) {
      connection.disconnect();
    }
    this.connections.clear();
  }
}

// ============================================================================
// Cache Manager
// ============================================================================

/**
 * Manages caching of pool data and gas prices using LRU cache and Redis
 */
class DemoCacheManager {
  private poolCache: LRUCache<string, PoolData>;
  private gasPriceCache: LRUCache<NetworkName, GasPriceData>;
  private redis: Redis | null = null;

  constructor(cacheSize: number) {
    this.poolCache = new LRUCache<string, PoolData>({
      max: cacheSize,
      ttl: 60000, // 1 minute TTL
    });

    this.gasPriceCache = new LRUCache<NetworkName, GasPriceData>({
      max: 10,
      ttl: 10000, // 10 second TTL for gas prices
    });
  }

  /**
   * Initialize Redis connection (simulated)
   */
  public async initializeRedis(): Promise<void> {
    console.log('Simulating Redis connection...');
    // In a real application, this would connect to Redis:
    // this.redis = new Redis(process.env.REDIS_URL);
    await sleep(50);
    console.log('Redis connection simulated');
  }

  /**
   * Get pool data from cache
   */
  public getPool(address: string): PoolData | undefined {
    return this.poolCache.get(address);
  }

  /**
   * Store pool data in cache
   */
  public setPool(address: string, pool: PoolData): void {
    this.poolCache.set(address, pool);
  }

  /**
   * Get gas price data for a network
   */
  public getGasPrice(network: NetworkName): GasPriceData | undefined {
    return this.gasPriceCache.get(network);
  }

  /**
   * Update gas price data for a network
   */
  public setGasPrice(network: NetworkName, data: GasPriceData): void {
    this.gasPriceCache.set(network, data);
  }

  /**
   * Get cache statistics
   */
  public getStats(): { poolCacheSize: number; gasPriceCacheSize: number } {
    return {
      poolCacheSize: this.poolCache.size,
      gasPriceCacheSize: this.gasPriceCache.size,
    };
  }
}

// ============================================================================
// Price Calculator (using precision math libraries)
// ============================================================================

/**
 * Demonstrates usage of precision math libraries for price calculations
 */
class PriceCalculator {
  /**
   * Calculate output amount for a swap using Big.js
   */
  public static calculateSwapOutput(
    amountIn: bigint,
    reserveIn: bigint,
    reserveOut: bigint,
    feeBps: number
  ): Big {
    const amountInBig = new Big(amountIn.toString());
    const reserveInBig = new Big(reserveIn.toString());
    const reserveOutBig = new Big(reserveOut.toString());

    // Apply fee
    const feeMultiplier = new Big(10000 - feeBps).div(10000);
    const amountInWithFee = amountInBig.times(feeMultiplier);

    // Constant product formula: (reserveIn + amountIn) * (reserveOut - amountOut) = k
    const numerator = amountInWithFee.times(reserveOutBig);
    const denominator = reserveInBig.plus(amountInWithFee);

    return numerator.div(denominator);
  }

  /**
   * Calculate price impact using Decimal.js
   */
  public static calculatePriceImpact(
    amountIn: bigint,
    reserveIn: bigint,
    reserveOut: bigint
  ): Decimal {
    const amountInDec = new Decimal(amountIn.toString());
    const reserveInDec = new Decimal(reserveIn.toString());
    const reserveOutDec = new Decimal(reserveOut.toString());

    // Price before swap
    const priceBefore = reserveOutDec.div(reserveInDec);

    // Price after swap (simplified)
    const newReserveIn = reserveInDec.plus(amountInDec);
    const priceAfter = reserveOutDec.div(newReserveIn);

    // Price impact as percentage
    return priceBefore.minus(priceAfter).div(priceBefore).times(100);
  }

  /**
   * Calculate tick from sqrt price using JSBI for arbitrary precision
   */
  public static sqrtPriceX96ToTick(sqrtPriceX96: bigint): number {
    const sqrtPriceJSBI = JSBI.BigInt(sqrtPriceX96.toString());
    const Q96 = JSBI.exponentiate(JSBI.BigInt(2), JSBI.BigInt(96));

    // Calculate price = (sqrtPriceX96 / 2^96)^2
    const sqrtPrice = JSBI.divide(sqrtPriceJSBI, Q96);

    // Simplified tick calculation (not exact, just for demo)
    const priceNum = Number(sqrtPrice.toString());
    const tick = Math.floor(Math.log(priceNum * priceNum) / Math.log(1.0001));

    return tick;
  }
}

// ============================================================================
// Utility Functions
// ============================================================================

/** Sleep for specified milliseconds */
function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/** Generate random hex string of specified length */
function randomHex(length: number): string {
  const chars = '0123456789abcdef';
  let result = '';
  for (let i = 0; i < length; i++) {
    result += chars[Math.floor(Math.random() * chars.length)];
  }
  return result;
}

/** Format a bigint as a human-readable number with thousands separators */
function formatWithThousands(value: bigint): string {
  return value.toString().replace(/\B(?=(\d{3})+(?!\d))/g, ',');
}

/** Validate network name */
function isValidNetwork(name: string): name is NetworkName {
  return ['ethereum', 'polygon', 'arbitrum', 'optimism', 'base'].includes(name);
}

// ============================================================================
// Demo Data Generation
// ============================================================================

/** Generate simulated token data */
function generateDemoTokens(): TokenData[] {
  return [
    new TokenData('WETH', '0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2', 18, 'Wrapped Ether', 1),
    new TokenData('USDC', '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48', 6, 'USD Coin', 2),
    new TokenData('USDT', '0xdAC17F958D2ee523a2206206994597C13D831ec7', 6, 'Tether USD', 3),
    new TokenData('WBTC', '0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599', 8, 'Wrapped Bitcoin', 4),
    new TokenData('DAI', '0x6B175474E89094C44Da98b954EescdeCB5BFDA', 18, 'Dai Stablecoin', 5),
  ];
}

/** Generate simulated pool data */
function generateDemoPools(tokens: TokenData[]): PoolData[] {
  const pools: PoolData[] = [];

  // WETH/USDC pool
  pools.push(
    new PoolData({
      id: 1,
      token1: tokens[0], // WETH
      token2: tokens[1], // USDC
      network: 'ethereum',
      pairAddress: `0x${randomHex(40)}`,
      reserve1: ethers.parseUnits('10000', 18), // 10,000 WETH
      reserve2: ethers.parseUnits('25000000', 6), // 25M USDC
      fee: 3000n,
      liquidity: ethers.parseUnits('500000', 18),
      tick: 200000n,
      sqrtRatioX96: 1771595571142957166518320255467520n,
      isActive: true,
    })
  );

  // WETH/USDT pool
  pools.push(
    new PoolData({
      id: 2,
      token1: tokens[0], // WETH
      token2: tokens[2], // USDT
      network: 'ethereum',
      pairAddress: `0x${randomHex(40)}`,
      reserve1: ethers.parseUnits('8000', 18),
      reserve2: ethers.parseUnits('20000000', 6),
      fee: 3000n,
      liquidity: ethers.parseUnits('400000', 18),
      tick: 199500n,
      sqrtRatioX96: 1770000000000000000000000000000000n,
      isActive: true,
    })
  );

  // USDC/USDT pool
  pools.push(
    new PoolData({
      id: 3,
      token1: tokens[1], // USDC
      token2: tokens[2], // USDT
      network: 'ethereum',
      pairAddress: `0x${randomHex(40)}`,
      reserve1: ethers.parseUnits('50000000', 6),
      reserve2: ethers.parseUnits('50000000', 6),
      fee: 100n, // 0.01% for stablecoin pairs
      liquidity: ethers.parseUnits('100000000', 6),
      tick: 0n,
      sqrtRatioX96: 79228162514264337593543950336n, // ~1.0
      isActive: true,
    })
  );

  return pools;
}

// ============================================================================
// Main Demo Application
// ============================================================================

/**
 * Main demo application class
 */
class BlockchainDemo {
  private config: DemoConfig;
  private connectionManager: MultiNetworkConnectionManager | null = null;
  private cacheManager: DemoCacheManager;
  private tokens: TokenData[] = [];
  private pools: PoolData[] = [];
  private isRunning: boolean = false;

  constructor(config: DemoConfig) {
    this.config = config;
    this.cacheManager = new DemoCacheManager(config.cacheSize);
  }

  /**
   * Initialize the demo application
   */
  public async initialize(): Promise<void> {
    console.log('='.repeat(60));
    console.log('Blockchain Demo Application');
    console.log('='.repeat(60));
    console.log('');
    console.log('This is a DEMONSTRATION application.');
    console.log('No actual blockchain interactions occur.');
    console.log('');

    // Initialize cache
    await this.cacheManager.initializeRedis();

    // Generate demo data
    this.tokens = generateDemoTokens();
    this.pools = generateDemoPools(this.tokens);

    // Cache pools
    for (const pool of this.pools) {
      this.cacheManager.setPool(pool.pairAddress, pool);
    }

    console.log(`Generated ${this.tokens.length} demo tokens`);
    console.log(`Generated ${this.pools.length} demo pools`);
    console.log('');

    // Display token info
    console.log('Demo Tokens:');
    for (const token of this.tokens) {
      console.log(`  - ${token.symbol}: ${token.name} (${token.decimals} decimals)`);
    }
    console.log('');

    // Display pool info
    console.log('Demo Pools:');
    for (const pool of this.pools) {
      const price = pool.getPrice().toFixed(2);
      const tvl = pool.getTVL().toFixed(0);
      console.log(`  - ${pool.token1.symbol}/${pool.token2.symbol}: Price $${price}, TVL $${tvl}`);
    }
    console.log('');

    // Initialize connection manager
    this.connectionManager = new MultiNetworkConnectionManager(this.config);

    this.connectionManager.on('event', (event: ProcessedEvent) => {
      this.handleEvent(event);
    });

    await this.connectionManager.initialize();
  }

  /**
   * Handle incoming blockchain events
   */
  private handleEvent(event: ProcessedEvent): void {
    console.log(`[EVENT] ${event.network}: Block ${event.blockNumber}, TX ${event.transactionHash.slice(0, 10)}...`);
  }

  /**
   * Demonstrate price calculations
   */
  public demonstratePriceCalculations(): void {
    console.log('');
    console.log('='.repeat(60));
    console.log('Price Calculation Demonstrations');
    console.log('='.repeat(60));
    console.log('');

    const pool = this.pools[0]; // WETH/USDC pool
    const swapAmount = ethers.parseUnits('10', 18); // 10 WETH

    // Calculate swap output
    const output = PriceCalculator.calculateSwapOutput(
      swapAmount,
      pool.reserve1,
      pool.reserve2,
      30 // 0.3% fee
    );
    console.log(`Swap ${pool.token1.formatAmount(swapAmount)} WETH:`);
    console.log(`  Output: ${output.div(1e6).toFixed(2)} USDC`);

    // Calculate price impact
    const impact = PriceCalculator.calculatePriceImpact(swapAmount, pool.reserve1, pool.reserve2);
    console.log(`  Price Impact: ${impact.toFixed(4)}%`);

    // Calculate tick from sqrt price
    const tick = PriceCalculator.sqrtPriceX96ToTick(pool.sqrtRatioX96);
    console.log(`  Current Tick: ${tick}`);

    console.log('');
  }

  /**
   * Demonstrate order creation
   */
  public demonstrateOrderCreation(): void {
    console.log('');
    console.log('='.repeat(60));
    console.log('Order Management Demonstration');
    console.log('='.repeat(60));
    console.log('');

    const order = new Order(
      `order-${Date.now()}`,
      'ethereum',
      this.tokens[0], // WETH
      this.tokens[1], // USDC
      ethers.parseUnits('1', 18), // 1 WETH
      ethers.parseUnits('2400', 6), // Min 2400 USDC
      Math.floor(Date.now() / 1000) + 3600 // 1 hour deadline
    );

    console.log('Created Order:');
    console.log(`  ID: ${order.id}`);
    console.log(`  Swap: ${order.tokenIn.formatAmount(order.amountIn)} ${order.tokenIn.symbol}`);
    console.log(`  For: ${order.tokenOut.formatAmount(order.amountOutMin)} ${order.tokenOut.symbol} (min)`);
    console.log(`  Status: ${order.status}`);
    console.log(`  Expired: ${order.isExpired()}`);

    // Simulate filling the order
    order.fill(`0x${randomHex(64)}`);
    console.log('');
    console.log('After filling:');
    console.log(`  Status: ${order.status}`);
    console.log(`  TX Hash: ${order.transactionHash?.slice(0, 20)}...`);
    console.log('');
  }

  /**
   * Run the demo application
   */
  public async run(): Promise<void> {
    this.isRunning = true;

    await this.initialize();
    this.demonstratePriceCalculations();
    this.demonstrateOrderCreation();

    // Display cache stats
    const stats = this.cacheManager.getStats();
    console.log('Cache Statistics:');
    console.log(`  Pool Cache Size: ${stats.poolCacheSize}`);
    console.log(`  Gas Price Cache Size: ${stats.gasPriceCacheSize}`);
    console.log('');

    // Run for a bit to show events
    console.log('Listening for simulated events (Ctrl+C to stop)...');
    console.log('');

    // Handle graceful shutdown
    process.on('SIGINT', () => {
      this.shutdown();
    });

    process.on('SIGTERM', () => {
      this.shutdown();
    });
  }

  /**
   * Shutdown the demo application
   */
  public shutdown(): void {
    if (!this.isRunning) return;

    console.log('');
    console.log('Shutting down...');
    this.isRunning = false;

    if (this.connectionManager) {
      this.connectionManager.shutdown();
    }

    console.log('Demo complete!');
    process.exit(0);
  }
}

// ============================================================================
// CLI Entry Point
// ============================================================================

const program = new Command();

program
  .name('blockchain-demo')
  .description('Demonstration application for blockchain library usage')
  .version('1.0.0')
  .option('-n, --networks <networks>', 'Comma-separated list of networks', 'ethereum,polygon')
  .option('-c, --cache-size <size>', 'LRU cache size', '1000')
  .option('-r, --reconnect-attempts <attempts>', 'Max reconnect attempts', '5')
  .action(async (options) => {
    const networks = options.networks.split(',').filter(isValidNetwork) as NetworkName[];

    if (networks.length === 0) {
      console.error('No valid networks specified');
      process.exit(1);
    }

    const config: DemoConfig = {
      networks,
      cacheSize: parseInt(options.cacheSize),
      reconnectAttempts: parseInt(options.reconnectAttempts),
      healthCheckInterval: 30000,
      simulationMode: true,
    };

    const demo = new BlockchainDemo(config);
    await demo.run();
  });

program.parse();
