import { EventEmitter } from 'events';

interface ProcessedEvent {
  network: string;
  data: string;
}

class DemoWebSocketConnection extends EventEmitter {
  private network: string;
  
  constructor(network: string) {
    super();
    this.network = network;
  }
  
  public connect(): void {
    const event: ProcessedEvent = {
      network: this.network,
      data: "test"
    };
    this.emit('event', event);
  }
}

class MultiNetworkManager extends EventEmitter {
  private connections: Map<string, DemoWebSocketConnection> = new Map();
  
  constructor() {
    super();
  }
  
  public initialize(): void {
    const networks = ['ethereum'];  // Only one network
    
    networks.map((network) => {
      const connection = new DemoWebSocketConnection(network);
      
      connection.on('event', (event: ProcessedEvent) => {
        this.emit('event', event);
      });
      
      connection.connect();
      this.connections.set(network, connection);
    });
  }
}

const manager = new MultiNetworkManager();
manager.initialize();
console.log("Test");
