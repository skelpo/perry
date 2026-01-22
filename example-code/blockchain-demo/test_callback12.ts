import { EventEmitter } from 'events';

class DemoWebSocketConnection extends EventEmitter {
  private network: string;
  
  constructor(network: string) {
    super();
    this.network = network;
  }
}

class MultiNetworkManager extends EventEmitter {
  private connections: Map<string, DemoWebSocketConnection> = new Map();
  
  public initialize(): void {
    const networks = ['ethereum'];
    
    networks.map((network) => {
      const connection = new DemoWebSocketConnection(network);
      this.connections.set(network, connection);
    });
  }
}

const manager = new MultiNetworkManager();
manager.initialize();
console.log("Test");
