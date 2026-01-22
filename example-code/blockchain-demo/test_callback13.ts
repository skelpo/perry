import { EventEmitter } from 'events';

class Inner extends EventEmitter {
  private network: string;
  
  constructor(network: string) {
    super();
    this.network = network;
  }
}

class Outer extends EventEmitter {
  private connections: Map<string, Inner> = new Map();
  
  public initialize(): void {
    const networks = ['ethereum'];
    
    networks.map((network) => {
      const connection = new Inner(network);
      
      // This nested callback captures 'this' from Outer
      connection.on('event', (event) => {
        this.emit('event', event);
      });
      
      this.connections.set(network, connection);
    });
  }
}

const outer = new Outer();
outer.initialize();
console.log("Test");
