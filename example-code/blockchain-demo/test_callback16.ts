import { EventEmitter } from 'events';

class Inner extends EventEmitter {
  private network: string;
  
  constructor(network: string) {
    super();
    this.network = network;
  }
  
  // Remove the connect method
}

class Outer extends EventEmitter {
  public initialize(): void {
    const networks = ['ethereum'];
    
    networks.map((network) => {
      const connection = new Inner(network);
      
      connection.on('event', (event) => {
        this.emit('event', event);
      });
    });
  }
}

const outer = new Outer();
outer.initialize();
console.log("Test");
