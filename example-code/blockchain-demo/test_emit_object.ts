import { EventEmitter } from 'events';

interface ProcessedEvent {
  network: string;
}

class MyEmitter extends EventEmitter {
  public test(): void {
    const event: ProcessedEvent = {
      network: "ethereum"
    };
    this.emit('event', event);
  }
}

const emitter = new MyEmitter();
emitter.test();
console.log("Done");
