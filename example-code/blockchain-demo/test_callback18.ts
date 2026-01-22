import { EventEmitter } from 'events';

class Inner extends EventEmitter {
  private network: string;
  
  constructor(network: string) {
    super();
    this.network = network;
  }
  
  public connect(): void {
    this.emit('event', { network: this.network });  // Uses this.network
  }
}

class Outer extends EventEmitter {
  public initialize(): void {
    const networks = ['ethereum'];
    
    networks.map((network) => {
      const connection = new Inner(network);
      
      connection.on('event', (event) => {
        this.emit('event', event);
      });
      
      connection.connect();
    });
  }
}

const outer = new Outer();
outer.initialize();
console.log("Test");
