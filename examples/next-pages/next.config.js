const { withFastCritters } = require('critters/next');

module.exports = withFastCritters({
  output: process.env.STANDALONE === 'true' ? 'standalone' : undefined,
  // Пример лежит внутри репозитория пакета со своим lockfile.
  outputFileTracingRoot: __dirname,
  turbopack: { root: __dirname },
});
