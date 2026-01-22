const config = { networks: ['ethereum', 'polygon'] };

const connectionPromises = config.networks.map(async (network) => {
  console.log(network);
});

Promise.all(connectionPromises);
console.log("Test");
