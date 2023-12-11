// Replace with the actual path of the module you want to clear from the cache
const modulePath = '/home/jonathan/Bitcoin-Static/bitcoin-abc-ergon/calory/node_modules/ergonaddrjs';

delete require.cache[require.resolve(modulePath)];
console.log(`Cache cleared for module: ${modulePath}`);
