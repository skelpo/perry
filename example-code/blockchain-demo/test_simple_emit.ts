import { EventEmitter } from 'events';

class MyEmitter extends EventEmitter {
  public test(): void {
    this.emit('test', 42);
  }
}

const emitter = new MyEmitter();
emitter.test();
console.log("Done");
