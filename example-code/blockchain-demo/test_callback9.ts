import { EventEmitter } from 'events';

class DemoWebSocketConnection extends EventEmitter {
  public connect(): void {
    this.emit('event', { network: 'eth', data: 'test' });
  }
}

class MultiNetworkManager extends EventEmitter {
  private connections: Map<string, DemoWebSocketConnection> = new Map();
  
  public initialize(): void {
    const networks = ['ethereum'];
    
    networks.map((network) => {
      const connection = new DemoWebSocketConnection();
      
      connection.on('event', (event) => {
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
