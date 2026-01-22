import { EventEmitter } from 'events';

class MyEmitter extends EventEmitter {
  public doSomething(): void {
    this.emit('event', { data: 'test' });
  }
}

const emitter = new MyEmitter();
emitter.on('event', (data) => {
  console.log(data);
});
emitter.doSomething();
console.log("Test");
