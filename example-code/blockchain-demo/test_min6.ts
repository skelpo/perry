import { EventEmitter } from 'events';

class DemoWebSocketConnection extends EventEmitter {
  private network: string;

  constructor(network: string) {
    super();
    this.network = network;
  }
}

const conn = new DemoWebSocketConnection('ethereum');
console.log("Created connection");
