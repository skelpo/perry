import { EventEmitter } from 'events';

interface ProcessedEvent {
  network: string;
  data: string;
}

class Inner extends EventEmitter {
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

class Outer extends EventEmitter {
  private connections: Map<string, Inner> = new Map();
  
  public initialize(): void {
    const networks = ['ethereum'];
    
    networks.map((network) => {
      const connection = new Inner(network);
      
      connection.on('event', (event) => {
        this.emit('event', event);
      });
      
      connection.connect();
      this.connections.set(network, connection);
    });
  }
}

const outer = new Outer();
outer.initialize();
console.log("Test");
