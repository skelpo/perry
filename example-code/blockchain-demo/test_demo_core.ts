/**
 * Stripped down demo.ts to isolate compilation error
 */

import { EventEmitter } from 'events';

// Type Definitions
type NetworkName = 'ethereum' | 'polygon' | 'arbitrum' | 'optimism' | 'base';
type TierType = 'premium' | 'standard';

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

interface ConnectionStatus {
  connected: boolean;
  network: NetworkName;
  subscriptionId: string | null;
  lastMessageTime: number;
  messageCount: number;
  reconnectCount: number;
}

interface DemoConfig {
  networks: NetworkName[];
  cacheSize: number;
  reconnectAttempts: number;
  healthCheckInterval: number;
  simulationMode: boolean;
}

// Utility Functions
function randomHex(length: number): string {
  const chars = '0123456789abcdef';
  let result = '';
  for (let i = 0; i < length; i++) {
    result += chars[Math.floor(Math.random() * chars.length)];
  }
  return result;
}

// WebSocket Connection Manager
class DemoWebSocketConnection extends EventEmitter {
  private network: NetworkName;
  private tier: TierType;
  private subscriptionId: string | null = null;
  private isShuttingDown: boolean = false;
  private messageCount: number = 0;
  private lastMessageTime: number = 0;
  private reconnectCount: number = 0;

  constructor(network: NetworkName, tier: TierType) {
    super();
    this.network = network;
    this.tier = tier;
  }

  public connect(): boolean {
    if (this.isShuttingDown) {
      return false;
    }

    console.log('Simulating connection for ' + this.network);
    this.subscriptionId = 'demo-sub-' + Date.now().toString();
    this.lastMessageTime = Date.now();

    // Generate a fake event
    const event: ProcessedEvent = {
      network: this.network,
      contractAddress: '0x' + randomHex(40),
      transactionHash: '0x' + randomHex(64),
      blockNumber: Math.floor(Math.random() * 1000000) + 18000000,
      eventSignature: '0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef',
      data: '0x' + randomHex(64),
      topics: ['0x' + randomHex(64), '0x' + randomHex(64)],
      timestamp: Date.now(),
      source: this.tier,
    };

    this.messageCount++;
    this.lastMessageTime = Date.now();
    this.emit('event', event);

    return true;
  }

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

  public disconnect(): void {
    this.isShuttingDown = true;
    this.subscriptionId = null;
  }
}

// Multi-network Manager
class MultiNetworkConnectionManager extends EventEmitter {
  private connections: Map<NetworkName, DemoWebSocketConnection> = new Map();
  private config: DemoConfig;

  constructor(config: DemoConfig) {
    super();
    this.config = config;
  }

  public initialize(): void {
    console.log('Initializing connections...');

    this.config.networks.map((network) => {
      const connection = new DemoWebSocketConnection(network, 'premium');

      connection.on('event', (event: ProcessedEvent) => {
        this.emit('event', event);
      });

      connection.connect();
      this.connections.set(network, connection);
    });

    console.log('Initialized ' + this.connections.size.toString() + ' connections');
  }

  public shutdown(): void {
    for (const connection of this.connections.values()) {
      connection.disconnect();
    }
    this.connections.clear();
  }
}

// Main Demo
const config: DemoConfig = {
  networks: ['ethereum'],
  cacheSize: 1000,
  reconnectAttempts: 5,
  healthCheckInterval: 30000,
  simulationMode: true,
};

const manager = new MultiNetworkConnectionManager(config);

manager.on('event', (event: ProcessedEvent) => {
  console.log('Got event from ' + event.network);
});

manager.initialize();
manager.shutdown();
console.log('Done');
