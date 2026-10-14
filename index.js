// Drop-in замена `critters`: Next.js делает require('critters') для
// experimental.optimizeCss и получает отсюда порт (Rust → FastCritters → stock).
let Critters;
try {
  Critters = require('./dist/core/index.js').loadFastCritters();
} catch (error) {
  console.error('critters: port failed to load, using stock', error);
  Critters = require('critters-upstream');
}

module.exports = Critters;
